//! Stage 3 evidence contracts, including numerical exactness and semantic vetoes.

use std::collections::BTreeMap;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use fieldkin::{
    Config, DataType, Evidence, ExactDecimal, Field, MatchEngine, Matcher, NameMatcher,
    SampleMatcher, SampleReliability, SampleValue, Schema, SemanticHints, WeightedMatcher,
};
use proptest::prelude::*;

fn field(id: &str) -> Field {
    Field::new(id, "shared_name", DataType::Unknown)
}

fn sampled(samples: Vec<SampleValue>) -> Field {
    field("sampled").with_samples(samples)
}

fn integers(values: &[i128]) -> Field {
    sampled(values.iter().copied().map(SampleValue::Integer).collect())
}

fn decimal(coefficient: i128, scale: u32) -> SampleValue {
    SampleValue::Decimal(ExactDecimal::new(coefficient, scale).unwrap())
}

fn sample_score(source: &Field, target: &Field, reliability: SampleReliability) -> Option<f64> {
    SampleMatcher {
        reliability,
        ..SampleMatcher::default()
    }
    .evaluate(source, target)
    .unwrap()
    .score
}

fn one_hint(axis: usize, label: Option<&str>) -> SemanticHints {
    let label = label.map(str::to_owned);
    match axis {
        0 => SemanticHints {
            unit: label,
            ..SemanticHints::default()
        },
        1 => SemanticHints {
            currency: label,
            ..SemanticHints::default()
        },
        _ => SemanticHints {
            identifier_scope: label,
            ..SemanticHints::default()
        },
    }
}

fn name_engine(one_to_one: bool) -> MatchEngine {
    MatchEngine::with_matchers(
        Config {
            one_to_one,
            reject_incompatible_types: false,
            ..Config::default()
        },
        vec![WeightedMatcher::new(1.0, NameMatcher::default())],
    )
    .unwrap()
}

#[test]
fn distinct_support_is_the_default_and_repeated_constants_have_zero_support() {
    assert_eq!(
        SampleMatcher::default().reliability,
        SampleReliability::Distinct
    );
    for count in [3, 8, 128] {
        let constant = sampled(vec![SampleValue::Integer(73); count]);
        assert_eq!(
            sample_score(&constant, &constant, SampleReliability::Distinct),
            Some(0.0)
        );
        assert_eq!(
            sample_score(&constant, &constant, SampleReliability::Legacy),
            Some(1.0)
        );
    }
}

#[test]
fn two_distinct_values_receive_half_support_and_three_receive_full_support() {
    let booleans = sampled(vec![
        SampleValue::Boolean(false),
        SampleValue::Boolean(true),
        SampleValue::Boolean(true),
        SampleValue::Boolean(false),
    ]);
    assert_eq!(
        sample_score(&booleans, &booleans, SampleReliability::Distinct),
        Some(0.5)
    );
    let three = integers(&[1, 2, 3]);
    assert_eq!(
        sample_score(&three, &three, SampleReliability::Distinct),
        Some(1.0)
    );
    let two = integers(&[1, 2, 2]);
    // Jaccard 2/3, multiplied by the lower side's distinct support 1/2.
    assert!(
        (sample_score(&two, &three, SampleReliability::Distinct).unwrap() - 1.0 / 3.0).abs()
            < 1e-12
    );
}

#[test]
fn null_coverage_and_insufficient_observations_remain_separate_from_distinct_support() {
    let full = integers(&[1, 2, 3]);
    let mut sparse = full.clone();
    sparse
        .samples
        .as_mut()
        .unwrap()
        .extend(vec![SampleValue::Null; 27]);
    let evidence = SampleMatcher::default().evaluate(&full, &sparse).unwrap();
    assert_eq!(evidence.score, Some(0.1));
    assert!(evidence.explanation.contains("3/30"));
    assert!(evidence
        .explanation
        .contains("source 3 and target 3 distinct"));
    for absent in [
        field("missing"),
        sampled(vec![]),
        sampled(vec![SampleValue::Null; 20]),
        integers(&[1, 2]),
    ] {
        assert_eq!(
            sample_score(&absent, &full, SampleReliability::Distinct),
            None
        );
    }
}

#[test]
fn exact_integer_neighbors_above_float_precision_are_not_collapsed() {
    let base = 9_007_199_254_740_992_i128;
    let source = integers(&[base, base + 1, base + 2]);
    let target = integers(&[base + 1, base + 2, base + 3]);
    assert_eq!(
        sample_score(&source, &target, SampleReliability::Distinct),
        Some(0.5)
    );
    let boundaries = integers(&[i128::MIN, 0, i128::MAX]);
    assert_eq!(
        sample_score(&boundaries, &boundaries, SampleReliability::Distinct),
        Some(1.0)
    );
}

#[test]
fn decimal_scale_canonicalization_preserves_exact_neighbor_differences() {
    let source = sampled(vec![decimal(10, 1), decimal(200, 2), decimal(3000, 3)]);
    let target = sampled(vec![decimal(1, 0), decimal(2, 0), decimal(3, 0)]);
    assert_eq!(
        sample_score(&source, &target, SampleReliability::Distinct),
        Some(1.0)
    );
    let nearby = sampled(vec![decimal(1, 38), decimal(2, 38), decimal(3, 38)]);
    let shifted = sampled(vec![decimal(2, 38), decimal(3, 38), decimal(4, 38)]);
    assert_eq!(
        sample_score(&nearby, &shifted, SampleReliability::Distinct),
        Some(0.5)
    );
    let zeros = sampled(vec![decimal(0, 0), decimal(0, 18), decimal(0, 38)]);
    assert_eq!(
        sample_score(&zeros, &zeros, SampleReliability::Distinct),
        Some(0.0)
    );
}

#[test]
fn exact_overlap_does_not_coerce_sample_kinds_or_text_formats() {
    let groups = [
        integers(&[0, 1, 2]),
        sampled(vec![
            SampleValue::Number(0.0),
            SampleValue::Number(1.0),
            SampleValue::Number(2.0),
        ]),
        sampled(vec![decimal(0, 0), decimal(1, 0), decimal(2, 0)]),
        sampled(vec![
            SampleValue::Text("0".into()),
            SampleValue::Text("1".into()),
            SampleValue::Text("2".into()),
        ]),
        sampled(vec![
            SampleValue::Boolean(false),
            SampleValue::Boolean(true),
            SampleValue::Boolean(false),
        ]),
    ];
    for (source_index, source) in groups.iter().enumerate() {
        for (target_index, target) in groups.iter().enumerate() {
            assert_eq!(
                sample_score(source, target, SampleReliability::Legacy),
                Some(f64::from(source_index == target_index))
            );
        }
    }
    let padded = sampled(vec![
        SampleValue::Text(" 0 ".into()),
        SampleValue::Text(" 1 ".into()),
        SampleValue::Text(" 2 ".into()),
    ]);
    assert_eq!(
        sample_score(&groups[3], &padded, SampleReliability::Distinct),
        Some(0.0)
    );
}

#[test]
fn unsigned_conversion_is_checked_and_does_not_echo_overflow_values() {
    assert_eq!(
        SampleValue::from_unsigned(0).unwrap(),
        SampleValue::Integer(0)
    );
    assert_eq!(
        SampleValue::from_unsigned(i128::MAX as u128).unwrap(),
        SampleValue::Integer(i128::MAX)
    );
    for invalid in [i128::MAX as u128 + 1, u128::MAX] {
        let error = SampleValue::from_unsigned(invalid).unwrap_err();
        assert_eq!(
            error.0,
            "unsigned sample exceeds the exact signed integer range"
        );
        assert!(!format!("{error:?}").contains(&invalid.to_string()));
    }
}

#[test]
fn every_semantic_axis_vetoes_even_name_only_matching_with_type_veto_disabled() {
    for one_to_one in [false, true] {
        for axis in 0..3 {
            let source = Schema::new(vec![Field::new("s", "same", DataType::Text)
                .with_hints(one_hint(axis, Some("verified-source")))]);
            let target = Schema::new(vec![Field::new("t", "same", DataType::Integer)
                .with_hints(one_hint(axis, Some("verified-target")))]);
            let report = name_engine(one_to_one)
                .match_schemas(&source, &target)
                .unwrap();
            let candidate = &report.fields[0].candidates[0];
            assert_eq!(candidate.score, 1.0);
            assert!(!candidate.eligible);
            assert!(report.fields[0].selected.is_none());
            assert!(candidate
                .warnings
                .iter()
                .any(|warning| warning.contains("hints conflict")));
            assert_eq!(report.unmatched_sources.len(), 1);
            assert_eq!(report.unmatched_targets.len(), 1);
        }
    }
}

#[test]
fn equal_and_missing_hints_do_not_add_score_or_infer_agreement() {
    for one_to_one in [false, true] {
        for axis in 0..3 {
            for target_hint in [None, Some("verified")] {
                let source =
                    Schema::new(vec![field("s").with_hints(one_hint(axis, Some("verified")))]);
                let target = Schema::new(vec![field("t").with_hints(one_hint(axis, target_hint))]);
                let report = name_engine(one_to_one)
                    .match_schemas(&source, &target)
                    .unwrap();
                let selected = report.fields[0].selected.as_ref().unwrap();
                assert_eq!(selected.score, 1.0);
                let expected = if target_hint.is_some() {
                    "hints agree"
                } else {
                    "no agreement inferred"
                };
                assert!(selected
                    .warnings
                    .iter()
                    .any(|warning| warning.contains(expected)));
            }
            // No lexical evidence remains no evidence even with equal hints.
            let source =
                Schema::new(vec![Field::new("s", "", DataType::Unknown)
                    .with_hints(one_hint(axis, Some("verified")))]);
            let target =
                Schema::new(vec![Field::new("t", "", DataType::Unknown)
                    .with_hints(one_hint(axis, Some("verified")))]);
            let report = name_engine(one_to_one)
                .match_schemas(&source, &target)
                .unwrap();
            assert_eq!(report.fields[0].candidates[0].score, 0.0);
            assert!(report.fields[0].selected.is_none());
        }
    }
}

#[test]
fn semantic_labels_compare_exactly_and_independent_axes_cannot_cancel_conflicts() {
    let source = Schema::new(vec![field("s").with_hints(SemanticHints {
        unit: Some("kg".into()),
        currency: Some("USD".into()),
        identifier_scope: Some("tenant:one".into()),
    })]);
    let target = Schema::new(vec![field("t").with_hints(SemanticHints {
        unit: Some("kg".into()),
        currency: Some("usd".into()),
        identifier_scope: Some("tenant:one".into()),
    })]);
    let report = name_engine(false).match_schemas(&source, &target).unwrap();
    assert!(report.fields[0].selected.is_none());
    let warnings = &report.fields[0].candidates[0].warnings;
    assert_eq!(
        warnings
            .iter()
            .filter(|warning| warning.contains("hints agree"))
            .count(),
        2
    );
    assert_eq!(
        warnings
            .iter()
            .filter(|warning| warning.contains("hints conflict"))
            .count(),
        1
    );
}

#[test]
fn hint_disambiguation_and_assignment_are_invariant_to_schema_order() {
    let mut source = Schema::new(vec![
        field("s-a").with_hints(one_hint(2, Some("scope-a"))),
        field("s-b").with_hints(one_hint(2, Some("scope-b"))),
    ]);
    let mut target = Schema::new(vec![
        field("t-a").with_hints(one_hint(2, Some("scope-a"))),
        field("t-b").with_hints(one_hint(2, Some("scope-b"))),
    ]);
    for one_to_one in [false, true] {
        let engine = name_engine(one_to_one);
        let expected = engine.match_schemas(&source, &target).unwrap();
        assert_eq!(
            expected.fields[0].selected.as_ref().unwrap().target.0,
            "t-a"
        );
        assert_eq!(
            expected.fields[1].selected.as_ref().unwrap().target.0,
            "t-b"
        );
        source.fields.reverse();
        target.fields.reverse();
        assert_eq!(expected, engine.match_schemas(&source, &target).unwrap());
    }
}

struct CountingMatcher(Arc<AtomicUsize>);

impl Matcher for CountingMatcher {
    fn name(&self) -> &str {
        "counting"
    }
    fn evaluate(&self, _: &Field, _: &Field) -> Result<Evidence, String> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(Evidence {
            score: Some(1.0),
            explanation: "Custom agreement".into(),
        })
    }
}

#[test]
fn invalid_hints_fail_before_custom_matchers_and_valid_byte_boundaries_pass() {
    let calls = Arc::new(AtomicUsize::new(0));
    let engine = MatchEngine::with_matchers(
        Config::default(),
        vec![WeightedMatcher::new(1.0, CountingMatcher(calls.clone()))],
    )
    .unwrap();
    for axis in 0..3 {
        for invalid in [
            "".to_owned(),
            " leading".into(),
            "trailing\t".into(),
            "\u{2003}secret".into(),
            "s".repeat(129),
            "é".repeat(65),
        ] {
            let source = Schema::new(vec![field("s").with_hints(one_hint(axis, Some(&invalid)))]);
            let error = engine
                .match_schemas(&source, &Schema::new(vec![field("t")]))
                .unwrap_err();
            assert_eq!(
                error.0,
                "semantic hints must be nonempty, trimmed labels of at most 128 bytes"
            );
        }
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    for valid in [
        "s".repeat(128),
        "é".repeat(64),
        "application defined scope".into(),
    ] {
        let source = Schema::new(vec![field("s").with_hints(one_hint(2, Some(&valid)))]);
        assert!(engine
            .match_schemas(&source, &Schema::new(vec![field("t")]))
            .is_ok());
    }
    assert_eq!(calls.load(Ordering::SeqCst), 3);
}

#[test]
fn semantic_veto_also_applies_to_custom_matchers() {
    let calls = Arc::new(AtomicUsize::new(0));
    let engine = MatchEngine::with_matchers(
        Config::default(),
        vec![WeightedMatcher::new(1.0, CountingMatcher(calls))],
    )
    .unwrap();
    let source = Schema::new(vec![field("s").with_hints(one_hint(0, Some("m")))]);
    let target = Schema::new(vec![field("t").with_hints(one_hint(0, Some("ft")))]);
    let report = engine.match_schemas(&source, &target).unwrap();
    assert_eq!(report.fields[0].candidates[0].score, 1.0);
    assert!(!report.fields[0].candidates[0].eligible);
    assert!(report.fields[0].selected.is_none());
}

#[test]
fn debug_reports_and_errors_do_not_expose_sample_or_hint_values() {
    let secret = "private-sample-and-scope";
    let sensitive = field("public-id")
        .with_samples(vec![
            SampleValue::Text(secret.into()),
            SampleValue::Integer(9_007_199_254_740_993),
            decimal(12_345_678_901, 3),
        ])
        .with_hints(SemanticHints {
            unit: Some(secret.into()),
            currency: Some(secret.into()),
            identifier_scope: Some(secret.into()),
        });
    let debug = format!("{sensitive:?}");
    for raw in [secret, "9007199254740993", "12345678901"] {
        assert!(!debug.contains(raw));
    }
    let mut target = sensitive.clone();
    target.id = "target-public-id".into();
    let report = MatchEngine::new(Config::default())
        .unwrap()
        .match_schemas(&Schema::new(vec![sensitive]), &Schema::new(vec![target]))
        .unwrap();
    let rendered = format!("{report:?}");
    for raw in [secret, "9007199254740993", "12345678901"] {
        assert!(!rendered.contains(raw));
    }
    let invalid = field("s").with_hints(one_hint(2, Some(&format!(" {secret}"))));
    let error = name_engine(false)
        .match_schemas(&Schema::new(vec![invalid]), &Schema::new(vec![field("t")]))
        .unwrap_err();
    assert!(!format!("{error:?}").contains(secret));
}

#[test]
fn alias_builder_checks_both_sides_and_reports_applied_aliases() {
    for invalid in [
        "".to_owned(),
        "two tokens".into(),
        "UPPER".into(),
        "not_normal".into(),
        "a".repeat(257),
    ] {
        assert!(NameMatcher::default()
            .with_alias(&invalid, "target")
            .is_err());
        assert!(NameMatcher::default()
            .with_alias("source", &invalid)
            .is_err());
    }
    let matcher = NameMatcher::default().with_alias("sku", "product").unwrap();
    let source = Field::new("s", "sku_code", DataType::Text);
    let target = Field::new("t", "product_code", DataType::Text);
    let evidence = matcher.evaluate(&source, &target).unwrap();
    assert_eq!(evidence.score, Some(1.0));
    assert!(evidence
        .explanation
        .contains("Applied token aliases: sku -> product"));
    assert!(!evidence.explanation.contains("amt -> amount"));
    assert!(NameMatcher::default()
        .with_alias("a".repeat(256), "b".repeat(256))
        .is_ok());
}

#[test]
fn direct_alias_maps_validate_used_targets_and_ignore_unused_entries_without_recursion() {
    let matcher = NameMatcher {
        aliases: BTreeMap::from([
            ("used".into(), "invalid target".into()),
            ("Unused_Key".into(), "invalid target".into()),
        ]),
    };
    assert!(matcher
        .evaluate(&Field::new("s", "used", DataType::Text), &field("t"))
        .is_err());
    assert_eq!(
        matcher.evaluate(&field("s"), &field("t")).unwrap().score,
        Some(1.0)
    );
    let chained = NameMatcher::default()
        .with_alias("sku", "product")
        .unwrap()
        .with_alias("product", "item")
        .unwrap();
    let evidence = chained
        .evaluate(
            &Field::new("s", "sku", DataType::Text),
            &Field::new("t", "product", DataType::Text),
        )
        .unwrap();
    assert!(evidence.score.unwrap() < 1.0);
    assert!(evidence.explanation.contains("sku -> product"));
    assert!(evidence.explanation.contains("product -> item"));
}

#[test]
fn applied_alias_details_are_bounded_even_when_long_aliases_compress_names() {
    let mut matcher = NameMatcher::default();
    let mut source_tokens = Vec::new();
    let mut target_tokens = Vec::new();
    for index in 0..16_u8 {
        let short = char::from(b'a' + index).to_string();
        let long = format!("{}{short}", "x".repeat(255));
        matcher = matcher.with_alias(&long, &short).unwrap();
        if index < 8 {
            source_tokens.push(long);
        } else {
            target_tokens.push(long);
        }
    }
    let evidence = matcher
        .evaluate(
            &Field::new("s", source_tokens.join("_"), DataType::Text),
            &Field::new("t", target_tokens.join("_"), DataType::Text),
        )
        .unwrap();
    assert!(evidence.explanation.len() < 4096);
    assert!(evidence
        .explanation
        .contains("further alias details omitted"));
}

proptest! {
    #[test]
    fn exact_overlap_stays_bounded_symmetric_and_order_invariant(
        source in prop::collection::vec(prop::option::of(-10_i128..10), 0..40),
        target in prop::collection::vec(prop::option::of(-10_i128..10), 0..40),
    ) {
        let convert = |values: Vec<Option<i128>>| sampled(values.into_iter().map(|value| value.map_or(SampleValue::Null, SampleValue::Integer)).collect());
        let mut source = convert(source);
        let mut target = convert(target);
        for reliability in [SampleReliability::Legacy, SampleReliability::Distinct] {
            let original = sample_score(&source, &target, reliability);
            prop_assert_eq!(original, sample_score(&target, &source, reliability));
            if let Some(score) = original {
                prop_assert!(score.is_finite() && (0.0..=1.0).contains(&score));
            }
            source.samples.as_mut().unwrap().reverse();
            target.samples.as_mut().unwrap().reverse();
            prop_assert_eq!(original, sample_score(&source, &target, reliability));
        }
    }
}
