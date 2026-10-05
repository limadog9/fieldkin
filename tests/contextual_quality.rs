//! Behavioral contracts for the explicitly named contextual quality candidate.

use fieldkin::{
    CandidateIssue, Config, ContextualEvidence, ContextualReason, DataType, Decision, Field,
    FieldPair, MatchConstraints, MatchEngine, NameMatcher, SampleMatcher, SampleValue, Schema,
    TypeMatcher, WeightedMatcher,
};

fn field(id: &str, name: &str) -> Field {
    Field::new(id, name, DataType::Unknown)
}

fn sampled(id: &str, name: &str, values: &[i128]) -> Field {
    field(id, name).with_samples(values.iter().copied().map(SampleValue::Integer).collect())
}

fn report(source: Vec<Field>, target: Vec<Field>, config: Config) -> fieldkin::MatchReport {
    MatchEngine::new(config)
        .unwrap()
        .match_schemas(&Schema::new(source), &Schema::new(target))
        .unwrap()
}

fn quality(one_to_one: bool) -> Config {
    Config {
        one_to_one,
        ..Config::contextual_quality()
    }
}

#[test]
fn named_candidate_is_explicit_and_preserves_existing_policy_defaults() {
    assert!(Config::default().contextual_evidence.is_none());
    let old = ContextualEvidence::default();
    assert!(old.strict_identifier_samples && old.scoped_support);
    assert!(!old.distinguish_relationships);
    assert!(!old.independent_sample_populations);
    assert!(!old.preserve_score_ranking);
    assert!(!old.identifier_word_forms);
    let named = Config::contextual_quality();
    let new = named.contextual_evidence.unwrap();
    assert!(new.distinguish_relationships);
    assert!(new.independent_sample_populations);
    assert!(!new.preserve_score_ranking);
    assert!(new.identifier_word_forms);
    assert_eq!(named.name_conflicts.len(), 10);
}

#[test]
fn actual_role_conflicts_do_not_destroy_either_axis_of_sample_contrast() {
    for one_to_one in [false, true] {
        for reordered in [false, true] {
            let mut targets = vec![
                sampled("right", "customer_id", &[10, 20, 30]),
                sampled("wrong", "order_id", &[10, 20, 30]),
            ];
            if reordered {
                targets.reverse();
            }
            let matched = report(
                vec![sampled("s", "customer_id", &[10, 20, 30])],
                targets,
                quality(one_to_one),
            );
            assert_eq!(
                matched.fields[0].selected.as_ref().unwrap().target.0,
                "right"
            );
            let wrong = matched.fields[0]
                .candidates
                .iter()
                .find(|candidate| candidate.target.0 == "wrong")
                .unwrap();
            assert!(wrong.issues.contains(&CandidateIssue::ContextualReason(
                ContextualReason::Contradiction
            )));

            let mut sources = vec![
                sampled("a", "customer_id", &[10, 20, 30]),
                sampled("b", "order_id", &[10, 20, 30]),
            ];
            if reordered {
                sources.reverse();
            }
            let matched = report(
                sources,
                vec![sampled("right", "customer_id", &[10, 20, 30])],
                quality(one_to_one),
            );
            assert!(matched
                .fields
                .iter()
                .find(|source| source.source.0 == "a")
                .unwrap()
                .selected
                .is_some());
            assert!(matched
                .fields
                .iter()
                .find(|source| source.source.0 == "b")
                .unwrap()
                .selected
                .is_none());
        }
    }
}

#[test]
fn missing_qualifiers_units_and_direction_remain_competitors_on_both_axes() {
    for one_to_one in [false, true] {
        for max_candidates in [1, 5] {
            for (plain, qualified) in [
                ("account_id", "external_account_id"),
                ("account_id", "parent_account_id"),
                ("account_id", "billing_account_id"),
                ("account_id", "estimated_account_id"),
                ("account_id", "from_account_id"),
                ("account_id_1", "account_id"),
                ("warehouse_id_kg", "warehouse_id"),
            ] {
                let config = Config {
                    max_candidates,
                    ..quality(one_to_one)
                };
                let matched = report(
                    vec![sampled("s", plain, &[10, 20, 30])],
                    vec![
                        sampled("a", plain, &[10, 20, 30]),
                        sampled("b", qualified, &[10, 20, 30]),
                    ],
                    config.clone(),
                );
                assert!(
                    matched.fields[0].selected.is_none(),
                    "row: {plain} vs {qualified}"
                );
                let exact = matched.fields[0]
                    .candidates
                    .iter()
                    .find(|candidate| candidate.target.0 == "a")
                    .unwrap();
                assert!(exact.issues.contains(&CandidateIssue::ContextualReason(
                    ContextualReason::CompetingCandidate
                )));
                if max_candidates > 1 {
                    let alternative = matched.fields[0]
                        .candidates
                        .iter()
                        .find(|candidate| candidate.target.0 == "b")
                        .unwrap();
                    assert!(
                        alternative
                            .issues
                            .contains(&CandidateIssue::ContextualReason(
                                ContextualReason::UnresolvedRelationship
                            )),
                        "{plain} vs {qualified}: {:?}",
                        alternative.issues
                    );
                    assert!(!alternative
                        .issues
                        .contains(&CandidateIssue::ContextualReason(
                            ContextualReason::Contradiction
                        )));
                }
                let matched = report(
                    vec![
                        sampled("a", plain, &[10, 20, 30]),
                        sampled("b", qualified, &[10, 20, 30]),
                    ],
                    vec![sampled("t", plain, &[10, 20, 30])],
                    config,
                );
                assert!(
                    matched
                        .fields
                        .iter()
                        .all(|source| source.selected.is_none()),
                    "column: {plain} vs {qualified}"
                );
            }
        }
        for (qualified, opposed) in [
            ("external_account_id", "internal_account_id"),
            ("estimated_account_id", "actual_account_id"),
            (
                "from_customer_to_supplier_id",
                "from_supplier_to_customer_id",
            ),
            ("from_account_id", "to_account_id"),
            ("warehouse_id_kg", "warehouse_id_lb"),
        ] {
            let matched = report(
                vec![sampled("s", qualified, &[10, 20, 30])],
                vec![
                    sampled("a", qualified, &[10, 20, 30]),
                    sampled("b", opposed, &[10, 20, 30]),
                ],
                quality(one_to_one),
            );
            assert_eq!(matched.fields[0].selected.as_ref().unwrap().target.0, "a");
        }
    }
}

#[test]
fn unavailable_disjoint_and_partial_populations_support_informative_equivalent_names() {
    for one_to_one in [false, true] {
        for values in [
            None,
            Some(vec![]),
            Some(vec![SampleValue::Null; 10]),
            Some(vec![
                SampleValue::Integer(40),
                SampleValue::Integer(50),
                SampleValue::Integer(60),
            ]),
            Some(vec![
                SampleValue::Integer(20),
                SampleValue::Integer(30),
                SampleValue::Integer(40),
            ]),
        ] {
            let mut target = field("t", "customer_key");
            target.samples = values;
            let matched = report(
                vec![sampled("s", "customer_id", &[10, 20, 30])],
                vec![target],
                quality(one_to_one),
            );
            let selected = matched.fields[0].selected.as_ref().unwrap();
            assert!(selected.issues.contains(&CandidateIssue::ContextualReason(
                ContextualReason::SupportedEquivalence
            )));
        }
        let matched = report(
            vec![field("s", "customer_identifier")],
            vec![field("t", "customer_key")],
            quality(one_to_one),
        );
        assert_eq!(matched.fields[0].decision, Decision::Proposed);
    }
}

#[test]
fn unresolved_names_do_not_gain_meaning_from_a_corroborative_identifier_shape() {
    for one_to_one in [false, true] {
        // Identifier role and distinctive observations corroborate shape, but
        // do not establish equivalence between unknown business nouns.
        let source = vec![sampled("s", "employee_code", &[10, 20, 30])];
        let target = vec![sampled("t", "worker_id", &[10, 20, 30])];
        let matched = report(source.clone(), target.clone(), quality(one_to_one));
        assert!(matched.fields[0].selected.is_none());
        let matched = report(
            source,
            target,
            Config {
                contextual_evidence: Some(ContextualEvidence {
                    identifier_word_forms: false,
                    ..Config::contextual_quality().contextual_evidence.unwrap()
                }),
                ..quality(one_to_one)
            },
        );
        assert!(matched.fields[0].selected.is_none());
        let matched = report(
            vec![field("s", "employee_code")],
            vec![field("t", "worker_id")],
            quality(one_to_one),
        );
        assert!(matched.fields[0].selected.is_none());
        let matched = report(
            vec![sampled("s", "employee_code", &[10, 20, 30])],
            vec![sampled("t", "worker_id", &[10, 30, 40])],
            quality(one_to_one),
        );
        assert!(matched.fields[0].selected.is_none());
        let issues = &matched.fields[0].candidates[0].issues;
        assert!(issues.contains(&CandidateIssue::ContextualReason(
            ContextualReason::UnresolvedRelationship
        )));
        assert!(!issues.contains(&CandidateIssue::ContextualReason(
            ContextualReason::Contradiction
        )));
        assert!(!issues.contains(&CandidateIssue::ContextualReason(
            ContextualReason::SupportedEquivalence
        )));
    }
}

#[test]
fn constant_samples_do_not_open_the_independent_population_path() {
    for one_to_one in [false, true] {
        for name in ["warehouse", "customer_memo", "package_identifier"] {
            let matched = report(
                vec![field("s", name).with_samples(vec![SampleValue::Text("UNKNOWN".into()); 8])],
                vec![field("t", name).with_samples(vec![SampleValue::Text("UNKNOWN".into()); 8])],
                quality(one_to_one),
            );
            assert!(matched.fields[0].selected.is_none(), "{name}");
        }
    }
}

#[test]
fn unrelated_informative_identifiers_cannot_share_meaning_from_copied_values() {
    for one_to_one in [false, true] {
        let matched = report(
            vec![sampled("s", "warehouse_code", &[10, 20, 30])],
            vec![sampled("t", "paint_id", &[10, 20, 30])],
            quality(one_to_one),
        );
        assert!(matched.fields[0].selected.is_none());
    }
}

#[test]
fn opaque_competitors_survive_support_failure_top_k_and_caller_exclusion() {
    for one_to_one in [false, true] {
        let source = Schema::new(vec![sampled("s", "customer_id", &[10, 20, 30])]);
        let target = Schema::new(vec![
            sampled("a", "customer_key", &[10, 20, 30]),
            sampled("b", "opaque", &[10, 20, 30]),
        ]);
        for max_candidates in [1, 5] {
            let matched = MatchEngine::new(Config {
                max_candidates,
                ..quality(one_to_one)
            })
            .unwrap()
            .match_schemas_with_constraints(
                &source,
                &target,
                &MatchConstraints {
                    forbidden: vec![FieldPair::new("s", "b")],
                    ..Default::default()
                },
            )
            .unwrap();
            assert!(matched.fields[0].selected.is_none());
            assert!(matched.fields[0].candidates[0].issues.contains(
                &CandidateIssue::ContextualReason(ContextualReason::CompetingCandidate)
            ));
        }
        let matched = report(
            vec![
                sampled("a", "customer_id", &[10, 20, 30]),
                sampled("b", "opaque", &[10, 20, 30]),
            ],
            vec![sampled("t", "customer_key", &[10, 20, 30])],
            quality(one_to_one),
        );
        assert!(matched
            .fields
            .iter()
            .all(|source| source.selected.is_none()));
    }
}

#[test]
fn informative_names_do_not_erase_scope_events_polarity_units_or_representation() {
    for one_to_one in [false, true] {
        for (source, target) in [
            ("customer_id", "supplier_id"),
            ("account_id", "external_account_id"),
            ("parent_account_id", "account_id"),
            ("billing_account_id", "shipping_account_id"),
            ("estimated_total", "actual_total"),
            ("estimated_total", "total"),
            ("not_charge_amount", "charge_amount"),
            (
                "from_customer_to_supplier_id",
                "from_supplier_to_customer_id",
            ),
            ("shipment_created_at", "shipment_updated_at"),
            ("shipment_received_at", "shipment_dispatched_at"),
            ("pressure_kpa", "pressure_pa"),
            ("payment_usd", "payment_eur"),
        ] {
            let matched = report(
                vec![sampled("s", source, &[10, 20, 30])],
                vec![sampled("t", target, &[10, 20, 30])],
                quality(one_to_one),
            );
            assert!(matched.fields[0].selected.is_none(), "{source} -> {target}");
        }
        let matched = report(
            vec![Field::new("s", "shipment_received_at", DataType::Date)],
            vec![Field::new("t", "shipment_received_at", DataType::Timestamp)],
            quality(one_to_one),
        );
        assert!(matched.fields[0].selected.is_none());
        let matched = report(
            vec![sampled("s", "customer_id", &[10, 20, 30])],
            vec![field("t", "customer_id").with_samples(vec![
                SampleValue::Text("a".into()),
                SampleValue::Text("b".into()),
                SampleValue::Text("c".into()),
            ])],
            quality(one_to_one),
        );
        assert!(matched.fields[0].selected.is_none());
    }
}

#[test]
fn observed_temporal_representations_remain_distinct_when_declared_types_are_unknown() {
    for one_to_one in [false, true] {
        let values = |dates: &[&str]| {
            dates
                .iter()
                .map(|value| SampleValue::Text((*value).to_owned()))
                .collect::<Vec<_>>()
        };
        let source = field("s", "shipment_created_at").with_samples(values(&[
            "2026-01-01",
            "2026-01-02",
            "2026-01-03",
        ]));
        let disjoint_dates = field("t", "shipment_created_date").with_samples(values(&[
            "2026-02-01",
            "2026-02-02",
            "2026-02-03",
        ]));
        let timestamps = field("t", "shipment_created_timestamp").with_samples(values(&[
            "2026-02-01T12:00:00Z",
            "2026-02-02T12:00:00Z",
            "2026-02-03T12:00:00Z",
        ]));
        let matched = report(
            vec![source.clone()],
            vec![disjoint_dates],
            quality(one_to_one),
        );
        assert!(matched.fields[0].selected.is_some());
        let matched = report(vec![source], vec![timestamps], quality(one_to_one));
        assert!(matched.fields[0].selected.is_none());
        assert!(matched.fields[0].candidates[0].issues.contains(
            &CandidateIssue::ContextualReason(ContextualReason::Contradiction)
        ));
    }
}

#[test]
fn generic_and_opaque_names_with_missing_or_constant_samples_still_abstain() {
    for one_to_one in [false, true] {
        for name in [
            "id",
            "name",
            "status",
            "value",
            "field_1",
            "A1_Code",
            "opaque_code",
            "unknown_id",
            "record_id",
            "entity_key",
            "metric_code",
            "x9_key",
        ] {
            for samples in [
                None,
                Some(vec![SampleValue::Integer(10); 4]),
                Some(vec![
                    SampleValue::Integer(10),
                    SampleValue::Integer(20),
                    SampleValue::Integer(30),
                ]),
            ] {
                let mut source = field("s", name);
                let mut target = field("t", name);
                source.samples = samples.clone();
                target.samples = samples;
                let matched = report(vec![source], vec![target], quality(one_to_one));
                assert!(matched.fields[0].selected.is_none(), "{name}");
            }
        }
    }
}

#[test]
fn short_informative_roles_keep_supported_sample_correspondences() {
    for one_to_one in [false, true] {
        for name in ["tax_code", "zip_identifier", "age_key"] {
            let matched = report(
                vec![sampled("s", name, &[10, 20, 30])],
                vec![sampled("t", name, &[10, 20, 30])],
                quality(one_to_one),
            );
            assert!(matched.fields[0].selected.is_some(), "{name}");
        }
    }
}

#[test]
fn known_integral_count_roles_are_distinct_from_opaque_identifier_names() {
    for one_to_one in [false, true] {
        for name in ["event_count", "record_quantity", "measurement_total"] {
            for samples in [
                None,
                Some(vec![
                    SampleValue::Integer(10),
                    SampleValue::Integer(20),
                    SampleValue::Integer(30),
                ]),
            ] {
                let mut source = Field::new("s", name, DataType::Integer);
                let mut target = Field::new("t", name, DataType::Integer);
                source.samples = samples.clone();
                target.samples = samples;
                let matched = report(vec![source], vec![target], quality(one_to_one));
                assert!(matched.fields[0].selected.is_some(), "{name}");
            }
        }
        for (name, data_type, samples) in [
            (
                "event_key",
                DataType::Integer,
                vec![
                    SampleValue::Integer(10),
                    SampleValue::Integer(20),
                    SampleValue::Integer(30),
                ],
            ),
            (
                "record_id",
                DataType::Integer,
                vec![
                    SampleValue::Integer(10),
                    SampleValue::Integer(20),
                    SampleValue::Integer(30),
                ],
            ),
            (
                "opaque_count",
                DataType::Integer,
                vec![
                    SampleValue::Integer(10),
                    SampleValue::Integer(20),
                    SampleValue::Integer(30),
                ],
            ),
            (
                "count",
                DataType::Integer,
                vec![
                    SampleValue::Integer(10),
                    SampleValue::Integer(20),
                    SampleValue::Integer(30),
                ],
            ),
            (
                "event_count",
                DataType::Unknown,
                vec![
                    SampleValue::Integer(10),
                    SampleValue::Integer(20),
                    SampleValue::Integer(30),
                ],
            ),
            (
                "event_count",
                DataType::Integer,
                vec![
                    SampleValue::Number(1.5),
                    SampleValue::Number(2.5),
                    SampleValue::Number(3.5),
                ],
            ),
        ] {
            let source = Field::new("s", name, data_type).with_samples(samples.clone());
            let target = Field::new("t", name, data_type).with_samples(samples);
            let matched = report(vec![source], vec![target], quality(one_to_one));
            assert!(matched.fields[0].selected.is_none(), "{name} {data_type:?}");
        }
    }
}

#[test]
fn duplicates_remain_ambiguous_and_copied_sources_remain_assignment_competitors() {
    for one_to_one in [false, true] {
        let matched = report(
            vec![field("s", "customer_id")],
            vec![field("a", "customer_key"), field("b", "customer_key")],
            quality(one_to_one),
        );
        assert!(matched.fields[0].selected.is_none());
        let matched = report(
            vec![field("a", "customer_id"), field("b", "customer_id")],
            vec![field("t", "customer_key")],
            quality(one_to_one),
        );
        assert_eq!(
            matched
                .fields
                .iter()
                .filter(|source| source.selected.is_some())
                .count(),
            if one_to_one { 1 } else { 2 }
        );
    }
}

#[test]
fn syntactic_preferences_do_not_resolve_equivalent_boolean_roles() {
    for one_to_one in [false, true] {
        for preserve_score_ranking in [false, true] {
            let matched = report(
                vec![Field::new("s", "is_payment_settled", DataType::Boolean)],
                vec![
                    Field::new("a", "is_payment_settled", DataType::Boolean),
                    Field::new("b", "payment_settlement_flag", DataType::Boolean),
                ],
                Config {
                    contextual_evidence: Some(ContextualEvidence {
                        preserve_score_ranking,
                        ..Config::contextual_quality().contextual_evidence.unwrap()
                    }),
                    max_candidates: 1,
                    ..quality(one_to_one)
                },
            );
            assert!(matched.fields[0].selected.is_none());
        }
    }
}

#[test]
fn custom_weights_thresholds_and_margins_cannot_manufacture_semantic_uniqueness() {
    let source = Schema::new(vec![Field::new(
        "s",
        "is_payment_settled",
        DataType::Boolean,
    )]);
    let target = Schema::new(vec![
        Field::new("a", "is_payment_settled", DataType::Boolean),
        Field::new("b", "payment_settlement_flag", DataType::Boolean),
    ]);
    for one_to_one in [false, true] {
        for min_score in [0.70, 0.98] {
            for preserve_score_ranking in [false, true] {
                let config = Config {
                    min_score,
                    ambiguity_margin: 0.0,
                    max_candidates: 1,
                    contextual_evidence: Some(ContextualEvidence {
                        preserve_score_ranking,
                        ..Config::contextual_quality().contextual_evidence.unwrap()
                    }),
                    ..quality(one_to_one)
                };
                let engine = MatchEngine::with_matchers(
                    config,
                    vec![
                        WeightedMatcher::new(0.98, NameMatcher::default()),
                        WeightedMatcher::new(0.01, TypeMatcher),
                        WeightedMatcher::new(0.01, SampleMatcher::default()),
                    ],
                )
                .unwrap();
                let matched = engine.match_schemas(&source, &target).unwrap();
                assert!(matched.fields[0].selected.is_none());
                let matched = engine
                    .match_schemas_with_constraints(
                        &source,
                        &target,
                        &MatchConstraints {
                            forbidden: vec![FieldPair::new("s", "b")],
                            ..Default::default()
                        },
                    )
                    .unwrap();
                assert!(matched.fields[0].selected.is_none());
                let matched = engine
                    .match_schemas_with_constraints(
                        &source,
                        &target,
                        &MatchConstraints {
                            confirmed: vec![FieldPair::new("s", "a")],
                            ..Default::default()
                        },
                    )
                    .unwrap();
                assert_eq!(matched.fields[0].decision, Decision::Confirmed);
            }
        }
    }
}

#[test]
fn paired_identifier_inflections_preserve_scope_and_unresolved_business_nouns() {
    for one_to_one in [false, true] {
        for (source, target) in [
            ("warehouses_code", "warehouse_id"),
            ("projects_ref", "project_key"),
            ("departments_number", "department_identifier"),
        ] {
            let matched = report(
                vec![field("s", source)],
                vec![field("t", target)],
                quality(one_to_one),
            );
            assert!(matched.fields[0].selected.is_some(), "{source} -> {target}");
        }
        for (source, target) in [
            ("warehouses_code", "paint_id"),
            ("external_warehouses_code", "warehouse_id"),
            ("warehouse_1_code", "warehouses_2_id"),
            ("business_id", "busines_id"),
            ("analysis_code", "analysi_id"),
            ("status_code", "statu_id"),
        ] {
            let matched = report(
                vec![sampled("s", source, &[10, 20, 30])],
                vec![sampled("t", target, &[10, 20, 30])],
                quality(one_to_one),
            );
            assert!(matched.fields[0].selected.is_none(), "{source} -> {target}");
        }
        let matched = report(
            vec![field("s", "warehouse_id")],
            vec![field("a", "warehouse_id"), field("b", "warehouses_code")],
            Config {
                max_candidates: 1,
                ..quality(one_to_one)
            },
        );
        assert!(matched.fields[0].selected.is_none());
    }
}

#[cfg(feature = "json")]
#[test]
fn structured_contextual_reasons_survive_privacy_preserving_json() {
    let matched = report(
        vec![sampled("s", "employee_code", &[10, 20, 30])],
        vec![sampled("t", "worker_id", &[10, 20, 30])],
        quality(false),
    );
    let wire: serde_json::Value = serde_json::from_slice(
        &fieldkin::json::report_to_json(&matched, &Default::default()).unwrap(),
    )
    .unwrap();
    let reasons = wire["report"]["fields"][0]["candidates"][0]["issues"]
        .as_array()
        .unwrap();
    assert!(reasons.contains(&serde_json::json!({"code": "contextual_unresolved_relationship"})));
    assert!(wire["report"]["fields"][0]["candidates"][0]["warnings"].is_null());
}
