//! Native COMA: complex name and global TF-IDF instance matchers followed by
//! bidirectional multiple-match selection. Flat tables use self-node context,
//! exactly as the current Valentine implementation does.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use serde::{Deserialize, Serialize};

use super::Matcher;
use super::strings::{tokens_similarity, trigram_similarity};
use crate::{ColumnPair, Error, MatcherResults, Table};

/// COMA complex matcher combination and selection parameters.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default)]
pub struct ComaConfig {
    /// Nth-score cutoff per source and per target; zero disables this cutoff.
    /// Ties at the cutoff are retained, as in upstream COMA.
    pub max_n: usize,
    pub use_instances: bool,
    pub use_schema: bool,
    /// Fraction below each column's best score to retain. Zero disables it.
    pub delta: f64,
    pub threshold: f64,
    pub instance_weight: f64,
}

impl Default for ComaConfig {
    fn default() -> Self {
        Self {
            max_n: 0,
            use_instances: false,
            use_schema: true,
            delta: 0.15,
            threshold: 0.0,
            instance_weight: 1.0,
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Coma {
    pub config: ComaConfig,
}

impl Coma {
    pub fn new(config: ComaConfig) -> Result<Self, Error> {
        let matcher = Self { config };
        matcher.validate()?;
        Ok(matcher)
    }

    fn validate(&self) -> Result<(), Error> {
        let c = &self.config;
        if !c.use_schema && !c.use_instances {
            return Err(Error::InvalidConfig(
                "at least one COMA matcher must be enabled".into(),
            ));
        }
        for (name, value) in [("delta", c.delta), ("threshold", c.threshold)] {
            if !(0.0..=1.0).contains(&value) {
                return Err(Error::InvalidConfig(format!("{name} must be in [0, 1]")));
            }
        }
        if !c.instance_weight.is_finite() || c.instance_weight < 0.0 {
            return Err(Error::InvalidConfig(
                "instance_weight must be finite and nonnegative".into(),
            ));
        }
        Ok(())
    }

    fn match_tables(&self, tables: &[&Table]) -> Result<MatcherResults, Error> {
        self.validate()?;
        super::validate_tables(tables.iter().copied())?;
        let corpus = self.config.use_instances.then(|| TfidfCorpus::new(tables));
        let mut output = Vec::new();
        let mut details = BTreeMap::new();
        for source_index in 0..tables.len() {
            for target_index in source_index + 1..tables.len() {
                let source = tables[source_index];
                let target = tables[target_index];
                let mut matrix = vec![vec![0.0; target.columns.len()]; source.columns.len()];
                let mut pair_details = BTreeMap::new();
                for (i, s) in source.columns.iter().enumerate() {
                    for (j, t) in target.columns.iter().enumerate() {
                        let mut scores = BTreeMap::new();
                        let mut weighted_sum = 0.0;
                        let mut weights = 0.0;
                        if self.config.use_schema {
                            let score = trigram_similarity(&s.name, &t.name)
                                .max(tokens_similarity(&s.name, &t.name));
                            scores.insert("NameCM".into(), score);
                            weighted_sum += score;
                            weights += 1.0;
                        }
                        if let Some(corpus) = &corpus {
                            let score = corpus.similarity(source_index, i, target_index, j);
                            scores.insert("InstancesCM".into(), score);
                            weighted_sum += self.config.instance_weight * score;
                            weights += self.config.instance_weight;
                        }
                        matrix[i][j] = if weights == 0.0 {
                            0.0
                        } else {
                            weighted_sum / weights
                        };
                        pair_details.insert((i, j), scores);
                    }
                }
                for (i, j, score) in select_both_multiple(&matrix, &self.config) {
                    let pair = ColumnPair::new(
                        &source.name,
                        &source.columns[i].name,
                        &target.name,
                        &target.columns[j].name,
                    );
                    output.push((pair.clone(), score));
                    details.insert(pair, pair_details.remove(&(i, j)).unwrap_or_default());
                }
            }
        }
        MatcherResults::with_details(output, details)
    }
}

impl Matcher for Coma {
    fn get_matches(&self, source: &Table, target: &Table) -> Result<MatcherResults, Error> {
        self.match_tables(&[source, target])
    }

    fn get_matches_batch(&self, tables: &[Table]) -> Result<MatcherResults, Error> {
        // A single corpus across every table is crucial: pairwise IDF changes
        // the scores when a term common to two tables is rare globally.
        self.match_tables(&tables.iter().collect::<Vec<_>>())
    }
}

fn minimum(scores: impl Iterator<Item = f64>, config: &ComaConfig) -> f64 {
    let mut scores: Vec<_> = scores.collect();
    scores.sort_by(|a, b| b.total_cmp(a));
    let mut cutoff = config.threshold;
    if !scores.is_empty() {
        if config.max_n > 0 {
            cutoff = cutoff.max(scores[config.max_n.min(scores.len()) - 1]);
        }
        if config.delta > 0.0 {
            cutoff = cutoff.max(scores[0] * (1.0 - config.delta));
        }
    }
    cutoff
}

fn select_both_multiple(matrix: &[Vec<f64>], config: &ComaConfig) -> Vec<(usize, usize, f64)> {
    let columns = matrix.first().map_or(0, Vec::len);
    let forward: Vec<_> = matrix
        .iter()
        .map(|r| minimum(r.iter().copied(), config))
        .collect();
    let backward: Vec<_> = (0..columns)
        .map(|j| minimum(matrix.iter().map(|row| row[j]), config))
        .collect();
    let mut selected = Vec::new();
    for (i, row) in matrix.iter().enumerate() {
        for (j, &score) in row.iter().enumerate() {
            if score > 0.0 && score >= forward[i] && score >= backward[j] {
                selected.push((i, j, score));
            }
        }
    }
    selected
}

/// NLTK's English stopword corpus, used for both COMA and Cupid token types.
pub(crate) fn english_stopword(word: &str) -> bool {
    const WORDS: &str = "a about above after again against ain all am an and any are aren aren't as at be because been before being below between both but by can couldn couldn't d did didn didn't do does doesn doesn't doing don don't down during each few for from further had hadn hadn't has hasn hasn't have haven haven't having he he'd he'll her here hers herself he's him himself his how i i'd if i'll i'm in into is isn isn't it it'd it'll it's its itself i've just ll m ma me mightn mightn't more most mustn mustn't my myself needn needn't no nor not now o of off on once only or other our ours ourselves out over own re s same shan shan't she she'd she'll she's should shouldn shouldn't should've so some such t than that that'll the their theirs them themselves then there these they they'd they'll they're they've this those through to too under until up ve very was wasn wasn't we we'd we'll we're were weren weren't we've what when where which while who whom why will with won won't wouldn wouldn't y you you'd you'll your you're yours yourself yourselves you've";
    // This small immutable lookup has no corpus initialization dependency.
    static STOPWORDS: std::sync::OnceLock<BTreeSet<&'static str>> = std::sync::OnceLock::new();
    STOPWORDS
        .get_or_init(|| WORDS.split_ascii_whitespace().collect())
        .contains(word)
}

fn document_tokens(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|word| !word.is_empty() && !english_stopword(word))
        .map(str::to_owned)
        .collect()
}

type Vector = BTreeMap<String, f64>;

struct TfidfColumn {
    vectors: Vec<Vector>,
    // Keep original document order and multiplicity for IDF and score sums.
    occurrences: Vec<usize>,
}

struct TfidfCorpus {
    columns: Vec<Vec<TfidfColumn>>,
}

impl TfidfCorpus {
    fn new(tables: &[&Table]) -> Self {
        let docs: Vec<Vec<_>> = tables
            .iter()
            .map(|table| {
                table
                    .columns
                    .iter()
                    .map(|column| {
                        let mut documents = Vec::new();
                        let mut occurrences = Vec::new();
                        // Lookup only; vector indices follow sample order.
                        let mut indices = HashMap::new();
                        for sample in column.samples.iter().filter(|s| !s.is_empty()) {
                            let index = if let Some(&index) = indices.get(sample.as_str()) {
                                index
                            } else {
                                let tokens = document_tokens(sample);
                                if tokens.is_empty() {
                                    continue;
                                }
                                let index = documents.len();
                                documents.push(tokens);
                                indices.insert(sample.as_str(), index);
                                index
                            };
                            occurrences.push(index);
                        }
                        (documents, occurrences)
                    })
                    .collect()
            })
            .collect();
        let mut document_frequency = BTreeMap::<&str, usize>::new();
        let mut count = 0;
        for (documents, occurrences) in docs.iter().flatten() {
            count += occurrences.len();
            let mut multiplicities = vec![0; documents.len()];
            for &index in occurrences {
                multiplicities[index] += 1;
            }
            for (doc, multiplicity) in documents.iter().zip(multiplicities) {
                for token in doc.iter().collect::<BTreeSet<_>>() {
                    *document_frequency.entry(token.as_str()).or_default() += multiplicity;
                }
            }
        }
        let idf: BTreeMap<_, _> = document_frequency
            .into_iter()
            .map(|(token, freq)| (token.to_owned(), (count as f64 / freq as f64).ln()))
            .collect();
        let columns = docs
            .into_iter()
            .map(|table| {
                table
                    .into_iter()
                    .map(|(documents, occurrences)| {
                        let vectors = documents
                            .into_iter()
                            .map(|doc| {
                                let mut tf = BTreeMap::<String, f64>::new();
                                for token in doc {
                                    *tf.entry(token).or_default() += 1.0;
                                }
                                let mut vector: Vector = tf
                                    .into_iter()
                                    .map(|(token, freq)| {
                                        let weight = freq.sqrt() * idf[&token];
                                        (token, weight)
                                    })
                                    .filter(|(_, weight)| *weight > 0.0)
                                    .collect();
                                let norm = vector.values().map(|v| v * v).sum::<f64>().sqrt();
                                if norm > 0.0 {
                                    for value in vector.values_mut() {
                                        *value /= norm;
                                    }
                                }
                                vector
                            })
                            .collect();
                        TfidfColumn {
                            vectors,
                            occurrences,
                        }
                    })
                    .collect()
            })
            .collect();
        Self { columns }
    }

    fn similarity(&self, source: usize, s: usize, target: usize, t: usize) -> f64 {
        let a = &self.columns[source][s];
        let b = &self.columns[target][t];
        if a.vectors.is_empty() || b.vectors.is_empty() {
            return 0.0;
        }
        // A shared token already rules out the disjoint-vocabulary fast path.
        if !a.vectors[0]
            .keys()
            .any(|token| b.vectors[0].contains_key(token))
        {
            let vocabulary: BTreeSet<_> =
                a.vectors.iter().flat_map(|vector| vector.keys()).collect();
            // Preserve signed zero from the original loop for empty source vectors.
            if !vocabulary.is_empty()
                && !b
                    .vectors
                    .iter()
                    .flat_map(|vector| vector.keys())
                    .any(|token| vocabulary.contains(token))
            {
                return 0.0;
            }
        }
        let mut row_max: Vec<f64> = vec![0.0; a.vectors.len()];
        let mut col_max: Vec<f64> = vec![0.0; b.vectors.len()];
        for (i, av) in a.vectors.iter().enumerate() {
            for (j, bv) in b.vectors.iter().enumerate() {
                let cosine = av
                    .iter()
                    .map(|(token, value)| value * bv.get(token).unwrap_or(&0.0))
                    .sum::<f64>()
                    .clamp(0.0, 1.0);
                row_max[i] = row_max[i].max(cosine);
                col_max[j] = col_max[j].max(cosine);
            }
        }
        // Expanding only the maxima preserves the former floating-point addition
        // order, without repeating tokenization, normalization or cosine work.
        (a.occurrences.iter().map(|&i| row_max[i]).sum::<f64>()
            + b.occurrences.iter().map(|&j| col_max[j]).sum::<f64>())
            / (a.occurrences.len() + b.occurrences.len()) as f64
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Keep the original expanded document preparation as a bounded oracle:
    // every occurrence contributes independently to global IDF and its vector.
    fn expanded_vectors(tables: &[&Table]) -> Vec<Vec<Vec<Vector>>> {
        let documents: Vec<Vec<Vec<Vec<String>>>> = tables
            .iter()
            .map(|table| {
                table
                    .columns
                    .iter()
                    .map(|column| {
                        column
                            .samples
                            .iter()
                            .filter(|sample| !sample.is_empty())
                            .map(|sample| document_tokens(sample))
                            .filter(|tokens| !tokens.is_empty())
                            .collect()
                    })
                    .collect()
            })
            .collect();
        let mut frequency = BTreeMap::<&str, usize>::new();
        let mut count = 0;
        for document in documents.iter().flatten().flatten() {
            count += 1;
            for token in document.iter().map(String::as_str).collect::<BTreeSet<_>>() {
                *frequency.entry(token).or_default() += 1;
            }
        }
        documents
            .iter()
            .map(|table| {
                table
                    .iter()
                    .map(|column| {
                        column
                            .iter()
                            .map(|document| {
                                let mut terms = BTreeMap::<String, f64>::new();
                                for token in document {
                                    *terms.entry(token.clone()).or_default() += 1.0;
                                }
                                let mut vector: Vector = terms
                                    .into_iter()
                                    .map(|(token, count_in_doc)| {
                                        let idf =
                                            (count as f64 / frequency[token.as_str()] as f64).ln();
                                        (token, count_in_doc.sqrt() * idf)
                                    })
                                    .filter(|(_, value)| *value > 0.0)
                                    .collect();
                                let norm = vector
                                    .values()
                                    .map(|value| value * value)
                                    .sum::<f64>()
                                    .sqrt();
                                if norm > 0.0 {
                                    for value in vector.values_mut() {
                                        *value /= norm;
                                    }
                                }
                                vector
                            })
                            .collect()
                    })
                    .collect()
            })
            .collect()
    }

    fn expanded_similarity(a: &[Vector], b: &[Vector]) -> f64 {
        if a.is_empty() || b.is_empty() {
            return 0.0;
        }
        let matrix: Vec<Vec<f64>> = a
            .iter()
            .map(|av| {
                b.iter()
                    .map(|bv| {
                        av.iter()
                            .map(|(token, value)| value * bv.get(token).unwrap_or(&0.0))
                            .sum::<f64>()
                            .clamp(0.0, 1.0)
                    })
                    .collect()
            })
            .collect();
        let rows: f64 = matrix
            .iter()
            .map(|row| row.iter().copied().fold(0.0, f64::max))
            .sum();
        let columns: f64 = (0..b.len())
            .map(|j| matrix.iter().map(|row| row[j]).fold(0.0, f64::max))
            .sum();
        (rows + columns) / (a.len() + b.len()) as f64
    }

    #[test]
    fn selection_is_bidirectional_retains_ties_and_delta_zero_disables_filter() {
        let c = ComaConfig {
            max_n: 1,
            delta: 0.0,
            ..Default::default()
        };
        assert_eq!(
            select_both_multiple(&[vec![0.8, 0.8], vec![0.9, 0.1]], &c),
            vec![(0, 1, 0.8), (1, 0, 0.9)]
        );
        let c = ComaConfig {
            delta: 0.0,
            ..Default::default()
        };
        assert_eq!(
            select_both_multiple(&[vec![0.8, 0.8], vec![0.9, 0.1]], &c).len(),
            4
        );
    }

    #[test]
    fn instance_scores_preserve_zero_and_overlapping_document_cosines() {
        let a = BTreeMap::from([("a".into(), 1.0)]);
        let b = BTreeMap::from([("b".into(), 1.0)]);
        let cases = [
            vec![],
            vec![BTreeMap::new()],
            vec![a.clone()],
            vec![b.clone()],
            vec![a.clone(), a.clone()],
            vec![
                BTreeMap::from([("a".into(), 0.6), ("b".into(), 0.8)]),
                BTreeMap::from([("b".into(), 0.8), ("c".into(), 0.6)]),
            ],
            vec![BTreeMap::new(), a, b],
        ];
        let columns = || {
            cases
                .iter()
                .map(|vectors| TfidfColumn {
                    occurrences: (0..vectors.len()).collect(),
                    vectors: vectors.clone(),
                })
                .collect()
        };
        let corpus = TfidfCorpus {
            columns: vec![columns(), columns()],
        };
        for (i, a) in cases.iter().enumerate() {
            for (j, b) in cases.iter().enumerate() {
                let expected = expanded_similarity(a, b);
                assert_eq!(
                    corpus.similarity(0, i, 1, j).to_bits(),
                    expected.to_bits(),
                    "case {i}, {j}"
                );
            }
        }
    }

    #[test]
    fn repeated_documents_preserve_expanded_idf_scores_details_and_selection() {
        let samples = [
            vec!["alpha alpha beta", "alpha gamma", "alpha alpha beta"],
            vec!["beta delta", "alpha gamma", "beta delta", "", "the"],
            vec![
                "gamma gamma",
                "zeta alpha beta",
                "gamma gamma",
                "gamma gamma",
            ],
            vec!["", "the", "東京", "🙂", "", "the"],
            vec![
                "ALPHA_beta",
                "alpha beta",
                "β alpha",
                "İstanbul gamma",
                "ALPHA_beta",
            ],
            vec!["single", "single", "single"],
            vec!["other vocabulary", "other vocabulary", "isolated"],
        ];
        let tables: Vec<_> = (0..3)
            .map(|table| {
                Table::new(
                    format!("table_{table}"),
                    (0..samples.len())
                        .map(|column| crate::Field {
                            name: format!("column_{column}"),
                            data_type: crate::DataType::Text,
                            samples: samples[(column + table) % samples.len()]
                                .iter()
                                .map(|value| (*value).into())
                                .collect(),
                        })
                        .collect(),
                )
                .unwrap()
            })
            .collect();
        let references: Vec<_> = tables.iter().collect();
        let expected = expanded_vectors(&references);
        let corpus = TfidfCorpus::new(&references);
        assert_eq!(corpus.columns[0][0].vectors.len(), 2);
        assert_eq!(corpus.columns[0][0].occurrences, [0, 1, 0]);
        assert!(corpus.columns[0][3].vectors.is_empty());
        for (i, table) in expected.iter().enumerate() {
            for (s, vectors) in table.iter().enumerate() {
                let compressed = &corpus.columns[i][s];
                for (position, &index) in compressed.occurrences.iter().enumerate() {
                    let bits = |vector: &Vector| {
                        vector
                            .iter()
                            .map(|(token, value)| (token.clone(), value.to_bits()))
                            .collect::<Vec<_>>()
                    };
                    assert_eq!(bits(&compressed.vectors[index]), bits(&vectors[position]));
                }
                for (j, other) in expected.iter().enumerate() {
                    for (t, target) in other.iter().enumerate() {
                        assert_eq!(
                            corpus.similarity(i, s, j, t).to_bits(),
                            expanded_similarity(vectors, target).to_bits(),
                            "tables {i}/{j}, columns {s}/{t}"
                        );
                    }
                }
            }
        }
        for (schema, max_n, delta, threshold, instance_weight) in [
            (false, 0, 0.0, 0.0, 1.0),
            (true, 0, 0.15, 0.0, 1.0),
            (true, 1, 0.0, 0.25, 0.75),
            (true, 1, 1.0, 1.0, 0.0),
        ] {
            let config = ComaConfig {
                use_instances: true,
                use_schema: schema,
                max_n,
                delta,
                threshold,
                instance_weight,
            };
            let mut matches = Vec::new();
            let mut details = BTreeMap::new();
            for i in 0..tables.len() {
                for j in i + 1..tables.len() {
                    let mut pair_details = BTreeMap::new();
                    let matrix: Vec<Vec<_>> = tables[i]
                        .columns
                        .iter()
                        .enumerate()
                        .map(|(s, source)| {
                            tables[j]
                                .columns
                                .iter()
                                .enumerate()
                                .map(|(t, target)| {
                                    let instance =
                                        expanded_similarity(&expected[i][s], &expected[j][t]);
                                    let name = trigram_similarity(&source.name, &target.name)
                                        .max(tokens_similarity(&source.name, &target.name));
                                    let mut scores =
                                        BTreeMap::from([("InstancesCM".into(), instance)]);
                                    if schema {
                                        scores.insert("NameCM".into(), name);
                                    }
                                    pair_details.insert((s, t), scores);
                                    let weight = if schema { 1.0 } else { 0.0 } + instance_weight;
                                    if weight == 0.0 {
                                        0.0
                                    } else {
                                        (if schema { name } else { 0.0 }
                                            + instance_weight * instance)
                                            / weight
                                    }
                                })
                                .collect()
                        })
                        .collect();
                    for (s, t, score) in select_both_multiple(&matrix, &config) {
                        let pair = ColumnPair::new(
                            &tables[i].name,
                            &tables[i].columns[s].name,
                            &tables[j].name,
                            &tables[j].columns[t].name,
                        );
                        matches.push((pair.clone(), score));
                        details.insert(pair, pair_details.remove(&(s, t)).unwrap());
                    }
                }
            }
            let old = MatcherResults::with_details(matches, details).unwrap();
            let actual = Coma::new(config)
                .unwrap()
                .get_matches_batch(&tables)
                .unwrap();
            assert_eq!(
                actual
                    .iter()
                    .map(|(p, s)| (p, s.to_bits()))
                    .collect::<Vec<_>>(),
                old.iter()
                    .map(|(p, s)| (p, s.to_bits()))
                    .collect::<Vec<_>>()
            );
            assert_eq!(actual.details(), old.details());
        }
    }
}
