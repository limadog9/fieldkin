//! Independent synthetic contracts for the opt-in runtime evidence policy.

use std::sync::{Arc, Mutex};

use fieldkin::{
    CandidateIssue, Config, ConfigurationError, ContextualEvidence, DataType, Decision, Evidence,
    Field, FieldDiagnostic, FieldPair, MatchConstraints, MatchEngine, MatchError, Matcher,
    NameConflictKind, NameConflictRule, NameMatcher, SampleMatcher, SampleReliability, SampleValue,
    Schema, SemanticHints, TypeMatcher, WeightedMatcher,
};

fn config() -> Config {
    Config {
        contextual_evidence: Some(ContextualEvidence::default()),
        ..Config::default()
    }
}

fn field(id: &str, name: &str) -> Field {
    Field::new(id, name, DataType::Unknown)
}

fn sampled(id: &str, name: &str, values: &[i128]) -> Field {
    field(id, name).with_samples(values.iter().copied().map(SampleValue::Integer).collect())
}

fn match_fields(source: Vec<Field>, target: Vec<Field>, config: Config) -> fieldkin::MatchReport {
    MatchEngine::new(config)
        .unwrap()
        .match_schemas(&Schema::new(source), &Schema::new(target))
        .unwrap()
}

fn unit_rules() -> Vec<NameConflictRule> {
    vec![NameConflictRule {
        kind: NameConflictKind::Unit,
        alternatives: vec![vec!["kg".into(), "kilogram".into()], vec!["lb".into()]],
    }]
}

#[test]
fn defaults_remain_unmodified_and_score_transformation_retains_original_signals() {
    assert!(Config::default().contextual_evidence.is_none());
    assert!(ContextualEvidence::default().strict_identifier_samples);
    assert!(ContextualEvidence::default().scoped_support);
    let source = vec![sampled("s", "customer_id", &[10, 20, 30])];
    let target = vec![sampled("t", "customer_id", &[10, 20, 30])];
    let baseline = match_fields(source.clone(), target.clone(), Config::default());
    let contextual = match_fields(source, target, config());
    let before = &baseline.fields[0].candidates[0];
    let after = contextual.fields[0].selected.as_ref().unwrap();
    assert_eq!(before.signals, after.signals);
    assert!(after.score >= before.score);
    assert!(after.score >= 0.95);
    assert!(after
        .issues
        .contains(&CandidateIssue::ContextualScoreAdjustment));
    assert!(!after.issues.contains(&CandidateIssue::InsufficientScore));
    assert_eq!(contextual.fields[0].decision, Decision::Proposed);
}

#[test]
fn unsupported_high_scoring_pairs_abstain_with_context_issues() {
    for one_to_one in [false, true] {
        let report = match_fields(
            vec![sampled("s", "id", &[10, 20, 30])],
            vec![sampled("t", "id", &[10, 20, 30])],
            Config {
                one_to_one,
                ..config()
            },
        );
        let result = &report.fields[0];
        assert_eq!(result.decision, Decision::InsufficientEvidence);
        assert!(result.selected.is_none());
        assert!(result.candidates[0].score >= 0.95);
        assert!(result.candidates[0]
            .issues
            .contains(&CandidateIssue::InsufficientContextSupport));
        assert!(!result.candidates[0]
            .issues
            .contains(&CandidateIssue::ForbiddenByCaller));
        assert!(result
            .diagnostics
            .contains(&FieldDiagnostic::InsufficientEvidence));
    }
}

#[test]
fn strict_identifiers_and_unavailable_observations_are_explicit_options() {
    for (strict, proposed) in [(true, false), (false, true)] {
        let report = match_fields(
            vec![Field::new("s", "customer_id", DataType::Text)],
            vec![Field::new("t", "customer_id", DataType::Text)],
            Config {
                contextual_evidence: Some(ContextualEvidence {
                    strict_identifier_samples: strict,
                    ..ContextualEvidence::default()
                }),
                ..Config::default()
            },
        );
        assert_eq!(report.fields[0].selected.is_some(), proposed);
        assert_eq!(
            report.fields[0].decision,
            if proposed {
                Decision::Proposed
            } else {
                Decision::InsufficientEvidence
            }
        );
    }
    // Empty observed identifiers still fail the independent strict identifier gate.
    let report = match_fields(
        vec![field("s", "customer_id").with_samples(Vec::new())],
        vec![field("t", "customer_key").with_samples(Vec::new())],
        config(),
    );
    assert!(report.fields[0].selected.is_none());
}

#[test]
fn scoped_word_forms_recover_temporal_and_boolean_roles() {
    for (source_name, target_name, data_type) in [
        ("expiry", "expiration_date", DataType::Date),
        ("settled_at", "settlement_timestamp", DataType::Timestamp),
        ("is_reversed", "reversal_flag", DataType::Boolean),
    ] {
        for one_to_one in [false, true] {
            let report = match_fields(
                vec![Field::new("s", source_name, data_type)],
                vec![Field::new("t", target_name, data_type)],
                Config {
                    one_to_one,
                    ..config()
                },
            );
            assert!(
                report.fields[0].selected.is_some(),
                "{source_name} -> {target_name}"
            );
        }
    }
}

#[test]
fn scoped_word_forms_preserve_context_and_meaning() {
    for (source_name, target_name, data_type) in [
        ("coupon_expiry", "account_expiration_date", DataType::Date),
        ("estimated_expiry", "actual_expiration_date", DataType::Date),
        ("is_not_reversed", "reversal_flag", DataType::Boolean),
        ("settled", "settlement", DataType::Text),
        ("reversed", "reversal", DataType::Text),
    ] {
        let report = match_fields(
            vec![Field::new("s", source_name, data_type)],
            vec![Field::new("t", target_name, data_type)],
            config(),
        );
        assert!(
            report.fields[0].selected.is_none(),
            "{source_name} -> {target_name}"
        );
    }

    let report = match_fields(
        vec![Field::new("s", "expiry", DataType::Date)],
        vec![Field::new("t", "expiration_timestamp", DataType::Timestamp)],
        config(),
    );
    assert!(report.fields[0].selected.is_none());
}

#[test]
fn scoped_sample_observation_word_forms_are_temporal_only() {
    for one_to_one in [false, true] {
        let report = match_fields(
            vec![Field::new("s", "sampled_at", DataType::Timestamp)],
            vec![Field::new("t", "observation_time", DataType::Timestamp)],
            Config {
                one_to_one,
                ..config()
            },
        );
        assert!(report.fields[0].selected.is_some());
    }

    let report = match_fields(
        vec![Field::new("s", "sample", DataType::Text)],
        vec![Field::new("t", "observation", DataType::Text)],
        config(),
    );
    assert!(report.fields[0].selected.is_none());
}
#[test]
fn scoped_email_address_form_is_text_only_and_preserves_roles() {
    for one_to_one in [false, true] {
        let report = match_fields(
            vec![Field::new("s", "email", DataType::Text)],
            vec![Field::new("t", "email_address", DataType::Text)],
            Config {
                one_to_one,
                ..config()
            },
        );
        assert!(report.fields[0].selected.is_some());
    }

    let report = match_fields(
        vec![Field::new("s", "work_email", DataType::Text)],
        vec![Field::new("t", "email_address", DataType::Text)],
        config(),
    );
    assert!(report.fields[0].selected.is_none());

    let report = match_fields(
        vec![Field::new("s", "email", DataType::Text)],
        vec![Field::new("t", "postal_address", DataType::Text)],
        config(),
    );
    assert!(report.fields[0].selected.is_none());
}
#[test]
fn scoped_geo_roles_recover_start_and_origin_coordinates() {
    for (source_name, target_name) in [
        ("start-latitude", "origin_latitude"),
        ("start_longitude", "origin_longitude"),
    ] {
        for one_to_one in [false, true] {
            let report = match_fields(
                vec![Field::new("s", source_name, DataType::Float)],
                vec![Field::new("t", target_name, DataType::Float)],
                Config {
                    one_to_one,
                    ..config()
                },
            );
            assert!(
                report.fields[0].selected.is_some(),
                "{source_name} -> {target_name}"
            );
        }
    }
}

#[test]
fn scoped_geo_roles_do_not_generalize_beyond_coordinates_or_axes() {
    for (source_name, target_name, data_type) in [
        ("start_time", "origin_time", DataType::Timestamp),
        ("start_latitude", "origin_longitude", DataType::Float),
        ("start_longitude", "origin_latitude", DataType::Float),
    ] {
        let report = match_fields(
            vec![Field::new("s", source_name, data_type)],
            vec![Field::new("t", target_name, data_type)],
            config(),
        );
        assert!(
            report.fields[0].selected.is_none(),
            "{source_name} -> {target_name}"
        );
    }
}

#[test]
fn exact_non_identifier_roles_use_adequate_samples_without_inventing_distinctiveness() {
    let report = match_fields(
        vec![sampled("s", "customer_name", &[10, 20, 30])],
        vec![
            sampled("right", "customer_name", &[10, 20, 30]),
            sampled("other", "invoice_name", &[10, 20, 30]),
        ],
        config(),
    );
    assert_eq!(
        report.fields[0].selected.as_ref().unwrap().target.0,
        "right"
    );
    let tied = match_fields(
        vec![sampled("s", "customer_name", &[10, 20, 30])],
        vec![
            sampled("a", "customer_name", &[10, 20, 30]),
            sampled("b", "customer_name", &[10, 20, 30]),
        ],
        Config {
            max_candidates: 1,
            ..config()
        },
    );
    assert_eq!(tied.fields[0].decision, Decision::Ambiguous);
    assert!(tied.fields[0].selected.is_none());
    assert_eq!(tied.fields[0].alternatives.len(), 2);
}

#[test]
fn empty_or_null_only_non_identifier_samples_preserve_informative_names() {
    for samples in [None, Some(Vec::new()), Some(vec![SampleValue::Null; 5])] {
        let mut source = Field::new("s", "customer_memo", DataType::Text);
        let mut target = Field::new("t", "customer_memo", DataType::Text);
        source.samples = samples.clone();
        target.samples = samples;
        let report = match_fields(vec![source], vec![target], config());
        assert_eq!(report.fields[0].decision, Decision::Proposed);
        assert_eq!(report.fields[0].selected.as_ref().unwrap().score, 0.9);
    }
    let report = match_fields(
        vec![
            Field::new("s", "customer_memo", DataType::Text).with_samples(
                ["a", "b", "c"]
                    .into_iter()
                    .map(|value| SampleValue::Text(value.into()))
                    .collect(),
            ),
        ],
        vec![
            Field::new("t", "customer_memo", DataType::Text).with_samples(
                ["x", "y", "z"]
                    .into_iter()
                    .map(|value| SampleValue::Text(value.into()))
                    .collect(),
            ),
        ],
        config(),
    );
    assert_eq!(report.fields[0].decision, Decision::InsufficientEvidence);
}

#[test]
fn known_integral_counts_are_distinct_from_unqualified_measurements() {
    let values = |fractional| {
        [1.0, 2.0, if fractional { 3.5 } else { 3.0 }]
            .into_iter()
            .map(SampleValue::Number)
            .collect::<Vec<_>>()
    };
    for (data_type, fractional, proposed) in [
        (DataType::Integer, false, true),
        (DataType::Integer, true, false),
        (DataType::Float, false, false),
        (DataType::Decimal, false, false),
        (DataType::Unknown, false, false),
    ] {
        let report = match_fields(
            vec![Field::new("s", "quantity", data_type).with_samples(values(fractional))],
            vec![Field::new("t", "quantity", data_type).with_samples(values(fractional))],
            config(),
        );
        assert_eq!(
            report.fields[0].selected.is_some(),
            proposed,
            "{data_type:?}, fractional={fractional}"
        );
        if !proposed {
            assert_eq!(report.fields[0].decision, Decision::InsufficientEvidence);
        }
    }
}

#[test]
fn temporal_sample_recovery_requires_shared_informative_roles() {
    let values = ["2026-01-02", "2026-01-03", "2026-01-04"]
        .into_iter()
        .map(|value| SampleValue::Text(value.into()))
        .collect::<Vec<_>>();
    let source = Field::new("s", "received_at", DataType::Timestamp).with_samples(values.clone());
    let wrong = Field::new("t", "dispatched_on", DataType::Timestamp).with_samples(values.clone());
    let report = match_fields(vec![source.clone()], vec![wrong], config());
    assert_eq!(report.fields[0].decision, Decision::InsufficientEvidence);
    let right = Field::new("t", "received_timestamp", DataType::Timestamp).with_samples(values);
    let report = match_fields(vec![source], vec![right], config());
    assert!(report.fields[0].selected.is_some());
}

#[test]
fn qualified_units_and_aliases_preserve_useful_matches_without_unit_conversion() {
    let report = match_fields(
        vec![sampled("s", "shipment_package_weight_kg", &[10, 20, 30])],
        vec![sampled(
            "t",
            "shipment_package_weight_kilogram",
            &[10, 20, 30],
        )],
        Config {
            name_conflicts: unit_rules(),
            ..config()
        },
    );
    assert!(report.fields[0].selected.is_some());
    let aliases = match_fields(
        vec![sampled("s", "TransDate", &[10, 20, 30])],
        vec![sampled("t", "transaction_date", &[10, 20, 30])],
        config(),
    );
    assert!(aliases.fields[0].selected.is_some());
    for name in ["shipment_package_weight", "shipment_package_weight_lb"] {
        let report = match_fields(
            vec![sampled("s", "shipment_package_weight_kg", &[10, 20, 30])],
            vec![sampled("t", name, &[10, 20, 30])],
            Config {
                name_conflicts: unit_rules(),
                ..config()
            },
        );
        assert!(report.fields[0].selected.is_none());
        let expected = if name.ends_with("lb") {
            Decision::BelowThreshold
        } else {
            Decision::InsufficientEvidence
        };
        assert_eq!(report.fields[0].decision, expected);
    }
}

#[test]
fn target_twins_remain_distinct_but_source_copies_ignore_ids_for_sample_contrast() {
    let duplicate_sources = vec![
        sampled("b", "customer_id", &[10, 20, 30]),
        sampled("a", "customer_id", &[10, 20, 30]),
    ];
    let target = vec![sampled("t", "customer_id", &[10, 20, 30])];
    for one_to_one in [false, true] {
        let report = match_fields(
            duplicate_sources.clone(),
            target.clone(),
            Config {
                one_to_one,
                ..config()
            },
        );
        assert_eq!(
            report
                .fields
                .iter()
                .filter(|field| field.selected.is_some())
                .count(),
            if one_to_one { 1 } else { 2 }
        );
        if one_to_one {
            assert_eq!(report.target_competition.len(), 1);
            assert_eq!(report.fields[1].decision, Decision::AssignmentConflict);
        }
    }
    let report = match_fields(
        vec![sampled("s", "customer_id", &[10, 20, 30])],
        vec![
            sampled("a", "customer_id", &[10, 20, 30]),
            sampled("b", "customer_id", &[10, 20, 30]),
        ],
        Config {
            max_candidates: 1,
            ..config()
        },
    );
    assert_eq!(report.fields[0].decision, Decision::InsufficientEvidence);
    assert!(report.fields[0].selected.is_none());
    // A different name with the same observations remains a competing source.
    let report = match_fields(
        vec![
            sampled("a", "customer_id", &[10, 20, 30]),
            sampled("b", "opaque", &[10, 20, 30]),
        ],
        target,
        config(),
    );
    assert!(report.fields.iter().all(|field| field.selected.is_none()));
    let reordered_observations = match_fields(
        vec![
            sampled("a", "customer_id", &[10, 20, 30]),
            sampled("b", "customer_id", &[30, 20, 10]),
        ],
        vec![sampled("t", "customer_id", &[10, 20, 30])],
        config(),
    );
    assert!(reordered_observations
        .fields
        .iter()
        .all(|field| field.selected.is_none()));
}

#[test]
fn contextual_policy_respects_the_configured_type_veto_and_corroboration() {
    for reject_incompatible_types in [false, true] {
        let mut source = sampled("s", "customer_id", &[10, 20, 30]);
        source.data_type = DataType::Text;
        let mut target = sampled("t", "customer_id", &[10, 20, 30]);
        target.data_type = DataType::Binary;
        let report = match_fields(
            vec![source],
            vec![target],
            Config {
                reject_incompatible_types,
                ..config()
            },
        );
        assert_eq!(
            report.fields[0].selected.is_none(),
            reject_incompatible_types
        );
        assert!(report.fields[0].candidates[0]
            .issues
            .contains(&CandidateIssue::IncompatibleTypes));
    }
    let mut sparse = vec![SampleValue::Null; 9];
    sparse.extend([10, 20, 30].into_iter().map(SampleValue::Integer));
    let report = match_fields(
        vec![field("s", "customer_id").with_samples(sparse.clone())],
        vec![field("t", "customer_id").with_samples(sparse)],
        Config {
            corroboration: Some(fieldkin::Corroboration::default()),
            ..config()
        },
    );
    assert_eq!(report.fields[0].decision, Decision::InsufficientEvidence);
    let candidate = &report.fields[0].candidates[0];
    assert!(candidate.score >= 0.95);
    assert!(candidate
        .issues
        .contains(&CandidateIssue::InsufficientSampleSupport));
    assert!(!candidate
        .issues
        .contains(&CandidateIssue::InsufficientContextSupport));
    assert!(!candidate
        .issues
        .contains(&CandidateIssue::InsufficientScore));
}

#[test]
fn contrast_includes_unreviewed_pairs_before_top_k_and_caller_exclusions() {
    let source = Schema::new(vec![sampled("s", "customer_id", &[10, 20, 30])]);
    let target = Schema::new(vec![
        sampled("a", "customer_id", &[10, 20, 30]),
        sampled("b", "opaque", &[10, 20, 30]),
    ]);
    let constraints = MatchConstraints {
        forbidden: vec![FieldPair::new("s", "b")],
        ..Default::default()
    };
    for max_candidates in [1, 5] {
        let report = MatchEngine::new(Config {
            max_candidates,
            ..config()
        })
        .unwrap()
        .match_schemas_with_constraints(&source, &target, &constraints)
        .unwrap();
        assert!(report.fields[0].selected.is_none());
        assert_eq!(report.fields[0].decision, Decision::InsufficientEvidence);
        assert!(report.fields[0].candidates[0]
            .issues
            .contains(&CandidateIssue::InsufficientContextSupport));
    }
}

#[test]
fn caller_confirmations_override_soft_context_but_preserve_hard_hints_and_types() {
    let source = Schema::new(vec![sampled("s", "id", &[10, 20, 30])]);
    let target = Schema::new(vec![sampled("t", "id", &[10, 20, 30])]);
    let constraints = MatchConstraints {
        confirmed: vec![FieldPair::new("s", "t")],
        ..Default::default()
    };
    let engine = MatchEngine::new(config()).unwrap();
    let report = engine
        .match_schemas_with_constraints(&source, &target, &constraints)
        .unwrap();
    assert_eq!(report.fields[0].decision, Decision::Confirmed);
    let selected = report.fields[0].selected.as_ref().unwrap();
    assert!(!selected.eligible);
    assert!(selected
        .issues
        .contains(&CandidateIssue::InsufficientContextSupport));
    let mut typed_source = source.clone();
    typed_source.fields[0].data_type = DataType::Boolean;
    let mut typed_target = target.clone();
    typed_target.fields[0].data_type = DataType::Binary;
    assert!(engine
        .match_schemas_with_constraints(&typed_source, &typed_target, &constraints)
        .is_err());
    let mut source = source;
    let mut target = target;
    source.fields[0].hints = SemanticHints {
        unit: Some("kg".into()),
        ..Default::default()
    };
    target.fields[0].hints = SemanticHints {
        unit: Some("lb".into()),
        ..Default::default()
    };
    assert!(engine
        .match_schemas_with_constraints(&source, &target, &constraints)
        .is_err());
}

struct Spoof(&'static str);

impl Matcher for Spoof {
    fn name(&self) -> &str {
        self.0
    }
    fn evaluate(&self, _: &Field, _: &Field) -> Result<Evidence, String> {
        Ok(Evidence {
            score: Some(1.0),
            explanation: "Custom evidence".into(),
        })
    }
}

#[test]
fn only_active_concrete_builtins_can_enable_contextual_evidence() {
    let invalid = vec![
        vec![
            WeightedMatcher::new(1.0, Spoof("name")),
            WeightedMatcher::new(1.0, SampleMatcher::default()),
        ],
        vec![
            WeightedMatcher::new(1.0, NameMatcher::default()),
            WeightedMatcher::new(1.0, Spoof("samples")),
        ],
        vec![
            WeightedMatcher::new(1.0, NameMatcher::default()),
            WeightedMatcher::new(0.0, SampleMatcher::default()),
        ],
        vec![
            WeightedMatcher::new(1.0, NameMatcher::default()),
            WeightedMatcher::new(
                1.0,
                SampleMatcher {
                    reliability: SampleReliability::Legacy,
                    ..Default::default()
                },
            ),
        ],
        vec![
            WeightedMatcher::new(1.0, NameMatcher::default()),
            WeightedMatcher::new(
                1.0,
                SampleMatcher {
                    min_non_null: 2,
                    ..Default::default()
                },
            ),
        ],
    ];
    for matchers in invalid {
        assert!(matches!(
            MatchEngine::with_matchers(config(), matchers),
            Err(MatchError::InvalidConfiguration(
                ConfigurationError::ContextualEvidence
            ))
        ));
    }
}

struct Counting(Arc<Mutex<Vec<(String, String)>>>);

impl Matcher for Counting {
    fn name(&self) -> &str {
        "counting"
    }
    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        self.0
            .lock()
            .unwrap()
            .push((source.id.0.clone(), target.id.0.clone()));
        Ok(Evidence {
            score: Some(0.0),
            explanation: "Synthetic callback".into(),
        })
    }
}

fn counted(config: Config, calls: Arc<Mutex<Vec<(String, String)>>>) -> MatchEngine {
    MatchEngine::with_matchers(
        config,
        vec![
            WeightedMatcher::new(0.65, NameMatcher::default()),
            WeightedMatcher::new(0.20, TypeMatcher),
            WeightedMatcher::new(0.15, SampleMatcher::default()),
            WeightedMatcher::new(1.0, Counting(calls)),
        ],
    )
    .unwrap()
}

#[test]
fn callbacks_run_once_in_stable_order_and_limits_cover_actual_work() {
    let source = Schema::new(vec![
        sampled("b", "customer_id", &[10, 20, 30]),
        sampled("a", "customer_id", &[10, 20, 30]),
    ]);
    let target = Schema::new(vec![
        sampled("y", "customer_id", &[10, 20, 30]),
        sampled("x", "other_id", &[40, 50, 60]),
    ]);
    let calls = Arc::new(Mutex::new(Vec::new()));
    let mut exact = config();
    exact.limits.max_signal_evaluations = 16;
    counted(exact.clone(), calls.clone())
        .match_schemas(&source, &target)
        .unwrap();
    assert_eq!(
        *calls.lock().unwrap(),
        vec![
            ("a".into(), "x".into()),
            ("a".into(), "y".into()),
            ("b".into(), "x".into()),
            ("b".into(), "y".into())
        ]
    );
    calls.lock().unwrap().clear();
    exact.limits.max_signal_evaluations = 15;
    assert!(counted(exact, calls.clone())
        .match_schemas(&source, &target)
        .is_err());
    assert!(calls.lock().unwrap().is_empty());
    let baseline = match_fields(
        source.fields.clone(),
        target.fields.clone(),
        Config::default(),
    );
    let signal_bytes = baseline
        .fields
        .iter()
        .flat_map(|field| &field.candidates)
        .flat_map(|candidate| &candidate.signals)
        .map(|signal| signal.evidence.explanation.len())
        .sum();
    let mut limited = config();
    limited.limits.max_explanation_bytes = signal_bytes;
    assert!(matches!(
        MatchEngine::new(limited)
            .unwrap()
            .match_schemas(&source, &target),
        Err(MatchError::BudgetExceeded(
            fieldkin::BudgetKind::ExplanationBytes
        ))
    ));
}

#[test]
fn context_explanations_do_not_disclose_sample_values() {
    let secret = "synthetic-private-context-value";
    let samples = (0..3)
        .map(|index| SampleValue::Text(format!("{secret}-{index}")))
        .collect::<Vec<_>>();
    let report = match_fields(
        vec![field("s", "customer_identifier").with_samples(samples.clone())],
        vec![field("t", "customer_key").with_samples(samples)],
        config(),
    );
    assert!(report.fields[0].selected.is_some());
    assert!(!format!("{report:?}").contains(secret));
}

#[cfg(feature = "json")]
#[test]
fn json_reports_real_contextual_support_and_transformation_reasons() {
    use fieldkin::json::{report_to_json, ReportJsonOptions};
    use serde_json::{json, Value};
    let report = match_fields(
        vec![sampled("s", "id", &[10, 20, 30])],
        vec![sampled("t", "id", &[10, 20, 30])],
        config(),
    );
    let wire: Value =
        serde_json::from_slice(&report_to_json(&report, &ReportJsonOptions::default()).unwrap())
            .unwrap();
    assert_eq!(
        wire["report"]["fields"][0]["decision"],
        "insufficient_evidence"
    );
    let issues = wire["report"]["fields"][0]["candidates"][0]["issues"]
        .as_array()
        .unwrap();
    assert!(issues.contains(&json!({"code": "insufficient_context_support"})));
    assert!(issues.contains(&json!({"code": "contextual_score_adjustment"})));
    assert!(!issues.contains(&json!({"code": "forbidden_by_caller"})));
}
