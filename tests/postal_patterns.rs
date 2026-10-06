//! Contracts for scoped, sample-backed numeric postal-code spellings.
use fieldkin::{
    Config, ContextualEvidence, Corroboration, DataType, Field, FieldPair, MatchConstraints,
    MatchEngine, SampleValue, Schema, SemanticHints,
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
            .map(|v| SampleValue::Text((*v).into()))
            .collect(),
    )
}
fn pair() -> (Schema, Schema) {
    (
        Schema::new(vec![text(
            "s",
            "billing_postal_code",
            &["01234", "23456", "34567"],
        )]),
        Schema::new(vec![text("t", "bill_zip", &["45678", "56789", "67890"])]),
    )
}
fn selected(source: &Schema, target: &Schema, one_to_one: bool) -> bool {
    MatchEngine::new(config(one_to_one))
        .unwrap()
        .match_schemas(source, target)
        .unwrap()
        .fields
        .iter()
        .any(|f| f.selected.is_some())
}

#[test]
fn full_address_role_and_sample_format_match_in_both_directions_and_modes() {
    for (a, b) in [
        ("billing_postal_code", "bill_zip"),
        ("shipping_postcode", "ship_zip_code"),
        ("registered_office_postal_code", "registered_office_zipcode"),
    ] {
        let (mut source, mut target) = pair();
        source.fields[0].name = a.into();
        target.fields[0].name = b.into();
        for one_to_one in [false, true] {
            for (s, t) in [(&source, &target), (&target, &source)] {
                let before = (s.clone(), t.clone());
                let report = MatchEngine::new(config(one_to_one))
                    .unwrap()
                    .match_schemas(s, t)
                    .unwrap();
                let proposal = report.fields[0]
                    .selected
                    .as_ref()
                    .expect("scoped postal pattern");
                assert_eq!(proposal.target, t.fields[0].id);
                assert!(proposal
                    .warnings
                    .iter()
                    .any(|w| w.contains("scoped postal-code spelling")));
                assert_eq!(before, (s.clone(), t.clone()));
            }
        }
    }
}

#[test]
fn representation_widths_hyphens_and_leading_zeroes_are_not_converted() {
    let (source, mut target) = pair();
    for values in [
        ["4567", "5678", "6789"],
        ["45678-1234", "56789-2345", "67890-3456"],
        ["45 678", "56 789", "67 890"],
        ["A1B2C3", "D4E5F6", "G7H8J9"],
    ] {
        target.fields[0] = text("t", "bill_zip", &values);
        for mode in [false, true] {
            assert!(!selected(&source, &target, mode));
        }
    }
    let source = Schema::new(vec![text(
        "s",
        "billing_postal_code",
        &["01234-5678", "23456-6789", "34567-7890"],
    )]);
    let target = Schema::new(vec![text(
        "t",
        "bill_zip",
        &["45678-8901", "56789-9012", "67890-0123"],
    )]);
    for mode in [false, true] {
        assert!(selected(&source, &target, mode));
    }
}

#[test]
fn address_qualifiers_and_non_postal_code_roles_are_not_erased() {
    let (source, mut target) = pair();
    for name in [
        "shipping_zip",
        "parent_billing_zip",
        "billing_customer_id",
        "billing_postal_name",
    ] {
        target.fields[0].name = name.into();
        for mode in [false, true] {
            assert!(!selected(&source, &target, mode));
        }
    }
    let (mut source, mut target) = pair();
    source.fields[0].name = "postal_code".into();
    target.fields[0].name = "zip".into();
    for mode in [false, true] {
        assert!(!selected(&source, &target, mode));
    }
}

#[test]
fn constants_missing_two_value_sparse_and_mixed_samples_do_not_qualify() {
    let (source, target) = pair();
    for samples in [
        None,
        Some(vec![]),
        Some(vec![SampleValue::Null; 4]),
        Some(vec![SampleValue::Text("01234".into()); 6]),
        Some(vec![
            SampleValue::Text("01234".into()),
            SampleValue::Text("23456".into()),
        ]),
    ] {
        let mut s = source.clone();
        s.fields[0].samples = samples;
        for mode in [false, true] {
            assert!(!selected(&s, &target, mode));
        }
    }
    let mut sparse = source.clone();
    sparse.fields[0]
        .samples
        .as_mut()
        .unwrap()
        .extend(vec![SampleValue::Null; 9]);
    for mode in [false, true] {
        assert!(selected(&sparse, &target, mode));
    }
    sparse.fields[0]
        .samples
        .as_mut()
        .unwrap()
        .push(SampleValue::Null);
    for mode in [false, true] {
        assert!(!selected(&sparse, &target, mode));
    }
    let mut mixed = source;
    mixed.fields[0]
        .samples
        .as_mut()
        .unwrap()
        .push(SampleValue::Integer(12345));
    for mode in [false, true] {
        assert!(!selected(&mixed, &target, mode));
    }
}

#[test]
fn a_missing_unknown_same_role_competitor_still_blocks_new_support() {
    for mode in [false, true] {
        let (source, mut target) = pair();
        target
            .fields
            .push(Field::new("u", "billing_postcode", DataType::Unknown));
        let cfg = Config {
            max_candidates: 1,
            ..config(mode)
        };
        let report = MatchEngine::new(cfg)
            .unwrap()
            .match_schemas(&source, &target)
            .unwrap();
        assert!(report.fields.iter().all(|f| f.selected.is_none()));
        let (mut source, target) = pair();
        source
            .fields
            .push(Field::new("r", "bill_zipcode", DataType::Unknown));
        assert!(!selected(&source, &target, mode));
    }
}

#[test]
fn caller_constraints_corroboration_and_hints_remain_authoritative() {
    let (source, target) = pair();
    for mode in [false, true] {
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
            let report = MatchEngine::new(config(mode))
                .unwrap()
                .match_schemas_with_constraints(&source, &target, &constraints)
                .unwrap();
            assert!(report.fields[0].selected.is_none());
        }
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
        let (mut s, mut t) = pair();
        s.fields[0].hints = SemanticHints {
            identifier_scope: Some("region-a".into()),
            ..Default::default()
        };
        t.fields[0].hints = SemanticHints {
            identifier_scope: Some("region-b".into()),
            ..Default::default()
        };
        assert!(!selected(&s, &t, mode));
    }
}

#[test]
fn existing_target_support_is_not_displaced_and_competitors_survive_exclusion() {
    for mode in [false, true] {
        let (mut source, target) = pair();
        let mut incumbent = target.fields[0].clone();
        incumbent.id = "a".into();
        source.fields.push(incumbent);
        let report = MatchEngine::new(config(mode))
            .unwrap()
            .match_schemas(&source, &target)
            .unwrap();
        assert_eq!(report.fields[0].selected.as_ref().unwrap().target.0, "t");
        assert!(report.fields[1].selected.is_none());
        let (source, mut target) = pair();
        target
            .fields
            .push(Field::new("u", "billing_postcode", DataType::Unknown));
        let constraints = MatchConstraints {
            forbidden: vec![FieldPair::new("s", "u")],
            ..Default::default()
        };
        assert!(MatchEngine::new(config(mode))
            .unwrap()
            .match_schemas_with_constraints(&source, &target, &constraints)
            .unwrap()
            .fields[0]
            .selected
            .is_none());
    }
}

#[test]
fn ordinary_defaults_raw_signals_and_private_samples_are_preserved() {
    let (source, target) = pair();
    let original = MatchEngine::new(Config::default())
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap();
    assert!(original.fields[0].selected.is_none());
    let actual = MatchEngine::new(config(false))
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap();
    assert_eq!(
        actual.fields[0].selected.as_ref().unwrap().signals,
        original.fields[0].candidates[0].signals
    );
    let printed = format!("{actual:?}");
    for value in ["01234", "23456", "34567", "45678", "56789", "67890"] {
        assert!(!printed.contains(value));
    }
}
