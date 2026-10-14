//! Native value-set matching with Valentine's directional fuzzy Jaccard reduction.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    ops::Range,
    sync::{Arc, OnceLock},
};

use rapidfuzz::distance::levenshtein::{Args, BatchComparator};
use serde::{Deserialize, Serialize};

use crate::{ColumnPair, Error, Matcher, MatcherResults, Table, WorkLimits};

use super::strings::{StringDistanceFunction, similarity};

/// Configuration for lexical or embedding value-set matching.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct JaccardConfig {
    /// Inclusive normalized value-similarity threshold. Lexical comparisons round
    /// the cutoff to `f32`, preserving upstream RapidFuzz behavior at boundaries.
    /// Exact equality ignores it.
    pub threshold_dist: f64,
    pub distance_fun: StringDistanceFunction,
    /// Maximum native worker threads used to score independent column pairs.
    pub process_num: usize,
    /// Penalty on unmatched values on the reference side.
    pub tversky_alpha: f64,
    /// Penalty on unmatched values on the other side.
    pub tversky_beta: f64,
}

impl Default for JaccardConfig {
    fn default() -> Self {
        Self {
            threshold_dist: 0.8,
            distance_fun: StringDistanceFunction::Levenshtein,
            process_num: 1,
            tversky_alpha: 1.0,
            tversky_beta: 1.0,
        }
    }
}

impl JaccardConfig {
    pub fn validate(&self) -> Result<(), Error> {
        if !self.threshold_dist.is_finite() || !(0.0..=1.0).contains(&self.threshold_dist) {
            return Err(Error::InvalidConfig(
                "threshold_dist must be finite and between 0 and 1".into(),
            ));
        }
        if self.process_num == 0 {
            return Err(Error::InvalidConfig(
                "process_num must be at least 1".into(),
            ));
        }
        if !self.tversky_alpha.is_finite()
            || !self.tversky_beta.is_finite()
            || self.tversky_alpha < 0.0
            || self.tversky_beta < 0.0
        {
            return Err(Error::InvalidConfig(
                "Tversky alpha and beta must be finite and nonnegative".into(),
            ));
        }
        Ok(())
    }
}

/// Supplies real embedding vectors in the same order as the requested strings.
///
/// Implementations can run a native model or retrieve previously computed
/// vectors. Configure the model, device and batch size on the provider. The
/// matcher globally deduplicates strings before one encode call per batch, checks
/// dimensions and values, and normalizes vectors before cosine comparisons.
pub trait EmbeddingProvider: Send + Sync {
    fn encode(&self, values: &[String]) -> Result<Vec<Vec<f64>>, Error>;
}

/// A provider backed by caller-supplied embeddings, with no Python runtime.
#[derive(Clone, Debug)]
pub struct PrecomputedEmbeddings {
    vectors: BTreeMap<String, Vec<f64>>,
}

impl PrecomputedEmbeddings {
    /// Validate vectors eagerly. An empty vocabulary is allowed; populated
    /// vocabularies must contain finite, nonzero vectors of a common dimension.
    pub fn new(vectors: impl IntoIterator<Item = (String, Vec<f64>)>) -> Result<Self, Error> {
        let mut map = BTreeMap::new();
        for (value, vector) in vectors {
            if map.insert(value.clone(), vector).is_some() {
                return Err(Error::InvalidConfig(format!(
                    "duplicate precomputed embedding for {value:?}"
                )));
            }
        }
        normalize_vectors(map.values().cloned().collect(), map.len())?;
        Ok(Self { vectors: map })
    }
}

impl EmbeddingProvider for PrecomputedEmbeddings {
    fn encode(&self, values: &[String]) -> Result<Vec<Vec<f64>>, Error> {
        values
            .iter()
            .map(|value| {
                self.vectors
                    .get(value)
                    .cloned()
                    .ok_or_else(|| Error::InvalidConfig(format!("missing embedding for {value:?}")))
            })
            .collect()
    }
}

/// Compare distinct, case-sensitive column values with lexical or embedding
/// similarities, then reduce both directional match counts with Tversky.
///
/// Each value counts as matched when it has *any* partner above the threshold.
/// Several values may therefore match the same partner, faithfully preserving
/// Valentine's fuzzy-set behavior. The larger directional score is returned.
/// Fuzzy comparisons can still require the Cartesian product of distinct values
/// for each column pair. Use explicit table sampling to limit value counts;
/// [`Self::get_matches_with_limits`] can reject excessive column-pair and
/// distinct-value comparison counts before scoring. These counts do not bound
/// string lengths, model execution, process memory, or wall-clock time.
#[derive(Clone, Default)]
pub struct JaccardDistanceMatcher {
    config: JaccardConfig,
    embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
}

impl fmt::Debug for JaccardDistanceMatcher {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("JaccardDistanceMatcher")
            .field("config", &self.config)
            .field("embedding", &self.embedding_provider.is_some())
            .finish()
    }
}

impl JaccardDistanceMatcher {
    pub fn new(config: JaccardConfig) -> Result<Self, Error> {
        config.validate()?;
        Ok(Self {
            config,
            embedding_provider: None,
        })
    }

    pub fn config(&self) -> &JaccardConfig {
        &self.config
    }

    /// Match two tables with opt-in limits on column pairs and the conservative
    /// Cartesian product of distinct, nonempty sample values. The latter is
    /// checked after value preparation, before embedding inference or scoring.
    /// Exact and identical-set shortcuts may do less work than this estimate.
    pub fn get_matches_with_limits(
        &self,
        source: &Table,
        target: &Table,
        limits: &WorkLimits,
    ) -> Result<MatcherResults, Error> {
        self.match_tables(&[source, target], Some(limits))
    }

    /// Apply the same limits to the aggregate work across every table pair.
    /// Accepted calls retain batch-wide vocabulary preparation and encoding.
    pub fn get_matches_batch_with_limits(
        &self,
        tables: &[Table],
        limits: &WorkLimits,
    ) -> Result<MatcherResults, Error> {
        self.match_tables(&tables.iter().collect::<Vec<_>>(), Some(limits))
    }

    /// Activate cosine embedding comparisons in place of `distance_fun`.
    pub fn with_embedding_provider(mut self, provider: Arc<dyn EmbeddingProvider>) -> Self {
        self.embedding_provider = Some(provider);
        self
    }

    /// Compare two value collections directly. Duplicates are ignored and empty
    /// collections have similarity zero, including when both are empty.
    pub fn set_similarity(&self, source: &[String], target: &[String]) -> Result<f64, Error> {
        let source = PreparedValues::new(unique_values(source));
        let target = PreparedValues::new(unique_values(target));
        let embeddings =
            self.prepare_embeddings(source.values.iter().chain(&target.values).copied())?;
        Ok(self.compare_sets(&source, &target, embeddings.as_ref()))
    }

    fn prepare_embeddings<'a>(
        &self,
        values: impl IntoIterator<Item = &'a str>,
    ) -> Result<Option<BTreeMap<String, Vec<f64>>>, Error> {
        let Some(provider) = &self.embedding_provider else {
            return Ok(None);
        };
        let values: Vec<String> = values
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        if values.is_empty() {
            return Ok(Some(BTreeMap::new()));
        }
        let encoded = normalize_vectors(provider.encode(&values)?, values.len())?;
        Ok(Some(values.into_iter().zip(encoded).collect()))
    }

    fn compare_sets(
        &self,
        source: &PreparedValues<'_>,
        target: &PreparedValues<'_>,
        embeddings: Option<&BTreeMap<String, Vec<f64>>>,
    ) -> f64 {
        if source.values.is_empty() || target.values.is_empty() {
            return 0.0;
        }
        let (a, b) = if source.values.len() <= target.values.len() {
            (source, target)
        } else {
            (target, source)
        };
        if embeddings.is_none() && self.config.distance_fun == StringDistanceFunction::Exact {
            let intersection = a
                .values
                .iter()
                .filter(|value| b.values.binary_search(value).is_ok())
                .count();
            return self.aggregate(intersection, intersection, a.values.len(), b.values.len());
        }
        // Sorted identical lexical sets already give every value a partner.
        // Cosine must retain its actual rounded dot products, even for equality.
        if embeddings.is_none() && a.values == b.values {
            return 1.0;
        }
        let mut a_match = 0;
        let mut b_hits = vec![false; b.values.len()];
        let mut b_match = 0;
        // RapidFuzz's Cython scorer converts score_cutoff through a C float
        // before comparing its f64 similarity. Its cdist result also uses f32.
        // Preserve this subtle boundary behavior rather than changing matches.
        let threshold = if embeddings.is_some() {
            self.config.threshold_dist
        } else {
            f64::from(self.config.threshold_dist as f32)
        };
        let bounds = (embeddings.is_none()
            && self.config.distance_fun == StringDistanceFunction::Levenshtein
            && threshold > 0.0)
            .then(|| (a.bounds(), b.bounds()));
        let levenshtein_args = Args::default().score_cutoff(threshold);
        for (i, value_a) in a.values.iter().enumerate() {
            // Reuse the pattern for one-to-many comparisons and stop scoring
            // below the same rounded cutoff used to count qualifying partners.
            let levenshtein = (embeddings.is_none()
                && self.config.distance_fun == StringDistanceFunction::Levenshtein
                && b.values.len() > 1
                && b_match < b.values.len())
            .then(|| BatchComparator::new(value_a.chars()));
            let mut a_hit = false;
            let (a_length, a_bigrams) = bounds.map_or((0, [0; 2]), |(a, _)| a[i]);
            for (j, value_b) in b.values.iter().enumerate() {
                // Only the existence of a qualifying partner matters. A pair
                // whose endpoints both have partners cannot change the counts.
                if a_hit && b_hits[j] {
                    continue;
                }
                if let Some((_, b_bounds)) = bounds {
                    let (b_length, b_bigrams) = b_bounds[j];
                    // An edit changes at most four bigram-presence bits. Hash
                    // collisions only weaken this lower bound. Combine it with
                    // the length bound and retain the scorer's normalization.
                    let changed_bits = (a_bigrams[0] ^ b_bigrams[0]).count_ones()
                        + (a_bigrams[1] ^ b_bigrams[1]).count_ones();
                    let distance_bound = a_length
                        .abs_diff(b_length)
                        .max(changed_bits.div_ceil(4) as usize);
                    if distance_bound > 0
                        && 1.0 - distance_bound as f64 / (a_length.max(b_length) as f64) < threshold
                    {
                        continue;
                    }
                }
                let score = if let Some(embeddings) = embeddings {
                    // prepare_embeddings supplies every string from these sets.
                    embeddings[*value_a]
                        .iter()
                        .zip(&embeddings[*value_b])
                        .map(|(x, y)| x * y)
                        .sum::<f64>()
                        .clamp(-1.0, 1.0)
                } else if let Some(levenshtein) = &levenshtein {
                    levenshtein
                        .normalized_similarity_with_args(value_b.chars(), &levenshtein_args)
                        .unwrap_or(0.0)
                } else {
                    similarity(value_a, value_b, self.config.distance_fun)
                };
                if score >= threshold {
                    a_hit = true;
                    if !b_hits[j] {
                        b_hits[j] = true;
                        b_match += 1;
                    }
                    if b_match == b.values.len() {
                        break;
                    }
                }
            }
            a_match += usize::from(a_hit);
        }
        self.aggregate(a_match, b_match, a.values.len(), b.values.len())
    }

    fn aggregate(&self, a_match: usize, b_match: usize, a_size: usize, b_size: usize) -> f64 {
        let a_unmatched = (a_size - a_match) as f64;
        let b_unmatched = (b_size - b_match) as f64;
        let a_match = a_match as f64;
        let b_match = b_match as f64;
        let alpha = self.config.tversky_alpha;
        let beta = self.config.tversky_beta;
        let denom_ab = a_match + alpha * a_unmatched + beta * b_unmatched;
        let denom_ba = b_match + alpha * b_unmatched + beta * a_unmatched;
        let ab = if denom_ab > 0.0 {
            a_match / denom_ab
        } else {
            0.0
        };
        let ba = if denom_ba > 0.0 {
            b_match / denom_ba
        } else {
            0.0
        };
        ab.max(ba)
    }

    fn match_tables(
        &self,
        tables: &[&Table],
        limits: Option<&WorkLimits>,
    ) -> Result<MatcherResults, Error> {
        self.config.validate()?;
        super::validate_tables(tables.iter().copied())?;
        let pair_count = checked_product_sum(tables.iter().enumerate().flat_map(|(i, source)| {
            tables[i + 1..]
                .iter()
                .map(move |target| (source.columns.len(), target.columns.len()))
        }));
        if let Some(limits) = limits {
            crate::limits::check_work("Jaccard column pairs", pair_count, limits.max_matrix_cells)?;
        }
        let pair_count = pair_count
            .ok_or_else(|| Error::Algorithm("Jaccard column-pair count overflow".into()))?;
        let values: Vec<Vec<PreparedValues<'_>>> = tables
            .iter()
            .map(|table| {
                table
                    .columns
                    .iter()
                    .map(|column| {
                        PreparedValues::new(
                            unique_values(&column.samples)
                                .into_iter()
                                .filter(|value| !value.is_empty())
                                .collect(),
                        )
                    })
                    .collect()
            })
            .collect();
        if let Some(limits) = limits {
            let comparisons =
                checked_product_sum(values.iter().enumerate().flat_map(|(i, source)| {
                    values[i + 1..].iter().flat_map(move |target| {
                        source.iter().flat_map(move |a| {
                            target.iter().map(move |b| (a.values.len(), b.values.len()))
                        })
                    })
                }));
            crate::limits::check_work(
                "Jaccard distinct-value comparisons",
                comparisons,
                limits.max_jaccard_comparisons,
            )?;
        }
        let embeddings = self.prepare_embeddings(
            values
                .iter()
                .flatten()
                .flat_map(|column| column.values.iter().copied()),
        )?;
        let score_range = |range: Range<usize>| {
            let mut matches = Vec::new();
            let mut pair_start = 0;
            for (i, source) in tables.iter().enumerate() {
                for (j, target) in tables.iter().enumerate().skip(i + 1) {
                    if pair_start >= range.end {
                        return matches;
                    }
                    // The checked total above bounds each product and offset.
                    let pair_end = pair_start + source.columns.len() * target.columns.len();
                    if pair_end > range.start {
                        let start = range.start.saturating_sub(pair_start);
                        let end = range.end.min(pair_end) - pair_start;
                        for index in start..end {
                            let source_index = index / target.columns.len();
                            let target_index = index % target.columns.len();
                            let score = self.compare_sets(
                                &values[i][source_index],
                                &values[j][target_index],
                                embeddings.as_ref(),
                            );
                            if score > 0.0 {
                                matches.push((
                                    ColumnPair::new(
                                        &source.name,
                                        &source.columns[source_index].name,
                                        &target.name,
                                        &target.columns[target_index].name,
                                    ),
                                    score,
                                ));
                            }
                        }
                    }
                    pair_start = pair_end;
                }
            }
            matches
        };
        let matches =
            if self.config.process_num == 1 || pair_count < 2 {
                score_range(0..pair_count)
            } else {
                std::thread::scope(|scope| {
                    let workers = self.config.process_num.min(pair_count);
                    let chunk_size = pair_count.div_ceil(workers);
                    let handles: Vec<_> = (0..pair_count)
                        .step_by(chunk_size)
                        .map(|start| {
                            let end = start.saturating_add(chunk_size).min(pair_count);
                            std::thread::Builder::new()
                                .spawn_scoped(scope, move || score_range(start..end))
                        })
                        .collect::<Result<_, _>>()?;
                    let mut matches = Vec::new();
                    for handle in handles {
                        matches.extend(handle.join().map_err(|_| {
                            Error::Algorithm("Jaccard scoring worker panicked".into())
                        })?);
                    }
                    Ok::<_, Error>(matches)
                })?
            };
        MatcherResults::new(matches)
    }
}

impl Matcher for JaccardDistanceMatcher {
    fn get_matches(&self, source: &Table, target: &Table) -> Result<MatcherResults, Error> {
        self.match_tables(&[source, target], None)
    }

    fn get_matches_batch(&self, tables: &[Table]) -> Result<MatcherResults, Error> {
        self.match_tables(&tables.iter().collect::<Vec<_>>(), None)
    }
}

fn checked_product_sum(pairs: impl IntoIterator<Item = (usize, usize)>) -> Option<usize> {
    pairs.into_iter().try_fold(0_usize, |total, (a, b)| {
        a.checked_mul(b).and_then(|count| total.checked_add(count))
    })
}

struct PreparedValues<'a> {
    values: Vec<&'a str>,
    // Shared only during this matching call, including its scoring workers.
    bounds: OnceLock<Vec<(usize, [u64; 2])>>,
}

impl<'a> PreparedValues<'a> {
    fn new(values: Vec<&'a str>) -> Self {
        Self {
            values,
            bounds: OnceLock::new(),
        }
    }

    fn bounds(&self) -> &[(usize, [u64; 2])] {
        self.bounds.get_or_init(|| {
            self.values
                .iter()
                .map(|value| levenshtein_metadata(value))
                .collect()
        })
    }
}

fn unique_values(values: &[String]) -> Vec<&str> {
    values
        .iter()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn levenshtein_metadata(value: &str) -> (usize, [u64; 2]) {
    let mut length = 0;
    let mut bigrams = [0; 2];
    let mut previous = None;
    for character in value.chars() {
        let character = u64::from(u32::from(character));
        if let Some(previous) = previous.replace(character) {
            let pair = (previous << 32) | character;
            // The hash need not be collision-free: merged bits cannot make
            // one edit change more than four bits in the presence bitmap.
            let bucket = (pair.wrapping_mul(0x9e37_79b9_7f4a_7c15) >> 57) as usize;
            bigrams[bucket >> 6] |= 1 << (bucket & 63);
        }
        length += 1;
    }
    (length, bigrams)
}

fn normalize_vectors(
    vectors: Vec<Vec<f64>>,
    expected_count: usize,
) -> Result<Vec<Vec<f64>>, Error> {
    if vectors.len() != expected_count {
        return Err(Error::InvalidConfig(format!(
            "embedding provider returned {} vectors for {expected_count} values",
            vectors.len()
        )));
    }
    let dimension = vectors.first().map_or(0, Vec::len);
    let mut normalized = Vec::with_capacity(vectors.len());
    for vector in vectors {
        if dimension == 0 || vector.len() != dimension {
            return Err(Error::InvalidConfig(
                "embeddings must have a common nonzero dimension".into(),
            ));
        }
        if vector.iter().any(|value| !value.is_finite()) {
            return Err(Error::InvalidConfig(
                "embedding values must be finite".into(),
            ));
        }
        // Scale before computing the norm: very large and tiny finite vectors
        // normalize without overflow or underflow in their squared components.
        let scale = vector
            .iter()
            .fold(0.0_f64, |largest, x| largest.max(x.abs()));
        if scale == 0.0 {
            return Err(Error::InvalidConfig(
                "embedding vectors must be nonzero".into(),
            ));
        }
        let scaled: Vec<_> = vector.into_iter().map(|value| value / scale).collect();
        let norm = scaled.iter().map(|value| value * value).sum::<f64>().sqrt();
        normalized.push(scaled.into_iter().map(|value| value / norm).collect());
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn table(name: &str, columns: &[(&str, &[&str])]) -> Table {
        Table::new(
            name,
            columns
                .iter()
                .map(|(name, samples)| crate::Field {
                    name: (*name).into(),
                    data_type: crate::DataType::Text,
                    samples: samples.iter().map(|value| (*value).into()).collect(),
                })
                .collect(),
        )
        .unwrap()
    }

    fn assert_same_results(actual: &MatcherResults, expected: &MatcherResults) {
        let bits = |results: &MatcherResults| {
            results
                .iter()
                .map(|(pair, score)| (pair.clone(), score.to_bits()))
                .collect::<Vec<_>>()
        };
        assert_eq!(bits(actual), bits(expected));
        assert_eq!(actual.details(), expected.details());
    }

    #[test]
    fn guarded_work_counts_check_products_and_sums_without_allocating() {
        assert_eq!(checked_product_sum([]), Some(0));
        assert_eq!(checked_product_sum([(1, 10_000)]), Some(10_000));
        assert_eq!(checked_product_sum([(usize::MAX, 0)]), Some(0));
        assert_eq!(checked_product_sum([(usize::MAX, 1)]), Some(usize::MAX));
        assert_eq!(checked_product_sum([(usize::MAX, 2)]), None);
        assert_eq!(checked_product_sum([(usize::MAX, 1), (1, 1)]), None);
        assert!(
            crate::limits::check_work(
                "Jaccard distinct-value comparisons",
                checked_product_sum([(usize::MAX, 2)]),
                usize::MAX,
            )
            .is_err()
        );
    }

    #[test]
    fn guarded_direct_and_batch_calls_preserve_scores_and_aggregate_limits() {
        let tables = [
            table("a", &[("v", &["cat", "cat", "dog", ""]), ("empty", &[""])]),
            table("b", &[("v", &["cat", "cats"]), ("w", &["dog"])]),
            table("c", &[("v", &["dogs", "dog"])]),
        ];
        // Duplicates and empty strings do not increase distinct-value work:
        // table totals 2, 3, 2 give 2*3 + 2*2 + 3*2 = 16 comparisons.
        let limits = WorkLimits {
            max_matrix_cells: 8,
            max_jaccard_comparisons: 16,
            ..Default::default()
        };
        for process_num in [1, 4] {
            for distance_fun in [
                StringDistanceFunction::Exact,
                StringDistanceFunction::Levenshtein,
            ] {
                let matcher = JaccardDistanceMatcher::new(JaccardConfig {
                    process_num,
                    distance_fun,
                    ..Default::default()
                })
                .unwrap();
                assert_same_results(
                    &matcher
                        .get_matches_batch_with_limits(&tables, &limits)
                        .unwrap(),
                    &matcher.get_matches_batch(&tables).unwrap(),
                );
                let direct_limits = WorkLimits {
                    max_matrix_cells: 4,
                    max_jaccard_comparisons: 6,
                    ..Default::default()
                };
                assert_same_results(
                    &matcher
                        .get_matches_with_limits(&tables[0], &tables[1], &direct_limits)
                        .unwrap(),
                    &matcher.get_matches(&tables[0], &tables[1]).unwrap(),
                );
                for limits in [
                    WorkLimits {
                        max_matrix_cells: 7,
                        ..limits
                    },
                    WorkLimits {
                        max_jaccard_comparisons: 15,
                        ..limits
                    },
                ] {
                    assert!(
                        matcher
                            .get_matches_batch_with_limits(&tables, &limits)
                            .is_err()
                    );
                }
                assert!(
                    matcher
                        .get_matches_with_limits(
                            &tables[0],
                            &tables[1],
                            &WorkLimits {
                                max_jaccard_comparisons: 5,
                                ..Default::default()
                            }
                        )
                        .is_err()
                );
            }
        }
    }

    #[test]
    fn guarded_calls_reject_before_encoding_and_encode_accepted_batch_once() {
        #[derive(Default)]
        struct CountingProvider(std::sync::Mutex<Vec<Vec<String>>>);
        impl EmbeddingProvider for CountingProvider {
            fn encode(&self, values: &[String]) -> Result<Vec<Vec<f64>>, Error> {
                self.0.lock().unwrap().push(values.to_vec());
                Ok(values.iter().map(|_| vec![1.0, 0.0]).collect())
            }
        }
        let provider = Arc::new(CountingProvider::default());
        let matcher = JaccardDistanceMatcher::new(JaccardConfig {
            process_num: 4,
            ..Default::default()
        })
        .unwrap()
        .with_embedding_provider(provider.clone());
        let tables = [
            table("a", &[("v", &["a", "a", "", "b"])]),
            table("b", &[("v", &["b", "c"])]),
            table("c", &[("v", &["a", "d"])]),
        ];
        let limits = WorkLimits {
            max_matrix_cells: 3,
            max_jaccard_comparisons: 12,
            ..Default::default()
        };
        let error = matcher
            .get_matches_batch_with_limits(
                &tables,
                &WorkLimits {
                    max_jaccard_comparisons: 11,
                    ..limits
                },
            )
            .unwrap_err();
        let message = error.to_string();
        assert!(
            message.contains("Jaccard distinct-value comparisons"),
            "{message}"
        );
        assert!(
            message.contains("11") && message.contains("12"),
            "{message}"
        );
        assert!(
            matcher
                .get_matches_with_limits(
                    &tables[0],
                    &tables[1],
                    &WorkLimits {
                        max_jaccard_comparisons: 3,
                        ..limits
                    }
                )
                .is_err()
        );
        assert!(provider.0.lock().unwrap().is_empty());
        let actual = matcher
            .get_matches_batch_with_limits(&tables, &limits)
            .unwrap();
        assert_eq!(
            *provider.0.lock().unwrap(),
            vec![vec![
                "a".to_owned(),
                "b".to_owned(),
                "c".to_owned(),
                "d".to_owned()
            ]]
        );
        assert_same_results(&actual, &matcher.get_matches_batch(&tables).unwrap());

        let zero = WorkLimits {
            max_matrix_cells: 0,
            max_jaccard_comparisons: 0,
            ..Default::default()
        };
        assert!(
            matcher
                .get_matches_batch_with_limits(&[], &zero)
                .unwrap()
                .is_empty()
        );
        assert!(
            matcher
                .get_matches_batch_with_limits(&[table("empty", &[])], &zero)
                .unwrap()
                .is_empty()
        );
        assert!(
            JaccardDistanceMatcher::default()
                .get_matches_batch_with_limits(&tables[..1], &zero)
                .unwrap()
                .is_empty()
        );
        let empty_columns = [
            table("empty_a", &[("a", &[""])]),
            table("empty_b", &[("b", &[]), ("c", &[""])]),
        ];
        assert!(
            matcher
                .get_matches_batch_with_limits(&empty_columns, &zero)
                .is_err()
        );
        assert!(
            matcher
                .get_matches_batch_with_limits(
                    &empty_columns,
                    &WorkLimits {
                        max_matrix_cells: 2,
                        ..zero
                    }
                )
                .unwrap()
                .is_empty()
        );
        // Empty vocabularies do not invoke the provider.
        assert_eq!(provider.0.lock().unwrap().len(), 2);
    }

    fn edit_distance(a: &str, b: &str) -> usize {
        let b: Vec<_> = b.chars().collect();
        let mut row: Vec<_> = (0..=b.len()).collect();
        for (i, a) in a.chars().enumerate() {
            let mut next = vec![i + 1];
            for (j, b) in b.iter().enumerate() {
                next.push(
                    (next[j] + 1)
                        .min(row[j + 1] + 1)
                        .min(row[j] + usize::from(a != *b)),
                );
            }
            row = next;
        }
        row[b.len()]
    }

    #[test]
    fn bigram_bound_is_below_exhaustive_unicode_edit_distance() {
        let alphabet = ['a', 'é', '🙂'];
        let mut values = vec![String::new()];
        for length in 1..=4 {
            for mut index in 0..alphabet.len().pow(length) {
                values.push(
                    (0..length)
                        .map(|_| {
                            let character = alphabet[index % alphabet.len()];
                            index /= alphabet.len();
                            character
                        })
                        .collect(),
                );
            }
        }
        // More distinct bigrams than bits also exercises hash collisions.
        values.extend((0..=255).map(|character| format!("a{}", char::from(character))));
        for a in &values {
            let (a_length, a_bigrams) = levenshtein_metadata(a);
            assert_eq!(a_length, a.chars().count());
            for b in &values {
                let (b_length, b_bigrams) = levenshtein_metadata(b);
                let changed_bits = (a_bigrams[0] ^ b_bigrams[0]).count_ones()
                    + (a_bigrams[1] ^ b_bigrams[1]).count_ones();
                let bound = a_length
                    .abs_diff(b_length)
                    .max(changed_bits.div_ceil(4) as usize);
                assert!(bound <= edit_distance(a, b), "{a:?} / {b:?}: {bound}");
            }
        }
    }
}
