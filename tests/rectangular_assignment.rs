use fieldkin::{ColumnPair, MatchDetails, MatcherResults, WorkLimits};
use std::collections::{BTreeMap, BTreeSet};

fn pair(row: usize, col: usize) -> ColumnPair {
    ColumnPair::new("source", row.to_string(), "target", col.to_string())
}

fn select(results: &MatcherResults, threshold: Option<f64>, aware: bool) -> MatcherResults {
    if aware {
        results.one_to_one_rectangular_threshold_aware(threshold)
    } else {
        results.one_to_one_rectangular(threshold)
    }
    .unwrap()
}

fn checked(
    results: &MatcherResults,
    threshold: Option<f64>,
    aware: bool,
    limits: &WorkLimits,
) -> Result<MatcherResults, fieldkin::Error> {
    if aware {
        results.one_to_one_rectangular_threshold_aware_with_limits(threshold, limits)
    } else {
        results.one_to_one_rectangular_with_limits(threshold, limits)
    }
}

fn cutoff(results: &MatcherResults, threshold: Option<f64>) -> f64 {
    threshold.unwrap_or_else(|| {
        let mut scores: Vec<_> = results.iter().map(|(_, score)| *score).collect();
        scores.dedup();
        scores
            .get(scores.len().div_ceil(2).min(scores.len().saturating_sub(1)))
            .copied()
            .unwrap_or(0.0)
    })
}

// Enumerate every injection from the smaller side into the larger side. Missing
// edges are neutral completion slots; only real, eligible edges count. This
// oracle deliberately contains no augmenting-path or dual-potential logic.
fn optimum(results: &MatcherResults, threshold: f64, aware: bool) -> (usize, f64) {
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
    let transpose = sources.len() > targets.len();
    let (small, large) = if transpose {
        (targets.len(), sources.len())
    } else {
        (sources.len(), targets.len())
    };
    let mut values = vec![vec![(0, 0.0); large]; small];
    for (pair, score) in results {
        if aware && *score < threshold {
            continue;
        }
        let row = sources.binary_search(&pair.source()).unwrap();
        let col = targets.binary_search(&pair.target()).unwrap();
        let (row, col) = if transpose { (col, row) } else { (row, col) };
        values[row][col] = (usize::from(aware), *score);
    }
    fn visit(
        values: &[Vec<(usize, f64)>],
        row: usize,
        used: &mut [bool],
        current: (usize, f64),
        best: &mut (usize, f64),
    ) {
        if row == values.len() {
            if current > *best {
                *best = current;
            }
            return;
        }
        for col in 0..used.len() {
            if !used[col] {
                used[col] = true;
                let value = values[row][col];
                visit(
                    values,
                    row + 1,
                    used,
                    (current.0 + value.0, current.1 + value.1),
                    best,
                );
                used[col] = false;
            }
        }
    }
    let mut best = (0, 0.0);
    visit(&values, 0, &mut vec![false; large], (0, 0.0), &mut best);
    best
}

fn assert_bits_equal(left: &MatcherResults, right: &MatcherResults) {
    assert_eq!(left.len(), right.len());
    assert_eq!(left.details().len(), right.details().len());
    for ((pair, score), (other_pair, other_score)) in left.iter().zip(right) {
        assert_eq!(pair, other_pair);
        assert_eq!(score.to_bits(), other_score.to_bits());
        let details = left.get_details(pair);
        let other_details = right.get_details(pair);
        assert_eq!(details.is_some(), other_details.is_some());
        if let (Some(details), Some(other_details)) = (details, other_details) {
            assert_eq!(details.len(), other_details.len());
            for (name, value) in details {
                assert_eq!(value.to_bits(), other_details[name].to_bits());
            }
        }
    }
}

fn assert_retained(results: &MatcherResults, selected: &MatcherResults, threshold: f64) {
    let mut sources = BTreeSet::new();
    let mut targets = BTreeSet::new();
    for (pair, score) in selected {
        assert!(sources.insert(pair.source()));
        assert!(targets.insert(pair.target()));
        assert!(*score >= threshold);
        assert_eq!(score.to_bits(), results.get(pair).unwrap().to_bits());
    }
    let retained = MatcherResults::with_details(
        selected
            .iter()
            .map(|(pair, score)| (pair.clone(), *score))
            .collect(),
        results.details().clone(),
    )
    .unwrap();
    assert_bits_equal(selected, &retained);
}

fn verify_objectives(results: &MatcherResults) {
    // Establish score optimality BEFORE filtering, which can discard the edge
    // responsible for a better total. Filtering alone cannot prove optimality.
    let raw = select(results, Some(-1.0), false);
    assert_eq!(
        raw.iter().map(|(_, score)| score).sum::<f64>(),
        optimum(results, -1.0, false).1
    );
    for threshold in [None, Some(-1.0), Some(0.0), Some(0.5), Some(1.0), Some(1.1)] {
        let cutoff = cutoff(results, threshold);
        for aware in [false, true] {
            let selected = select(results, threshold, aware);
            assert_retained(results, &selected, cutoff);
            if aware {
                assert_eq!(
                    (
                        selected.len(),
                        selected.iter().map(|(_, score)| score).sum::<f64>()
                    ),
                    optimum(results, cutoff, true),
                    "threshold {threshold:?}, input {results:?}"
                );
            } else {
                assert_bits_equal(&selected, &raw.filter(cutoff).unwrap());
            }
            assert_bits_equal(
                &selected,
                &checked(results, threshold, aware, &WorkLimits::default()).unwrap(),
            );
        }
    }
}

#[test]
fn rectangular_objectives_match_exhaustive_tiny_problems() {
    for (rows, cols, scores) in [
        (1usize, 1usize, &[None, Some(0.0), Some(0.5), Some(1.0)][..]),
        (1, 2, &[None, Some(0.0), Some(0.5), Some(1.0)][..]),
        (2, 1, &[None, Some(0.0), Some(0.5), Some(1.0)][..]),
        (2, 2, &[None, Some(0.0), Some(0.5), Some(1.0)][..]),
        (2, 3, &[None, Some(0.0), Some(0.5)][..]),
        (3, 2, &[None, Some(0.0), Some(0.5)][..]),
    ] {
        for mut code in 0..scores.len().pow((rows * cols) as u32) {
            let mut entries = Vec::new();
            for row in 0..rows {
                for col in 0..cols {
                    if let Some(score) = scores[code % scores.len()] {
                        entries.push((pair(row, col), score));
                    }
                    code /= scores.len();
                }
            }
            verify_objectives(&MatcherResults::new(entries).unwrap());
        }
    }
}

#[test]
fn rectangular_objectives_match_seeded_sparse_problems() {
    let mut state = 0x4f3a_219c_8d71_6b05u64;
    for rows in 1..=4 {
        for cols in 1..=4 {
            for _ in 0..24 {
                let mut entries = Vec::new();
                for row in 0..rows {
                    for col in 0..cols {
                        state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                        if !state.is_multiple_of(5) {
                            let score = (state.rotate_right(32) % 9) as f64 / 8.0;
                            entries.push((pair(row, col), score));
                        }
                    }
                }
                verify_objectives(&MatcherResults::new(entries).unwrap());
            }
        }
    }
}

#[test]
fn rectangular_threshold_policy_and_default_cutoff_are_distinct() {
    let results = MatcherResults::new(vec![
        (pair(0, 0), 0.90),
        (pair(0, 1), 0.80),
        (pair(1, 0), 0.81),
        (pair(1, 1), 0.79),
    ])
    .unwrap();
    let score_first = select(&results, Some(0.80), false);
    assert_eq!(score_first.len(), 1);
    assert_eq!(score_first.get(&pair(0, 0)), Some(0.90));
    let aware = select(&results, Some(0.80), true);
    assert_eq!(aware.len(), 2);
    assert_eq!(aware.get(&pair(0, 1)), Some(0.80));
    assert_eq!(aware.get(&pair(1, 0)), Some(0.81));
    // Four distinct scores resolve None to index ceil(4/2), namely 0.80.
    for cardinality_first in [false, true] {
        assert_bits_equal(
            &select(&results, None, cardinality_first),
            &select(&results, Some(0.80), cardinality_first),
        );
        assert!(select(&results, Some(1.01), cardinality_first).is_empty());
        assert_eq!(select(&results, Some(-1.0), cardinality_first).len(), 2);
    }
}

#[test]
fn rectangular_and_legacy_optimal_ties_can_filter_to_different_counts() {
    // Reuse the legacy transposition regression without changing its contract.
    // Both pre-filter objectives are exactly 1.0, but .75+.25 and .5+.5
    // retain different cardinalities at .5. This is explicitly opt-in behavior.
    let results = MatcherResults::new(vec![
        (pair(0, 1), 0.0),
        (pair(1, 0), 0.5),
        (pair(1, 1), 0.25),
        (pair(2, 0), 0.75),
        (pair(2, 1), 0.5),
    ])
    .unwrap();
    for raw in [
        results.one_to_one_hungarian(Some(0.0)).unwrap(),
        results.one_to_one_rectangular(Some(0.0)).unwrap(),
    ] {
        assert_eq!(raw.iter().map(|(_, score)| score).sum::<f64>(), 1.0);
        assert_eq!(optimum(&results, 0.0, false).1, 1.0);
    }
    assert_eq!(results.one_to_one_hungarian(Some(0.5)).unwrap().len(), 2);
    assert_eq!(results.one_to_one_rectangular(Some(0.5)).unwrap().len(), 1);
    assert_eq!(
        results
            .one_to_one_rectangular_threshold_aware(Some(0.5))
            .unwrap()
            .len(),
        2
    );
}

#[test]
fn rectangular_threshold_aware_counts_real_zero_edges_but_not_missing_slots() {
    let results = MatcherResults::new(vec![
        (pair(0, 0), 0.0),
        (pair(0, 1), -0.0),
        (pair(1, 0), 0.0),
    ])
    .unwrap();
    let selected = select(&results, Some(-0.0), true);
    assert_eq!(selected.len(), 2);
    assert_eq!(
        selected.get(&pair(0, 1)).unwrap().to_bits(),
        (-0.0f64).to_bits()
    );
    assert_eq!(selected.get(&pair(1, 0)), Some(0.0));
    assert!(selected.get(&pair(1, 1)).is_none());
    verify_objectives(&results);
}

#[test]
fn rectangular_preserves_bits_details_and_determinism_across_permutations() {
    let entries = vec![
        (ColumnPair::new("s1", "same", "t1", "same"), 0.75),
        (ColumnPair::new("s1", "same", "t2", "same"), 0.75),
        (ColumnPair::new("s2", "same", "t1", "same"), 0.75),
        (ColumnPair::new("s2", "same", "t2", "same"), 0.75),
        (ColumnPair::new("s3", "same", "t3", "same"), -0.0),
        (ColumnPair::new("s4", "same", "t3", "same"), 0.0),
    ];
    let details: MatchDetails = entries
        .iter()
        .enumerate()
        .map(|(index, (pair, score))| {
            (
                pair.clone(),
                BTreeMap::from([
                    ("component".into(), *score),
                    ("index".into(), index as f64),
                    ("signed zero".into(), -0.0),
                ]),
            )
        })
        .collect();
    let original = MatcherResults::with_details(entries.clone(), details.clone()).unwrap();
    for aware in [false, true] {
        let expected = select(&original, Some(0.0), aware);
        assert_retained(&original, &expected, 0.0);
        assert_eq!(expected.len(), 3);
        for offset in 0..entries.len() {
            let mut permuted = entries.clone();
            permuted.rotate_left(offset);
            if offset.is_multiple_of(2) {
                permuted.reverse();
            }
            let permuted = MatcherResults::with_details(permuted, details.clone()).unwrap();
            for _ in 0..3 {
                assert_bits_equal(&expected, &select(&permuted, Some(0.0), aware));
            }
        }
    }
    assert_bits_equal(
        &original,
        &MatcherResults::with_details(entries, details).unwrap(),
    );
}

#[test]
fn rectangular_unique_optima_keep_near_equal_float_scores() {
    let above = f64::from_bits(0.5_f64.to_bits() + 1);
    for transpose in [false, true] {
        let mut entries = vec![
            (pair(0, 0), 0.5),
            (pair(0, 1), above),
            (pair(1, 0), 0.25),
            (pair(1, 1), 0.25),
            (pair(2, 0), 0.0),
        ];
        if transpose {
            for (identity, _) in &mut entries {
                std::mem::swap(&mut identity.source_column, &mut identity.target_column);
            }
        }
        let results = MatcherResults::new(entries).unwrap();
        for aware in [false, true] {
            let selected = select(&results, Some(0.0), aware);
            assert_eq!(selected.len(), 2);
            assert_eq!(
                selected.iter().map(|(_, score)| score).sum::<f64>(),
                0.75 + f64::EPSILON / 2.0
            );
            let best = if transpose { pair(1, 0) } else { pair(0, 1) };
            assert_eq!(selected.get(&best).unwrap().to_bits(), above.to_bits());
            assert_eq!(
                optimum(&results, 0.0, aware).1,
                selected.iter().map(|(_, score)| score).sum::<f64>()
            );
        }
    }
}

#[test]
fn rectangular_near_ties_match_independent_scaled_integer_objectives() {
    // All these binary64 values are exact multiples of 2^-55. Integer sums
    // distinguish objectives even when ordinary floating summation rounds them
    // together. In the first case, uncompensated reduced costs lost 2^-55 and
    // selected (0,1)+(1,0), rather than the unique optimum (0,2)+(1,0).
    let quarter = 1u64 << 53;
    let half = 1u64 << 54;
    let scale = 1u64 << 55;
    let weights = [
        0,
        quarter - 1,
        quarter,
        quarter + 2,
        half - 2,
        half,
        half + 4,
        scale - 4,
        scale,
    ];
    let mut cases = vec![[
        [half + 4, quarter - 1, quarter],
        [scale - 4, quarter + 2, half + 4],
    ]];
    let mut state = 0x91a3_5ed8_b72c_4f06u64;
    for _ in 0..256 {
        let mut matrix = [[0; 3]; 2];
        for row in &mut matrix {
            for weight in row {
                state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
                *weight = weights[(state.rotate_right(32) % weights.len() as u64) as usize];
            }
        }
        cases.push(matrix);
    }
    for matrix in cases {
        for transpose in [false, true] {
            let results = MatcherResults::new(
                matrix
                    .iter()
                    .enumerate()
                    .flat_map(|(row, values)| {
                        values.iter().enumerate().map(move |(col, weight)| {
                            (
                                if transpose {
                                    pair(col, row)
                                } else {
                                    pair(row, col)
                                },
                                *weight as f64 / scale as f64,
                            )
                        })
                    })
                    .collect(),
            )
            .unwrap();
            let thresholds = [0, quarter, half, scale];
            for aware in [false, true] {
                for threshold in if aware {
                    &thresholds[..]
                } else {
                    &thresholds[..1]
                } {
                    let selected = select(&results, Some(*threshold as f64 / scale as f64), aware);
                    let actual = (
                        if aware { selected.len() } else { 0 },
                        selected
                            .iter()
                            .map(|(_, score)| (*score * scale as f64) as u128)
                            .sum::<u128>(),
                    );
                    let mut expected = (0, 0);
                    for first in 0..3 {
                        for second in 0..3 {
                            if first != second {
                                let selected = [matrix[0][first], matrix[1][second]];
                                let candidate = selected
                                    .into_iter()
                                    .filter(|score| !aware || *score >= *threshold)
                                    .fold((0, 0), |(count, sum), score| {
                                        (count + usize::from(aware), sum + u128::from(score))
                                    });
                                expected = expected.max(candidate);
                            }
                        }
                    }
                    assert_eq!(
                        actual, expected,
                        "matrix {matrix:?}, transpose {transpose}, aware {aware}, threshold {threshold}"
                    );
                    assert_retained(&results, &selected, *threshold as f64 / scale as f64);
                }
            }
        }
    }
}

#[test]
fn rectangular_subnormal_scores_keep_exact_objectives_and_bits() {
    for transpose in [false, true] {
        let entries = [(0, 0, 2), (0, 1, 4), (1, 0, 3), (1, 1, 3), (2, 0, 1)]
            .into_iter()
            .map(|(row, col, bits)| {
                let identity = if transpose {
                    pair(col, row)
                } else {
                    pair(row, col)
                };
                (identity, f64::from_bits(bits))
            })
            .collect();
        let results = MatcherResults::new(entries).unwrap();
        verify_objectives(&results);
        for aware in [false, true] {
            for threshold in [
                None,
                Some(0.0),
                Some(f64::from_bits(2)),
                Some(f64::from_bits(4)),
            ] {
                let selected = select(&results, threshold, aware);
                assert_retained(&results, &selected, cutoff(&results, threshold));
                if aware {
                    assert_eq!(
                        (
                            selected.len(),
                            selected.iter().map(|(_, score)| score).sum::<f64>()
                        ),
                        optimum(&results, cutoff(&results, threshold), true)
                    );
                }
            }
        }
        assert_eq!(
            select(&results, Some(0.0), false)
                .iter()
                .map(|(_, score)| score)
                .sum::<f64>()
                .to_bits(),
            7
        );
    }
}

#[test]
fn rectangular_limits_include_all_identities_even_when_threshold_excludes_them() {
    let results = MatcherResults::new(
        (0..2)
            .flat_map(|row| (0..3).map(move |col| (pair(row, col), 0.25)))
            .collect(),
    )
    .unwrap();
    let limits = WorkLimits {
        max_assignment_dimension: 3,
        max_matrix_cells: 6,
        ..Default::default()
    };
    for aware in [false, true] {
        for threshold in [Some(0.0), Some(0.5)] {
            assert_bits_equal(
                &select(&results, threshold, aware),
                &checked(&results, threshold, aware, &limits).unwrap(),
            );
            for too_small in [
                WorkLimits {
                    max_assignment_dimension: 2,
                    ..limits
                },
                WorkLimits {
                    max_matrix_cells: 5,
                    ..limits
                },
            ] {
                assert!(checked(&results, threshold, aware, &too_small).is_err());
            }
        }
        let empty = MatcherResults::default();
        let zero = WorkLimits {
            max_assignment_dimension: 0,
            max_matrix_cells: 0,
            ..limits
        };
        assert!(checked(&empty, None, aware, &zero).unwrap().is_empty());
        let singleton = MatcherResults::new(vec![(pair(0, 0), 0.0)]).unwrap();
        assert!(checked(&singleton, None, aware, &zero).is_err());
        for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(checked(&results, Some(invalid), aware, &limits).is_err());
            assert!(checked(&empty, Some(invalid), aware, &zero).is_err());
            let error = if aware {
                results.one_to_one_rectangular_threshold_aware(Some(invalid))
            } else {
                results.one_to_one_rectangular(Some(invalid))
            };
            assert!(error.is_err());
        }
    }
}
