//! Contracts for using observed key evidence to scope one bare description.
use fieldkin::{
    CandidateIssue, Config, ContextualEvidence, Corroboration, DataType, Decision, Field,
    FieldPair, MatchConstraints, MatchEngine, MatchReport, SampleValue, Schema, SemanticHints,
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
fn pair() -> (Schema, Schema) {
    (
        Schema::new(vec![
            text("s_key", "asset_id", &["EQ-101", "EQ-202", "EQ-303"]),
            text(
                "s_desc",
                "description",
                &[
                    "Portable pump assembly",
                    "Inspection tool carriage",
                    "Replacement sensor housing",
                ],
            ),
        ]),
        Schema::new(vec![
            text("t_key", "equipment_id", &["EQ-404", "EQ-505", "EQ-606"]),
            text(
                "t_desc",
                "equipment_description",
                &[
                    "Precision valve module",
                    "Auxiliary support bracket",
                    "Stainless fastening kit",
                ],
            ),
        ]),
    )
}
fn run(source: &Schema, target: &Schema, cfg: Config) -> MatchReport {
    MatchEngine::new(cfg)
        .unwrap()
        .match_schemas(source, target)
        .unwrap()
}
fn selected<'a>(report: &'a MatchReport, source: &str) -> Option<&'a fieldkin::Candidate> {
    report
        .fields
        .iter()
        .find(|f| f.source.0 == source)
        .unwrap()
        .selected
        .as_ref()
}
fn uses_context(report: &MatchReport) -> bool {
    report.fields.iter().flat_map(|f| &f.candidates).any(|c| {
        c.warnings
            .iter()
            .any(|w| w.starts_with("Entity-description support:"))
    })
}

#[test]
fn scoped_descriptions_use_real_selectors_in_both_directions_and_modes() {
    for one_to_one in [false, true] {
        let (source, target) = pair();
        for (source, target, source_id, target_id) in [
            (&source, &target, "s_desc", "t_desc"),
            (&target, &source, "t_desc", "s_desc"),
        ] {
            let report = run(source, target, config(one_to_one));
            let choice = selected(&report, source_id).expect("key-backed description");
            assert_eq!(choice.target.0, target_id);
            assert!(uses_context(&report));
            assert_eq!(
                report
                    .fields
                    .iter()
                    .filter(|f| f.selected.is_some())
                    .count(),
                2
            );
        }
    }
}

#[test]
fn an_eligible_sample_backed_key_is_required_not_just_similar_schema_names() {
    for one_to_one in [false, true] {
        for missing in [None, Some(Vec::new()), Some(vec![SampleValue::Null; 4])] {
            let (mut source, mut target) = pair();
            source.fields[0].samples = missing.clone();
            target.fields[0].samples = missing;
            let report = run(&source, &target, config(one_to_one));
            assert!(!uses_context(&report));
            assert!(selected(&report, "s_desc").is_none());
        }
        let (mut source, mut target) = pair();
        source.fields.remove(0);
        target.fields.remove(0);
        assert!(!uses_context(&run(&source, &target, config(one_to_one))));

        let (mut source, mut target) = pair();
        source.fields[0].hints = SemanticHints {
            identifier_scope: Some("assets".into()),
            ..Default::default()
        };
        target.fields[0].hints = SemanticHints {
            identifier_scope: Some("unrelated".into()),
            ..Default::default()
        };
        let report = run(&source, &target, config(one_to_one));
        assert!(selected(&report, "s_key").is_none());
        assert!(!uses_context(&report));
    }
}

#[test]
fn extra_keys_or_descriptions_block_before_observations_types_and_top_k() {
    for one_to_one in [false, true] {
        for name in [
            "supplier_id",
            "equipment_guid",
            "supplier_sku",
            "supplier_no",
            "equipment_description",
            "localized_description",
            "desc",
        ] {
            for on_source in [false, true] {
                let (mut source, mut target) = pair();
                let extra = Field::new("extra", name, DataType::Unknown);
                if on_source {
                    source.fields.push(extra);
                } else {
                    target.fields.push(extra);
                }
                let report = run(
                    &source,
                    &target,
                    Config {
                        max_candidates: 1,
                        ..config(one_to_one)
                    },
                );
                assert!(!uses_context(&report), "{name}, source={on_source}");
            }
        }
    }
}

#[test]
fn qualifiers_and_description_scope_are_not_discarded() {
    for name in [
        "supplier_description",
        "equipment_short_description",
        "equipment_description_en",
        "description_equipment",
        "equipment_desc",
        "equipment_name",
    ] {
        let (source, mut target) = pair();
        target.fields[1].name = name.into();
        assert!(
            !uses_context(&run(&source, &target, config(false))),
            "{name}"
        );
    }
    // A qualified identifier cannot be shortened into an unqualified entity.
    for name in ["parent_asset_id", "record_id", "status_code"] {
        let (mut source, target) = pair();
        source.fields[0].name = name.into();
        assert!(
            !uses_context(&run(&source, &target, config(false))),
            "{name}"
        );
    }
}

#[test]
fn missing_repeated_coded_or_mixed_description_samples_do_not_qualify() {
    let mut bad = vec![None, Some(Vec::new()), Some(vec![SampleValue::Null; 5])];
    for values in [
        vec!["Unknown"; 3],
        vec!["Alpha", "Beta", "Alpha"],
        vec!["AA-001", "AA-002", "AA-003"],
        vec!["ACTIVE", "PENDING", "CLOSED"],
    ] {
        bad.push(Some(
            values
                .iter()
                .map(|v| SampleValue::Text((*v).into()))
                .collect(),
        ));
    }
    let (source, _) = pair();
    let mut mixed = source.fields[1].samples.clone().unwrap();
    mixed.push(SampleValue::Integer(3));
    bad.push(Some(mixed));
    let mut sparse = source.fields[1].samples.clone().unwrap();
    sparse.extend(vec![SampleValue::Null; 10]);
    bad.push(Some(sparse));
    for samples in bad {
        for on_source in [false, true] {
            let (mut source, mut target) = pair();
            if on_source {
                source.fields[1].samples = samples.clone();
            } else {
                target.fields[1].samples = samples.clone();
            }
            assert!(!uses_context(&run(&source, &target, config(false))));
        }
    }
}

#[test]
fn original_eligible_description_targets_are_not_taken() {
    for one_to_one in [false, true] {
        let (mut source, target) = pair();
        let mut incumbent = target.fields[1].clone();
        incumbent.id = "incumbent".into();
        source.fields.push(incumbent);
        let report = run(&source, &target, config(one_to_one));
        assert_eq!(selected(&report, "incumbent").unwrap().target.0, "t_desc");
        assert!(selected(&report, "s_desc").is_none());
        assert!(!uses_context(&report));
    }
}

#[test]
fn default_threshold_corroboration_and_hard_semantics_are_preserved() {
    let (source, target) = pair();
    let ordinary = run(&source, &target, Config::default());
    assert!(!uses_context(&ordinary));
    let contextual = run(&source, &target, config(false));
    let choice = selected(&contextual, "s_desc").unwrap();
    let original = ordinary
        .fields
        .iter()
        .find(|f| f.source.0 == "s_desc")
        .unwrap()
        .candidates
        .iter()
        .find(|c| c.target.0 == "t_desc")
        .unwrap();
    assert_eq!(choice.signals, original.signals);
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
        assert!(!uses_context(&run(&source, &target, cfg)));
    }
    let (mut source, mut target) = pair();
    source.fields[1].hints = SemanticHints {
        identifier_scope: Some("one".into()),
        ..Default::default()
    };
    target.fields[1].hints = SemanticHints {
        identifier_scope: Some("two".into()),
        ..Default::default()
    };
    let report = run(&source, &target, config(false));
    assert!(!uses_context(&report));
    assert!(report
        .fields
        .iter()
        .find(|f| f.source.0 == "s_desc")
        .unwrap()
        .candidates
        .iter()
        .any(|c| c
            .issues
            .iter()
            .any(|i| matches!(i, CandidateIssue::SemanticConflict(_)))));
}

#[test]
fn caller_exclusions_confirmations_and_hidden_alternatives_remain_effective() {
    let (source, target) = pair();
    for one_to_one in [false, true] {
        let engine = MatchEngine::new(config(one_to_one)).unwrap();
        for constraints in [
            MatchConstraints {
                forbidden: vec![FieldPair::new("s_desc", "t_desc")],
                ..Default::default()
            },
            MatchConstraints {
                unmatched_sources: vec!["s_desc".into()],
                ..Default::default()
            },
        ] {
            let report = engine
                .match_schemas_with_constraints(&source, &target, &constraints)
                .unwrap();
            assert!(selected(&report, "s_desc").is_none());
        }
        let (mut source, mut target) = pair();
        source.fields[0].samples = None;
        target.fields[0].samples = None;
        let constraints = MatchConstraints {
            confirmed: vec![FieldPair::new("s_key", "t_key")],
            ..Default::default()
        };
        let report = engine
            .match_schemas_with_constraints(&source, &target, &constraints)
            .unwrap();
        assert_eq!(
            report
                .fields
                .iter()
                .find(|f| f.source.0 == "s_key")
                .unwrap()
                .decision,
            Decision::Confirmed
        );
        assert!(
            !uses_context(&report),
            "caller confirmation is not automatic scope evidence"
        );

        let (source, mut target) = pair();
        target.fields.push(Field::new(
            "extra",
            "localized_description",
            DataType::Unknown,
        ));
        let constraints = MatchConstraints {
            forbidden: vec![FieldPair::new("s_desc", "extra")],
            ..Default::default()
        };
        let report = engine
            .match_schemas_with_constraints(&source, &target, &constraints)
            .unwrap();
        assert!(
            !uses_context(&report),
            "review must not manufacture uniqueness"
        );
    }
}

#[test]
fn reordering_privacy_and_caller_reserved_description_targets_are_preserved() {
    for one_to_one in [false, true] {
        let (mut source, mut target) = pair();
        let engine = MatchEngine::new(config(one_to_one)).unwrap();
        let before = engine.match_schemas(&source, &target).unwrap();
        source.fields.reverse();
        target.fields.reverse();
        assert_eq!(before, engine.match_schemas(&source, &target).unwrap());
        let printed = format!("{before:?}");
        for secret in [
            "EQ-101",
            "EQ-404",
            "Portable pump assembly",
            "Precision valve module",
        ] {
            assert!(!printed.contains(secret));
        }
    }
    let (mut source, target) = pair();
    source
        .fields
        .push(Field::new("z", "opaque", DataType::Text));
    let constraints = MatchConstraints {
        confirmed: vec![FieldPair::new("z", "t_desc")],
        ..Default::default()
    };
    let report = MatchEngine::new(config(true))
        .unwrap()
        .match_schemas_with_constraints(&source, &target, &constraints)
        .unwrap();
    assert_eq!(selected(&report, "z").unwrap().target.0, "t_desc");
    assert!(selected(&report, "s_desc").is_none());
}
