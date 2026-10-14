//! Native implementations of Valentine's matching algorithms.

pub mod coma;
pub mod cupid;
pub mod distribution;
pub mod flooding;
pub mod jaccard;
pub mod strings;

pub use coma::{Coma, ComaConfig};
pub use cupid::{Cupid, CupidConfig};
pub use distribution::DistributionBased;
pub use flooding::{Formula, Policy, SimilarityFlooding, StringMatcher};
pub use jaccard::{
    EmbeddingProvider, JaccardConfig, JaccardDistanceMatcher, PrecomputedEmbeddings,
};
pub use strings::StringDistanceFunction;

use crate::{Error, MatchOptions, MatcherResults, Schema, Table, WorkLimits};

/// Matchers with checked implementations supported by
/// [`match_tables_one_to_one_with_limits`]. Other matchers are deliberately not
/// accepted: this path never falls back to unchecked matching.
#[derive(Clone, Copy, Debug)]
pub enum CheckedMatcher<'a> {
    Jaccard(&'a JaccardDistanceMatcher),
    DistributionBased(&'a DistributionBased),
}

/// Match with explicit work limits, then apply checked legacy Hungarian selection.
///
/// Sampling and batch-wide statistics are identical to [`match_tables`]. The same
/// limits check matching and the combined batch's assignment; their existing
/// per-matrix/per-model or batch-wide scopes are unchanged, rather than cumulative
/// budgets. `threshold` has the semantics of
/// [`MatcherResults::one_to_one_hungarian`], including its default cutoff.
///
/// Input validation, sampling, value preparation, string lengths and embedding
/// execution are not bounded by these counts. No metrics are computed: calling
/// `get_metrics` afterward can invoke an unchecked assignment. Limits are not
/// process-memory limits or deadlines; use external isolation for hard containment.
pub fn match_tables_one_to_one_with_limits(
    tables: &[Table],
    matcher: CheckedMatcher<'_>,
    options: MatchOptions,
    threshold: Option<f64>,
    limits: &WorkLimits,
) -> Result<MatcherResults, Error> {
    struct Checked<'a> {
        matcher: CheckedMatcher<'a>,
        limits: &'a WorkLimits,
    }
    impl Matcher for Checked<'_> {
        fn get_matches(&self, source: &Table, target: &Table) -> Result<MatcherResults, Error> {
            match self.matcher {
                CheckedMatcher::Jaccard(matcher) => {
                    matcher.get_matches_with_limits(source, target, self.limits)
                }
                CheckedMatcher::DistributionBased(matcher) => {
                    matcher.get_matches_with_limits(source, target, self.limits)
                }
            }
        }

        fn get_matches_batch(&self, tables: &[Table]) -> Result<MatcherResults, Error> {
            match self.matcher {
                CheckedMatcher::Jaccard(matcher) => {
                    matcher.get_matches_batch_with_limits(tables, self.limits)
                }
                CheckedMatcher::DistributionBased(matcher) => {
                    matcher.get_matches_batch_with_limits(tables, self.limits)
                }
            }
        }
    }
    match_tables(tables, &Checked { matcher, limits }, options)?
        .one_to_one_hungarian_with_limits(threshold, limits)
}

/// Implement this trait to plug a custom algorithm into batch matching.
pub trait Matcher {
    /// Match two tables. Built-in matchers require valid tables with distinct names.
    fn get_matches(&self, source: &Table, target: &Table) -> Result<MatcherResults, Error>;

    /// Match every unique table pair. Algorithms may override this for global statistics.
    /// Built-in implementations and this default validate all table identities first.
    /// Empty and single-table batches return empty results.
    fn get_matches_batch(&self, tables: &[Table]) -> Result<MatcherResults, Error> {
        validate_tables(tables)?;
        let mut entries = Vec::new();
        let mut details = crate::MatchDetails::new();
        for i in 0..tables.len() {
            for j in i + 1..tables.len() {
                let matches = self.get_matches(&tables[i], &tables[j])?;
                entries.extend(matches.iter().map(|(p, s)| (p.clone(), *s)));
                details.extend(matches.details().clone());
            }
        }
        MatcherResults::with_details(entries, details)
    }
}

fn validate_tables<'a>(tables: impl IntoIterator<Item = &'a Table>) -> Result<(), Error> {
    let mut names = std::collections::BTreeSet::new();
    for table in tables {
        table.validate()?;
        if !names.insert(&table.name) {
            return Err(Error::InvalidInput(format!(
                "duplicate table name: {}",
                table.name
            )));
        }
    }
    Ok(())
}

/// Match at least two named tables, keeping all instance values.
pub fn valentine_match(tables: &[Table], matcher: &dyn Matcher) -> Result<MatcherResults, Error> {
    match_tables(
        tables,
        matcher,
        MatchOptions {
            instance_sample_size: None,
        },
    )
}

/// Match tables with deterministic, evenly spaced sampling of nonempty instance values.
pub fn match_tables(
    tables: &[Table],
    matcher: &dyn Matcher,
    options: MatchOptions,
) -> Result<MatcherResults, Error> {
    if tables.len() < 2 {
        return Err(Error::InvalidInput(
            "at least two tables are required".into(),
        ));
    }
    validate_tables(tables)?;
    if options.instance_sample_size.is_none()
        && tables
            .iter()
            .flat_map(|table| &table.columns)
            .all(|column| column.samples.iter().all(|value| !value.is_empty()))
    {
        return matcher.get_matches_batch(tables);
    }
    let sampled: Vec<_> = tables
        .iter()
        .map(|t| t.sampled(options.instance_sample_size))
        .collect();
    matcher.get_matches_batch(&sampled)
}

/// Match the existing Fieldkin schema representation with a Valentine algorithm.
pub fn match_schemas_with(
    source: &Schema,
    target: &Schema,
    matcher: &dyn Matcher,
) -> Result<MatcherResults, Error> {
    valentine_match(
        &[
            Table::from_schema("aaa", source),
            Table::from_schema("bbb", target),
        ],
        matcher,
    )
}
