//! Runtime contracts for literal-prefix support of weak schema names.
//! Sample spellings are evidence only, never proof that entity scopes agree.

use fieldkin::{
    Config, ContextualEvidence, Corroboration, DataType, Field, FieldPair, MatchConstraints,
    MatchEngine, NameConflictKind, NameConflictRule, SampleValue, Schema, SemanticHints,
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
            .map(|value| SampleValue::Text((*value).to_owned()))
            .collect(),
    )
}

fn pair() -> (Schema, Schema) {
    (
        Schema::new(vec![text("s", "q7", &["ND-010", "ND-011", "ND-012"])]),
        Schema::new(vec![text(
            "t",
            "record_key",
            &["ND-810", "ND-811", "ND-812"],
        )]),
    )
}

#[test]
fn literal_prefix_supports_weak_names_in_both_modes_without_changing_defaults() {
    let (source, target) = pair();
    let ordinary = MatchEngine::new(Config::default())
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap();
    assert!(ordinary.fields[0].selected.is_none());
    for mode in [false, true] {
        let report = MatchEngine::new(config(mode))
            .unwrap()
            .match_schemas(&source, &target)
            .unwrap();
        let selected = report.fields[0]
            .selected
            .as_ref()
            .expect("literal-prefix support");
        assert_eq!(selected.target.0, "t");
        assert_eq!(selected.signals, ordinary.fields[0].candidates[0].signals);
        assert!(selected
            .signals
            .iter()
            .any(|s| s.name == "name" && s.evidence.score.is_some_and(|score| score < 0.4)));
        assert!(selected
            .warnings
            .iter()
            .any(|w| w.contains("exact literal code prefix")));
        assert!(!format!("{report:?}").contains("ND-"));
    }
}

#[test]
fn matching_shape_alone_does_not_admit_weak_names() {
    let (source, _) = pair();
    for values in [
        ["XX-810", "XX-811", "XX-812"],
        ["ND-810", "ND-811", "XX-812"],
        ["nd-810", "nd-811", "nd-812"],
        ["ND_810", "ND_811", "ND_812"],
    ] {
        let target = Schema::new(vec![text("t", "record_key", &values)]);
        for mode in [false, true] {
            let report = MatchEngine::new(config(mode))
                .unwrap()
                .match_schemas(&source, &target)
                .unwrap();
            assert!(report.fields[0].selected.is_none(), "{values:?}");
        }
    }
}

#[test]
fn weak_name_prefix_support_retains_reliability_and_sample_requirements() {
    let (source, target) = pair();
    let mut sparse = source.clone();
    sparse.fields[0]
        .samples
        .as_mut()
        .unwrap()
        .extend(vec![SampleValue::Null; 10]);
    let mut mixed = source.clone();
    mixed.fields[0]
        .samples
        .as_mut()
        .unwrap()
        .push(SampleValue::Boolean(true));
    let mut absent = source.clone();
    absent.fields[0].samples = None;
    let mut non_text = source.clone();
    non_text.fields[0].data_type = DataType::Unknown;
    for input in [
        Schema::new(vec![text("s", "q7", &["ND-010"; 4])]),
        Schema::new(vec![text("s", "q7", &["ND-010", "ND-011", "ND-010"])]),
        sparse,
        mixed,
        absent,
        non_text,
    ] {
        for mode in [false, true] {
            assert!(MatchEngine::new(config(mode))
                .unwrap()
                .match_schemas(&input, &target)
                .unwrap()
                .fields[0]
                .selected
                .is_none());
        }
    }
    let mut quarter = source;
    quarter.fields[0]
        .samples
        .as_mut()
        .unwrap()
        .extend(vec![SampleValue::Null; 9]);
    for mode in [false, true] {
        assert!(MatchEngine::new(config(mode))
            .unwrap()
            .match_schemas(&quarter, &target)
            .unwrap()
            .fields[0]
            .selected
            .is_some());
    }
}

#[test]
fn prefix_mismatched_competitor_remains_visible_even_when_forbidden() {
    let (source, mut target) = pair();
    // Same shape/name, different literal prefix. It must still count in margins.
    target
        .fields
        .push(text("u", "record_key", &["XX-810", "XX-811", "XX-812"]));
    for mode in [false, true] {
        for max_candidates in [1, 5] {
            for constraints in [
                MatchConstraints::default(),
                MatchConstraints {
                    forbidden: vec![FieldPair::new("s", "u")],
                    ..Default::default()
                },
            ] {
                let report = MatchEngine::new(Config {
                    max_candidates,
                    ..config(mode)
                })
                .unwrap()
                .match_schemas_with_constraints(&source, &target, &constraints)
                .unwrap();
                assert!(report.fields[0].selected.is_none());
            }
        }
    }
}

#[test]
fn prefix_support_does_not_take_existing_targets_or_hide_source_twins() {
    let (source, target) = pair();
    let mut claimed = source.clone();
    let mut incumbent = target.fields[0].clone();
    incumbent.id = "a".into();
    claimed.fields.push(incumbent);
    let mut twins = source;
    let mut twin = twins.fields[0].clone();
    twin.id = "r".into();
    twins.fields.push(twin);
    for mode in [false, true] {
        let engine = MatchEngine::new(config(mode)).unwrap();
        let report = engine.match_schemas(&claimed, &target).unwrap();
        let incumbent = report.fields.iter().find(|f| f.source.0 == "a").unwrap();
        let newcomer = report.fields.iter().find(|f| f.source.0 == "s").unwrap();
        assert_eq!(incumbent.selected.as_ref().unwrap().target.0, "t");
        assert!(newcomer.selected.is_none());
        assert!(engine
            .match_schemas(&twins, &target)
            .unwrap()
            .fields
            .iter()
            .all(|f| f.selected.is_none()));
    }
}

#[test]
fn prefix_support_still_respects_caller_gates_hints_and_name_conflicts() {
    let (source, target) = pair();
    for mode in [false, true] {
        for cfg in [
            Config {
                min_score: 0.99,
                ..config(mode)
            },
            Config {
                corroboration: Some(Corroboration::default()),
                ..config(mode)
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
            assert!(MatchEngine::new(config(mode))
                .unwrap()
                .match_schemas_with_constraints(&source, &target, &constraints)
                .unwrap()
                .fields[0]
                .selected
                .is_none());
        }
        let mut hinted_source = source.clone();
        let mut hinted_target = target.clone();
        hinted_source.fields[0].hints = SemanticHints {
            identifier_scope: Some("departments".into()),
            ..Default::default()
        };
        hinted_target.fields[0].hints = SemanticHints {
            identifier_scope: Some("depots".into()),
            ..Default::default()
        };
        assert!(MatchEngine::new(config(mode))
            .unwrap()
            .match_schemas(&hinted_source, &hinted_target)
            .unwrap()
            .fields[0]
            .selected
            .is_none());

        let mut named_source = source.clone();
        let mut named_target = target.clone();
        named_source.fields[0].name = "q7_gross".into();
        named_target.fields[0].name = "record_key_net".into();
        let cfg = Config {
            name_conflicts: vec![NameConflictRule {
                kind: NameConflictKind::Qualifier,
                alternatives: vec![vec!["gross".into()], vec!["net".into()]],
            }],
            ..config(mode)
        };
        assert!(MatchEngine::new(cfg)
            .unwrap()
            .match_schemas(&named_source, &named_target)
            .unwrap()
            .fields[0]
            .selected
            .is_none());
    }
}
