//! Same-role identifier formats must be supported by reliable observed values.

use fieldkin::{
    Config, ContextualEvidence, Corroboration, DataType, Field, FieldPair, MatchConstraints,
    MatchEngine, MatchReport, SampleValue, Schema, SemanticHints,
};

const LEFT: [&str; 3] = ["C-0A7F", "C-4BC2", "C-98DE"];
const RIGHT: [&str; 3] = ["C-A071", "C-BC24", "C-DE89"];

fn text(id: &str, name: &str, values: &[&str]) -> Field {
    Field::new(id, name, DataType::Text).with_samples(
        values
            .iter()
            .map(|v| SampleValue::Text((*v).into()))
            .collect(),
    )
}

fn config(one_to_one: bool) -> Config {
    Config {
        one_to_one,
        contextual_evidence: Some(ContextualEvidence::default()),
        ..Config::default()
    }
}

fn pair() -> (Schema, Schema) {
    (
        Schema::new(vec![text("s", "contact_uuid", &LEFT)]),
        Schema::new(vec![text("t", "contact_id", &RIGHT)]),
    )
}

fn run(source: &Schema, target: &Schema, cfg: Config) -> MatchReport {
    MatchEngine::new(cfg)
        .unwrap()
        .match_schemas(source, target)
        .unwrap()
}

#[test]
fn equal_roles_and_hexadecimal_patterns_work_in_both_directions_and_modes() {
    for one_to_one in [false, true] {
        for reverse in [false, true] {
            let (mut source, mut target) = pair();
            if reverse {
                source.fields[0].name = "contact_id".into();
                target.fields[0].name = "contact_uuid".into();
            }
            let ordinary = run(&source, &target, Config::default());
            assert!(ordinary.fields[0].selected.is_none());
            let report = run(&source, &target, config(one_to_one));
            let chosen = report.fields[0]
                .selected
                .as_ref()
                .expect("sample-backed identifier");
            assert_eq!(chosen.target.0, "t");
            assert_eq!(chosen.signals, ordinary.fields[0].candidates[0].signals);
            assert!(chosen
                .warnings
                .iter()
                .any(|w| w.contains("identical complete identifier role")));
        }
    }
}

#[test]
fn no_missing_constant_sparse_or_mixed_samples_can_supply_the_new_support() {
    let (source, target) = pair();
    let mut bad = Vec::new();
    for values in [
        None,
        Some(vec![]),
        Some(vec![SampleValue::Null; 5]),
        Some(vec![SampleValue::Text(LEFT[0].into()); 4]),
        Some(vec![
            SampleValue::Text(LEFT[0].into()),
            SampleValue::Text(LEFT[1].into()),
        ]),
    ] {
        let mut field = source.fields[0].clone();
        field.samples = values;
        bad.push(field);
    }
    let mut mixed = source.fields[0].clone();
    mixed
        .samples
        .as_mut()
        .unwrap()
        .push(SampleValue::Boolean(true));
    bad.push(mixed);
    let mut sparse = source.fields[0].clone();
    sparse
        .samples
        .as_mut()
        .unwrap()
        .extend(vec![SampleValue::Null; 10]);
    bad.push(sparse);
    for one_to_one in [false, true] {
        for field in &bad {
            assert!(run(
                &Schema::new(vec![field.clone()]),
                &target,
                config(one_to_one)
            )
            .fields[0]
                .selected
                .is_none());
        }
        let mut boundary = source.clone();
        boundary.fields[0]
            .samples
            .as_mut()
            .unwrap()
            .extend(vec![SampleValue::Null; 9]);
        assert!(run(&boundary, &target, config(one_to_one)).fields[0]
            .selected
            .is_some());
        let mut target_without_samples = target.clone();
        target_without_samples.fields[0].samples = None;
        assert!(
            run(&source, &target_without_samples, config(one_to_one)).fields[0]
                .selected
                .is_none()
        );
    }
}

#[test]
fn entity_qualifiers_identifier_markers_and_literal_prefixes_are_not_discarded() {
    for one_to_one in [false, true] {
        for (a, b) in [
            ("contact_uuid", "organization_id"),
            ("primary_contact_uuid", "contact_id"),
            ("contact_uuid", "parent_contact_id"),
            ("contact_uuid", "contact_code"),
            ("status_uuid", "status_id"),
            ("uuid", "id"),
        ] {
            let source = Schema::new(vec![text("s", a, &LEFT)]);
            let target = Schema::new(vec![text("t", b, &RIGHT)]);
            assert!(
                run(&source, &target, config(one_to_one)).fields[0]
                    .selected
                    .is_none(),
                "{a} -> {b}"
            );
        }
        for values in [
            ["D-A071", "D-BC24", "D-DE89"],
            ["c-A071", "c-BC24", "c-DE89"],
            ["C_A071", "C_BC24", "C_DE89"],
            ["C-A0710", "C-BC240", "C-DE890"],
            ["C-Q071", "C-QR24", "C-RS89"],
            ["C-A071", "D-BC24", "C-DE89"],
        ] {
            let (source, _) = pair();
            let target = Schema::new(vec![text("t", "contact_id", &values)]);
            assert!(
                run(&source, &target, config(one_to_one)).fields[0]
                    .selected
                    .is_none(),
                "{values:?}"
            );
        }
    }
}

#[test]
fn same_role_competitors_count_before_samples_review_and_top_k() {
    for one_to_one in [false, true] {
        for unknown in [false, true] {
            let (source, mut target) = pair();
            let kind = if unknown {
                DataType::Unknown
            } else {
                DataType::Text
            };
            target.fields.push(Field::new("u", "contact_guid", kind));
            for max_candidates in [1, 5] {
                let cfg = Config {
                    max_candidates,
                    ..config(one_to_one)
                };
                let constraints = MatchConstraints {
                    forbidden: vec![FieldPair::new("s", "u")],
                    ..Default::default()
                };
                let report = MatchEngine::new(cfg)
                    .unwrap()
                    .match_schemas_with_constraints(&source, &target, &constraints)
                    .unwrap();
                assert!(report.fields[0].selected.is_none());
            }
        }
        let (mut source, target) = pair();
        source
            .fields
            .push(Field::new("r", "contact_guid", DataType::Text));
        assert!(run(&source, &target, config(one_to_one))
            .fields
            .iter()
            .all(|f| f.selected.is_none()));
    }
}

#[test]
fn corroboration_hard_semantics_and_caller_choices_still_control_selection() {
    for one_to_one in [false, true] {
        let (source, target) = pair();
        for cfg in [
            Config {
                corroboration: Some(Corroboration::default()),
                ..config(one_to_one)
            },
            Config {
                min_score: 0.99,
                ..config(one_to_one)
            },
        ] {
            assert!(run(&source, &target, cfg).fields[0].selected.is_none());
        }
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
            let report = MatchEngine::new(config(one_to_one))
                .unwrap()
                .match_schemas_with_constraints(&source, &target, &constraints)
                .unwrap();
            assert!(report.fields[0].selected.is_none());
        }
        let mut a = source.clone();
        let mut b = target.clone();
        a.fields[0].hints = SemanticHints {
            identifier_scope: Some("customer-directory".into()),
            ..Default::default()
        };
        b.fields[0].hints = SemanticHints {
            identifier_scope: Some("supplier-directory".into()),
            ..Default::default()
        };
        assert!(run(&a, &b, config(one_to_one)).fields[0].selected.is_none());
    }
}

#[test]
fn existing_target_claims_are_preserved() {
    for one_to_one in [false, true] {
        let (mut source, target) = pair();
        let mut incumbent = target.fields[0].clone();
        incumbent.id = "a".into();
        incumbent.data_type = DataType::Unknown;
        source.fields.push(incumbent);
        let report = run(&source, &target, config(one_to_one));
        assert_eq!(
            report
                .fields
                .iter()
                .find(|f| f.source.0 == "a")
                .unwrap()
                .selected
                .as_ref()
                .unwrap()
                .target
                .0,
            "t"
        );
        assert!(report
            .fields
            .iter()
            .find(|f| f.source.0 == "s")
            .unwrap()
            .selected
            .is_none());
    }
}

#[test]
fn sample_order_and_field_ids_cannot_create_evidence_or_expose_values() {
    let (source, target) = pair();
    for one_to_one in [false, true] {
        let report = run(&source, &target, config(one_to_one));
        let mut a = source.clone();
        let mut b = target.clone();
        a.fields[0].samples.as_mut().unwrap().reverse();
        b.fields[0].samples.as_mut().unwrap().reverse();
        assert_eq!(report, run(&a, &b, config(one_to_one)));
        a.fields[0].id = "unrelated-source-key".into();
        b.fields[0].id = "unrelated-target-key".into();
        assert_eq!(
            run(&a, &b, config(one_to_one)).fields[0]
                .selected
                .as_ref()
                .unwrap()
                .target
                .0,
            "unrelated-target-key"
        );
        let debug = format!("{report:?}");
        for value in LEFT.iter().chain(RIGHT.iter()) {
            assert!(!debug.contains(value));
        }
    }
}
