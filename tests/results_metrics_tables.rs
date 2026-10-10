use fieldkin::metrics::{
    F1Score, GroundTruth, MeanReciprocalRank, Metric, OneToOneMethod, Precision, Recall,
};
use fieldkin::{ColumnPair, DataType, MatcherResults, Table};

fn pair(s: &str, t: &str) -> ColumnPair {
    ColumnPair::new("source", s, "target", t)
}

#[test]
fn hungarian_finds_global_optimum_that_greedy_misses() {
    let results = MatcherResults::new(vec![
        (pair("a", "x"), 0.9),
        (pair("a", "y"), 0.8),
        (pair("b", "x"), 0.85),
        (pair("b", "y"), 0.1),
    ])
    .unwrap();
    let optimal = results.one_to_one_hungarian(Some(0.0)).unwrap();
    assert_eq!(optimal.get(&pair("a", "y")), Some(0.8));
    assert_eq!(optimal.get(&pair("b", "x")), Some(0.85));
    assert!(
        optimal.iter().map(|(_, s)| s).sum::<f64>()
            > results
                .one_to_one_greedy(Some(0.0))
                .unwrap()
                .iter()
                .map(|(_, s)| s)
                .sum::<f64>()
    );
}

#[test]
fn hungarian_threshold_filtering_can_discard_an_eligible_assignment() {
    let results = MatcherResults::new(vec![
        (pair("a", "x"), 0.90),
        (pair("a", "y"), 0.80),
        (pair("b", "x"), 0.81),
        (pair("b", "y"), 0.79),
    ])
    .unwrap();
    let legacy = results.one_to_one_hungarian(Some(0.80)).unwrap();
    assert_eq!(legacy.len(), 1);
    assert_eq!(legacy.get(&pair("a", "x")), Some(0.90));
    let selected = results
        .one_to_one_hungarian_threshold_aware(Some(0.80))
        .unwrap();
    assert_eq!(selected.len(), 2, "a -> y and b -> x are both eligible");
    assert_eq!(selected.get(&pair("a", "y")), Some(0.80));
    assert_eq!(selected.get(&pair("b", "x")), Some(0.81));
}

#[test]
fn threshold_aware_hungarian_prioritizes_cardinality_then_similarity() {
    let sparse = MatcherResults::new(vec![
        (pair("a", "x"), 1.0),
        (pair("a", "y"), 0.4),
        (pair("b", "x"), 0.4),
    ])
    .unwrap();
    assert_eq!(sparse.one_to_one_hungarian(Some(0.4)).unwrap().len(), 1);
    let selected = sparse
        .one_to_one_hungarian_threshold_aware(Some(0.4))
        .unwrap();
    assert_eq!(selected.len(), 2);
    assert_eq!(selected.get(&pair("a", "y")), Some(0.4));
    assert_eq!(selected.get(&pair("b", "x")), Some(0.4));

    let complete = MatcherResults::new(vec![
        (pair("a", "x"), 0.9),
        (pair("a", "y"), 0.8),
        (pair("b", "x"), 0.81),
        (pair("b", "y"), 0.8),
    ])
    .unwrap();
    let selected = complete
        .one_to_one_hungarian_threshold_aware(Some(0.8))
        .unwrap();
    assert_eq!(selected.len(), 2);
    assert_eq!(selected.get(&pair("a", "x")), Some(0.9));
    assert_eq!(selected.get(&pair("b", "y")), Some(0.8));
}

#[test]
fn threshold_aware_hungarian_breaks_equal_score_ties_deterministically() {
    let entries = vec![
        (pair("b", "y"), 0.8),
        (pair("a", "y"), 0.8),
        (pair("b", "x"), 0.8),
        (pair("a", "x"), 0.8),
    ];
    let expected = MatcherResults::new(vec![(pair("a", "x"), 0.8), (pair("b", "y"), 0.8)]).unwrap();
    for entries in [entries.clone(), entries.into_iter().rev().collect()] {
        let results = MatcherResults::new(entries).unwrap();
        for threshold in [None, Some(0.0), Some(0.8)] {
            let selected = results
                .one_to_one_hungarian_threshold_aware(threshold)
                .unwrap();
            assert_eq!(selected, expected);
            assert_eq!(selected, results.one_to_one_hungarian(threshold).unwrap());
        }
    }
}

#[test]
fn threshold_aware_hungarian_preserves_close_similarity_differences() {
    let higher = f64::from_bits(0.5f64.to_bits() + 2);
    let results = MatcherResults::new(vec![
        (pair("a", "x"), 0.5),
        (pair("a", "y"), higher),
        (pair("b", "x"), 0.25),
        (pair("b", "y"), 0.25),
    ])
    .unwrap();
    let selected = results
        .one_to_one_hungarian_threshold_aware(Some(0.0))
        .unwrap();
    assert_eq!(selected.len(), 2);
    assert_eq!(selected.get(&pair("a", "y")), Some(higher));
    assert_eq!(selected.get(&pair("b", "x")), Some(0.25));
}

#[test]
fn threshold_aware_hungarian_keeps_the_distinct_score_default_cutoff() {
    for (scores, cutoff) in [
        (vec![0.8], 0.8),
        (vec![0.8, 0.8, 0.8], 0.8),
        (vec![1.0, 0.9, 0.8, 0.7], 0.8),
        (vec![1.0, 0.9, 0.9, 0.8, 0.7, 0.6], 0.7),
    ] {
        let results = MatcherResults::new(
            scores
                .into_iter()
                .enumerate()
                .map(|(i, score)| (pair(&i.to_string(), &i.to_string()), score))
                .collect(),
        )
        .unwrap();
        let selected = results.one_to_one_hungarian_threshold_aware(None).unwrap();
        assert_eq!(selected, results.filter(cutoff).unwrap());
        assert_eq!(
            selected,
            results
                .one_to_one_hungarian_threshold_aware(Some(cutoff))
                .unwrap()
        );
        assert_eq!(selected, results.one_to_one_hungarian(None).unwrap());
    }
}

#[test]
fn threshold_aware_hungarian_handles_zero_scores_and_threshold_validation() {
    let results = MatcherResults::new(vec![
        (pair("a", "x"), 0.0),
        (pair("a", "y"), 0.0),
        (pair("b", "y"), 0.0),
    ])
    .unwrap();
    let expected = MatcherResults::new(vec![(pair("a", "x"), 0.0), (pair("b", "y"), 0.0)]).unwrap();
    for threshold in [None, Some(0.0), Some(-1.0), Some(f64::MIN)] {
        assert_eq!(
            results
                .one_to_one_hungarian_threshold_aware(threshold)
                .unwrap(),
            expected
        );
    }
    for threshold in [f64::MIN_POSITIVE, 1.1, f64::MAX] {
        assert!(
            results
                .one_to_one_hungarian_threshold_aware(Some(threshold))
                .unwrap()
                .is_empty()
        );
    }
    for collection in [&results, &MatcherResults::default()] {
        for threshold in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(matches!(
                collection.one_to_one_hungarian_threshold_aware(Some(threshold)),
                Err(fieldkin::Error::InvalidConfig(_))
            ));
            assert!(collection.one_to_one_hungarian(Some(threshold)).is_err());
        }
    }
}

#[test]
fn one_to_one_selection_is_global_across_table_pairs() {
    for entries in [
        vec![
            (ColumnPair::new("a", "id", "b", "id"), 0.9),
            (ColumnPair::new("a", "id", "c", "id"), 0.8),
        ],
        vec![
            (ColumnPair::new("b", "id", "a", "id"), 0.9),
            (ColumnPair::new("c", "id", "a", "id"), 0.8),
        ],
    ] {
        let results = MatcherResults::new(entries.clone()).unwrap();
        for selected in [
            results.one_to_one_hungarian(Some(0.0)).unwrap(),
            results
                .one_to_one_hungarian_threshold_aware(Some(0.0))
                .unwrap(),
            results.one_to_one_greedy(Some(0.0)).unwrap(),
            results.one_to_one_mutual_top(1).unwrap(),
        ] {
            assert_eq!(selected.len(), 1);
            assert_eq!(selected.get(&entries[0].0), Some(0.9));
        }
        // Each table pair has a valid mapping in isolation, but the shared
        // source or target column may only be used once in the global collection.
        for entry in entries {
            let independent = MatcherResults::new(vec![entry]).unwrap();
            assert_eq!(
                independent.one_to_one_hungarian(Some(0.0)).unwrap().len(),
                1
            );
        }
    }
}

#[test]
fn default_cutoff_uses_scores_from_all_table_pairs() {
    let high = MatcherResults::new(vec![
        (ColumnPair::new("a", "id", "b", "id"), 0.9),
        (ColumnPair::new("a", "name", "b", "name"), 0.8),
        (ColumnPair::new("a", "value", "b", "value"), 0.7),
    ])
    .unwrap();
    let low = MatcherResults::new(vec![
        (ColumnPair::new("c", "id", "d", "id"), 0.6),
        (ColumnPair::new("c", "name", "d", "name"), 0.5),
        (ColumnPair::new("c", "value", "d", "value"), 0.4),
    ])
    .unwrap();
    let combined = MatcherResults::new(
        high.iter()
            .chain(low.iter())
            .map(|(p, s)| (p.clone(), *s))
            .collect(),
    )
    .unwrap();
    assert_eq!(high.one_to_one_hungarian(None).unwrap().len(), 3);
    assert_eq!(low.one_to_one_hungarian(None).unwrap().len(), 3);
    for selected in [
        combined.one_to_one_hungarian(None).unwrap(),
        combined.one_to_one_hungarian_threshold_aware(None).unwrap(),
        combined.one_to_one_greedy(None).unwrap(),
    ] {
        assert_eq!(selected.len(), 4);
        assert!(selected.iter().all(|(_, score)| *score >= 0.6));
    }
    // Table names participate in identity: with an explicit threshold these
    // disjoint pairs do not compete despite identical column names.
    for selected in [
        combined.one_to_one_hungarian(Some(0.0)).unwrap(),
        combined
            .one_to_one_hungarian_threshold_aware(Some(0.0))
            .unwrap(),
        combined.one_to_one_greedy(Some(0.0)).unwrap(),
        combined.one_to_one_mutual_top(1).unwrap(),
    ] {
        assert_eq!(selected, combined);
    }
}

#[test]
fn tied_and_rectangular_assignments_are_truly_one_to_one() {
    let results = MatcherResults::new(vec![
        (pair("a", "x"), 1.0),
        (pair("b", "x"), 1.0),
        (pair("a", "y"), 1.0),
        (pair("b", "y"), 1.0),
        (pair("c", "y"), 1.0),
    ])
    .unwrap();
    for selected in [
        results.one_to_one_hungarian(None).unwrap(),
        results.one_to_one_hungarian_threshold_aware(None).unwrap(),
        results.one_to_one_greedy(None).unwrap(),
        results.one_to_one_mutual_top(1).unwrap(),
    ] {
        let mut sources = std::collections::BTreeSet::new();
        let mut targets = std::collections::BTreeSet::new();
        assert!(selected.len() <= 2);
        for (p, _) in &selected {
            assert!(sources.insert(p.source()));
            assert!(targets.insert(p.target()));
        }
    }
    assert_eq!(results.len(), 5);
    assert!(results.one_to_one_hungarian(Some(1.1)).unwrap().is_empty());
}

#[test]
fn selectors_preserve_immutable_details() {
    let a = pair("a", "x");
    let b = pair("a", "y");
    let details = [
        (a.clone(), [("NameCM".to_owned(), 0.9)].into()),
        (b.clone(), [("NameCM".to_owned(), 0.1)].into()),
    ]
    .into();
    let results =
        MatcherResults::with_details(vec![(a.clone(), 0.9), (b.clone(), 0.1)], details).unwrap();
    let selected = results.take_top_n_per_source(1);
    assert_eq!(selected.len(), 1);
    assert_eq!(selected.details().len(), 1);
    assert_eq!(selected.get_details(&a).unwrap()["NameCM"], 0.9);
    assert_eq!(results.len(), 2);
    let json = serde_json::to_value(&selected).unwrap();
    assert_eq!(json["matches"][0]["details"]["NameCM"], 0.9);
    assert_eq!(results.take_top_percent(1.0).unwrap().len(), 1);
    assert!(results.filter(f64::NAN).is_err());
    assert!(results.take_top_percent(101.0).is_err());
    assert!(results.one_to_one_mutual_top(0).is_err());
}

#[test]
fn every_selector_retains_only_selected_details_and_preserves_order() {
    let a = pair("a", "x");
    let b = pair("a", "y");
    let c = pair("b", "x");
    let d = pair("b", "y");
    let results = MatcherResults::with_details(
        vec![
            (d.clone(), 0.1),
            (c.clone(), 0.8),
            (b.clone(), 0.8),
            (a.clone(), 0.9),
        ],
        [
            (
                a.clone(),
                [("NameCM".into(), 0.7), ("InstancesCM".into(), 0.9)].into(),
            ),
            (
                c.clone(),
                [("NameCM".into(), 0.2), ("InstancesCM".into(), 0.8)].into(),
            ),
            (d.clone(), Default::default()),
        ]
        .into(),
    )
    .unwrap();
    let original = results.clone();
    let cases = [
        (
            "filter",
            results.filter(0.8).unwrap(),
            vec![(a.clone(), 0.9), (b.clone(), 0.8), (c.clone(), 0.8)],
            vec![a.clone(), c.clone()],
        ),
        (
            "top n",
            results.take_top_n(2),
            vec![(a.clone(), 0.9), (b.clone(), 0.8)],
            vec![a.clone()],
        ),
        (
            "top percent",
            results.take_top_percent(50.0).unwrap(),
            vec![(a.clone(), 0.9), (b.clone(), 0.8)],
            vec![a.clone()],
        ),
        (
            "top n per source",
            results.take_top_n_per_source(1),
            vec![(a.clone(), 0.9), (c.clone(), 0.8)],
            vec![a.clone(), c.clone()],
        ),
        (
            "Hungarian",
            results.one_to_one_hungarian(Some(0.0)).unwrap(),
            vec![(b.clone(), 0.8), (c.clone(), 0.8)],
            vec![c.clone()],
        ),
        (
            "threshold-aware Hungarian",
            results
                .one_to_one_hungarian_threshold_aware(Some(0.0))
                .unwrap(),
            vec![(b.clone(), 0.8), (c.clone(), 0.8)],
            vec![c.clone()],
        ),
        (
            "greedy",
            results.one_to_one_greedy(Some(0.0)).unwrap(),
            vec![(a.clone(), 0.9), (d.clone(), 0.1)],
            vec![a.clone(), d.clone()],
        ),
        (
            "mutual top",
            results.one_to_one_mutual_top(1).unwrap(),
            vec![(a.clone(), 0.9)],
            vec![a.clone()],
        ),
    ];
    for (name, selected, entries, detail_keys) in cases {
        assert_eq!(
            selected
                .iter()
                .map(|(pair, score)| (pair.clone(), *score))
                .collect::<Vec<_>>(),
            entries,
            "{name}: entries or ordering changed"
        );
        assert_eq!(
            selected.details().keys().cloned().collect::<Vec<_>>(),
            detail_keys,
            "{name}: details retained for the wrong entries"
        );
        for (pair, _) in &original {
            let expected = if selected.get(pair).is_some() {
                original.get_details(pair)
            } else {
                None
            };
            assert_eq!(selected.get_details(pair), expected, "{name}: {pair:?}");
        }
        assert_eq!(results, original, "{name}: original results changed");
    }
}

#[test]
fn selecting_no_entries_discards_all_details_without_changing_original() {
    let key = pair("a", "x");
    let results = MatcherResults::with_details(
        vec![(key.clone(), 0.9)],
        [(key, [("NameCM".into(), 0.8)].into())].into(),
    )
    .unwrap();
    let original = results.clone();
    for selected in [
        results.filter(1.1).unwrap(),
        results.take_top_n(0),
        results.take_top_percent(0.0).unwrap(),
        results.take_top_n_per_source(0),
        results.one_to_one_hungarian(Some(1.1)).unwrap(),
        results
            .one_to_one_hungarian_threshold_aware(Some(1.1))
            .unwrap(),
        results.one_to_one_greedy(Some(1.1)).unwrap(),
    ] {
        assert!(selected.is_empty());
        assert!(selected.details().is_empty());
        assert_eq!(results, original);
    }
}

#[test]
fn known_metrics_and_source_local_reciprocal_rank() {
    let results = MatcherResults::new(vec![
        (pair("a", "wrong"), 0.95),
        (pair("b", "yes"), 0.9),
        (pair("a", "yes"), 0.8),
    ])
    .unwrap();
    let truth = GroundTruth::Names(vec![
        ("a".into(), "yes".into()),
        ("b".into(), "yes".into()),
        ("c".into(), "absent".into()),
    ]);
    let method = OneToOneMethod::Hungarian;
    let p = Precision { one_to_one: false }
        .apply(&results, &truth, method)
        .unwrap();
    let r = Recall { one_to_one: false }
        .apply(&results, &truth, method)
        .unwrap();
    let f = F1Score { one_to_one: false }
        .apply(&results, &truth, method)
        .unwrap();
    assert!((p - 2.0 / 3.0).abs() < 1e-12);
    assert!((r - 2.0 / 3.0).abs() < 1e-12);
    assert!((f - 2.0 / 3.0).abs() < 1e-12);
    assert!(
        (MeanReciprocalRank::default()
            .apply(&results, &truth, method)
            .unwrap()
            - 0.5)
            .abs()
            < 1e-12
    );
    assert_eq!(
        MatcherResults::default().get_metrics(&truth).unwrap()["F1Score"],
        0.0
    );
}

#[test]
fn table_identity_and_duplicate_gold_cannot_inflate_metrics() {
    let results = MatcherResults::new(vec![(pair("a", "x"), 1.0)]).unwrap();
    let truth = GroundTruth::Columns(vec![ColumnPair::new("other", "a", "target", "x")]);
    assert_eq!(results.get_metrics(&truth).unwrap()["Precision"], 0.0);
    let truth = GroundTruth::Names(vec![("a".into(), "x".into()), ("a".into(), "x".into())]);
    assert_eq!(results.get_metrics(&truth).unwrap()["F1Score"], 1.0);
}

#[test]
fn predefined_metric_sets_keep_both_assignment_configurations() {
    use fieldkin::metrics::{
        METRICS_ALL, METRICS_PRECISION_INCREASING_N, METRICS_PRECISION_RECALL,
    };
    let results = MatcherResults::new(vec![
        (pair("a", "x"), 1.0),
        (pair("a", "y"), 0.8),
        (pair("b", "z"), 0.6),
    ])
    .unwrap();
    let truth = GroundTruth::Names(vec![("a".into(), "x".into()), ("b".into(), "z".into())]);
    let metrics = results
        .get_metrics_with(&truth, &METRICS_ALL, OneToOneMethod::Hungarian)
        .unwrap();
    assert_eq!(metrics.len(), 9);
    assert_eq!(metrics["Precision"], 1.0);
    assert!((metrics["PrecisionWithoutOneToOne"] - 2.0 / 3.0).abs() < 1e-12);
    assert_eq!(
        results
            .get_metrics_with(&truth, &METRICS_PRECISION_RECALL, OneToOneMethod::Greedy)
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        results
            .get_metrics_with(
                &truth,
                &METRICS_PRECISION_INCREASING_N,
                OneToOneMethod::MutualTop
            )
            .unwrap()
            .len(),
        10
    );
}

#[test]
fn csv_and_json_handle_quotes_nulls_and_type_inference() {
    let csv = Table::from_csv(
        "a",
        "id,note,flag\n1,\"Hello, world\",true\n2,,false\n".as_bytes(),
    )
    .unwrap();
    assert_eq!(csv.columns[0].data_type, DataType::Integer);
    assert_eq!(csv.columns[1].samples, ["Hello, world"]);
    assert_eq!(csv.columns[2].data_type, DataType::Boolean);
    let json = Table::from_json(
        "b",
        r#"[{"id":1,"name":"Alice"},{"id":2,"name":null},{"name":"Bob"}]"#.as_bytes(),
    )
    .unwrap();
    assert_eq!(json.columns[0].samples, ["1", "2"]);
    assert_eq!(json.columns[1].samples, ["Alice", "Bob"]);
    assert!(Table::from_csv("x", "id,id\n1,2\n".as_bytes()).is_err());
    assert!(Table::from_csv("x", "id,name\n1\n".as_bytes()).is_err());
    assert!(Table::from_json("x", r#"[{"a":{}}]"#.as_bytes()).is_err());
}

#[test]
fn invalid_scores_and_empty_results_are_handled() {
    for s in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
        assert!(MatcherResults::new(vec![(pair("a", "x"), s)]).is_err());
    }
    let results = MatcherResults::default();
    assert!(results.one_to_one_hungarian(None).unwrap().is_empty());
    assert!(
        results
            .one_to_one_hungarian_threshold_aware(None)
            .unwrap()
            .is_empty()
    );
    assert!(results.one_to_one_greedy(None).unwrap().is_empty());
    assert!(results.one_to_one_mutual_top(1).unwrap().is_empty());
}

#[test]
fn hungarian_matches_exhaustive_search_on_small_sparse_matrices() {
    fn brute(matrix: &[Vec<f64>], row: usize, used: &mut [bool]) -> f64 {
        if row == matrix.len() {
            return 0.0;
        }
        let mut best = 0.0f64;
        for col in 0..matrix.len() {
            if !used[col] {
                used[col] = true;
                best = best.max(matrix[row][col] + brute(matrix, row + 1, used));
                used[col] = false;
            }
        }
        best
    }
    for seed in 0..30usize {
        let mut matrix = vec![vec![0.0; 4]; 4];
        let mut entries = Vec::new();
        for (i, row) in matrix.iter_mut().enumerate() {
            for (j, score) in row.iter_mut().enumerate() {
                *score = ((seed * 17 + i * 13 + j * 7 + i * j * 19) % 11) as f64 / 10.0;
                if *score > 0.0 {
                    entries.push((pair(&i.to_string(), &j.to_string()), *score));
                }
            }
        }
        let results = MatcherResults::new(entries).unwrap();
        let score = results
            .one_to_one_hungarian(Some(0.0))
            .unwrap()
            .iter()
            .map(|(_, s)| s)
            .sum::<f64>();
        assert!((score - brute(&matrix, 0, &mut [false; 4])).abs() < 1e-10);
    }
}

#[test]
fn threshold_aware_hungarian_matches_exhaustive_sparse_rectangular_assignments() {
    fn brute(
        matrix: &[Vec<Option<f64>>],
        threshold: f64,
        row: usize,
        used: &mut [bool],
    ) -> (usize, f64) {
        if row == matrix.len() {
            return (0, 0.0);
        }
        let mut best = brute(matrix, threshold, row + 1, used);
        for (col, score) in matrix[row].iter().enumerate() {
            if let Some(score) = score.filter(|score| *score >= threshold && !used[col]) {
                used[col] = true;
                let (count, total) = brute(matrix, threshold, row + 1, used);
                used[col] = false;
                let candidate = (count + 1, total + score);
                if candidate.0 > best.0 || (candidate.0 == best.0 && candidate.1 > best.1) {
                    best = candidate;
                }
            }
        }
        best
    }

    for rows in 1..=4 {
        for cols in 1..=4 {
            for seed in 0..12 {
                let mut matrix = vec![vec![None; cols]; rows];
                let mut entries = Vec::new();
                for (row, values) in matrix.iter_mut().enumerate() {
                    for (col, value) in values.iter_mut().enumerate() {
                        let code = seed * 17 + row * 13 + col * 7 + row * col * 19;
                        if code % 5 != 0 {
                            // Dyadic scores make the exhaustive total exact, including zero.
                            let score = (code % 9) as f64 / 8.0;
                            *value = Some(score);
                            entries.push((pair(&row.to_string(), &col.to_string()), score));
                        }
                    }
                }
                let results = MatcherResults::new(entries).unwrap();
                for threshold in [-1.0, 0.0, 0.5, 1.0, 1.1] {
                    let selected = results
                        .one_to_one_hungarian_threshold_aware(Some(threshold))
                        .unwrap();
                    let mut sources = std::collections::BTreeSet::new();
                    let mut targets = std::collections::BTreeSet::new();
                    for (pair, score) in &selected {
                        assert!(*score >= threshold);
                        assert_eq!(results.get(pair), Some(*score));
                        assert!(sources.insert(pair.source()));
                        assert!(targets.insert(pair.target()));
                    }
                    let actual = (
                        selected.len(),
                        selected.iter().map(|(_, score)| *score).sum::<f64>(),
                    );
                    assert_eq!(
                        actual,
                        brute(&matrix, threshold, 0, &mut vec![false; cols]),
                        "{rows}x{cols}, seed {seed}, threshold {threshold}"
                    );
                }
            }
        }
    }
}
