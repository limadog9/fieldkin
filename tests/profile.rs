//! Contracts and failure cases for the optional sample-profile signal.

use fieldkin::{
    Config, DataType, Evidence, ExactDecimal, Field, MatchEngine, Matcher, SampleProfileMatcher,
    SampleValue, Schema, WeightedMatcher,
};
use proptest::prelude::*;

fn field(values: Vec<SampleValue>) -> Field {
    Field::new("field", "unrelated_name", DataType::Unknown).with_samples(values)
}

fn numbers(start: u32, count: u32) -> Field {
    field(
        (start..start + count)
            .map(|value| SampleValue::Number(f64::from(value)))
            .collect(),
    )
}

fn score(source: &Field, target: &Field) -> Option<f64> {
    SampleProfileMatcher::default()
        .evaluate(source, target)
        .unwrap()
        .score
}

#[test]
fn missing_empty_and_insufficient_profiles_are_absent() {
    let enough = numbers(0, 8);
    for source in [
        Field::new("missing", "missing", DataType::Unknown),
        field(vec![]),
        field(vec![SampleValue::Null; 16]),
        numbers(0, 2),
    ] {
        assert_eq!(score(&source, &enough), None);
    }
}

#[test]
fn repetitions_do_not_make_constants_reliable() {
    let matcher = SampleProfileMatcher::default();
    for count in [3, 8, 128] {
        let constant = field(vec![SampleValue::Number(17.0); count]);
        let evidence = matcher.evaluate(&constant, &constant).unwrap();
        assert_eq!(evidence.score, Some(0.0));
        assert!(evidence.explanation.contains("with 1 distinct"));
        assert!(evidence.explanation.contains("distinct reliability 0.000"));
    }
}

#[test]
fn observation_and_distinct_counts_attenuate_small_and_categorical_samples() {
    let small = numbers(0, 3);
    assert!((score(&small, &small).unwrap() - 3.0 / 28.0).abs() < 1e-12);
    let categorical = field(
        (0..80)
            .map(|value| SampleValue::Boolean(value % 2 == 0))
            .collect(),
    );
    assert!((score(&categorical, &categorical).unwrap() - 1.0 / 7.0).abs() < 1e-12);
    let diverse = numbers(0, 8);
    assert_eq!(score(&diverse, &diverse), Some(1.0));
}

#[test]
fn null_coverage_attenuates_even_diverse_profiles() {
    let dense = numbers(0, 8);
    let mut sparse = dense.clone();
    sparse
        .samples
        .as_mut()
        .unwrap()
        .extend(vec![SampleValue::Null; 72]);
    assert_eq!(score(&dense, &sparse), Some(0.1));
}

#[test]
fn runtime_kind_proportions_are_compared_without_casts() {
    let floats = numbers(0, 8);
    let integers = field((0..8).map(SampleValue::Integer).collect());
    assert_eq!(score(&floats, &integers), Some(0.0));
    let mut mixed_values: Vec<_> = (0..4).map(SampleValue::Integer).collect();
    mixed_values.extend((0..4).map(|value| SampleValue::Text(value.to_string())));
    let mixed = field(mixed_values);
    assert_eq!(score(&integers, &mixed), Some(0.5));
}

#[test]
fn text_bins_count_unicode_scalars_and_distinguish_short_from_long() {
    let unicode = field(
        (0..8)
            .map(|index| SampleValue::Text(format!("ééé{index}")))
            .collect(),
    );
    let short = field(
        (0..8)
            .map(|index| SampleValue::Text(format!("abc{index}")))
            .collect(),
    );
    let long = field(
        (0..8)
            .map(|index| SampleValue::Text(format!("long confidential sample {index}")))
            .collect(),
    );
    assert_eq!(score(&unicode, &short), Some(1.0));
    assert_eq!(score(&short, &long), Some(0.0));
}

#[test]
fn similar_profiles_can_fully_agree_on_unrelated_fields() {
    let source = numbers(0, 8);
    let target = numbers(10_000, 8);
    let evidence = SampleProfileMatcher::default()
        .evaluate(&source, &target)
        .unwrap();
    assert_eq!(evidence.score, Some(1.0));
    assert!(evidence
        .explanation
        .contains("do not establish field meaning"));
    // This is an explicit failure case: profile agreement must not be mistaken
    // for a check that numeric values, units, or identifier scopes agree.
}

#[test]
fn floating_point_signed_zero_is_one_distinct_value() {
    let zeroes = field(vec![
        SampleValue::Number(0.0),
        SampleValue::Number(-0.0),
        SampleValue::Number(0.0),
        SampleValue::Number(-0.0),
    ]);
    assert_eq!(score(&zeroes, &zeroes), Some(0.0));
}

#[test]
fn canonical_decimal_values_are_distinct_from_other_numeric_kinds() {
    let same_decimal = field(
        [(1, 0), (10, 1), (100, 2)]
            .into_iter()
            .map(|(coefficient, scale)| {
                SampleValue::Decimal(ExactDecimal::new(coefficient, scale).unwrap())
            })
            .collect(),
    );
    assert_eq!(score(&same_decimal, &same_decimal), Some(0.0));
    let decimals = field(
        (0..8)
            .map(|coefficient| SampleValue::Decimal(ExactDecimal::new(coefficient, 0).unwrap()))
            .collect(),
    );
    assert_eq!(score(&decimals, &decimals), Some(1.0));
    assert_eq!(score(&decimals, &numbers(0, 8)), Some(0.0));
    assert_eq!(
        score(
            &decimals,
            &field((0..8).map(SampleValue::Integer).collect())
        ),
        Some(0.0),
    );
}

#[test]
fn direct_calls_validate_configuration_non_finite_samples_and_input_bounds() {
    let enough = numbers(0, 8);
    for min_non_null in [0, 65_537] {
        assert!(SampleProfileMatcher { min_non_null }
            .evaluate(&enough, &enough)
            .is_err());
    }
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(SampleProfileMatcher::default()
            .evaluate(&field(vec![SampleValue::Number(invalid); 3]), &enough)
            .is_err());
    }
    for oversize in [
        field(vec![SampleValue::Null; 65_537]),
        field(vec![SampleValue::Text("x".repeat(1025)); 3]),
    ] {
        assert!(SampleProfileMatcher::default()
            .evaluate(&oversize, &enough)
            .is_err());
    }
    assert!(SampleProfileMatcher::default()
        .evaluate(
            &field(vec![SampleValue::Text("x".repeat(1024)); 3]),
            &enough
        )
        .is_ok());
}

#[test]
fn explanations_contain_counts_but_no_sample_contents() {
    let private = field(
        (0..8)
            .map(|index| SampleValue::Text(format!("private-secret-{index}")))
            .collect(),
    );
    let evidence = SampleProfileMatcher::default()
        .evaluate(&private, &private)
        .unwrap();
    assert!(!evidence.explanation.contains("private-secret"));
    assert!(evidence
        .explanation
        .contains("8/8 non-null with 8 distinct"));
}

struct DirectProfile;

impl Matcher for DirectProfile {
    fn name(&self) -> &str {
        "sample_profile"
    }

    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        SampleProfileMatcher::default().evaluate(source, target)
    }
}

#[test]
fn cached_and_custom_direct_paths_produce_identical_reports() {
    let source = Schema::new(vec![numbers(0, 8)]);
    let target = Schema::new(vec![numbers(10, 8)]);
    let cached = MatchEngine::with_matchers(
        Config::default(),
        vec![WeightedMatcher::new(1.0, SampleProfileMatcher::default())],
    )
    .unwrap();
    let direct = MatchEngine::with_matchers(
        Config::default(),
        vec![WeightedMatcher::new(1.0, DirectProfile)],
    )
    .unwrap();
    assert_eq!(
        cached.match_schemas(&source, &target).unwrap(),
        direct.match_schemas(&source, &target).unwrap(),
    );
}

proptest! {
    #[test]
    fn profiles_are_bounded_symmetric_and_sample_order_invariant(
        source in prop::collection::vec((0_u8..4, -100_i16..100), 0..40),
        target in prop::collection::vec((0_u8..4, -100_i16..100), 0..40),
    ) {
        let convert = |items: Vec<(u8, i16)>| {
            field(items.into_iter().map(|(kind, value)| match kind {
                0 => SampleValue::Null,
                1 => SampleValue::Integer(i128::from(value)),
                2 => SampleValue::Boolean(value % 2 == 0),
                _ => SampleValue::Text(value.to_string()),
            }).collect())
        };
        let mut source = convert(source);
        let mut target = convert(target);
        let original = score(&source, &target);
        prop_assert_eq!(original, score(&target, &source));
        if let Some(value) = original {
            prop_assert!(value.is_finite() && (0.0..=1.0).contains(&value));
        }
        source.samples.as_mut().unwrap().reverse();
        target.samples.as_mut().unwrap().reverse();
        prop_assert_eq!(original, score(&source, &target));
    }
}
