use std::sync::{Arc, Mutex};

use fieldkin::algorithms::{DistributionBased, EmbeddingProvider, JaccardDistanceMatcher};
use fieldkin::{
    CheckedMatcher, DataType, Error, Field, MatchOptions, Matcher, MatcherResults, Table,
    WorkLimits, match_tables, match_tables_one_to_one_with_limits, match_tables_with_limits,
};

fn table(name: &str, width: usize) -> Table {
    Table::new(
        name,
        (0..width)
            .map(|i| Field {
                name: format!("column{i}"),
                data_type: DataType::Text,
                samples: vec!["".into(), "shared".into(), "shared".into(), "other".into()],
            })
            .collect(),
    )
    .unwrap()
}

fn assert_identical(actual: &MatcherResults, expected: &MatcherResults) {
    assert_eq!(
        actual
            .iter()
            .map(|(p, s)| (p, s.to_bits()))
            .collect::<Vec<_>>(),
        expected
            .iter()
            .map(|(p, s)| (p, s.to_bits()))
            .collect::<Vec<_>>()
    );
    let detail_bits = |results: &MatcherResults| {
        results
            .details()
            .iter()
            .map(|(pair, details)| {
                (
                    pair.clone(),
                    details
                        .iter()
                        .map(|(key, value)| (key.clone(), value.to_bits()))
                        .collect::<Vec<_>>(),
                )
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(detail_bits(actual), detail_bits(expected));
}

#[test]
fn empty_and_singleton_distribution_batches_skip_matrix_work() {
    let matcher = DistributionBased::default();
    let singleton = [table("only", 2)];
    let zero = WorkLimits {
        max_matrix_cells: 0,
        ..Default::default()
    };
    for batch in [&singleton[..0], &singleton[..]] {
        assert!(matcher.get_matches_batch(batch).unwrap().is_empty());
        assert!(
            matcher
                .get_matches_batch_with_limits(batch, &zero)
                .unwrap()
                .is_empty()
        );
    }
    let pair = [singleton[0].clone(), table("target", 2)];
    let expected = matcher.get_matches_batch(&pair).unwrap();
    assert_eq!(expected.len(), 4);
    let limits = WorkLimits {
        max_matrix_cells: 16,
        ..zero
    };
    assert_identical(
        &matcher
            .get_matches_batch_with_limits(&pair, &limits)
            .unwrap(),
        &expected,
    );
    let error = matcher
        .get_matches_batch_with_limits(
            &pair,
            &WorkLimits {
                max_matrix_cells: 15,
                ..limits
            },
        )
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("distance-matrix cells workload 16 exceeds configured limit 15")
    );
}

#[test]
fn noop_batch_validation_still_rejects_invalid_tables_and_configuration() {
    let jaccard = JaccardDistanceMatcher::default();
    let distribution = DistributionBased::default();
    let limits = WorkLimits::default();
    let mut invalid_name = table("only", 2);
    invalid_name.name.clear();
    let mut invalid_column = table("only", 2);
    invalid_column.columns[0].name.clear();
    let mut duplicate_column = table("only", 2);
    duplicate_column.columns[1] = duplicate_column.columns[0].clone();
    for invalid in [invalid_name, invalid_column, duplicate_column] {
        let batch = std::slice::from_ref(&invalid);
        for result in [
            jaccard.get_matches_batch(batch),
            jaccard.get_matches_batch_with_limits(batch, &limits),
            distribution.get_matches_batch(batch),
            distribution.get_matches_batch_with_limits(batch, &limits),
        ] {
            assert!(matches!(result, Err(Error::InvalidInput(_))));
        }
    }
    let invalid = DistributionBased {
        quantiles: 0,
        ..Default::default()
    };
    let singleton = [table("only", 2)];
    for batch in [&singleton[..0], &singleton[..]] {
        for result in [
            invalid.get_matches_batch(batch),
            invalid.get_matches_batch_with_limits(batch, &limits),
        ] {
            assert!(
                matches!(result, Err(Error::InvalidConfig(message)) if message == "quantiles must be >= 1")
            );
        }
    }
}

#[test]
fn checked_composition_preserves_sampling_batch_statistics_and_selection_bits() {
    let jaccard = JaccardDistanceMatcher::default();
    let distribution = DistributionBased {
        threshold1: 1.0,
        threshold2: 1.0,
        quantiles: 4,
        ..Default::default()
    };
    let tables = [table("a", 2), table("b", 3), table("c", 1)];
    let original = tables.clone();
    let limits = WorkLimits {
        max_assignment_dimension: 5,
        max_matrix_cells: 36,
        max_clustering_variables: 0,
        max_clustering_constraints: 0,
        max_jaccard_comparisons: 44,
    };
    for (checked, unchecked) in [
        (
            CheckedMatcher::Jaccard(&jaccard),
            &jaccard as &dyn fieldkin::Matcher,
        ),
        (
            CheckedMatcher::DistributionBased(&distribution),
            &distribution as &dyn fieldkin::Matcher,
        ),
    ] {
        for sample_size in [None, Some(0), Some(1)] {
            for threshold in [None, Some(1.0)] {
                let options = MatchOptions {
                    instance_sample_size: sample_size,
                };
                let expected = match_tables(&tables, unchecked, options).unwrap();
                let raw = match_tables_with_limits(&tables, checked, options, &limits).unwrap();
                assert_identical(&raw, &expected);
                for cardinality_first in [false, true] {
                    let (actual, expected) = if cardinality_first {
                        (
                            raw.one_to_one_rectangular_threshold_aware_with_limits(
                                threshold, &limits,
                            ),
                            expected.one_to_one_rectangular_threshold_aware(threshold),
                        )
                    } else {
                        (
                            raw.one_to_one_rectangular_with_limits(threshold, &limits),
                            expected.one_to_one_rectangular(threshold),
                        )
                    };
                    assert_identical(&actual.unwrap(), &expected.unwrap());
                }
                let expected = expected.one_to_one_hungarian(threshold).unwrap();
                let actual = match_tables_one_to_one_with_limits(
                    &tables, checked, options, threshold, &limits,
                )
                .unwrap();
                assert_identical(&actual, &expected);
            }
        }
    }
    assert_eq!(tables, original);
}

#[test]
fn checked_composition_checks_both_stages_even_for_rectangular_ties() {
    let jaccard = JaccardDistanceMatcher::default();
    let tables = [table("a", 2), table("b", 3)];
    let limits = WorkLimits {
        max_assignment_dimension: 3,
        max_matrix_cells: 6,
        max_jaccard_comparisons: 24,
        ..Default::default()
    };
    let run = |tables: &[Table], limits: &WorkLimits| {
        match_tables_one_to_one_with_limits(
            tables,
            CheckedMatcher::Jaccard(&jaccard),
            MatchOptions {
                instance_sample_size: None,
            },
            None,
            limits,
        )
    };
    assert_eq!(run(&tables, &limits).unwrap().len(), 2);
    for (limits, operation) in [
        (
            WorkLimits {
                max_assignment_dimension: 2,
                ..limits
            },
            "Hungarian",
        ),
        (
            WorkLimits {
                max_matrix_cells: 5,
                ..limits
            },
            "Jaccard column pairs",
        ),
        (
            WorkLimits {
                max_jaccard_comparisons: 23,
                ..limits
            },
            "Jaccard distinct-value comparisons",
        ),
    ] {
        assert!(
            run(&tables, &limits)
                .unwrap_err()
                .to_string()
                .contains(operation)
        );
    }
    let empty = [table("a", 0), table("b", 0)];
    let zero = WorkLimits {
        max_assignment_dimension: 0,
        max_matrix_cells: 0,
        max_jaccard_comparisons: 0,
        ..limits
    };
    assert!(run(&empty, &zero).unwrap().is_empty());
    assert!(run(&[], &zero).is_err());
    assert!(run(&empty[..1], &zero).is_err());

    let distribution = DistributionBased {
        quantiles: 4,
        ..Default::default()
    };
    let run_distribution = |limits: &WorkLimits| {
        match_tables_one_to_one_with_limits(
            &tables,
            CheckedMatcher::DistributionBased(&distribution),
            MatchOptions {
                instance_sample_size: None,
            },
            None,
            limits,
        )
    };
    // Distribution's five populated columns need a 5x5 distance matrix;
    // assignment still checks its separate 2x3 candidate matrix.
    let distribution_limits = WorkLimits {
        max_matrix_cells: 25,
        max_clustering_variables: 0,
        max_clustering_constraints: 0,
        ..limits
    };
    assert_eq!(run_distribution(&distribution_limits).unwrap().len(), 2);
    for (limits, operation) in [
        (
            WorkLimits {
                max_matrix_cells: 24,
                ..distribution_limits
            },
            "DistributionBased distance-matrix cells",
        ),
        (
            WorkLimits {
                max_assignment_dimension: 2,
                ..distribution_limits
            },
            "Hungarian",
        ),
    ] {
        assert!(
            run_distribution(&limits)
                .unwrap_err()
                .to_string()
                .contains(operation)
        );
    }
}

#[test]
fn checked_composition_rejects_before_encoding_and_keeps_batch_wide_limits() {
    #[derive(Default)]
    struct Provider(Mutex<usize>);
    impl EmbeddingProvider for Provider {
        fn encode(&self, values: &[String]) -> Result<Vec<Vec<f64>>, Error> {
            *self.0.lock().unwrap() += 1;
            Ok(values.iter().map(|_| vec![1.0, 0.0]).collect())
        }
    }
    let provider = Arc::new(Provider::default());
    let matcher = JaccardDistanceMatcher::default().with_embedding_provider(provider.clone());
    let tables = [table("a", 1), table("b", 1), table("c", 1)];
    let limits = WorkLimits {
        max_assignment_dimension: 2,
        max_matrix_cells: 4,
        max_jaccard_comparisons: 12,
        ..Default::default()
    };
    let run = |limits: &WorkLimits| {
        match_tables_one_to_one_with_limits(
            &tables,
            CheckedMatcher::Jaccard(&matcher),
            MatchOptions {
                instance_sample_size: None,
            },
            Some(0.5),
            limits,
        )
    };
    let raw = |limits: &WorkLimits| {
        match_tables_with_limits(
            &tables,
            CheckedMatcher::Jaccard(&matcher),
            MatchOptions {
                instance_sample_size: None,
            },
            limits,
        )
    };
    assert!(
        raw(&WorkLimits {
            max_jaccard_comparisons: 11,
            ..limits
        })
        .is_err()
    );
    assert_eq!(*provider.0.lock().unwrap(), 0);
    assert!(
        run(&WorkLimits {
            max_jaccard_comparisons: 11,
            ..limits
        })
        .is_err()
    );
    assert_eq!(*provider.0.lock().unwrap(), 0);
    assert_eq!(run(&limits).unwrap().len(), 2);
    assert_eq!(*provider.0.lock().unwrap(), 1);
    // The same limits reach selection: matching succeeds, but assignment rejects.
    assert!(
        run(&WorkLimits {
            max_assignment_dimension: 1,
            ..limits
        })
        .is_err()
    );
    assert_eq!(*provider.0.lock().unwrap(), 2);
    // Raw matching does not secretly run assignment. Each optional checked
    // selector enforces its own limits without encoding the vocabulary again.
    let raw_results = raw(&WorkLimits {
        max_assignment_dimension: 0,
        ..limits
    })
    .unwrap();
    assert_eq!(*provider.0.lock().unwrap(), 3);
    assert_eq!(raw_results.len(), 3);
    assert!(
        raw_results
            .one_to_one_rectangular_with_limits(
                None,
                &WorkLimits {
                    max_assignment_dimension: 0,
                    ..limits
                }
            )
            .is_err()
    );
    assert_eq!(*provider.0.lock().unwrap(), 3);
}
