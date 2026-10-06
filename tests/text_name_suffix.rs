//! Runtime contracts for sample-backed removal of a trailing text-name suffix.
use fieldkin::{
    CandidateIssue, Config, ContextualEvidence, Corroboration, DataType, Decision, Field,
    FieldPair, MatchConstraints, MatchEngine, MatchReport, SampleValue, Schema, SemanticHints,
};

const SOURCE: [&str; 3] = ["Amber", "Indigo", "Ivory"];
const TARGET: [&str; 3] = ["Cobalt", "Ochre", "Violet"];

fn cfg(one_to_one: bool) -> Config {
    Config {
        one_to_one,
        contextual_evidence: Some(ContextualEvidence::default()),
        ..Config::default()
    }
}
fn text(id: &str, name: &str, values: &[&str]) -> Field {
    Field::new(id, name, DataType::Text).with_samples(
        values
            .iter()
            .map(|v| SampleValue::Text((*v).into()))
            .collect(),
    )
}
fn pair() -> (Schema, Schema) {
    (
        Schema::new(vec![text("s", "finish_name", &SOURCE)]),
        Schema::new(vec![text("t", "finish", &TARGET)]),
    )
}
fn run(source: &Schema, target: &Schema, config: Config) -> MatchReport {
    MatchEngine::new(config)
        .unwrap()
        .match_schemas(source, target)
        .unwrap()
}
fn uses_suffix(report: &MatchReport) -> bool {
    report.fields.iter().flat_map(|f| &f.candidates).any(|c| {
        c.warnings
            .iter()
            .any(|w| w.starts_with("Display-name suffix support:"))
    })
}

#[test]
fn descriptive_suffix_uses_both_real_selectors_in_both_directions() {
    for one_to_one in [false, true] {
        for (left, right) in [("finish_name", "finish"), ("finish", "finish_name")] {
            let source = Schema::new(vec![text("s", left, &SOURCE)]);
            let target = Schema::new(vec![text("t", right, &TARGET)]);
            let ordinary = run(&source, &target, Config::default());
            assert!(ordinary.fields[0].selected.is_none());
            let report = run(&source, &target, cfg(one_to_one));
            let selected = report.fields[0]
                .selected
                .as_ref()
                .expect("descriptive suffix proposal");
            assert_eq!(selected.target.0, "t");
            assert_eq!(report.fields[0].decision, Decision::Proposed);
            assert_eq!(selected.signals, ordinary.fields[0].candidates[0].signals);
            assert!(uses_suffix(&report));
        }
    }
}

#[test]
fn missing_sparse_repeated_coded_or_mixed_samples_do_not_enable_suffix_support() {
    let (source, target) = pair();
    let mut cases = vec![None, Some(Vec::new()), Some(vec![SampleValue::Null; 5])];
    for values in [
        vec!["Unknown"; 4],
        vec!["Amber", "Ivory", "Amber"],
        vec!["ALPHA", "BETA", "GAMMA"],
        vec!["US", "CA", "GB"],
        vec!["CODE-001", "CODE-002", "CODE-003"],
        vec!["a@host.test", "b@host.test", "c@host.test"],
    ] {
        cases.push(Some(
            values
                .into_iter()
                .map(|s| SampleValue::Text(s.into()))
                .collect(),
        ));
    }
    let mut mixed = source.fields[0].samples.clone().unwrap();
    mixed.push(SampleValue::Boolean(true));
    cases.push(Some(mixed));
    let mut sparse = source.fields[0].samples.clone().unwrap();
    sparse.extend(vec![SampleValue::Null; 10]);
    cases.push(Some(sparse));
    for one_to_one in [false, true] {
        for samples in &cases {
            let mut changed = source.clone();
            changed.fields[0].samples = samples.clone();
            assert!(!uses_suffix(&run(&changed, &target, cfg(one_to_one))));
            let mut changed = target.clone();
            changed.fields[0].samples = samples.clone();
            assert!(!uses_suffix(&run(&source, &changed, cfg(one_to_one))));
        }
    }
    for one_to_one in [false, true] {
        let mut source = source.clone();
        let mut target = target.clone();
        for field in source.fields.iter_mut().chain(&mut target.fields) {
            field
                .samples
                .as_mut()
                .unwrap()
                .extend(vec![SampleValue::Null; 9]);
        }
        assert!(uses_suffix(&run(&source, &target, cfg(one_to_one)))); // 25% exactly
    }
}

#[test]
fn all_role_tokens_scope_polarity_and_declared_type_remain_significant() {
    for (left, right) in [
        ("supplier_finish_name", "customer_finish"),
        ("primary_finish_name", "secondary_finish"),
        ("estimated_finish_name", "actual_finish"),
        ("not_finish_name", "finish"),
        ("finish_name", "finish_code"),
        ("finish_uuid_name", "finish_uuid"),
        ("name", "name"),
        ("status_name", "status"),
        ("name_finish", "finish"),
    ] {
        for one_to_one in [false, true] {
            let source = Schema::new(vec![text("s", left, &SOURCE)]);
            let target = Schema::new(vec![text("t", right, &TARGET)]);
            assert!(
                !uses_suffix(&run(&source, &target, cfg(one_to_one))),
                "{left} -> {right}"
            );
        }
    }
    let (mut source, target) = pair();
    for kind in [DataType::Unknown, DataType::Integer, DataType::Boolean] {
        source.fields[0].data_type = kind;
        assert!(!uses_suffix(&run(&source, &target, cfg(false))));
    }
}

#[test]
fn competing_full_role_names_stay_visible_without_usable_samples() {
    for one_to_one in [false, true] {
        let (source, mut target) = pair();
        target
            .fields
            .push(Field::new("u", "finish_name", DataType::Text));
        let engine = MatchEngine::new(Config {
            max_candidates: 1,
            ..cfg(one_to_one)
        })
        .unwrap();
        let reviewed = engine
            .match_schemas_with_constraints(
                &source,
                &target,
                &MatchConstraints {
                    forbidden: vec![FieldPair::new("s", "u")],
                    ..Default::default()
                },
            )
            .unwrap();
        assert!(reviewed.fields[0].selected.is_none());
        assert!(!uses_suffix(&reviewed));
        let (mut source, target) = pair();
        source
            .fields
            .push(Field::new("r", "finish", DataType::Text));
        assert!(!uses_suffix(&run(&source, &target, cfg(one_to_one))));
    }
}

#[test]
fn existing_eligible_rows_and_target_claims_are_preserved() {
    for one_to_one in [false, true] {
        let (mut source, target) = pair();
        let mut incumbent = target.fields[0].clone();
        incumbent.id = "a".into();
        source.fields.push(incumbent);
        let report = run(&source, &target, cfg(one_to_one));
        assert_eq!(report.fields[0].source.0, "a");
        assert_eq!(report.fields[0].selected.as_ref().unwrap().target.0, "t");
        assert!(report.fields[1].selected.is_none());
        assert!(!uses_suffix(&report));
    }
}

#[test]
fn caller_constraints_corroboration_and_semantic_hints_still_win() {
    let (source, target) = pair();
    for one_to_one in [false, true] {
        let engine = MatchEngine::new(cfg(one_to_one)).unwrap();
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
        for config in [
            Config {
                min_score: 0.99,
                ..cfg(one_to_one)
            },
            Config {
                corroboration: Some(Corroboration::default()),
                ..cfg(one_to_one)
            },
        ] {
            assert!(run(&source, &target, config).fields[0].selected.is_none());
        }
        let mut different_scope = target.clone();
        let mut source_scope = source.clone();
        source_scope.fields[0].hints = SemanticHints {
            identifier_scope: Some("left".into()),
            ..Default::default()
        };
        different_scope.fields[0].hints = SemanticHints {
            identifier_scope: Some("right".into()),
            ..Default::default()
        };
        let report = run(&source_scope, &different_scope, cfg(one_to_one));
        assert!(report.fields[0].selected.is_none());
        assert!(report.fields[0].candidates[0]
            .issues
            .iter()
            .any(|i| matches!(i, CandidateIssue::SemanticConflict(_))));
    }
    let mut source = source;
    source
        .fields
        .push(Field::new("z", "opaque", DataType::Text));
    let report = MatchEngine::new(cfg(true))
        .unwrap()
        .match_schemas_with_constraints(
            &source,
            &target,
            &MatchConstraints {
                confirmed: vec![FieldPair::new("z", "t")],
                ..Default::default()
            },
        )
        .unwrap();
    assert!(report.fields[0].selected.is_none());
    assert_eq!(report.fields[1].decision, Decision::Confirmed);
}

#[test]
fn suffix_route_is_order_invariant_and_does_not_disclose_values() {
    let (mut source, mut target) = pair();
    source
        .fields
        .push(text("r", "fabric_name", &["Cotton", "Linen", "Denim"]));
    target
        .fields
        .push(text("u", "fabric", &["Velvet", "Rayon", "Fleece"]));
    for one_to_one in [false, true] {
        let before = run(&source, &target, cfg(one_to_one));
        assert_eq!(
            before
                .fields
                .iter()
                .filter(|f| f.selected.is_some())
                .count(),
            2
        );
        let mut reversed_source = source.clone();
        let mut reversed_target = target.clone();
        reversed_source.fields.reverse();
        reversed_target.fields.reverse();
        for field in reversed_source
            .fields
            .iter_mut()
            .chain(&mut reversed_target.fields)
        {
            field.samples.as_mut().unwrap().reverse();
        }
        assert_eq!(
            before,
            run(&reversed_source, &reversed_target, cfg(one_to_one))
        );
        let debug = format!("{before:?}");
        for value in SOURCE.iter().chain(&TARGET) {
            assert!(!debug.contains(*value));
        }
    }
}
