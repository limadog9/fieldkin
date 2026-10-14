use fieldkin::{ColumnPair, MatchDetails, MatcherResults};
use std::collections::{BTreeMap, BTreeSet};

// Bounded oracle copied from 88d6438: retain the square traversal, including
// dummy rows/columns, independently of the production storage and fast paths.
fn square_assignment(cost: &[Vec<(isize, f64)>], processed_rows: usize) -> Vec<usize> {
    let n = cost.len();
    let mut u = vec![(0isize, 0.0); n + 1];
    let mut v = vec![(0isize, 0.0); n + 1];
    let mut p = vec![0; n + 1];
    let mut way = vec![0; n + 1];
    for i in 1..=processed_rows {
        p[0] = i;
        let mut j0 = 0;
        let mut minv = vec![(isize::MAX, f64::INFINITY); n + 1];
        let mut used = vec![false; n + 1];
        loop {
            used[j0] = true;
            let i0 = p[j0];
            let mut delta = (isize::MAX, f64::INFINITY);
            let mut j1 = 0;
            for j in 1..=n {
                if !used[j] {
                    let (count, score) = cost[i0 - 1][j - 1];
                    let cur = (count - u[i0].0 - v[j].0, score - u[i0].1 - v[j].1);
                    if cur < minv[j] {
                        minv[j] = cur;
                        way[j] = j0;
                    }
                    if minv[j] < delta {
                        delta = minv[j];
                        j1 = j;
                    }
                }
            }
            for j in 0..=n {
                if used[j] {
                    u[p[j]].0 += delta.0;
                    u[p[j]].1 += delta.1;
                    v[j].0 -= delta.0;
                    v[j].1 -= delta.1;
                } else {
                    minv[j].0 -= delta.0;
                    minv[j].1 -= delta.1;
                }
            }
            j0 = j1;
            if p[j0] == 0 {
                break;
            }
        }
        loop {
            let j1 = way[j0];
            p[j0] = p[j1];
            j0 = j1;
            if j0 == 0 {
                break;
            }
        }
    }
    let mut assignment = vec![0; n];
    for j in 1..=n {
        if p[j] > 0 {
            assignment[p[j] - 1] = j - 1;
        }
    }
    assignment
}

fn square_selection(
    results: &MatcherResults,
    threshold: Option<f64>,
    cardinality_first: bool,
) -> Vec<ColumnPair> {
    let mut scores: Vec<_> = results.iter().map(|(_, score)| *score).collect();
    scores.dedup();
    let threshold = threshold.unwrap_or_else(|| {
        scores
            .get(scores.len().div_ceil(2).min(scores.len().saturating_sub(1)))
            .copied()
            .unwrap_or(0.0)
    });
    let sources: Vec<_> = results
        .iter()
        .map(|(pair, _)| pair.source())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let targets: Vec<_> = results
        .iter()
        .map(|(pair, _)| pair.target())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let n = sources.len().max(targets.len());
    let mut costs = vec![vec![(0, 0.0); n]; n];
    let mut lookup = BTreeMap::new();
    for (pair, score) in results {
        if cardinality_first && *score < threshold {
            continue;
        }
        let row = sources.binary_search(&pair.source()).unwrap();
        let col = targets.binary_search(&pair.target()).unwrap();
        costs[row][col] = (if cardinality_first { -1 } else { 0 }, -*score);
        lookup.insert((row, col), pair);
    }
    let assignment = square_assignment(&costs, n);
    let chosen: BTreeSet<_> = assignment
        .iter()
        .enumerate()
        .filter_map(|(row, col)| lookup.get(&(row, *col)).copied())
        .collect();
    results
        .iter()
        .filter(|(pair, score)| chosen.contains(pair) && **score >= threshold)
        .map(|(pair, _)| pair.clone())
        .collect()
}

fn pair(row: usize, col: usize) -> ColumnPair {
    ColumnPair::new("source", row.to_string(), "target", col.to_string())
}

#[test]
fn rectangular_selectors_preserve_square_oracle_pairs_scores_and_details() {
    for rows in 1..=5 {
        for cols in 1..=5 {
            for seed in 0..24 {
                let mut entries = Vec::new();
                let mut details = MatchDetails::new();
                for row in 0..rows {
                    for col in 0..cols {
                        let code = seed * 17 + row * 13 + col * 7 + row * col * 19;
                        if code % 5 != 0 {
                            let score = [
                                -0.0,
                                0.0,
                                0.25,
                                0.5,
                                f64::from_bits(0.5_f64.to_bits() + 1),
                                0.75,
                                1.0,
                            ][code % 7];
                            let identity = pair(row, col);
                            details.insert(
                                identity.clone(),
                                BTreeMap::from([("score".into(), score), ("zero".into(), -0.0)]),
                            );
                            entries.push((identity, score));
                        }
                    }
                }
                let results = MatcherResults::with_details(entries, details).unwrap();
                let original = results.clone();
                for threshold in [
                    None,
                    Some(-1.0),
                    Some(-0.0),
                    Some(0.5),
                    Some(1.0),
                    Some(1.1),
                ] {
                    for cardinality_first in [false, true] {
                        let actual = if cardinality_first {
                            results.one_to_one_hungarian_threshold_aware(threshold)
                        } else {
                            results.one_to_one_hungarian(threshold)
                        }
                        .unwrap();
                        let expected = square_selection(&results, threshold, cardinality_first);
                        assert_eq!(
                            actual
                                .iter()
                                .map(|(pair, _)| pair.clone())
                                .collect::<Vec<_>>(),
                            expected,
                            "{rows}x{cols}, seed {seed}, threshold {threshold:?}, cardinality {cardinality_first}"
                        );
                        for (pair, score) in &actual {
                            assert_eq!(score.to_bits(), results.get(pair).unwrap().to_bits());
                            let retained = actual.get_details(pair).unwrap();
                            let original = results.get_details(pair).unwrap();
                            assert_eq!(retained.len(), original.len());
                            for (name, value) in retained {
                                assert_eq!(value.to_bits(), original[name].to_bits());
                            }
                        }
                        assert_eq!(actual.details().len(), actual.len());
                    }
                }
                assert_eq!(results, original);
            }
        }
    }
}

#[test]
fn transposition_can_change_ties_and_legacy_threshold_survivors() {
    let results = MatcherResults::new(vec![
        (pair(0, 1), 0.0),
        (pair(1, 0), 0.5),
        (pair(1, 1), 0.25),
        (pair(2, 0), 0.75),
        (pair(2, 1), 0.5),
    ])
    .unwrap();
    let transposed = MatcherResults::new(
        results
            .iter()
            .map(|(identity, score)| {
                (
                    ColumnPair::new(
                        "source",
                        &identity.target_column,
                        "target",
                        &identity.source_column,
                    ),
                    *score,
                )
            })
            .collect(),
    )
    .unwrap();
    let legacy = results.one_to_one_hungarian(Some(0.5)).unwrap();
    assert_eq!(
        legacy
            .iter()
            .map(|(pair, _)| pair.clone())
            .collect::<Vec<_>>(),
        vec![pair(1, 0), pair(2, 1)]
    );
    assert_eq!(legacy.len(), 2);
    let alternate = transposed.one_to_one_hungarian(Some(0.5)).unwrap();
    assert_eq!(alternate.len(), 1);
    assert_eq!(alternate.get(&pair(0, 2)), Some(0.75));
    for collection in [&results, &transposed] {
        assert_eq!(
            collection
                .one_to_one_hungarian(Some(0.0))
                .unwrap()
                .iter()
                .map(|(_, score)| *score)
                .sum::<f64>(),
            1.0
        );
    }
}

#[test]
fn skipping_dummy_rows_can_change_real_pairs_near_floating_point_ties() {
    let values = [
        [0.9, 0.0, 0.30000000000000004, 0.2, 0.3],
        [
            0.5000000000000001,
            0.30000000000000004,
            0.0,
            0.1,
            0.49999999999999994,
        ],
        [
            0.5,
            0.10000000000000002,
            0.10000000000000002,
            0.7,
            0.10000000000000002,
        ],
        [
            0.09999999999999999,
            0.29999999999999993,
            0.3,
            0.9,
            0.09999999999999999,
        ],
    ];
    let mut costs = vec![vec![(0, 0.0); 5]; 5];
    let mut entries = Vec::new();
    for (row, scores) in values.iter().enumerate() {
        for (col, score) in scores.iter().enumerate() {
            costs[row][col] = (0, -*score);
            entries.push((pair(row, col), *score));
        }
    }
    assert_eq!(&square_assignment(&costs, 5)[..4], &[0, 4, 2, 3]);
    assert_eq!(&square_assignment(&costs, 4)[..4], &[0, 4, 1, 3]);
    let results = MatcherResults::new(entries).unwrap();
    let selected = results.one_to_one_hungarian(Some(0.0)).unwrap();
    assert_eq!(selected.get(&pair(2, 2)), Some(0.10000000000000002));
    assert_eq!(selected.get(&pair(2, 1)), None);
}

#[test]
fn singleton_assignment_ties_use_numeric_equality_including_signed_zero() {
    for transposed in [false, true] {
        for scores in [[-0.0, 0.0, 0.0], [0.5, 0.5, 0.25], [0.25, 0.75, 0.75]] {
            let results = MatcherResults::new(
                scores
                    .iter()
                    .enumerate()
                    .map(|(index, score)| {
                        (
                            if transposed {
                                pair(index, 0)
                            } else {
                                pair(0, index)
                            },
                            *score,
                        )
                    })
                    .collect(),
            )
            .unwrap();
            for threshold in [None, Some(-1.0), Some(0.0), Some(0.5), Some(1.1)] {
                for cardinality_first in [false, true] {
                    let selected = if cardinality_first {
                        results.one_to_one_hungarian_threshold_aware(threshold)
                    } else {
                        results.one_to_one_hungarian(threshold)
                    }
                    .unwrap();
                    assert_eq!(
                        selected
                            .iter()
                            .map(|(pair, _)| pair.clone())
                            .collect::<Vec<_>>(),
                        square_selection(&results, threshold, cardinality_first)
                    );
                    if let Some((identity, score)) = selected.iter().next() {
                        assert_eq!(score.to_bits(), results.get(identity).unwrap().to_bits());
                    }
                }
            }
        }
    }
}

#[test]
fn exhaustive_bounded_singleton_scores_match_square_oracle() {
    let alphabet = [-0.0, 0.0, 0.5, f64::from_bits(0.5_f64.to_bits() + 1), 1.0];
    for length in 1_usize..=4 {
        for mut code in 0..alphabet.len().pow(length as u32) {
            let scores: Vec<_> = (0..length)
                .map(|_| {
                    let score = alphabet[code % alphabet.len()];
                    code /= alphabet.len();
                    score
                })
                .collect();
            for transposed in [false, true] {
                let results = MatcherResults::new(
                    scores
                        .iter()
                        .enumerate()
                        .map(|(index, score)| {
                            (
                                if transposed {
                                    pair(index, 0)
                                } else {
                                    pair(0, index)
                                },
                                *score,
                            )
                        })
                        .collect(),
                )
                .unwrap();
                for threshold in [None, Some(0.0), Some(0.5), Some(alphabet[3]), Some(1.1)] {
                    for cardinality_first in [false, true] {
                        let selected = if cardinality_first {
                            results.one_to_one_hungarian_threshold_aware(threshold)
                        } else {
                            results.one_to_one_hungarian(threshold)
                        }
                        .unwrap();
                        assert_eq!(
                            selected
                                .iter()
                                .map(|(pair, _)| pair.clone())
                                .collect::<Vec<_>>(),
                            square_selection(&results, threshold, cardinality_first)
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn tall_singleton_float_ties_need_square_traversal() {
    let scores = [
        0.1,
        0.6,
        0.3,
        0.6,
        0.6999999999999998,
        0.5000000000000001,
        0.6,
        0.7,
        0.30000000000000004,
        0.7,
        0.9000000000000001,
        0.9000000000000001,
        0.30000000000000004,
        0.5000000000000001,
    ];
    let results = MatcherResults::new(
        scores
            .iter()
            .enumerate()
            .map(|(index, score)| {
                (
                    ColumnPair::new("source", format!("{index:02}"), "target", "0"),
                    *score,
                )
            })
            .collect(),
    )
    .unwrap();
    let expected = square_selection(&results, Some(0.0), true);
    assert_eq!(
        expected,
        vec![ColumnPair::new("source", "11", "target", "0")]
    );
    let actual = results
        .one_to_one_hungarian_threshold_aware(Some(0.0))
        .unwrap();
    assert_eq!(
        actual
            .iter()
            .map(|(pair, _)| pair.clone())
            .collect::<Vec<_>>(),
        expected
    );
}

#[test]
fn highly_rectangular_single_row_and_excluded_tall_selection_avoid_dummy_solves() {
    for transposed in [false, true] {
        let identity = |index| {
            if transposed {
                pair(index, 0)
            } else {
                pair(0, index)
            }
        };
        let expected = identity(0);
        let results = MatcherResults::with_details(
            (0..10_000)
                .map(|index| {
                    (
                        identity(index),
                        if index == 0 || index == 9_999 {
                            0.9
                        } else {
                            0.5
                        },
                    )
                })
                .collect(),
            MatchDetails::from([(
                expected.clone(),
                BTreeMap::from([("component".into(), -0.0)]),
            )]),
        )
        .unwrap();
        for threshold in [None, Some(0.0), Some(0.9), Some(1.1)] {
            if transposed && threshold != Some(1.1) {
                // Tall inputs retain square traversal; exercise their storage
                // dimensions separately rather than starting a huge solve.
                continue;
            }
            for selected in [
                results.one_to_one_hungarian(threshold).unwrap(),
                results
                    .one_to_one_hungarian_threshold_aware(threshold)
                    .unwrap(),
            ] {
                if threshold == Some(1.1) {
                    assert!(selected.is_empty());
                    assert!(selected.details().is_empty());
                } else {
                    assert_eq!(selected.len(), 1);
                    assert_eq!(selected.get(&expected), Some(0.9));
                    assert_eq!(
                        selected.get_details(&expected).unwrap()["component"].to_bits(),
                        (-0.0_f64).to_bits()
                    );
                }
            }
        }
        assert_eq!(results.len(), 10_000);
    }
}
