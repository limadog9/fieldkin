//! Contracts for exact, explicitly unit-qualified physical measurement names.
use fieldkin::{
    CandidateIssue, Config, ContextualEvidence, Corroboration, DataType, Decision, Field,
    FieldPair, MatchConstraints, MatchEngine, SampleValue, Schema, SemanticHints,
};

fn config(one_to_one: bool) -> Config {
    Config {
        one_to_one,
        contextual_evidence: Some(ContextualEvidence::default()),
        ..Config::default()
    }
}

fn numeric(id: &str, name: &str, values: &[f64]) -> Field {
    Field::new(id, name, DataType::Float)
        .with_samples(values.iter().copied().map(SampleValue::Number).collect())
}

fn pair(source_name: &str, target_name: &str) -> (Schema, Schema) {
    (
        Schema::new(vec![numeric("s", source_name, &[10.25, 20.5, 30.75])]),
        Schema::new(vec![numeric("t", target_name, &[40.25, 50.5, 60.75])]),
    )
}

fn evidence_used(candidate: &fieldkin::Candidate) -> bool {
    candidate
        .warnings
        .iter()
        .any(|warning| warning.starts_with("Explicit measurement-name support:"))
}

#[test]
fn same_physical_quantity_and_unit_work_with_or_without_samples_in_both_modes() {
    for (source_name, target_name) in [
        ("odometer_km", "odometer_km"),
        ("temperature_c", "temp_c"),
        ("net_weight_kg", "net_weight_kg"),
        ("inlet_pressure_psi", "inlet_pressure_psi"),
        ("battery_voltage_mv", "battery_voltage_mv"),
        ("request_latency_ms", "request_latency_ms"),
    ] {
        for one_to_one in [false, true] {
            for with_samples in [false, true] {
                let (mut source, mut target) = pair(source_name, target_name);
                if !with_samples {
                    source.fields[0].samples = None;
                    target.fields[0].samples = None;
                }
                let report = MatchEngine::new(config(one_to_one))
                    .unwrap()
                    .match_schemas(&source, &target)
                    .unwrap();
                assert_eq!(report.fields[0].selected.as_ref().map(|c| c.target.0.as_str()), Some("t"),
                    "{source_name} -> {target_name}, samples={with_samples}, one_to_one={one_to_one}");
                if with_samples {
                    assert!(evidence_used(report.fields[0].selected.as_ref().unwrap()));
                }
            }
        }
    }
}

#[test]
fn units_qualifiers_and_quantities_are_not_erased() {
    for (s, t) in [
        ("temperature_c", "temp_f"),
        ("odometer_km", "odometer_m"),
        ("weight_kg", "weight_g"),
        ("gross_weight_kg", "net_weight_kg"),
        ("estimated_distance_km", "actual_distance_km"),
        ("inlet_pressure_psi", "outlet_pressure_psi"),
        ("travel_distance_km", "odometer_km"),
        ("temperature_c", "temperature"),
        ("weight_kg", "weight"),
        ("value_kg", "value_kg"),
        ("wind_speed", "wind_speed"),
        ("temperature", "temperature"),
        ("temperature_code_c", "temperature_code_c"),
        ("temp_length_km", "temperature_length_km"),
    ] {
        for one_to_one in [false, true] {
            let (source, target) = pair(s, t);
            let report = MatchEngine::new(config(one_to_one))
                .unwrap()
                .match_schemas(&source, &target)
                .unwrap();
            assert!(report.fields[0].selected.is_none(), "{s} -> {t}");
        }
    }
}

#[test]
fn representation_is_not_inferred_from_strings_or_incompatible_declarations() {
    for kind in [
        DataType::Text,
        DataType::Unknown,
        DataType::Date,
        DataType::Timestamp,
    ] {
        let (mut source, mut target) = pair("odometer_km", "odometer_km");
        source.fields[0].data_type = kind;
        target.fields[0].data_type = kind;
        let report = MatchEngine::new(config(false))
            .unwrap()
            .match_schemas(&source, &target)
            .unwrap();
        assert!(report.fields[0].selected.is_none(), "{kind:?}");
    }
    let (mut source, target) = pair("odometer_km", "odometer_km");
    source.fields[0].samples = Some(vec![SampleValue::Text("not-a-number".into()); 3]);
    assert!(MatchEngine::new(config(false))
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap()
        .fields[0]
        .selected
        .is_none());
    let (source, mut target) = pair("odometer_km", "odometer_km");
    target.fields[0].data_type = DataType::Decimal;
    assert!(MatchEngine::new(config(false))
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap()
        .fields[0]
        .selected
        .is_none());
}

#[test]
fn integer_measurements_need_integral_observations() {
    let (mut source, mut target) = pair("distance_km", "distance_km");
    source.fields[0].data_type = DataType::Integer;
    target.fields[0].data_type = DataType::Integer;
    // These fractional samples contradict the declarations.
    assert!(MatchEngine::new(config(false))
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap()
        .fields[0]
        .selected
        .is_none());
    source.fields[0].samples = Some(vec![
        SampleValue::Integer(1),
        SampleValue::Integer(2),
        SampleValue::Integer(3),
    ]);
    target.fields[0].samples = Some(vec![
        SampleValue::Integer(4),
        SampleValue::Integer(5),
        SampleValue::Integer(6),
    ]);
    for one_to_one in [false, true] {
        assert!(MatchEngine::new(config(one_to_one))
            .unwrap()
            .match_schemas(&source, &target)
            .unwrap()
            .fields[0]
            .selected
            .is_some());
    }
}

#[test]
fn duplicate_targets_remain_ambiguous_before_top_k_in_both_modes() {
    for one_to_one in [false, true] {
        let (source, mut target) = pair("temperature_c", "temp_c");
        target
            .fields
            .push(numeric("u", "temperature_c", &[70.25, 80.5, 90.75]));
        let report = MatchEngine::new(Config {
            max_candidates: 1,
            ..config(one_to_one)
        })
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap();
        assert!(report.fields[0].selected.is_none());
        assert_eq!(report.fields[0].decision, Decision::Ambiguous);
        assert_eq!(report.fields[0].alternatives.len(), 2);
    }
}

#[test]
fn original_eligible_rows_and_targets_are_not_modified() {
    for one_to_one in [false, true] {
        let (mut source, target) = pair("odometer_km", "odometer_km");
        let mut incumbent = target.fields[0].clone();
        incumbent.id = "a".into();
        source.fields.push(incumbent);
        let report = MatchEngine::new(config(one_to_one))
            .unwrap()
            .match_schemas(&source, &target)
            .unwrap();
        assert_eq!(report.fields[0].source.0, "a");
        assert_eq!(report.fields[0].selected.as_ref().unwrap().target.0, "t");
        assert!(report.fields[1].selected.is_none());
        let (source, mut target) = pair("odometer_km", "odometer_km");
        let mut supported = source.fields[0].clone();
        supported.id = "u".into();
        target.fields.push(supported);
        let report = MatchEngine::new(config(one_to_one))
            .unwrap()
            .match_schemas(&source, &target)
            .unwrap();
        assert_eq!(report.fields[0].selected.as_ref().unwrap().target.0, "u");
        assert!(!report.fields[0].candidates.iter().any(evidence_used));
    }
}

#[test]
fn defaults_corroboration_thresholds_and_semantic_vetoes_remain_in_force() {
    let (source, target) = pair("odometer_km", "odometer_km");
    let ordinary = MatchEngine::new(Config::default())
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap();
    assert!(!ordinary.fields[0].candidates.iter().any(evidence_used));
    for cfg in [
        Config {
            min_score: 0.99,
            ..config(false)
        },
        Config {
            corroboration: Some(Corroboration::default()),
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
    let (mut source, mut target) = pair("odometer_km", "odometer_km");
    source.fields[0].hints = SemanticHints {
        unit: Some("km".into()),
        ..Default::default()
    };
    target.fields[0].hints = SemanticHints {
        unit: Some("m".into()),
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
        .any(|i| matches!(i, CandidateIssue::SemanticConflict(_))));
}

#[test]
fn exclusions_confirmations_and_reordering_use_the_existing_selector() {
    for one_to_one in [false, true] {
        let (source, target) = pair("temperature_c", "temp_c");
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
            assert!(engine
                .match_schemas_with_constraints(&source, &target, &constraints)
                .unwrap()
                .fields[0]
                .selected
                .is_none());
        }
        let (mut source, mut target) = pair("odometer_km", "odometer_km");
        source
            .fields
            .push(numeric("r", "temperature_c", &[1.2, 2.3, 3.4]));
        target.fields.push(numeric("u", "temp_c", &[4.5, 5.6, 6.7]));
        let before = engine.match_schemas(&source, &target).unwrap();
        assert_eq!(
            before
                .fields
                .iter()
                .filter(|f| f.selected.is_some())
                .count(),
            2
        );
        source.fields.reverse();
        target.fields.reverse();
        assert_eq!(before, engine.match_schemas(&source, &target).unwrap());
        let printed = format!("{before:?}");
        assert!(!printed.contains("30.75"));
        assert!(!printed.contains("60.75"));
    }
    let (mut source, target) = pair("odometer_km", "odometer_km");
    source
        .fields
        .push(Field::new("z", "unclassified", DataType::Float));
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
}
