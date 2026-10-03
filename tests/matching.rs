//! Synthetic end-to-end contracts and property tests for the public API.

use std::collections::BTreeSet;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use fieldkin::{
    Config, DataType, Decision, Evidence, Field, FieldId, MatchEngine, Matcher, SampleValue,
    Schema, WeightedMatcher,
};
use proptest::prelude::*;

fn engine() -> MatchEngine {
    MatchEngine::new(Config::default()).unwrap()
}

fn field(id: &str, name: &str, data_type: DataType) -> Field {
    Field::new(id, name, data_type)
}

fn numbers() -> Vec<SampleValue> {
    vec![1.0, 2.0, 3.0]
        .into_iter()
        .map(SampleValue::Number)
        .collect()
}

#[test]
fn renamed_and_reordered_columns_match_by_stable_identity() {
    let source = Schema::new(vec![
        field("s-date", "TransDate", DataType::Date),
        field("s-name", "CustomerName", DataType::Text),
    ]);
    let target = Schema::new(vec![
        field("t-name", "customer_name", DataType::Text),
        field("t-date", "transaction_date", DataType::Date),
    ]);
    let report = engine().match_schemas(&source, &target).unwrap();
    assert_eq!(report.fields[0].source, FieldId::from("s-date"));
    assert_eq!(
        report.fields[0].selected.as_ref().unwrap().target,
        FieldId::from("t-date")
    );
    assert_eq!(
        report.fields[1].selected.as_ref().unwrap().target,
        FieldId::from("t-name")
    );
    let mut reversed_source = source;
    let mut reversed_target = target;
    reversed_source.fields.reverse();
    reversed_target.fields.reverse();
    assert_eq!(
        report,
        engine()
            .match_schemas(&reversed_source, &reversed_target)
            .unwrap()
    );
}

#[test]
fn duplicate_names_are_valid_when_ids_are_distinct() {
    let source = Schema::new(vec![
        field("s-number", "value", DataType::Integer),
        field("s-text", "value", DataType::Text),
    ]);
    let target = Schema::new(vec![
        field("t-text", "value", DataType::Text),
        field("t-number", "value", DataType::Integer),
    ]);
    let report = engine().match_schemas(&source, &target).unwrap();
    assert_eq!(report.fields.len(), 2);
    assert_eq!(
        report.fields[0].selected.as_ref().unwrap().target,
        FieldId::from("t-number")
    );
    assert_eq!(
        report.fields[1].selected.as_ref().unwrap().target,
        FieldId::from("t-text")
    );
}

#[test]
fn duplicate_or_empty_ids_return_validation_errors() {
    let duplicate = Schema::new(vec![
        field("same", "one", DataType::Text),
        field("same", "two", DataType::Text),
    ]);
    let empty_id = Schema::new(vec![field("", "one", DataType::Text)]);
    for schema in [duplicate, empty_id] {
        let error = engine()
            .match_schemas(&schema, &Schema::default())
            .unwrap_err();
        assert!(error.to_string().contains("unique"));
    }
}

#[test]
fn type_veto_rejects_a_misleading_exact_name_above_threshold() {
    let source = Schema::new(vec![field("s", "active", DataType::Boolean)]);
    let target = Schema::new(vec![field("t", "active", DataType::Integer)]);
    let config = Config {
        min_score: 0.5,
        ..Config::default()
    };
    let report = MatchEngine::new(config)
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap();
    let candidate = &report.fields[0].candidates[0];
    assert!(candidate.score > 0.5);
    assert!(!candidate.eligible);
    assert!(candidate
        .warnings
        .iter()
        .any(|w| w.contains("incompatible")));
    assert_eq!(report.fields[0].decision, Decision::BelowThreshold);
}

#[test]
fn type_and_sample_evidence_can_overcome_a_misleading_exact_name() {
    let source = Schema::new(vec![
        field("s", "account", DataType::Integer).with_samples(numbers())
    ]);
    let target = Schema::new(vec![
        field("wrong", "account", DataType::Boolean),
        field("right", "account_id", DataType::Integer).with_samples(numbers()),
    ]);
    let report = engine().match_schemas(&source, &target).unwrap();
    assert_eq!(
        report.fields[0].selected.as_ref().unwrap().target,
        FieldId::from("right")
    );
}

#[test]
fn unknown_types_and_absent_samples_do_not_renormalize_name_weight() {
    let source = Schema::new(vec![field("s", "value", DataType::Unknown)]);
    let target = Schema::new(vec![field("t", "value", DataType::Unknown)]);
    let report = engine().match_schemas(&source, &target).unwrap();
    let candidate = &report.fields[0].candidates[0];
    assert!((candidate.score - 0.65).abs() < 1e-12);
    assert!(report.fields[0].selected.is_none());
    assert_eq!(
        candidate
            .signals
            .iter()
            .filter(|signal| signal.evidence.score.is_none())
            .count(),
        2
    );
}

#[test]
fn empty_missing_and_insufficient_samples_remain_absent_evidence() {
    let target = Schema::new(vec![
        field("t", "value", DataType::Integer).with_samples(numbers())
    ]);
    for samples in [
        None,
        Some(vec![]),
        Some(vec![SampleValue::Null; 10]),
        Some(vec![SampleValue::Number(1.0); 2]),
    ] {
        let mut source_field = field("s", "value", DataType::Integer);
        source_field.samples = samples;
        let report = engine()
            .match_schemas(&Schema::new(vec![source_field]), &target)
            .unwrap();
        let candidate = &report.fields[0].candidates[0];
        assert!((candidate.score - 0.85).abs() < 1e-12);
        assert_eq!(
            candidate
                .signals
                .iter()
                .find(|s| s.name == "samples")
                .unwrap()
                .evidence
                .score,
            None
        );
    }
}

#[test]
fn null_heavy_samples_attenuate_evidence_and_do_not_expose_values() {
    let secret = "synthetic-private-value-123";
    let values = vec![SampleValue::Text(secret.to_owned()); 3];
    let source = Schema::new(vec![
        field("s", "value", DataType::Text).with_samples(values.clone())
    ]);
    let mut sparse = vec![SampleValue::Null; 27];
    sparse.extend(values);
    let target = Schema::new(vec![
        field("t", "value", DataType::Text).with_samples(sparse)
    ]);
    let report = engine().match_schemas(&source, &target).unwrap();
    let candidate = &report.fields[0].candidates[0];
    assert!((candidate.score - 0.865).abs() < 1e-12);
    assert!(!format!("{report:?}").contains(secret));
    assert!(!format!("{source:?}").contains(secret));
}

#[test]
fn gross_amount_is_not_assumed_to_be_the_requested_amount() {
    let source = Schema::new(vec![field("s", "GrossAmt", DataType::Decimal)]);
    let target = Schema::new(vec![field("t", "amount", DataType::Decimal)]);
    let report = engine().match_schemas(&source, &target).unwrap();
    assert_eq!(report.fields[0].decision, Decision::BelowThreshold);
    assert!(report.fields[0].selected.is_none());
}

#[test]
fn unrelated_schemas_leave_both_sides_unmatched() {
    let source = Schema::new(vec![field("s", "humidity", DataType::Float)]);
    let target = Schema::new(vec![field("t", "document", DataType::Binary)]);
    let report = engine().match_schemas(&source, &target).unwrap();
    assert_eq!(report.unmatched_sources, vec![FieldId::from("s")]);
    assert_eq!(report.unmatched_targets, vec![FieldId::from("t")]);
}

#[test]
fn ambiguity_uses_all_pairs_before_top_k_truncation() {
    let source = Schema::new(vec![field("s", "address", DataType::Text)]);
    let target = Schema::new(vec![
        field("z", "address", DataType::Text),
        field("a", "address", DataType::Text),
    ]);
    let report = MatchEngine::new(Config {
        max_candidates: 1,
        one_to_one: true,
        ..Config::default()
    })
    .unwrap()
    .match_schemas(&source, &target)
    .unwrap();
    let matched = &report.fields[0];
    assert_eq!(matched.decision, Decision::Ambiguous);
    assert!(matched.selected.is_none());
    assert_eq!(matched.candidates.len(), 1);
    assert_eq!(matched.candidates[0].target, FieldId::from("a"));
    assert_eq!(
        matched.alternatives,
        vec![FieldId::from("a"), FieldId::from("z")]
    );
}

#[test]
fn unequal_sizes_and_competing_sources_permit_unmatched_fields() {
    let source = Schema::new(vec![
        field("z", "value", DataType::Text),
        field("a", "value", DataType::Text),
        field("b", "other", DataType::Binary),
    ]);
    let target = Schema::new(vec![field("t", "value", DataType::Text)]);
    let report = MatchEngine::new(Config {
        one_to_one: true,
        ..Config::default()
    })
    .unwrap()
    .match_schemas(&source, &target)
    .unwrap();
    assert_eq!(report.fields[0].source, FieldId::from("a"));
    let selected = report.fields[0].selected.as_ref().unwrap();
    assert_eq!(selected.target, FieldId::from("t"));
    assert!(selected.warnings.iter().any(|w| w.contains("competing")));
    assert_eq!(report.fields[2].decision, Decision::AssignmentConflict);
    assert_eq!(
        report.unmatched_sources,
        vec![FieldId::from("b"), FieldId::from("z")]
    );
}

#[test]
fn empty_schemas_are_valid_in_either_assignment_mode() {
    let populated = Schema::new(vec![field("id", "value", DataType::Text)]);
    for one_to_one in [false, true] {
        let engine = MatchEngine::new(Config {
            one_to_one,
            ..Config::default()
        })
        .unwrap();
        let empty_source = engine
            .match_schemas(&Schema::default(), &populated)
            .unwrap();
        assert!(empty_source.fields.is_empty());
        assert_eq!(empty_source.unmatched_targets, vec![FieldId::from("id")]);
        let empty_target = engine
            .match_schemas(&populated, &Schema::default())
            .unwrap();
        assert_eq!(empty_target.unmatched_sources, vec![FieldId::from("id")]);
        assert!(empty_target.fields[0].candidates.is_empty());
    }
}

struct MatrixMatcher;

impl Matcher for MatrixMatcher {
    fn name(&self) -> &str {
        "synthetic-matrix"
    }

    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        let score = match (source.id.0.as_str(), target.id.0.as_str()) {
            ("a", "x") => 0.9,
            ("a", "y") => 0.8,
            ("b", "x") => 0.85,
            _ => 0.1,
        };
        Ok(Evidence {
            score: Some(score),
            explanation: "Synthetic test matrix".into(),
        })
    }
}

#[test]
fn global_assignment_can_select_outside_displayed_top_k() {
    let engine = MatchEngine::with_matchers(
        Config {
            min_score: 0.5,
            ambiguity_margin: 0.01,
            max_candidates: 1,
            one_to_one: true,
            ..Config::default()
        },
        vec![WeightedMatcher::new(1.0, MatrixMatcher)],
    )
    .unwrap();
    let source = Schema::new(vec![
        field("a", "", DataType::Unknown),
        field("b", "", DataType::Unknown),
    ]);
    let target = Schema::new(vec![
        field("x", "", DataType::Unknown),
        field("y", "", DataType::Unknown),
    ]);
    let report = engine.match_schemas(&source, &target).unwrap();
    assert_eq!(report.fields[0].candidates[0].target, FieldId::from("x"));
    assert_eq!(
        report.fields[0].selected.as_ref().unwrap().target,
        FieldId::from("y")
    );
    assert_eq!(
        report.fields[1].selected.as_ref().unwrap().target,
        FieldId::from("x")
    );
}

struct ConstantMatcher(f64);

impl Matcher for ConstantMatcher {
    fn name(&self) -> &str {
        "constant"
    }

    fn evaluate(&self, _: &Field, _: &Field) -> Result<Evidence, String> {
        Ok(Evidence {
            score: Some(self.0),
            explanation: "Synthetic constant".into(),
        })
    }
}

#[test]
fn extension_scores_must_be_finite_and_bounded() {
    let schema = Schema::new(vec![field("id", "value", DataType::Text)]);
    for score in [-0.1, 1.1, f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let engine = MatchEngine::with_matchers(
            Config::default(),
            vec![WeightedMatcher::new(1.0, ConstantMatcher(score))],
        )
        .unwrap();
        assert!(engine.match_schemas(&schema, &schema).is_err());
    }
    let zero = MatchEngine::with_matchers(
        Config {
            min_score: 0.0,
            ..Config::default()
        },
        vec![WeightedMatcher::new(1.0, ConstantMatcher(0.0))],
    )
    .unwrap()
    .match_schemas(&schema, &schema)
    .unwrap();
    assert!(zero.fields[0].selected.is_none());
}

#[test]
fn invalid_configuration_and_matcher_weights_return_errors() {
    for min_score in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
        assert!(MatchEngine::new(Config {
            min_score,
            ..Config::default()
        })
        .is_err());
    }
    for ambiguity_margin in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
        assert!(MatchEngine::new(Config {
            ambiguity_margin,
            ..Config::default()
        })
        .is_err());
    }
    assert!(MatchEngine::new(Config {
        max_candidates: 0,
        ..Config::default()
    })
    .is_err());
    for weight in [-1.0, 0.0, f64::NAN, f64::INFINITY] {
        assert!(MatchEngine::with_matchers(
            Config::default(),
            vec![WeightedMatcher::new(weight, ConstantMatcher(1.0))]
        )
        .is_err());
    }
    assert!(MatchEngine::with_matchers(Config::default(), vec![]).is_err());
    assert!(MatchEngine::with_matchers(
        Config::default(),
        vec![
            WeightedMatcher::new(1.0, ConstantMatcher(1.0)),
            WeightedMatcher::new(1.0, ConstantMatcher(1.0))
        ]
    )
    .is_err());
}

struct CountingMatcher(Arc<AtomicUsize>);

impl Matcher for CountingMatcher {
    fn name(&self) -> &str {
        "counting"
    }

    fn evaluate(&self, _: &Field, _: &Field) -> Result<Evidence, String> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Err("synthetic-secret-from-extension".into())
    }
}

#[test]
fn resource_budgets_are_checked_before_extensions_run() {
    let schema = Schema::new(vec![field("field-id", "value", DataType::Text)
        .with_samples(vec![SampleValue::Text("synthetic-secret".into())])]);
    for budget in 0..7 {
        let calls = Arc::new(AtomicUsize::new(0));
        let mut config = Config::default();
        match budget {
            0 => config.limits.max_fields = 0,
            1 => config.limits.max_pairs = 0,
            2 => config.limits.max_name_bytes = 1,
            3 => config.limits.max_samples_per_field = 0,
            4 => config.limits.max_sample_bytes = 1,
            5 => config.limits.max_total_sample_bytes = 1,
            _ => config.limits.max_signal_evaluations = 0,
        }
        let engine = MatchEngine::with_matchers(
            config,
            vec![WeightedMatcher::new(1.0, CountingMatcher(calls.clone()))],
        )
        .unwrap();
        let error = engine.match_schemas(&schema, &schema).unwrap_err();
        assert_eq!(calls.load(Ordering::Relaxed), 0);
        assert!(!error.to_string().contains("synthetic-secret"));
    }
}

#[test]
fn aggregate_explanation_budget_limits_report_growth() {
    let schema = Schema::new(vec![field("id", "value", DataType::Text)]);
    let mut config = Config::default();
    config.limits.max_explanation_bytes = 1;
    let engine = MatchEngine::with_matchers(
        config,
        vec![WeightedMatcher::new(1.0, ConstantMatcher(1.0))],
    )
    .unwrap();
    let error = engine.match_schemas(&schema, &schema).unwrap_err();
    assert!(error.to_string().contains("explanation"));
}

#[test]
fn extension_errors_and_invalid_samples_do_not_disclose_values() {
    let calls = Arc::new(AtomicUsize::new(0));
    let engine = MatchEngine::with_matchers(
        Config::default(),
        vec![WeightedMatcher::new(1.0, CountingMatcher(calls.clone()))],
    )
    .unwrap();
    let schema = Schema::new(vec![field("id", "value", DataType::Text)]);
    let error = engine.match_schemas(&schema, &schema).unwrap_err();
    assert_eq!(calls.load(Ordering::Relaxed), 1);
    assert!(!error.to_string().contains("synthetic-secret"));
    let nonfinite = Schema::new(vec![
        field("id", "value", DataType::Float).with_samples(vec![SampleValue::Number(f64::NAN)])
    ]);
    assert!(engine.match_schemas(&nonfinite, &schema).is_err());
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(64))]

    #[test]
    fn report_is_reorder_invariant_and_scores_are_bounded(
        source_names in prop::collection::vec("[a-zA-Z_]{0,12}", 0..9),
        target_names in prop::collection::vec("[a-zA-Z_]{0,12}", 0..9),
        one_to_one in any::<bool>(),
    ) {
        let make_schema = |names: &[String]| Schema::new(names.iter().enumerate().map(|(i, name)| {
            Field::new(FieldId(format!("id-{i:02}")), name.clone(), DataType::Text)
        }).collect());
        let mut source = make_schema(&source_names);
        let mut target = make_schema(&target_names);
        let engine = MatchEngine::new(Config {
            min_score: 0.25,
            ambiguity_margin: 0.03,
            abstain_on_ambiguity: false,
            one_to_one,
            ..Config::default()
        }).unwrap();
        let report = engine.match_schemas(&source, &target).unwrap();
        prop_assert_eq!(&report, &engine.match_schemas(&source, &target).unwrap());
        source.fields.reverse();
        target.fields.reverse();
        prop_assert_eq!(&report, &engine.match_schemas(&source, &target).unwrap());
        let mut selected = BTreeSet::new();
        for field in &report.fields {
            for candidate in field.candidates.iter().chain(field.selected.iter()) {
                prop_assert!(candidate.score.is_finite());
                prop_assert!((0.0..=1.0).contains(&candidate.score));
                for signal in &candidate.signals {
                    if let Some(score) = signal.evidence.score {
                        prop_assert!(score.is_finite() && (0.0..=1.0).contains(&score));
                    }
                }
            }
            if let Some(candidate) = &field.selected {
                prop_assert!(candidate.eligible);
                prop_assert!(candidate.score >= 0.25);
                if one_to_one {
                    prop_assert!(selected.insert(candidate.target.clone()));
                }
            }
            for pair in field.candidates.windows(2) {
                prop_assert!(pair[0].score >= pair[1].score);
                if pair[0].score == pair[1].score {
                    prop_assert!(pair[0].target < pair[1].target);
                }
            }
        }
    }
}
