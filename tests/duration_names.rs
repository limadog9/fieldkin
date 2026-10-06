//! Contracts for explicit, integral warranty/retention duration names.
use fieldkin::{
    Config, ContextualEvidence, Corroboration, DataType, Decision, Field, FieldPair,
    MatchConstraints, MatchEngine, SampleValue, Schema, SemanticHints,
};

fn config(one_to_one: bool) -> Config {
    Config {
        one_to_one,
        contextual_evidence: Some(ContextualEvidence::default()),
        ..Config::default()
    }
}

fn field(id: &str, name: &str, values: Option<&[i128]>) -> Field {
    let mut result = Field::new(id, name, DataType::Integer);
    result.samples = values.map(|v| v.iter().copied().map(SampleValue::Integer).collect());
    result
}

fn pair(a: &str, b: &str, sampled: bool) -> (Schema, Schema) {
    let a_values: Option<&[i128]> = sampled.then_some(&[12, 24, 36]);
    let b_values: Option<&[i128]> = sampled.then_some(&[48, 60, 72]);
    (
        Schema::new(vec![field("s", a, a_values)]),
        Schema::new(vec![field("t", b, b_values)]),
    )
}

fn check(s: &Schema, t: &Schema, one: bool) -> fieldkin::MatchReport {
    MatchEngine::new(config(one))
        .unwrap()
        .match_schemas(s, t)
        .unwrap()
}

#[test]
fn complete_duration_roles_work_with_disjoint_or_missing_observations_in_both_directions() {
    for (a, b) in [
        ("warranty_months", "warranty_period_months"),
        ("backup_retention_days", "backup_retention_period_days"),
        ("extended_warranty_years", "extended_warranty_period_years"),
        (
            "document_retention_weeks",
            "document_retention_period_weeks",
        ),
    ] {
        for (a, b) in [(a, b), (b, a)] {
            for one in [false, true] {
                for samples in [false, true] {
                    let (s, t) = pair(a, b, samples);
                    let r = check(&s, &t, one);
                    let chosen = r.fields[0].selected.as_ref().expect("duration proposal");
                    assert_eq!(chosen.target.0, "t", "{a} -> {b}");
                    assert!(chosen
                        .warnings
                        .iter()
                        .any(|w| w.starts_with("Explicit measurement-name support:")));
                }
            }
        }
    }
}

#[test]
fn units_entity_qualifiers_and_period_roles_are_not_erased() {
    for (a, b) in [
        ("warranty_months", "warranty_period_years"),
        ("warranty", "warranty_period_months"),
        ("estimated_warranty_months", "actual_warranty_period_months"),
        ("minimum_retention_days", "maximum_retention_period_days"),
        ("customer_retention_days", "archive_retention_period_days"),
        ("reporting_months", "reporting_period_months"),
        ("period_months", "months"),
        ("warranty_periods_months", "warranty_months"),
        ("warranty_id_months", "warranty_id_period_months"),
    ] {
        for one in [false, true] {
            for samples in [false, true] {
                let (s, t) = pair(a, b, samples);
                assert!(
                    check(&s, &t, one).fields[0].selected.is_none(),
                    "{a} -> {b}"
                );
            }
        }
    }
}

#[test]
fn only_integral_nonnegative_duration_representations_get_new_support() {
    for kind in [
        DataType::Text,
        DataType::Unknown,
        DataType::Float,
        DataType::Decimal,
        DataType::Date,
        DataType::Timestamp,
    ] {
        let (mut s, mut t) = pair("warranty_months", "warranty_period_months", true);
        s.fields[0].data_type = kind;
        t.fields[0].data_type = kind;
        assert!(
            check(&s, &t, false).fields[0].selected.is_none(),
            "{kind:?}"
        );
    }
    for invalid in [
        SampleValue::Integer(-1),
        SampleValue::Number(1.5),
        SampleValue::Text("12".into()),
        SampleValue::Boolean(true),
    ] {
        let (mut s, t) = pair("warranty_months", "warranty_period_months", true);
        s.fields[0].samples.as_mut().unwrap().push(invalid);
        assert!(check(&s, &t, false).fields[0].selected.is_none());
    }
    let (mut s, mut t) = pair("warranty_months", "warranty_period_months", false);
    s.fields[0].samples = Some(vec![SampleValue::Number(12.0), SampleValue::Null]);
    t.fields[0].samples = Some(vec![SampleValue::Number(24.0), SampleValue::Null]);
    assert!(check(&s, &t, false).fields[0].selected.is_some());
    t.fields[0].data_type = DataType::Float;
    assert!(check(&s, &t, false).fields[0].selected.is_none());
}

#[test]
fn duplicate_expansions_stay_ambiguous_before_display_truncation() {
    for one in [false, true] {
        let (s, mut t) = pair("warranty_months", "warranty_period_months", false);
        let mut twin = t.fields[0].clone();
        twin.id = "u".into();
        t.fields.push(twin);
        let r = MatchEngine::new(Config {
            max_candidates: 1,
            ..config(one)
        })
        .unwrap()
        .match_schemas(&s, &t)
        .unwrap();
        assert!(r.fields[0].selected.is_none());
        assert_eq!(r.fields[0].decision, Decision::Ambiguous);
        assert_eq!(r.fields[0].alternatives.len(), 2);
    }
}

#[test]
fn original_target_claims_and_existing_choices_are_not_displaced() {
    for one in [false, true] {
        let (mut s, t) = pair("warranty_months", "warranty_period_months", true);
        let mut incumbent = t.fields[0].clone();
        incumbent.id = "a".into();
        s.fields.push(incumbent);
        let r = check(&s, &t, one);
        assert_eq!(r.fields[0].source.0, "a");
        assert_eq!(r.fields[0].selected.as_ref().unwrap().target.0, "t");
        assert!(r.fields[1].selected.is_none());
        let (s, mut t) = pair("warranty_months", "warranty_period_months", true);
        let mut incumbent = s.fields[0].clone();
        incumbent.id = "u".into();
        t.fields.push(incumbent);
        assert_eq!(
            check(&s, &t, one).fields[0]
                .selected
                .as_ref()
                .unwrap()
                .target
                .0,
            "u"
        );
    }
}

#[test]
fn configured_gates_and_explicit_hint_conflicts_remain_in_force() {
    let (s, t) = pair("warranty_months", "warranty_period_months", true);
    let ordinary = MatchEngine::new(Config::default())
        .unwrap()
        .match_schemas(&s, &t)
        .unwrap();
    let contextual = check(&s, &t, false);
    assert_eq!(
        ordinary.fields[0].candidates[0].signals,
        contextual.fields[0].selected.as_ref().unwrap().signals
    );
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
            .match_schemas(&s, &t)
            .unwrap()
            .fields[0]
            .selected
            .is_none());
    }
    let (mut s, mut t) = pair("warranty_months", "warranty_period_months", true);
    s.fields[0].hints = SemanticHints {
        unit: Some("months".into()),
        ..Default::default()
    };
    t.fields[0].hints = SemanticHints {
        unit: Some("years".into()),
        ..Default::default()
    };
    assert!(check(&s, &t, false).fields[0].selected.is_none());
}

#[test]
fn review_constraints_assignment_and_reordering_remain_owned_by_the_selector() {
    for one in [false, true] {
        let (mut s, mut t) = pair("warranty_months", "warranty_period_months", false);
        let engine = MatchEngine::new(config(one)).unwrap();
        for c in [
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
                .match_schemas_with_constraints(&s, &t, &c)
                .unwrap()
                .fields[0]
                .selected
                .is_none());
        }
        s.fields.push(field("r", "backup_retention_days", None));
        t.fields
            .push(field("u", "backup_retention_period_days", None));
        let original = engine.match_schemas(&s, &t).unwrap();
        assert_eq!(
            original
                .fields
                .iter()
                .filter(|f| f.selected.is_some())
                .count(),
            2
        );
        s.fields.reverse();
        t.fields.reverse();
        assert_eq!(original, engine.match_schemas(&s, &t).unwrap());
    }
}
