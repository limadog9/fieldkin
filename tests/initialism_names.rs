//! Complete initialisms need independent, reliable code-format evidence.
use fieldkin::{
    Config, ContextualEvidence, Corroboration, DataType, Field, FieldPair, MatchConstraints,
    MatchEngine, MatchReport, SampleValue, Schema, SemanticHints,
};

fn config(one_to_one: bool) -> Config {
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
            .map(|v| SampleValue::Text((*v).to_owned()))
            .collect(),
    )
}

fn pair(short: &str, expanded: &str) -> (Schema, Schema) {
    (
        Schema::new(vec![text("s", short, &["AB-123X", "C4-567", "DE-89F"])]),
        Schema::new(vec![text("t", expanded, &["GH-789Y", "J5-234", "KL-12Z"])]),
    )
}

fn run(source: &Schema, target: &Schema, cfg: Config) -> MatchReport {
    MatchEngine::new(cfg)
        .unwrap()
        .match_schemas(source, target)
        .unwrap()
}

fn none(report: &MatchReport) -> bool {
    report.fields.iter().all(|f| f.selected.is_none())
}

#[test]
fn complete_initialisms_with_multiple_code_shapes_work_in_both_directions_and_modes() {
    for (short, expanded) in [
        ("sri", "serial_reference_identifier"),
        ("SRI", "SerialReferenceIdentifier"),
        ("__sri--", "  serial.reference.identifier  "),
        ("pci", "part_catalog_identifier"),
    ] {
        for one_to_one in [false, true] {
            let (source, target) = pair(short, expanded);
            let ordinary = run(
                &source,
                &target,
                Config {
                    one_to_one,
                    ..Config::default()
                },
            );
            assert!(none(&ordinary));
            let report = run(&source, &target, config(one_to_one));
            let selected = report.fields[0]
                .selected
                .as_ref()
                .expect("initialism proposal");
            assert_eq!(selected.target.0, "t");
            assert_eq!(selected.signals, ordinary.fields[0].candidates[0].signals);
            assert!(selected
                .warnings
                .iter()
                .any(|w| w.contains("whole-name initialism")));
            let reversed = run(&target, &source, config(one_to_one));
            assert_eq!(reversed.fields[0].selected.as_ref().unwrap().target.0, "s");
        }
    }
}

#[test]
fn initialism_alone_never_supplies_missing_sample_evidence() {
    for one_to_one in [false, true] {
        for samples in [None, Some(vec![]), Some(vec![SampleValue::Null; 4])] {
            let (mut source, mut target) = pair("sri", "serial_reference_identifier");
            source.fields[0].samples = samples.clone();
            assert!(none(&run(&source, &target, config(one_to_one))));
            let (reset, _) = pair("sri", "serial_reference_identifier");
            source = reset;
            target.fields[0].samples = samples;
            assert!(none(&run(&source, &target, config(one_to_one))));
        }
    }
}

#[test]
fn reliability_checks_still_reject_constants_two_values_and_mixed_kinds() {
    let (source, target) = pair("sri", "serial_reference_identifier");
    for values in [vec!["AB-123X"; 4], vec!["AB-123X", "C4-567", "AB-123X"]] {
        let unreliable = Schema::new(vec![text("s", "sri", &values)]);
        assert!(none(&run(&unreliable, &target, config(false))));
    }
    let mut mixed = source;
    mixed.fields[0]
        .samples
        .as_mut()
        .unwrap()
        .push(SampleValue::Boolean(true));
    assert!(none(&run(&mixed, &target, config(false))));
}

#[test]
fn null_coverage_boundary_is_preserved() {
    for one_to_one in [false, true] {
        let (mut source, target) = pair("sri", "serial_reference_identifier");
        source.fields[0]
            .samples
            .as_mut()
            .unwrap()
            .extend(vec![SampleValue::Null; 9]);
        assert!(!none(&run(&source, &target, config(one_to_one))));
        source.fields[0]
            .samples
            .as_mut()
            .unwrap()
            .push(SampleValue::Null);
        assert!(none(&run(&source, &target, config(one_to_one))));
    }
}

#[test]
fn expansion_keeps_all_words_and_rejects_partial_short_or_wrong_initialisms() {
    for (source_name, target_name) in [
        ("sri", "serial_reference_identifier_actual"),
        ("sri", "primary_serial_reference_identifier"),
        ("sri", "serial_identifier_reference"),
        ("ri", "reference_identifier"),
        ("sr1", "serial_reference_identifier"),
        ("sri", "serial_reference"),
        ("sri", "serial_reference_id_2"),
        ("short_reference_identifier", "serial_reference_identifier"),
    ] {
        let (source, target) = pair(source_name, target_name);
        assert!(
            none(&run(&source, &target, config(false))),
            "{source_name} -> {target_name}"
        );
    }
}

#[test]
fn incompatible_shape_sets_and_noncode_observations_do_not_supply_support() {
    for values in [
        vec!["AB-123X", "C4-567", "DE-89"], // missing the final letter shape
        vec!["ALPHA", "BRAVO", "CHARLIE"],
        vec!["101", "202", "303"],
        vec!["AB 123X", "C4 567", "DE 89F"],
        vec!["a@sample.invalid", "b@sample.invalid", "c@sample.invalid"],
    ] {
        let (_, target) = pair("sri", "serial_reference_identifier");
        let source = Schema::new(vec![text("s", "sri", &values)]);
        assert!(none(&run(&source, &target, config(false))));
    }
}

#[test]
fn competing_expansions_remain_visible_after_top_k_and_caller_exclusion() {
    for one_to_one in [false, true] {
        let (source, mut target) = pair("sri", "serial_reference_identifier");
        let mut competitor = target.fields[0].clone();
        competitor.id = "u".into();
        competitor.name = "shipment_record_index".into();
        target.fields.push(competitor);
        for max_candidates in [1, 5] {
            let engine = MatchEngine::new(Config {
                max_candidates,
                ..config(one_to_one)
            })
            .unwrap();
            assert!(none(&engine.match_schemas(&source, &target).unwrap()));
            let constraints = MatchConstraints {
                forbidden: vec![FieldPair::new("s", "u")],
                ..Default::default()
            };
            assert!(none(
                &engine
                    .match_schemas_with_constraints(&source, &target, &constraints)
                    .unwrap()
            ));
        }
    }
}

#[test]
fn duplicate_sources_and_existing_target_claims_are_not_overridden() {
    for one_to_one in [false, true] {
        let (mut source, target) = pair("sri", "serial_reference_identifier");
        let mut duplicate = source.fields[0].clone();
        duplicate.id = "r".into();
        source.fields.push(duplicate);
        assert!(none(&run(&source, &target, config(one_to_one))));

        let (mut source, target) = pair("sri", "serial_reference_identifier");
        let mut incumbent = target.fields[0].clone();
        incumbent.id = "a".into();
        source.fields.push(incumbent);
        let report = run(&source, &target, config(one_to_one));
        assert!(report
            .fields
            .iter()
            .find(|f| f.source.0 == "s")
            .unwrap()
            .selected
            .is_none());
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
    }
}

#[test]
fn caller_constraints_corroboration_and_high_thresholds_still_apply() {
    let (source, target) = pair("sri", "serial_reference_identifier");
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
            assert!(none(
                &engine
                    .match_schemas_with_constraints(&source, &target, &constraints)
                    .unwrap()
            ));
        }
        for cfg in [
            Config {
                min_score: 0.99,
                ..config(one_to_one)
            },
            Config {
                corroboration: Some(Corroboration::default()),
                ..config(one_to_one)
            },
        ] {
            assert!(none(&run(&source, &target, cfg)));
        }
    }
}

#[test]
fn semantic_scope_conflicts_and_nontext_types_are_not_bypassed() {
    let (mut source, mut target) = pair("sri", "serial_reference_identifier");
    source.fields[0].hints = SemanticHints {
        identifier_scope: Some("suppliers".into()),
        ..Default::default()
    };
    target.fields[0].hints = SemanticHints {
        identifier_scope: Some("customers".into()),
        ..Default::default()
    };
    assert!(none(&run(&source, &target, config(false))));
    for kind in [DataType::Unknown, DataType::Integer, DataType::Binary] {
        let (mut source, mut target) = pair("sri", "serial_reference_identifier");
        source.fields[0].data_type = kind;
        target.fields[0].data_type = kind;
        assert!(none(&run(&source, &target, config(false))));
    }
}

#[test]
fn sample_order_and_privacy_are_preserved() {
    let (mut source, mut target) = pair("sri", "serial_reference_identifier");
    for one_to_one in [false, true] {
        let before = run(&source, &target, config(one_to_one));
        source.fields[0].samples.as_mut().unwrap().reverse();
        target.fields[0].samples.as_mut().unwrap().reverse();
        assert_eq!(before, run(&source, &target, config(one_to_one)));
        let printed = format!("{before:?}");
        for sample in ["AB-123X", "C4-567", "DE-89F", "GH-789Y", "J5-234", "KL-12Z"] {
            assert!(!printed.contains(sample));
        }
    }
}
