use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

use fieldkin::algorithms::jaccard::{
    EmbeddingProvider, JaccardConfig, JaccardDistanceMatcher, PrecomputedEmbeddings,
};
use fieldkin::algorithms::strings::{
    StringDistanceFunction as Distance, is_abbreviation, similarity, tokens, tokens_similarity,
    trigram_similarity, type_similarity,
};
use fieldkin::{ColumnPair, DataType, Error, Field, Matcher, MatcherResults, Table};

fn values(values: &[&str]) -> Vec<String> {
    values.iter().map(|value| (*value).to_owned()).collect()
}

fn table(name: &str, columns: &[(&str, &[&str])]) -> Table {
    Table::new(
        name,
        columns
            .iter()
            .map(|(name, samples)| Field {
                name: (*name).to_owned(),
                data_type: DataType::Text,
                samples: values(samples),
            })
            .collect(),
    )
    .unwrap()
}

fn close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-12,
        "expected {expected}, got {actual}"
    );
}

#[test]
fn normalized_distances_count_characters_and_support_empty_strings() {
    for distance in [
        Distance::Levenshtein,
        Distance::DamerauLevenshtein,
        Distance::Hamming,
        Distance::Jaro,
        Distance::JaroWinkler,
        Distance::Exact,
    ] {
        close(similarity("", "", distance), 1.0);
        close(similarity("", "a", distance), 0.0);
        close(similarity("東京🙂", "東京🙂", distance), 1.0);
    }
    close(
        similarity("kitten", "sitting", Distance::Levenshtein),
        4.0 / 7.0,
    );
    // Unrestricted Damerau-Levenshtein, rather than optimal string alignment.
    close(
        similarity("CA", "ABC", Distance::DamerauLevenshtein),
        1.0 / 3.0,
    );
    close(similarity("café", "cafe", Distance::Hamming), 0.75);
    close(similarity("ab", "abc", Distance::Hamming), 2.0 / 3.0);
    close(similarity("東京", "東海", Distance::Levenshtein), 0.5);
    close(similarity("MARTHA", "MARHTA", Distance::Jaro), 17.0 / 18.0);
    close(
        similarity("MARTHA", "MARHTA", Distance::JaroWinkler),
        0.9611111111111111,
    );
    close(similarity("Paris", "paris", Distance::Exact), 0.0);
}

#[test]
fn levenshtein_scores_preserve_unicode_and_bit_vector_boundaries() {
    fn check(a: &str, b: &str) {
        assert_eq!(
            similarity(a, b, Distance::Levenshtein).to_bits(),
            strsim::normalized_levenshtein(a, b).to_bits(),
            "{a:?}, {b:?}"
        );
    }
    let mut strings = vec![String::new()];
    let mut generation = vec![String::new()];
    for _ in 0..4 {
        generation = generation
            .iter()
            .flat_map(|s| ['a', 'é', '界'].map(move |c| format!("{s}{c}")))
            .collect();
        strings.extend(generation.clone());
    }
    strings.extend(
        [
            "\0",
            "a\0b",
            "İ",
            "i\u{307}",
            "e\u{301}",
            "é",
            "ΣΟΣ",
            "σος",
            "🙂🙃🙂",
        ]
        .map(str::to_owned),
    );
    for a in &strings {
        for b in &strings {
            check(a, b);
        }
    }
    for length in [31, 32, 33, 63, 64, 65, 127, 128, 129, 255, 256, 257] {
        for alphabet in ["abcdefg", "é界🙂\0e\u{301}"] {
            let a: String = alphabet.chars().cycle().take(length).collect();
            for b in [
                a.clone(),
                a.chars().rev().collect(),
                format!("{a}Z"),
                format!("Z{a}"),
                "xyz".chars().cycle().take(length).collect(),
                a.chars().skip(3).collect(),
            ] {
                check(&a, &b);
                check(&b, &a);
            }
        }
    }
}

#[test]
fn coma_name_helpers_preserve_tokens_abbreviations_and_multisets() {
    assert_eq!(
        tokens("XMLHTTPRequest42_id_ID"),
        ["xmlhttp", "request", "42", "id"]
    );
    assert_eq!(tokens("Date١٢_value"), ["date", "١٢", "value"]);
    close(
        tokens_similarity("ApproxDate", "date_created_approximation"),
        0.8,
    );
    close(tokens_similarity("mgr", "manager"), 1.0);
    assert!(is_abbreviation("fname", "firstname"));
    assert!(!is_abbreviation("at", "attention"));
    assert!(!is_abbreviation("cat", "customer"));
    close(tokens_similarity("", ""), 0.0);
    close(trigram_similarity("", ""), 1.0);
    close(trigram_similarity("a", "b"), 0.0);
    close(trigram_similarity("é界", "É界"), 1.0);
    close(trigram_similarity("aaaa", "aaa"), 2.0 / 3.0);
    close(type_similarity(&DataType::Integer, &DataType::Float), 0.5);
    close(type_similarity(&DataType::Text, &DataType::Date), 0.3);
    close(type_similarity(&DataType::Float, &DataType::Decimal), 1.0);
    close(type_similarity(&DataType::Boolean, &DataType::Integer), 0.0);
}

#[test]
fn trigram_scores_preserve_unicode_and_repeated_gram_multiplicities() {
    fn owned_grams(value: &str) -> Vec<String> {
        let value = value.to_lowercase();
        let characters: Vec<_> = value.chars().collect();
        if characters.len() < 3 {
            vec![value]
        } else {
            characters
                .windows(3)
                .map(|gram| gram.iter().collect())
                .collect()
        }
    }
    let cases = [
        "",
        "a",
        "A",
        "ab",
        "abc",
        "aaaa",
        "aaa",
        "ababab",
        "bababa",
        "é界",
        "É界",
        "é界🙂é界",
        "界🙂é界🙂",
        "İ",
        "i\u{307}",
        "Aİx",
        "ai\u{307}x",
        "e\u{301}xy",
        "éxy",
        "a\0bc",
        "\0bc\0",
        "ΣΟΣ",
        "σος",
    ];
    for a in cases {
        for b in cases {
            let expected = if a.is_empty() || b.is_empty() {
                f64::from(a.is_empty() && b.is_empty())
            } else {
                let a = owned_grams(a);
                let mut b = owned_grams(b);
                let total = a.len() + b.len();
                let mut shared = 0;
                for gram in a {
                    if let Some(index) = b.iter().position(|candidate| *candidate == gram) {
                        b.remove(index);
                        shared += 1;
                    }
                }
                2.0 * shared as f64 / total as f64
            };
            assert_eq!(
                trigram_similarity(a, b).to_bits(),
                expected.to_bits(),
                "{a:?}, {b:?}"
            );
        }
    }
}

#[test]
fn exact_sets_ignore_duplicates_and_threshold_but_keep_raw_values() {
    let matcher = JaccardDistanceMatcher::new(JaccardConfig {
        threshold_dist: 0.0,
        distance_fun: Distance::Exact,
        ..Default::default()
    })
    .unwrap();
    close(
        matcher
            .set_similarity(&values(&["a", "b", "a"]), &values(&["b", "c"]))
            .unwrap(),
        1.0 / 3.0,
    );
    close(
        matcher
            .set_similarity(&values(&[" Paris"]), &values(&["Paris"]))
            .unwrap(),
        0.0,
    );
    close(
        matcher
            .set_similarity(&values(&[""]), &values(&[""]))
            .unwrap(),
        1.0,
    );
    close(matcher.set_similarity(&[], &[]).unwrap(), 0.0);
    close(matcher.set_similarity(&values(&["a"]), &[]).unwrap(), 0.0);
}

#[test]
fn fuzzy_threshold_is_inclusive_and_counts_both_directions_independently() {
    let matcher = JaccardDistanceMatcher::new(JaccardConfig {
        threshold_dist: 0.75,
        ..Default::default()
    })
    .unwrap();
    close(
        matcher
            .set_similarity(&values(&["cat"]), &values(&["cats"]))
            .unwrap(),
        1.0,
    );
    // The upstream Cython scorer rounds 0.8 upward through f32 before applying
    // its inclusive cutoff. A raw f64 similarity of exactly 0.8 is rejected.
    close(
        JaccardDistanceMatcher::default()
            .set_similarity(&values(&["abcde"]), &values(&["abcdz"]))
            .unwrap(),
        0.0,
    );
    let matcher = JaccardDistanceMatcher::new(JaccardConfig {
        threshold_dist: 0.59,
        ..Default::default()
    })
    .unwrap();
    // All three values have a partner; many-to-one matching is deliberate.
    let a = values(&["cat"]);
    let b = values(&["cat", "cats", "catsx"]);
    close(matcher.set_similarity(&a, &b).unwrap(), 1.0);
    close(matcher.set_similarity(&b, &a).unwrap(), 1.0);
    let matcher = JaccardDistanceMatcher::new(JaccardConfig {
        threshold_dist: 0.0,
        ..Default::default()
    })
    .unwrap();
    close(
        matcher
            .set_similarity(&values(&["x"]), &values(&["y"]))
            .unwrap(),
        1.0,
    );
}

#[test]
fn tversky_supports_containment_and_zero_penalties() {
    let a = values(&["x"]);
    let b = values(&["x", "y", "z"]);
    let jaccard = JaccardDistanceMatcher::new(JaccardConfig {
        distance_fun: Distance::Exact,
        ..Default::default()
    })
    .unwrap();
    close(jaccard.set_similarity(&a, &b).unwrap(), 1.0 / 3.0);
    for (alpha, beta) in [(1.0, 0.0), (0.0, 1.0), (0.0, 0.0)] {
        let matcher = JaccardDistanceMatcher::new(JaccardConfig {
            distance_fun: Distance::Exact,
            tversky_alpha: alpha,
            tversky_beta: beta,
            ..Default::default()
        })
        .unwrap();
        close(matcher.set_similarity(&a, &b).unwrap(), 1.0);
        close(matcher.set_similarity(&b, &a).unwrap(), 1.0);
        close(matcher.set_similarity(&a, &values(&["q"])).unwrap(), 0.0);
    }
}

// The original full Cartesian comparison is deliberately kept in this test
// oracle: every pair contributes to both directional existence counts.
fn cartesian_set_similarity(
    source: &[String],
    target: &[String],
    config: &JaccardConfig,
    threshold: f64,
    score: impl Fn(&str, &str) -> f64,
) -> f64 {
    let source: Vec<_> = source.iter().collect::<BTreeSet<_>>().into_iter().collect();
    let target: Vec<_> = target.iter().collect::<BTreeSet<_>>().into_iter().collect();
    if source.is_empty() || target.is_empty() {
        return 0.0;
    }
    let (a, b) = if source.len() <= target.len() {
        (source, target)
    } else {
        (target, source)
    };
    let mut a_hits = vec![false; a.len()];
    let mut b_hits = vec![false; b.len()];
    for (i, value_a) in a.iter().enumerate() {
        for (j, value_b) in b.iter().enumerate() {
            if score(value_a, value_b) >= threshold {
                a_hits[i] = true;
                b_hits[j] = true;
            }
        }
    }
    let a_match = a_hits.into_iter().filter(|hit| *hit).count();
    let b_match = b_hits.into_iter().filter(|hit| *hit).count();
    let a_unmatched = (a.len() - a_match) as f64;
    let b_unmatched = (b.len() - b_match) as f64;
    let a_match = a_match as f64;
    let b_match = b_match as f64;
    let alpha = config.tversky_alpha;
    let beta = config.tversky_beta;
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

#[test]
fn lexical_hit_skipping_is_bitwise_identical_to_full_cartesian_comparison() {
    let sets = [
        values(&[]),
        values(&[""]),
        values(&["cat", "cat", "cats", "catxx"]),
        values(&["bat", "cat", "catx", "dog", "zebra"]),
        values(&["", "café", "cafe", "東京", "東海", "🙂"]),
        values(&["abcde", "abcdz"]),
        values(&["a", "unrelatedlong", "123", "789"]),
        // Equal sets have score one; partially shared sets still need fuzzy
        // partners to be discovered in both directions.
        values(&["cat", "cats", "catxx", "dog", "dogs"]),
    ];
    for distance_fun in [
        Distance::Levenshtein,
        Distance::DamerauLevenshtein,
        Distance::Hamming,
        Distance::Jaro,
        Distance::JaroWinkler,
        Distance::Exact,
    ] {
        for threshold_dist in [0.0, 0.59, 0.75, 0.8, 1.0] {
            for (tversky_alpha, tversky_beta) in
                [(1.0, 1.0), (0.0, 0.0), (2.0, 0.5), (0.0, 1.0), (1.0, 0.0)]
            {
                let config = JaccardConfig {
                    distance_fun,
                    threshold_dist,
                    tversky_alpha,
                    tversky_beta,
                    ..Default::default()
                };
                let matcher = JaccardDistanceMatcher::new(config.clone()).unwrap();
                let threshold = if distance_fun == Distance::Exact {
                    1.0
                } else {
                    f64::from(threshold_dist as f32)
                };
                // Both orientations, unequal cardinalities, and identical sets.
                for source in &sets {
                    for target in &sets {
                        let expected =
                            cartesian_set_similarity(source, target, &config, threshold, |a, b| {
                                similarity(a, b, distance_fun)
                            });
                        let actual = matcher.set_similarity(source, target).unwrap();
                        assert_eq!(
                            actual.to_bits(),
                            expected.to_bits(),
                            "{config:?}: {source:?} / {target:?}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn levenshtein_pruning_and_cached_patterns_preserve_unicode_and_cutoff_boundaries() {
    let mut source = values(&["", "a", "abcd", "abcdefghij", "é", "東京🙂"]);
    source.extend(values(&[
        "abcdefghijklmnop",
        "abcdwxyzijklmnop",
        "abcdefghijklmnoé",
        "東京🙂abcdefghijkl",
        "repeated repeated",
        "abcdefghijklmnop",
    ]));
    source.push("a".repeat(200));
    let mut target = values(&["", "ab", "abcde", "abcdefghi🙂", "éé", "東京"]);
    target.extend(values(&[
        "abcdefghijklmnop",
        "abcdpqrsijklmnop",
        "abcdefghijklmno🙂",
        "東京🙂abcdefghijkX",
        "repeated repeatedly",
        "abcdpqrsijklmnop",
    ]));
    target.push("a".repeat(160));
    for length in [31, 32, 33, 63, 64, 65, 127, 128, 129, 255, 256, 257] {
        let value: String = "é界🙂\0e\u{301}".chars().cycle().take(length).collect();
        source.push(value.clone());
        target.push(value.chars().rev().collect());
        target.push(format!("{value}Z"));
    }
    for threshold_dist in [
        0.0,
        0.5,
        0.75,
        f64::from(0.8_f32.next_down()),
        0.8,
        f64::from(0.8_f32.next_up()),
        1.0,
    ] {
        for (source, target) in [(&source, &target), (&target, &source)] {
            let config = JaccardConfig {
                threshold_dist,
                tversky_alpha: 2.0,
                tversky_beta: 0.5,
                ..Default::default()
            };
            let expected = cartesian_set_similarity(
                source,
                target,
                &config,
                f64::from(threshold_dist as f32),
                |a, b| similarity(a, b, Distance::Levenshtein),
            );
            let actual = JaccardDistanceMatcher::new(config)
                .unwrap()
                .set_similarity(source, target)
                .unwrap();
            assert_eq!(actual.to_bits(), expected.to_bits());
        }
    }
    // Character counts admit this match; a bound using UTF-8 byte lengths
    // would incorrectly reject it because the emoji occupies four bytes.
    assert_eq!(
        JaccardDistanceMatcher::default()
            .set_similarity(&values(&["abcdefghij"]), &values(&["abcdefghi🙂"]))
            .unwrap(),
        1.0,
    );
}

#[test]
fn embedding_hit_skipping_is_bitwise_identical_to_full_cartesian_comparison() {
    let vectors: BTreeMap<String, Vec<f64>> = [
        ("x", vec![1.0, 0.0]),
        ("y", vec![0.0, 1.0]),
        ("diagonal", vec![1.0, 1.0]),
        ("large", vec![1e300, 1e300]),
        ("tiny", vec![1e-300, 1e-300]),
        ("opposite", vec![-1.0, -1.0]),
        ("", vec![0.0, -1.0]),
    ]
    .into_iter()
    .map(|(name, vector)| (name.to_owned(), vector))
    .collect();
    let provider = Arc::new(PrecomputedEmbeddings::new(vectors.clone()).unwrap());
    let normalized: BTreeMap<_, Vec<_>> = vectors
        .into_iter()
        .map(|(name, vector)| {
            let scale = vector
                .iter()
                .fold(0.0_f64, |largest, x| largest.max(x.abs()));
            let scaled: Vec<_> = vector.into_iter().map(|value| value / scale).collect();
            let norm = scaled.iter().map(|value| value * value).sum::<f64>().sqrt();
            (name, scaled.into_iter().map(|value| value / norm).collect())
        })
        .collect();
    let sets = [
        values(&[]),
        values(&[""]),
        values(&["diagonal"]),
        values(&["x", "y", "x"]),
        values(&["diagonal", "large", "tiny"]),
        values(&["", "x", "diagonal", "opposite"]),
    ];
    for threshold_dist in [0.0, 0.5, 0.8, 1.0] {
        for (tversky_alpha, tversky_beta) in [(1.0, 1.0), (0.0, 0.0), (2.0, 0.5)] {
            let config = JaccardConfig {
                // The embedding provider overrides even the Exact distance.
                distance_fun: Distance::Exact,
                threshold_dist,
                tversky_alpha,
                tversky_beta,
                ..Default::default()
            };
            let matcher = JaccardDistanceMatcher::new(config.clone())
                .unwrap()
                .with_embedding_provider(provider.clone());
            for source in &sets {
                for target in &sets {
                    let expected = cartesian_set_similarity(
                        source,
                        target,
                        &config,
                        threshold_dist,
                        |a, b| {
                            normalized[a]
                                .iter()
                                .zip(&normalized[b])
                                .map(|(x, y)| x * y)
                                .sum::<f64>()
                                .clamp(-1.0, 1.0)
                        },
                    );
                    let actual = matcher.set_similarity(source, target).unwrap();
                    assert_eq!(
                        actual.to_bits(),
                        expected.to_bits(),
                        "{config:?}: {source:?} / {target:?}"
                    );
                }
            }
        }
    }
}

#[test]
fn column_matching_omits_zero_scores_and_threads_preserve_results() {
    let a = table("a", &[("first", &["one", "two"]), ("empty", &[])]);
    let b = table("b", &[("second", &["two", "three"]), ("other", &["four"])]);
    let single = JaccardDistanceMatcher::new(JaccardConfig {
        distance_fun: Distance::Exact,
        ..Default::default()
    })
    .unwrap()
    .get_matches(&a, &b)
    .unwrap();
    let threaded = JaccardDistanceMatcher::new(JaccardConfig {
        distance_fun: Distance::Exact,
        process_num: 4,
        ..Default::default()
    })
    .unwrap()
    .get_matches(&a, &b)
    .unwrap();
    assert_eq!(
        single.iter().collect::<Vec<_>>(),
        threaded.iter().collect::<Vec<_>>()
    );
    assert_eq!(single.iter().count(), 1);
    close(*single.iter().next().unwrap().1, 1.0 / 3.0);
}

#[test]
fn rectangular_batch_ranges_preserve_full_cartesian_scores() {
    let tables: Vec<_> = [0, 1, 3, 7]
        .into_iter()
        .enumerate()
        .map(|(index, width)| {
            Table::new(
                format!("table{}", 4 - index),
                (0..width)
                    .map(|column| Field {
                        name: format!("column{}", width - column),
                        data_type: DataType::Text,
                        samples: if column % 3 == 2 {
                            values(&["", ""])
                        } else {
                            vec!["shared".into(), format!("value{index}-{column}")]
                        },
                    })
                    .collect(),
            )
            .unwrap()
        })
        .collect();
    let serial = JaccardDistanceMatcher::new(JaccardConfig {
        distance_fun: Distance::Exact,
        ..Default::default()
    })
    .unwrap();
    let mut entries = Vec::new();
    for (i, source) in tables.iter().enumerate() {
        for target in &tables[i + 1..] {
            for source_column in &source.columns {
                for target_column in &target.columns {
                    let source_values: Vec<_> = source_column
                        .samples
                        .iter()
                        .filter(|value| !value.is_empty())
                        .cloned()
                        .collect();
                    let target_values: Vec<_> = target_column
                        .samples
                        .iter()
                        .filter(|value| !value.is_empty())
                        .cloned()
                        .collect();
                    let score = serial
                        .set_similarity(&source_values, &target_values)
                        .unwrap();
                    if score > 0.0 {
                        entries.push((
                            ColumnPair::new(
                                &source.name,
                                &source_column.name,
                                &target.name,
                                &target_column.name,
                            ),
                            score,
                        ));
                    }
                }
            }
        }
    }
    let expected = MatcherResults::new(entries).unwrap();
    let bits = |results: &MatcherResults| {
        results
            .iter()
            .map(|(pair, score)| (pair.clone(), score.to_bits()))
            .collect::<Vec<_>>()
    };
    assert!(!expected.is_empty());
    for process_num in [1, 2, 4, 64] {
        let matcher = JaccardDistanceMatcher::new(JaccardConfig {
            distance_fun: Distance::Exact,
            process_num,
            ..Default::default()
        })
        .unwrap();
        let actual = matcher.get_matches_batch(&tables).unwrap();
        assert_eq!(bits(&actual), bits(&expected));
        assert_eq!(actual.details(), expected.details());
        for (i, source) in tables.iter().enumerate() {
            for target in &tables[i + 1..] {
                let actual = matcher.get_matches(source, target).unwrap();
                let expected: Vec<_> = bits(&expected)
                    .into_iter()
                    .filter(|(pair, _)| {
                        pair.source_table == source.name && pair.target_table == target.name
                    })
                    .collect();
                assert_eq!(bits(&actual), expected);
            }
        }
    }
}

#[test]
fn wide_empty_disjoint_and_dense_outputs_preserve_pair_identities() {
    for (source_width, target_width, mode) in [(257, 129, 0), (65, 33, 1), (17, 9, 2)] {
        let make_table = |name: &str, width| {
            Table::new(
                name,
                (0..width)
                    .map(|column| Field {
                        name: format!("column{column}"),
                        data_type: DataType::Text,
                        samples: match mode {
                            0 => values(&["", ""]),
                            1 => vec![format!("{name}-{column}")],
                            _ => values(&["shared", "shared"]),
                        },
                    })
                    .collect(),
            )
            .unwrap()
        };
        let source = make_table("source", source_width);
        let target = make_table("target", target_width);
        for process_num in [1, 4] {
            let results = JaccardDistanceMatcher::new(JaccardConfig {
                distance_fun: Distance::Exact,
                process_num,
                ..Default::default()
            })
            .unwrap()
            .get_matches(&source, &target)
            .unwrap();
            assert!(results.details().is_empty());
            if mode < 2 {
                assert!(results.is_empty());
            } else {
                assert_eq!(results.len(), source_width * target_width);
                let expected = MatcherResults::new(
                    source
                        .columns
                        .iter()
                        .flat_map(|a| {
                            target.columns.iter().map(|b| {
                                (ColumnPair::new("source", &a.name, "target", &b.name), 1.0)
                            })
                        })
                        .collect(),
                )
                .unwrap();
                assert_eq!(results, expected);
            }
        }
    }
}

#[test]
fn fuzzy_columns_share_prepared_bounds_across_workers_and_batches() {
    let tables = ['a', 'b', 'c'].map(|suffix| {
        Table::new(
            suffix.to_string(),
            (0..4)
                .map(|column| Field {
                    name: format!("column{column}"),
                    data_type: DataType::Text,
                    samples: if column == 3 {
                        values(&["", ""])
                    } else {
                        let stem = char::from(b'a' + column).to_string().repeat(16);
                        vec![
                            format!("{stem}-{suffix}"),
                            format!("東京🙂{column}-long-{suffix}"),
                            format!("shared-value-{suffix}"),
                        ]
                    },
                })
                .collect(),
        )
        .unwrap()
    });
    let serial = JaccardDistanceMatcher::default();
    let parallel = JaccardDistanceMatcher::new(JaccardConfig {
        process_num: 4,
        ..Default::default()
    })
    .unwrap();
    for (serial, parallel) in [
        (
            serial.get_matches(&tables[0], &tables[1]).unwrap(),
            parallel.get_matches(&tables[0], &tables[1]).unwrap(),
        ),
        (
            serial.get_matches_batch(&tables).unwrap(),
            parallel.get_matches_batch(&tables).unwrap(),
        ),
    ] {
        assert!(!serial.is_empty());
        let bits = |results: &fieldkin::MatcherResults| {
            results
                .iter()
                .map(|(pair, score)| (pair.clone(), score.to_bits()))
                .collect::<Vec<_>>()
        };
        assert_eq!(bits(&serial), bits(&parallel));
        assert_eq!(serial.details(), parallel.details());
    }
}

#[test]
fn embedding_cosine_normalizes_real_vectors_and_handles_extreme_scales() {
    let provider = PrecomputedEmbeddings::new([
        ("x".to_owned(), vec![10.0, 0.0]),
        ("y".to_owned(), vec![80.0, 60.0]),
        ("opposite".to_owned(), vec![-1.0, 0.0]),
        ("large".to_owned(), vec![1e300, 1e300]),
        ("tiny".to_owned(), vec![1e-300, 1e-300]),
    ])
    .unwrap();
    let matcher = JaccardDistanceMatcher::default().with_embedding_provider(Arc::new(provider));
    close(
        matcher
            .set_similarity(&values(&["x"]), &values(&["y"]))
            .unwrap(),
        1.0,
    );
    close(
        matcher
            .set_similarity(&values(&["x"]), &values(&["opposite"]))
            .unwrap(),
        0.0,
    );
    close(
        matcher
            .set_similarity(&values(&["large"]), &values(&["tiny"]))
            .unwrap(),
        1.0,
    );
    assert!(
        matcher
            .set_similarity(&values(&["missing"]), &values(&["x"]))
            .is_err()
    );
}

#[derive(Default)]
struct RecordingProvider {
    calls: Mutex<Vec<Vec<String>>>,
}

impl EmbeddingProvider for RecordingProvider {
    fn encode(&self, values: &[String]) -> Result<Vec<Vec<f64>>, Error> {
        self.calls.lock().unwrap().push(values.to_vec());
        Ok(values.iter().map(|_| vec![2.0, 0.0]).collect())
    }
}

#[test]
fn batch_embeddings_encode_global_vocabulary_once() {
    let tables = [
        table("empty", &[]),
        table("a", &[("v", &["cat", "cat", "dog"]), ("w", &["cat"])]),
        table(
            "b",
            &[("v", &["cat", "cats"]), ("empty", &[""]), ("w", &["dog"])],
        ),
        table("c", &[("v", &["dogs", "dog"])]),
    ];
    for process_num in [1, 4, 64] {
        let provider = Arc::new(RecordingProvider::default());
        let matcher = JaccardDistanceMatcher::new(JaccardConfig {
            process_num,
            ..Default::default()
        })
        .unwrap()
        .with_embedding_provider(provider.clone());
        assert_eq!(matcher.get_matches_batch(&tables).unwrap().len(), 8);
        assert_eq!(
            *provider.calls.lock().unwrap(),
            vec![values(&["cat", "cats", "dog", "dogs"])]
        );
        assert!(
            matcher
                .get_matches_batch(&[table("empty_a", &[("v", &[""])]), table("empty_b", &[])])
                .unwrap()
                .is_empty()
        );
        assert_eq!(provider.calls.lock().unwrap().len(), 1);
    }
}

#[test]
fn rejects_invalid_configs_and_embeddings_without_panics() {
    for threshold in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
        assert!(
            JaccardDistanceMatcher::new(JaccardConfig {
                threshold_dist: threshold,
                ..Default::default()
            })
            .is_err()
        );
    }
    for alpha in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(
            JaccardDistanceMatcher::new(JaccardConfig {
                tversky_alpha: alpha,
                ..Default::default()
            })
            .is_err()
        );
    }
    assert!(
        JaccardDistanceMatcher::new(JaccardConfig {
            process_num: 0,
            ..Default::default()
        })
        .is_err()
    );
    for vector in [vec![], vec![0.0], vec![f64::NAN], vec![f64::INFINITY]] {
        assert!(PrecomputedEmbeddings::new([("x".to_owned(), vector)]).is_err());
    }
    assert!(
        PrecomputedEmbeddings::new([
            ("x".to_owned(), vec![1.0]),
            ("y".to_owned(), vec![1.0, 0.0]),
        ])
        .is_err()
    );
    assert!(
        PrecomputedEmbeddings::new([("x".to_owned(), vec![1.0]), ("x".to_owned(), vec![2.0]),])
            .is_err()
    );
}

struct MalformedProvider;
impl EmbeddingProvider for MalformedProvider {
    fn encode(&self, _: &[String]) -> Result<Vec<Vec<f64>>, Error> {
        Ok(Vec::new())
    }
}

#[test]
fn validates_provider_output_and_skips_encoding_an_empty_vocabulary() {
    let matcher =
        JaccardDistanceMatcher::default().with_embedding_provider(Arc::new(MalformedProvider));
    assert!(
        matcher
            .set_similarity(&values(&["x"]), &values(&["y"]))
            .is_err()
    );
    close(matcher.set_similarity(&[], &[]).unwrap(), 0.0);
}

#[derive(serde::Deserialize)]
struct GoldenFixture {
    distances: Vec<GoldenDistance>,
    sets: Vec<GoldenSet>,
    tokens: Vec<GoldenTokens>,
    names: Vec<GoldenName>,
}

#[derive(serde::Deserialize)]
struct GoldenDistance {
    function: Distance,
    left: String,
    right: String,
    score: f64,
}

#[derive(serde::Deserialize)]
struct GoldenSet {
    config: JaccardConfig,
    source: Vec<String>,
    target: Vec<String>,
    score: f64,
}

#[derive(serde::Deserialize)]
struct GoldenTokens {
    name: String,
    tokens: Vec<String>,
}

#[derive(serde::Deserialize)]
struct GoldenName {
    left: String,
    right: String,
    tokens: f64,
    trigrams: f64,
}

#[test]
fn matches_upstream_python_golden_distances_sets_and_name_helpers() {
    let fixture: GoldenFixture =
        serde_json::from_str(include_str!("fixtures/jaccard.json")).unwrap();
    for case in fixture.distances {
        let actual = similarity(&case.left, &case.right, case.function);
        assert!(
            (actual - case.score).abs() < 1e-12,
            "{:?} {:?} {:?}: native={actual} Python={}",
            case.function,
            case.left,
            case.right,
            case.score
        );
    }
    for case in fixture.sets {
        let matcher = JaccardDistanceMatcher::new(case.config.clone()).unwrap();
        let actual = matcher.set_similarity(&case.source, &case.target).unwrap();
        assert!(
            (actual - case.score).abs() < 1e-12,
            "{:?} {:?} {:?}: native={actual} Python={}",
            case.config,
            case.source,
            case.target,
            case.score
        );
    }
    for case in fixture.tokens {
        assert_eq!(tokens(&case.name), case.tokens, "name {:?}", case.name);
    }
    for case in fixture.names {
        close(tokens_similarity(&case.left, &case.right), case.tokens);
        close(trigram_similarity(&case.left, &case.right), case.trigrams);
    }
}
