//! Synthetic precision regressions; no evaluation labels or held-out examples.

use fieldkin::{
    CandidateIssue, Config, ConfigurationError, Corroboration, DataType, Decision, Field,
    FieldDiagnostic, FieldPair, MatchConstraints, MatchEngine, MatchError, MatchReport,
    NameConflictKind, NameConflictRule, NameMatcher, SampleMatcher, SampleValue, Schema,
    SemanticHints, TypeMatcher, WeightedMatcher,
};

fn rule(kind: NameConflictKind, alternatives: &[&[&str]]) -> NameConflictRule {
    NameConflictRule {
        kind,
        alternatives: alternatives
            .iter()
            .map(|synonyms| synonyms.iter().map(|token| (*token).to_owned()).collect())
            .collect(),
    }
}

fn qualifiers() -> NameConflictRule {
    rule(NameConflictKind::Qualifier, &[&["gross"], &["net"]])
}

fn units() -> NameConflictRule {
    rule(
        NameConflictKind::Unit,
        &[&["kg", "kilogram"], &["lb", "pound"]],
    )
}

fn configured() -> Config {
    Config {
        name_conflicts: vec![qualifiers(), units()],
        corroboration: Some(Corroboration::default()),
        ..Config::default()
    }
}

fn field(id: &str, name: &str) -> Field {
    Field::new(id, name, DataType::Decimal)
}

fn sampled(id: &str, name: &str) -> Field {
    field(id, name).with_samples([10, 20, 30].into_iter().map(SampleValue::Integer).collect())
}

fn pair(config: Config, source: Field, target: Field) -> MatchReport {
    MatchEngine::new(config)
        .unwrap()
        .match_schemas(&Schema::new(vec![source]), &Schema::new(vec![target]))
        .unwrap()
}

fn alias_engine(config: Config) -> MatchEngine {
    MatchEngine::with_matchers(
        config,
        vec![
            WeightedMatcher::new(
                0.65,
                NameMatcher::default().with_alias("net", "gross").unwrap(),
            ),
            WeightedMatcher::new(0.20, TypeMatcher),
            WeightedMatcher::new(0.15, SampleMatcher::default()),
        ],
    )
    .unwrap()
}

#[test]
fn opt_in_conflicts_reject_high_scoring_wrong_meanings_without_retuning_evidence() {
    assert!(Config::default().name_conflicts.is_empty());
    assert!(Config::default().corroboration.is_none());
    for (source_name, target_name, kind) in [
        (
            "customer_invoice_gross_amount",
            "customer_invoice_net_amount",
            NameConflictKind::Qualifier,
        ),
        (
            "shipment_package_weight_kg",
            "shipment_package_weight_lb",
            NameConflictKind::Unit,
        ),
    ] {
        let source = sampled("s", source_name);
        let target = sampled("t", target_name);
        let baseline = pair(Config::default(), source.clone(), target.clone());
        let guarded = pair(configured(), source, target);
        let original = baseline.fields[0].selected.as_ref().unwrap();
        let rejected = &guarded.fields[0].candidates[0];
        assert!(original.score >= Config::default().min_score);
        assert_eq!(rejected.score, original.score);
        assert_eq!(rejected.signals, original.signals);
        assert!(!rejected.eligible);
        assert!(rejected
            .issues
            .contains(&CandidateIssue::NameConflict(kind)));
        assert_eq!(guarded.fields[0].decision, Decision::BelowThreshold);
        assert!(guarded.fields[0].selected.is_none());
        assert_eq!(guarded.unmatched_sources[0].0, "s");
        assert_eq!(guarded.unmatched_targets[0].0, "t");
    }
}

#[test]
fn useful_name_aliases_unit_synonyms_and_one_sided_qualifiers_remain_matches() {
    for (source_name, target_name) in [
        ("TransDate", "transaction_date"),
        (
            "shipment_package_weight_kg",
            "shipment_package_weight_kilogram",
        ),
        ("customer_invoice_amount", "customer_invoice_net_amount"),
        (
            "customer_invoice_gross_amount",
            "customer_invoice_gross_amount",
        ),
    ] {
        let report = pair(
            configured(),
            sampled("s", source_name),
            sampled("t", target_name),
        );
        let selected = report.fields[0].selected.as_ref().unwrap();
        assert_eq!(report.fields[0].decision, Decision::Proposed);
        assert!(!selected
            .issues
            .iter()
            .any(|issue| matches!(issue, CandidateIssue::NameConflict(_))));
    }
}

#[test]
fn conflict_checks_use_normalized_whole_tokens_and_intersecting_meanings() {
    // Lower the total-score floor solely to isolate the conflict rule.
    let config = Config {
        min_score: 0.0,
        corroboration: None,
        ..configured()
    };
    for (source_name, target_name, conflict) in [
        ("invoice_internet_amount", "invoice_gross_amount", false),
        ("invoice_gross_amount", "invoice_network_amount", false),
        ("invoice_gross_net_amount", "invoice_gross_amount", false),
        ("invoiceGrossAmount", "invoiceNetAmount", true),
        ("invoice_Ｇｒｏｓｓ_amount", "invoice_net_amount", true),
    ] {
        let report = pair(
            config.clone(),
            sampled("s", source_name),
            sampled("t", target_name),
        );
        assert_eq!(
            report.fields[0].candidates[0]
                .issues
                .contains(&CandidateIssue::NameConflict(NameConflictKind::Qualifier)),
            conflict,
            "{source_name} versus {target_name}",
        );
        assert_eq!(report.fields[0].selected.is_none(), conflict);
    }
    let unicode = Config {
        name_conflicts: vec![rule(NameConflictKind::Qualifier, &[&["brut"], &["nét"]])],
        ..config
    };
    let report = pair(
        unicode,
        sampled("s", "invoice_brut_amount"),
        sampled("t", "invoice_ne\u{301}t_amount"),
    );
    assert!(report.fields[0].candidates[0]
        .issues
        .contains(&CandidateIssue::NameConflict(NameConflictKind::Qualifier)));
}

#[test]
fn longest_normalized_unit_phrase_prevents_pa_from_hiding_kpa_conflicts() {
    let config = Config {
        name_conflicts: vec![rule(
            NameConflictKind::Unit,
            &[&["kpa", "k pa", "kilopascal"], &["pa", "pascal"]],
        )],
        ..configured()
    };
    let report = pair(
        config.clone(),
        sampled("s", "sensor_chamber_air_pressure_kPa"),
        sampled("t", "sensor_chamber_air_pressure_Pa"),
    );
    assert!(report.fields[0].candidates[0].score >= config.min_score);
    assert!(report.fields[0].candidates[0]
        .issues
        .contains(&CandidateIssue::NameConflict(NameConflictKind::Unit)));
    assert!(report.fields[0].selected.is_none());
    let synonym = pair(
        config,
        sampled("s", "sensor_chamber_air_pressure_kPa"),
        sampled("t", "sensor_chamber_air_pressure_kilopascal"),
    );
    assert!(synonym.fields[0].selected.is_some());
}

#[test]
fn aliases_cannot_conceal_conflicts_and_filtering_preserves_the_correct_candidate() {
    for one_to_one in [false, true] {
        let source = Schema::new(vec![sampled("s", "customer_invoice_gross_amount")]);
        let target = Schema::new(vec![
            sampled("wrong", "customer_invoice_net_amount"),
            sampled("right", "customer_invoice_gross_amount"),
        ]);
        let baseline = alias_engine(Config {
            one_to_one,
            ..Config::default()
        })
        .match_schemas(&source, &target)
        .unwrap();
        assert_eq!(baseline.fields[0].decision, Decision::Ambiguous);
        let report = alias_engine(Config {
            one_to_one,
            max_candidates: 1,
            ..configured()
        })
        .match_schemas(&source, &target)
        .unwrap();
        assert_eq!(report.fields[0].decision, Decision::Proposed);
        assert_eq!(
            report.fields[0].selected.as_ref().unwrap().target.0,
            "right"
        );
        assert_eq!(report.fields[0].alternatives.len(), 1);
        assert_eq!(report.fields[0].alternatives[0].0, "right");
    }
}

#[test]
fn equally_supported_valid_targets_still_abstain_as_ambiguous() {
    for one_to_one in [false, true] {
        let report = MatchEngine::new(Config {
            one_to_one,
            max_candidates: 1,
            ..configured()
        })
        .unwrap()
        .match_schemas(
            &Schema::new(vec![sampled("s", "customer_invoice_gross_amount")]),
            &Schema::new(vec![
                sampled("a", "customer_invoice_gross_amount"),
                sampled("b", "customer_invoice_gross_amount"),
            ]),
        )
        .unwrap();
        assert_eq!(report.fields[0].decision, Decision::Ambiguous);
        assert!(report.fields[0].selected.is_none());
        assert_eq!(report.fields[0].alternatives.len(), 2);
        assert_eq!(report.fields[0].candidates.len(), 1);
    }
}

#[test]
fn insufficient_evidence_is_explicit_when_other_requirements_pass() {
    for one_to_one in [false, true] {
        let no_samples = pair(
            Config {
                one_to_one,
                ..configured()
            },
            field("s", "amount"),
            field("t", "amount"),
        );
        assert!(no_samples.fields[0].candidates[0].score >= Config::default().min_score);
        assert_eq!(
            no_samples.fields[0].decision,
            Decision::InsufficientEvidence
        );
        assert!(no_samples.fields[0]
            .diagnostics
            .contains(&FieldDiagnostic::InsufficientEvidence));
        assert!(no_samples.fields[0].selected.is_none());
        let no_names = pair(
            Config {
                min_score: 0.3,
                one_to_one,
                ..configured()
            },
            sampled("s", ""),
            sampled("t", ""),
        );
        assert_eq!(no_names.fields[0].decision, Decision::InsufficientEvidence);
        assert!(no_names.fields[0].candidates[0]
            .issues
            .contains(&CandidateIssue::InsufficientNameSupport));
    }
}

#[test]
fn insufficient_evidence_does_not_replace_score_or_semantic_vetoes() {
    let hints = |unit: &str| SemanticHints {
        unit: Some(unit.to_owned()),
        ..Default::default()
    };
    for one_to_one in [false, true] {
        let config = Config {
            one_to_one,
            ..configured()
        };
        let too_low = pair(
            Config {
                min_score: 0.9,
                ..config.clone()
            },
            field("s", "amount"),
            field("t", "amount"),
        );
        assert_eq!(too_low.fields[0].decision, Decision::BelowThreshold);
        let incompatible = pair(
            Config {
                min_score: 0.5,
                ..config.clone()
            },
            field("s", "amount"),
            Field::new("t", "amount", DataType::Binary),
        );
        assert_eq!(incompatible.fields[0].decision, Decision::BelowThreshold);
        let semantic = pair(
            config.clone(),
            field("s", "amount").with_hints(hints("kg")),
            field("t", "amount").with_hints(hints("lb")),
        );
        assert_eq!(semantic.fields[0].decision, Decision::BelowThreshold);
        let no_targets = MatchEngine::new(config)
            .unwrap()
            .match_schemas(&Schema::new(vec![field("s", "amount")]), &Schema::default())
            .unwrap();
        assert_eq!(no_targets.fields[0].decision, Decision::BelowThreshold);
        for report in [too_low, incompatible, semantic, no_targets] {
            assert!(!report.fields[0]
                .diagnostics
                .contains(&FieldDiagnostic::InsufficientEvidence));
        }
    }
}

#[test]
fn insufficient_evidence_uses_candidates_hidden_by_display_truncation() {
    for one_to_one in [false, true] {
        let source = Schema::new(vec![sampled("s", "customer_invoice_gross_amount")]);
        let target = Schema::new(vec![
            sampled("high_conflict", "customer_invoice_net_amount"),
            field("hidden_unsupported", "customer_invoice_gross_amount"),
        ]);
        let full = alias_engine(Config {
            one_to_one,
            ..configured()
        })
        .match_schemas(&source, &target)
        .unwrap();
        let clipped = alias_engine(Config {
            one_to_one,
            max_candidates: 1,
            ..configured()
        })
        .match_schemas(&source, &target)
        .unwrap();
        assert_eq!(clipped.fields[0].candidates.len(), 1);
        assert_eq!(clipped.fields[0].candidates[0].target.0, "high_conflict");
        assert_eq!(clipped.fields[0].decision, Decision::InsufficientEvidence);
        assert_eq!(clipped.fields[0].decision, full.fields[0].decision);
        assert_eq!(clipped.fields[0].diagnostics, full.fields[0].diagnostics);
    }
}

#[test]
fn conflicting_pairs_never_enter_one_to_one_competition() {
    let source = Schema::new(vec![
        sampled("gross", "customer_invoice_gross_amount"),
        sampled("net", "customer_invoice_net_amount"),
    ]);
    let target = Schema::new(vec![
        sampled("gross_target", "customer_invoice_gross_amount"),
        sampled("net_target", "customer_invoice_net_amount"),
    ]);
    let report = alias_engine(Config {
        one_to_one: true,
        ..configured()
    })
    .match_schemas(&source, &target)
    .unwrap();
    assert_eq!(
        report.fields[0].selected.as_ref().unwrap().target.0,
        "gross_target"
    );
    assert_eq!(
        report.fields[1].selected.as_ref().unwrap().target.0,
        "net_target"
    );
    assert!(report.target_competition.is_empty());
    assert!(report.unmatched_sources.is_empty());
    assert!(report.unmatched_targets.is_empty());
}

#[test]
fn caller_confirmations_override_automatic_gates_and_exclusions_remain_decisive() {
    let source = Schema::new(vec![field("s", "customer_invoice_gross_amount")]);
    let target = Schema::new(vec![field("t", "customer_invoice_net_amount")]);
    for one_to_one in [false, true] {
        let engine = alias_engine(Config {
            one_to_one,
            ..configured()
        });
        let confirmed = engine
            .match_schemas_with_constraints(
                &source,
                &target,
                &MatchConstraints {
                    confirmed: vec![FieldPair::new("s", "t")],
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(confirmed.fields[0].decision, Decision::Confirmed);
        let selected = confirmed.fields[0].selected.as_ref().unwrap();
        assert!(!selected.eligible);
        assert!(selected
            .issues
            .contains(&CandidateIssue::NameConflict(NameConflictKind::Qualifier)));
        assert!(selected
            .issues
            .contains(&CandidateIssue::InsufficientSampleSupport));
        let ordinary = Schema::new(vec![field("t", "customer_invoice_gross_amount")]);
        for (constraints, expected) in [
            (
                MatchConstraints {
                    forbidden: vec![FieldPair::new("s", "t")],
                    ..Default::default()
                },
                Decision::BelowThreshold,
            ),
            (
                MatchConstraints {
                    unmatched_sources: vec!["s".into()],
                    ..Default::default()
                },
                Decision::ExcludedByCaller,
            ),
        ] {
            let excluded = engine
                .match_schemas_with_constraints(&source, &ordinary, &constraints)
                .unwrap();
            assert_eq!(excluded.fields[0].decision, expected);
            assert!(!excluded.fields[0]
                .diagnostics
                .contains(&FieldDiagnostic::InsufficientEvidence));
        }
    }
}

#[test]
fn conflict_configuration_rejects_unbounded_or_noncanonical_vocabulary() {
    let invalid = [
        Vec::new(),
        vec![vec!["gross".to_owned()]],
        vec![Vec::new(), vec!["net".to_owned()]],
        vec![vec!["gross".to_owned(); 17], vec!["net".to_owned()]],
        (0..17).map(|i| vec![format!("token {i}")]).collect(),
        vec![vec!["gross".to_owned()], vec!["gross".to_owned()]],
        vec![
            vec!["net".to_owned(), "net".to_owned()],
            vec!["gross".to_owned()],
        ],
        vec![vec![String::new()], vec!["net".to_owned()]],
        vec![vec!["Gross".to_owned()], vec!["net".to_owned()]],
        vec![vec!["kPa".to_owned()], vec!["pa".to_owned()]],
        vec![vec![" gross".to_owned()], vec!["net".to_owned()]],
        vec![vec!["k  pa".to_owned()], vec!["pa".to_owned()]],
        vec![vec!["k_pa".to_owned()], vec!["pa".to_owned()]],
        vec![
            vec!["one two three four five".to_owned()],
            vec!["net".to_owned()],
        ],
        vec![vec!["x".repeat(65)], vec!["net".to_owned()]],
    ];
    for alternatives in invalid {
        assert!(matches!(
            MatchEngine::new(Config {
                name_conflicts: vec![NameConflictRule {
                    kind: NameConflictKind::Qualifier,
                    alternatives,
                }],
                ..Config::default()
            }),
            Err(MatchError::InvalidConfiguration(
                ConfigurationError::NameConflicts
            )),
        ));
    }
    assert!(matches!(
        MatchEngine::new(Config {
            name_conflicts: vec![qualifiers(); 33],
            ..Config::default()
        }),
        Err(MatchError::InvalidConfiguration(
            ConfigurationError::NameConflicts
        )),
    ));
    assert!(MatchEngine::new(Config {
        name_conflicts: vec![qualifiers(); 32],
        ..Config::default()
    })
    .is_ok());
    assert!(MatchEngine::new(Config {
        name_conflicts: vec![rule(
            NameConflictKind::Unit,
            &[&["one two three four"], &["x"]],
        )],
        ..Config::default()
    })
    .is_ok());
}

#[cfg(feature = "json")]
#[test]
fn report_json_preserves_conflict_kind_and_insufficient_evidence_without_labels() {
    use fieldkin::json::{report_to_json, ReportJsonOptions};
    use serde_json::{json, Value};

    for (rule, source_name, target_name, kind) in [
        (
            qualifiers(),
            "customer_invoice_gross_amount",
            "customer_invoice_net_amount",
            "qualifier",
        ),
        (
            units(),
            "shipment_package_weight_kg",
            "shipment_package_weight_lb",
            "unit",
        ),
    ] {
        let report = pair(
            Config {
                name_conflicts: vec![rule],
                ..configured()
            },
            sampled("s", source_name),
            sampled("t", target_name),
        );
        let bytes = report_to_json(&report, &ReportJsonOptions::default()).unwrap();
        let text = String::from_utf8(bytes.clone()).unwrap();
        assert!(!text.contains(source_name));
        assert!(!text.contains(target_name));
        let wire: Value = serde_json::from_slice(&bytes).unwrap();
        assert!(wire["report"]["fields"][0]["candidates"][0]["issues"]
            .as_array()
            .unwrap()
            .contains(&json!({"code": "name_conflict", "kind": kind})));
    }
    let report = pair(configured(), field("s", "amount"), field("t", "amount"));
    let wire: Value =
        serde_json::from_slice(&report_to_json(&report, &ReportJsonOptions::default()).unwrap())
            .unwrap();
    assert_eq!(
        wire["report"]["fields"][0]["decision"],
        "insufficient_evidence"
    );
    assert!(wire["report"]["fields"][0]["diagnostics"]
        .as_array()
        .unwrap()
        .contains(&json!({"code": "insufficient_evidence"})));
    assert!(wire["report"]["fields"][0]["selected"].is_null());
}
