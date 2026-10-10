//! Native value-set matching with Valentine's directional fuzzy Jaccard reduction.

use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    sync::Arc,
};

use serde::{Deserialize, Serialize};

use crate::{ColumnPair, Error, Matcher, MatcherResults, Table};

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

    /// Activate cosine embedding comparisons in place of `distance_fun`.
    pub fn with_embedding_provider(mut self, provider: Arc<dyn EmbeddingProvider>) -> Self {
        self.embedding_provider = Some(provider);
        self
    }

    /// Compare two value collections directly. Duplicates are ignored and empty
    /// collections have similarity zero, including when both are empty.
    pub fn set_similarity(&self, source: &[String], target: &[String]) -> Result<f64, Error> {
        let source = unique_values(source);
        let target = unique_values(target);
        let embeddings = self.prepare_embeddings(source.iter().chain(&target).copied())?;
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
        source: &[&str],
        target: &[&str],
        embeddings: Option<&BTreeMap<String, Vec<f64>>>,
    ) -> f64 {
        if source.is_empty() || target.is_empty() {
            return 0.0;
        }
        let (a, b) = if source.len() <= target.len() {
            (source, target)
        } else {
            (target, source)
        };
        if embeddings.is_none() && self.config.distance_fun == StringDistanceFunction::Exact {
            let intersection = a
                .iter()
                .filter(|value| b.binary_search(value).is_ok())
                .count();
            return self.aggregate(intersection, intersection, a.len(), b.len());
        }
        let mut a_match = 0;
        let mut b_hits = vec![false; b.len()];
        let mut b_match = 0;
        // RapidFuzz's Cython scorer converts score_cutoff through a C float
        // before comparing its f64 similarity. Its cdist result also uses f32.
        // Preserve this subtle boundary behavior rather than changing matches.
        let threshold = if embeddings.is_some() {
            self.config.threshold_dist
        } else {
            f64::from(self.config.threshold_dist as f32)
        };
        for value_a in a {
            let mut a_hit = false;
            for (j, value_b) in b.iter().enumerate() {
                // Only the existence of a qualifying partner matters. A pair
                // whose endpoints both have partners cannot change the counts.
                if a_hit && b_hits[j] {
                    continue;
                }
                let score = if let Some(embeddings) = embeddings {
                    // prepare_embeddings supplies every string from these sets.
                    embeddings[*value_a]
                        .iter()
                        .zip(&embeddings[*value_b])
                        .map(|(x, y)| x * y)
                        .sum::<f64>()
                        .clamp(-1.0, 1.0)
                } else {
                    similarity(value_a, value_b, self.config.distance_fun)
                };
                if score >= threshold {
                    a_hit = true;
                    if !b_hits[j] {
                        b_hits[j] = true;
                        b_match += 1;
                    }
                    if b_match == b.len() {
                        break;
                    }
                }
            }
            a_match += usize::from(a_hit);
        }
        self.aggregate(a_match, b_match, a.len(), b.len())
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

    fn match_tables(&self, tables: &[&Table]) -> Result<MatcherResults, Error> {
        self.config.validate()?;
        let mut names = BTreeSet::new();
        for table in tables {
            table.validate()?;
            if !names.insert(&table.name) {
                return Err(Error::InvalidInput(format!(
                    "duplicate table name: {}",
                    table.name
                )));
            }
        }
        let values: Vec<Vec<Vec<&str>>> = tables
            .iter()
            .map(|table| {
                table
                    .columns
                    .iter()
                    .map(|column| {
                        unique_values(&column.samples)
                            .into_iter()
                            .filter(|value| !value.is_empty())
                            .collect()
                    })
                    .collect()
            })
            .collect();
        let embeddings = self.prepare_embeddings(values.iter().flatten().flatten().copied())?;
        let mut pairs = Vec::new();
        for (i, source) in tables.iter().enumerate() {
            for (j, target) in tables.iter().enumerate().skip(i + 1) {
                for (source_index, source_column) in source.columns.iter().enumerate() {
                    for (target_index, target_column) in target.columns.iter().enumerate() {
                        pairs.push((
                            ColumnPair::new(
                                source.name.clone(),
                                source_column.name.clone(),
                                target.name.clone(),
                                target_column.name.clone(),
                            ),
                            values[i][source_index].as_slice(),
                            values[j][target_index].as_slice(),
                        ));
                    }
                }
            }
        }
        let score_chunk = |chunk: &[(ColumnPair, &[&str], &[&str])]| {
            chunk
                .iter()
                .filter_map(|(pair, source, target)| {
                    let score = self.compare_sets(source, target, embeddings.as_ref());
                    (score > 0.0).then(|| (pair.clone(), score))
                })
                .collect::<Vec<_>>()
        };
        let matches =
            if self.config.process_num == 1 || pairs.len() < 2 {
                score_chunk(&pairs)
            } else {
                std::thread::scope(|scope| {
                    let workers = self.config.process_num.min(pairs.len());
                    let chunks = pairs.chunks(pairs.len().div_ceil(workers));
                    let handles: Vec<_> = chunks
                        .map(|chunk| scope.spawn(move || score_chunk(chunk)))
                        .collect();
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
        self.match_tables(&[source, target])
    }

    fn get_matches_batch(&self, tables: &[Table]) -> Result<MatcherResults, Error> {
        self.match_tables(&tables.iter().collect::<Vec<_>>())
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
