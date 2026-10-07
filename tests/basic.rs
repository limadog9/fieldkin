use fieldkin::{Config, DataType, Decision, Field, Schema, match_schemas};

fn field(name: &str, data_type: DataType, samples: &[&str]) -> Field {
    Field {
        name: name.into(),
        data_type,
        samples: samples.iter().map(|value| (*value).into()).collect(),
    }
}

#[test]
fn exact_name_and_type_match() {
    let source = Schema {
        fields: vec![Field {
            name: "email".into(),
            data_type: DataType::Text,
            samples: vec![],
        }],
    };

    let target = Schema {
        fields: vec![Field {
            name: "email".into(),
            data_type: DataType::Text,
            samples: vec![],
        }],
    };

    let report = match_schemas(&source, &target, Config::default());

    assert!(matches!(
        &report.fields[0].decision,
        Decision::Match { target, .. } if target == "email"
    ));
}

#[test]
fn empty_schemas_have_empty_or_unmatched_results() {
    let empty = Schema { fields: vec![] };
    let populated = Schema {
        fields: vec![field("id", DataType::Integer, &[])],
    };

    assert!(
        match_schemas(&empty, &populated, Config::default())
            .fields
            .is_empty()
    );

    let report = match_schemas(&populated, &empty, Config::default());
    assert_eq!(report.fields.len(), 1);
    assert!(report.fields[0].candidates.is_empty());
    assert_eq!(
        report.fields[0].decision,
        Decision::NoMatch { best_score: None }
    );
}

#[test]
fn empty_names_follow_normalized_name_matching() {
    for (source_name, target_name) in [("", ""), ("", "email"), ("email", "")] {
        let source = Schema {
            fields: vec![field(source_name, DataType::Text, &[])],
        };
        let target = Schema {
            fields: vec![field(target_name, DataType::Text, &[])],
        };
        let report = match_schemas(&source, &target, Config::default());

        if source_name == target_name {
            assert_eq!(
                report.fields[0].decision,
                Decision::Match {
                    target: String::new(),
                    score: 1.0,
                }
            );
        } else {
            assert_eq!(report.fields[0].candidates[0].name_score, 0.0);
            assert!(matches!(
                report.fields[0].decision,
                Decision::NoMatch {
                    best_score: Some(_)
                }
            ));
        }
    }
}

#[test]
fn duplicate_names_preserve_source_entries_and_target_ambiguity() {
    let source = Schema {
        fields: vec![
            field("id", DataType::Integer, &[]),
            field("id", DataType::Integer, &[]),
        ],
    };
    let target = source.clone();
    let report = match_schemas(&source, &target, Config::default());

    assert_eq!(report.fields.len(), 2);
    for result in report.fields {
        assert_eq!(result.source, "id");
        assert_eq!(result.candidates.len(), 2);
        assert_eq!(
            result.decision,
            Decision::Ambiguous {
                targets: vec!["id".into(), "id".into()],
                best_score: 1.0,
            }
        );
    }
}

#[test]
fn incompatible_type_can_leave_an_exact_name_below_threshold() {
    let source = Schema {
        fields: vec![field("email", DataType::Text, &[])],
    };
    let target = Schema {
        fields: vec![field("email", DataType::Boolean, &[])],
    };
    let report = match_schemas(&source, &target, Config::default());
    let candidate = &report.fields[0].candidates[0];

    assert_eq!(candidate.name_score, 1.0);
    assert_eq!(candidate.type_score, 0.0);
    assert_eq!(candidate.sample_score, None);
    assert!(candidate.score < Config::default().min_score);
    assert_eq!(
        report.fields[0].decision,
        Decision::NoMatch {
            best_score: Some(candidate.score),
        }
    );
}

#[test]
fn candidate_limit_does_not_hide_ambiguity_from_the_decision() {
    let source = Schema {
        fields: vec![field("status", DataType::Text, &[])],
    };
    let target = Schema {
        fields: vec![
            field("payment_status", DataType::Text, &[]),
            field("order_status", DataType::Text, &[]),
        ],
    };

    for max_candidates in [0, 1] {
        let report = match_schemas(
            &source,
            &target,
            Config {
                max_candidates,
                ..Config::default()
            },
        );
        assert_eq!(report.fields[0].candidates.len(), max_candidates);
        assert_eq!(
            report.fields[0].decision,
            Decision::Ambiguous {
                targets: vec!["order_status".into(), "payment_status".into()],
                best_score: 1.0,
            }
        );
    }
}

#[test]
fn tied_strong_sample_evidence_is_weakened() {
    let source = Schema {
        fields: vec![field("status", DataType::Text, &["A", "B"])],
    };
    let mut target = Schema {
        fields: vec![field("payment_status", DataType::Text, &["a", "b"])],
    };
    let single = match_schemas(&source, &target, Config::default());
    assert_eq!(single.fields[0].candidates[0].sample_score, Some(1.0));

    target
        .fields
        .push(field("order_status", DataType::Text, &["a", "b"]));
    let tied = match_schemas(&source, &target, Config::default());
    assert!(
        tied.fields[0]
            .candidates
            .iter()
            .all(|candidate| candidate.sample_score == Some(0.5))
    );
}

#[test]
fn abbreviation_fallback_runs_only_after_normal_no_match() {
    let source = Schema {
        fields: vec![field("cid", DataType::Integer, &[])],
    };
    let mut target = Schema {
        fields: vec![field("customer_id", DataType::Integer, &[])],
    };
    let normal_config = Config {
        min_score: 0.0,
        ..Config::default()
    };
    let normal = match_schemas(&source, &target, normal_config);
    let normal_candidate = &normal.fields[0].candidates[0];
    assert!(matches!(normal.fields[0].decision, Decision::Match { .. }));

    let fallback = match_schemas(
        &source,
        &target,
        Config {
            min_score: normal_candidate.score + 1e-6,
            ..Config::default()
        },
    );
    assert!(matches!(
        fallback.fields[0].decision,
        Decision::Match { .. }
    ));
    assert!(fallback.fields[0].candidates[0].name_score > normal_candidate.name_score);

    target
        .fields
        .push(field("customer-id", DataType::Integer, &[]));
    let ambiguous = match_schemas(&source, &target, normal_config);
    assert!(matches!(
        ambiguous.fields[0].decision,
        Decision::Ambiguous { .. }
    ));
    assert!(
        ambiguous.fields[0]
            .candidates
            .iter()
            .all(|candidate| candidate.name_score == normal_candidate.name_score)
    );
}

#[test]
fn invalid_configuration_values_are_rejected() {
    let schema = Schema {
        fields: vec![field("id", DataType::Integer, &[])],
    };
    for min_score in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            std::panic::catch_unwind(|| {
                match_schemas(
                    &schema,
                    &schema,
                    Config {
                        min_score,
                        ..Config::default()
                    },
                )
            })
            .is_err()
        );
    }
    for ambiguity_margin in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.01] {
        assert!(
            std::panic::catch_unwind(|| {
                match_schemas(
                    &schema,
                    &schema,
                    Config {
                        ambiguity_margin,
                        ..Config::default()
                    },
                )
            })
            .is_err()
        );
    }
}

#[test]
fn finite_thresholds_outside_unit_interval_remain_usable() {
    let schema = Schema {
        fields: vec![field("id", DataType::Integer, &[])],
    };
    for min_score in [-1.0, 2.0] {
        let report = match_schemas(
            &schema,
            &schema,
            Config {
                min_score,
                ..Config::default()
            },
        );
        if min_score < 0.0 {
            assert!(matches!(report.fields[0].decision, Decision::Match { .. }));
        } else {
            assert_eq!(
                report.fields[0].decision,
                Decision::NoMatch {
                    best_score: Some(1.0),
                }
            );
        }
    }
}
