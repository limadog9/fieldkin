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

use crate::{Error, MatchOptions, MatcherResults, Schema, Table};

/// Implement this trait to plug a custom algorithm into batch matching.
pub trait Matcher {
    fn get_matches(&self, source: &Table, target: &Table) -> Result<MatcherResults, Error>;

    /// Match every unique table pair. Algorithms may override this for global statistics.
    fn get_matches_batch(&self, tables: &[Table]) -> Result<MatcherResults, Error> {
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
