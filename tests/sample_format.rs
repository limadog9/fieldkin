//! Runtime contracts for the conservative contextual sample-format fallback.

use fieldkin::{
    CandidateIssue, Config, ContextualEvidence, Corroboration, DataType, Decision, Field,
    FieldPair, MatchConstraints, MatchEngine, NameConflictKind, NameConflictRule, NameMatcher,
    SampleMatcher, SampleValue, Schema, SemanticHints, TypeMatcher, WeightedMatcher,
};

fn config(one_to_one: bool) -> Config {
    Config {
        one_to_one,
        contextual_evidence: Some(ContextualEvidence::default()),
        ..Config::default()
    }
}

fn text(id: &str, name: &str, samples: &[&str]) -> Field {
    Field::new(id, name, DataType::Text).with_samples(
        samples
            .iter()
            .map(|value| SampleValue::Text((*value).to_owned()))
            .collect(),
    )
}

fn pair() -> (Schema, Schema) {
    (
        Schema::new(vec![text(
            "s",
            "tracking_number",
            &["TRK-100", "TRK-101", "TRK-102"],
        )]),
        Schema::new(vec![text(
            "t",
            "tracking_id",
            &["TRK-200", "TRK-201", "TRK-202"],
        )]),
    )
}

#[test]
fn disjoint_formats_use_the_real_selector_in_both_modes_without_changing_defaults() {
    let (source, target) = pair();
    let ordinary = MatchEngine::new(Config::default())
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap();
    assert!(ordinary.fields[0].selected.is_none());
    for one_to_one in [false, true] {
        let report = MatchEngine::new(config(one_to_one))
            .unwrap()
            .match_schemas(&source, &target)
            .unwrap();
        let chosen = report.fields[0]
            .selected
            .as_ref()
            .expect("format-backed proposal");
        assert_eq!(chosen.target.0, "t");
        assert_eq!(report.fields[0].decision, Decision::Proposed);
        assert_eq!(chosen.signals, ordinary.fields[0].candidates[0].signals);
        assert!(chosen
            .warnings
            .iter()
            .any(|w| w.starts_with("Sample-format fallback:")));
        assert!(!chosen
            .issues
            .contains(&CandidateIssue::InsufficientContextSupport));
    }
}

#[test]
fn repeats_two_values_sparse_or_mixed_observations_do_not_qualify() {
    let (_, target) = pair();
    for samples in [vec!["TRK-100"; 5], vec!["TRK-100", "TRK-101", "TRK-100"]] {
        let source = Schema::new(vec![text("s", "tracking_number", &samples)]);
        let report = MatchEngine::new(config(false))
            .unwrap()
            .match_schemas(&source, &target)
            .unwrap();
        assert!(report.fields[0].selected.is_none());
    }
    let (mut source, target) = pair();
    source.fields[0]
        .samples
        .as_mut()
        .unwrap()
        .extend(vec![SampleValue::Null; 9]);
    assert!(MatchEngine::new(config(false))
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap()
        .fields[0]
        .selected
        .is_some());
    source.fields[0]
        .samples
        .as_mut()
        .unwrap()
        .push(SampleValue::Null);
    assert!(MatchEngine::new(config(false))
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap()
        .fields[0]
        .selected
        .is_none());
    let (mut source, target) = pair();
    source.fields[0]
        .samples
        .as_mut()
        .unwrap()
        .push(SampleValue::Boolean(true));
    assert!(MatchEngine::new(config(false))
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap()
        .fields[0]
        .selected
        .is_none());
}

#[test]
fn target_and_source_twins_cannot_create_confidence_or_double_assignments() {
    for one_to_one in [false, true] {
        let (source, mut target) = pair();
        let mut twin = target.fields[0].clone();
        twin.id = "u".into();
        target.fields.push(twin);
        assert!(MatchEngine::new(config(one_to_one))
            .unwrap()
            .match_schemas(&source, &target)
            .unwrap()
            .fields
            .iter()
            .all(|field| field.selected.is_none()));
        let (mut source, target) = pair();
        let mut twin = source.fields[0].clone();
        twin.id = "r".into();
        source.fields.push(twin);
        assert!(MatchEngine::new(config(one_to_one))
            .unwrap()
            .match_schemas(&source, &target)
            .unwrap()
            .fields
            .iter()
            .all(|field| field.selected.is_none()));
    }
}

#[test]
fn target_with_an_original_eligible_edge_is_not_taken_by_fallback() {
    for one_to_one in [false, true] {
        let (mut source, target) = pair();
        // Unknown is excluded from format comparison, but the original exact
        // sampled-value policy correctly supports this incumbent.
        let mut incumbent = target.fields[0].clone();
        incumbent.id = "a".into();
        incumbent.data_type = DataType::Unknown;
        source.fields.push(incumbent);
        let report = MatchEngine::new(config(one_to_one))
            .unwrap()
            .match_schemas(&source, &target)
            .unwrap();
        assert_eq!(report.fields[0].source.0, "a");
        assert_eq!(report.fields[0].selected.as_ref().unwrap().target.0, "t");
        assert!(report.fields[1].selected.is_none());
    }
}

#[test]
fn caller_exclusions_and_confirmations_still_control_final_selection() {
    let (source, target) = pair();
    for one_to_one in [false, true] {
        let engine = MatchEngine::new(config(one_to_one)).unwrap();
        for constraints in [
            MatchConstraints {
                forbidden: vec![FieldPair::new("s", "t")],
                ..Default::default()
            },
            MatchConstraints {
                unmatched_sources: vec!["s".into()],
                ..Default::default()
            },
        ] {
            let report = engine
                .match_schemas_with_constraints(&source, &target, &constraints)
                .unwrap();
            assert!(report.fields[0].selected.is_none());
        }
    }
    let mut source = source;
    source
        .fields
        .push(Field::new("z", "opaque", DataType::Text));
    let constraints = MatchConstraints {
        confirmed: vec![FieldPair::new("z", "t")],
        ..Default::default()
    };
    let report = MatchEngine::new(config(true))
        .unwrap()
        .match_schemas_with_constraints(&source, &target, &constraints)
        .unwrap();
    assert!(report.fields[0].selected.is_none());
    assert_eq!(report.fields[1].decision, Decision::Confirmed);
    assert_eq!(report.fields[1].selected.as_ref().unwrap().target.0, "t");
}

#[test]
fn hidden_competitors_still_block_after_review_exclusion_and_top_k_truncation() {
    let (source, mut target) = pair();
    let mut twin = target.fields[0].clone();
    twin.id = "u".into();
    target.fields.push(twin);
    let constraints = MatchConstraints {
        forbidden: vec![FieldPair::new("s", "u")],
        ..Default::default()
    };
    for max_candidates in [1, 5] {
        let report = MatchEngine::new(Config {
            max_candidates,
            ..config(false)
        })
        .unwrap()
        .match_schemas_with_constraints(&source, &target, &constraints)
        .unwrap();
        assert!(report.fields[0].selected.is_none());
    }
}

#[test]
fn explicit_corroboration_thresholds_and_semantic_conflicts_are_not_bypassed() {
    let (source, target) = pair();
    for cfg in [
        Config {
            corroboration: Some(Corroboration::default()),
            ..config(false)
        },
        Config {
            min_score: 0.99,
            ..config(false)
        },
    ] {
        assert!(MatchEngine::new(cfg)
            .unwrap()
            .match_schemas(&source, &target)
            .unwrap()
            .fields[0]
            .selected
            .is_none());
    }
    let mut source = source;
    let mut target = target;
    source.fields[0].hints = SemanticHints {
        identifier_scope: Some("customers".into()),
        ..Default::default()
    };
    target.fields[0].hints = SemanticHints {
        identifier_scope: Some("suppliers".into()),
        ..Default::default()
    };
    let report = MatchEngine::new(config(false))
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap();
    assert!(report.fields[0].selected.is_none());
    assert!(report.fields[0].candidates[0]
        .issues
        .iter()
        .any(|issue| matches!(issue, CandidateIssue::SemanticConflict(_))));
}

#[test]
fn one_sided_units_and_raw_name_conflicts_still_block_the_fallback() {
    let (source, mut target) = pair();
    target.fields[0].name = "tracking_id_usd".into();
    let cfg = Config {
        name_conflicts: vec![NameConflictRule {
            kind: NameConflictKind::Unit,
            alternatives: vec![vec!["usd".into()], vec!["eur".into()]],
        }],
        ..config(false)
    };
    assert!(MatchEngine::new(cfg)
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap()
        .fields[0]
        .selected
        .is_none());
    let (mut source, mut target) = pair();
    source.fields[0].name = "gross_shipment_tracking_id".into();
    target.fields[0].name = "net_shipment_tracking_id".into();
    let cfg = Config {
        name_conflicts: vec![NameConflictRule {
            kind: NameConflictKind::Qualifier,
            alternatives: vec![vec!["gross".into()], vec!["net".into()]],
        }],
        ..config(false)
    };
    assert!(MatchEngine::new(cfg)
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap()
        .fields[0]
        .selected
        .is_none());
}

#[test]
fn actual_builtin_signal_order_is_used_and_samples_are_not_exposed() {
    let (source, target) = pair();
    let engine = MatchEngine::with_matchers(
        config(false),
        vec![
            WeightedMatcher::new(0.15, SampleMatcher::default()),
            WeightedMatcher::new(0.20, TypeMatcher),
            WeightedMatcher::new(0.65, NameMatcher::default()),
        ],
    )
    .unwrap();
    let report = engine.match_schemas(&source, &target).unwrap();
    assert!(report.fields[0].selected.is_some());
    let printed = format!("{report:?}");
    assert!(!printed.contains("TRK-100"));
    assert!(!printed.contains("TRK-200"));
}

#[test]
fn multiple_fallback_pairs_and_input_order_use_normal_one_to_one_assignment() {
    let (mut source, mut target) = pair();
    source.fields.push(text(
        "r",
        "requester_email",
        &[
            "a@example.invalid",
            "b@example.invalid",
            "c@example.invalid",
        ],
    ));
    target.fields.push(text(
        "u",
        "contact_email",
        &[
            "d@example.invalid",
            "e@example.invalid",
            "f@example.invalid",
        ],
    ));
    for one_to_one in [false, true] {
        let engine = MatchEngine::new(config(one_to_one)).unwrap();
        let before = engine.match_schemas(&source, &target).unwrap();
        assert_eq!(
            before
                .fields
                .iter()
                .filter(|field| field.selected.is_some())
                .count(),
            2
        );
        let mut reordered_source = source.clone();
        let mut reordered_target = target.clone();
        reordered_source.fields.reverse();
        reordered_target.fields.reverse();
        assert_eq!(
            before,
            engine
                .match_schemas(&reordered_source, &reordered_target)
                .unwrap()
        );
    }
}

#[test]
fn timestamp_like_text_does_not_bypass_existing_event_support() {
    let source = Schema::new(vec![text(
        "s",
        "event_time",
        &[
            "2026-01-01T10:00:00Z",
            "2026-01-02T10:00:00Z",
            "2026-01-03T10:00:00Z",
        ],
    )]);
    let target = Schema::new(vec![text(
        "t",
        "event_time",
        &[
            "2026-02-01T10:00:00Z",
            "2026-02-02T10:00:00Z",
            "2026-02-03T10:00:00Z",
        ],
    )]);
    let report = MatchEngine::new(config(false))
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap();
    assert!(report.fields[0].selected.is_none());
}
