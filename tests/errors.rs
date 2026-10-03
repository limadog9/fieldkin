//! Public failure categories and privacy-safe error handling.

use std::error::Error;

use fieldkin::{
    BudgetKind, Config, ConfigurationError, DataType, Evidence, ExactDecimal, Field, InputError,
    MatchEngine, MatchError, Matcher, NameMatcher, SampleValue, Schema, SemanticHints,
    WeightedMatcher,
};

fn schema() -> Schema {
    Schema::new(vec![Field::new("id", "value", DataType::Text)
        .with_samples(vec![SampleValue::Text(
            "private-sample-payload".into(),
        )])])
}

struct Extension {
    name: &'static str,
    score: f64,
    explanation_bytes: usize,
    fail: bool,
}

impl Default for Extension {
    fn default() -> Self {
        Self {
            name: "test-extension",
            score: 1.0,
            explanation_bytes: 4,
            fail: false,
        }
    }
}

impl Matcher for Extension {
    fn name(&self) -> &str {
        self.name
    }

    fn evaluate(&self, _: &Field, _: &Field) -> Result<Evidence, String> {
        if self.fail {
            return Err("private-extension-error-payload".into());
        }
        Ok(Evidence {
            score: Some(self.score),
            explanation: "x".repeat(self.explanation_bytes),
        })
    }
}

fn assert_error(error: MatchError, expected: MatchError) {
    assert_eq!(error, expected);
    for rendered in [
        error.to_string(),
        format!("{error:?}"),
        format!("{error:#?}"),
    ] {
        for secret in [
            "private-sample-payload",
            "private-extension-error-payload",
            "private-hint-payload",
            "private-field-id",
        ] {
            assert!(!rendered.contains(secret));
        }
    }
}

#[test]
fn public_constructors_return_typed_input_failures() {
    assert_error(
        ExactDecimal::new(i128::MAX, 39).unwrap_err(),
        MatchError::InvalidInput(InputError::DecimalScale),
    );
    assert_error(
        SampleValue::from_unsigned(u128::MAX).unwrap_err(),
        MatchError::InvalidInput(InputError::UnsignedRange),
    );
    assert_error(
        NameMatcher::default()
            .with_alias("private-sample-payload", "id")
            .unwrap_err(),
        MatchError::InvalidInput(InputError::Alias),
    );

    let engine = MatchEngine::new(Config::default()).unwrap();
    let repeated = Field::new("private-field-id", "field", DataType::Text);
    let invalid_cases = [
        (
            Schema::new(vec![repeated.clone(), repeated]),
            InputError::InvalidFieldIds,
        ),
        (
            Schema::new(vec![Field::new("", "field", DataType::Text)]),
            InputError::InvalidFieldIds,
        ),
        (
            Schema::new(vec![Field::new("id", "field", DataType::Float)
                .with_samples(vec![SampleValue::Number(f64::NEG_INFINITY)])]),
            InputError::NonFiniteSample,
        ),
        (
            Schema::new(vec![Field::new("id", "field", DataType::Text).with_hints(
                SemanticHints {
                    unit: Some(" private-hint-payload ".into()),
                    ..SemanticHints::default()
                },
            )]),
            InputError::SemanticHint,
        ),
    ];
    for (invalid_schema, reason) in invalid_cases {
        // Input validation must still happen when no target pair can be scored.
        assert_error(
            engine
                .match_schemas(&invalid_schema, &Schema::new(vec![]))
                .unwrap_err(),
            MatchError::InvalidInput(reason),
        );
    }
}

#[test]
fn invalid_configuration_is_distinct_from_invalid_input() {
    for (config, reason) in [
        (
            Config {
                min_score: f64::NAN,
                ..Config::default()
            },
            ConfigurationError::ThresholdOrMargin,
        ),
        (
            Config {
                max_candidates: 0,
                ..Config::default()
            },
            ConfigurationError::MaxCandidates,
        ),
    ] {
        assert_error(
            MatchEngine::new(config).err().unwrap(),
            MatchError::InvalidConfiguration(reason),
        );
    }
    for (matchers, reason) in [
        (vec![], ConfigurationError::MatcherCount),
        (
            vec![WeightedMatcher::new(-1.0, Extension::default())],
            ConfigurationError::SignalWeight,
        ),
        (
            vec![WeightedMatcher::new(0.0, Extension::default())],
            ConfigurationError::TotalWeight,
        ),
        (
            vec![
                WeightedMatcher::new(1.0, Extension::default()),
                WeightedMatcher::new(1.0, Extension::default()),
            ],
            ConfigurationError::SignalNames,
        ),
        (
            vec![WeightedMatcher::new(
                1.0,
                Extension {
                    name: "",
                    ..Extension::default()
                },
            )],
            ConfigurationError::SignalNames,
        ),
    ] {
        assert_error(
            MatchEngine::with_matchers(Config::default(), matchers)
                .err()
                .unwrap(),
            MatchError::InvalidConfiguration(reason),
        );
    }
}

#[test]
fn each_resource_rejection_identifies_its_budget() {
    for budget in [
        BudgetKind::Fields,
        BudgetKind::Pairs,
        BudgetKind::SignalEvaluations,
        BudgetKind::NameBytes,
        BudgetKind::SamplesPerField,
        BudgetKind::TextSampleBytes,
        BudgetKind::TotalSampleBytes,
        BudgetKind::ExplanationBytes,
        BudgetKind::SignalExplanation,
    ] {
        let mut config = Config::default();
        let mut extension = Extension::default();
        match budget {
            BudgetKind::Fields => config.limits.max_fields = 0,
            BudgetKind::Pairs => config.limits.max_pairs = 0,
            BudgetKind::SignalEvaluations => config.limits.max_signal_evaluations = 0,
            BudgetKind::NameBytes => config.limits.max_name_bytes = 0,
            BudgetKind::SamplesPerField => config.limits.max_samples_per_field = 0,
            BudgetKind::TextSampleBytes => config.limits.max_sample_bytes = 0,
            BudgetKind::TotalSampleBytes => config.limits.max_total_sample_bytes = 0,
            BudgetKind::ExplanationBytes => config.limits.max_explanation_bytes = 0,
            BudgetKind::SignalExplanation => extension.explanation_bytes = 4097,
            other => panic!("unexpected test budget: {other:?}"),
        }
        let engine =
            MatchEngine::with_matchers(config, vec![WeightedMatcher::new(1.0, extension)]).unwrap();
        assert_error(
            engine.match_schemas(&schema(), &schema()).unwrap_err(),
            MatchError::BudgetExceeded(budget),
        );
    }
}

#[test]
fn extension_failures_retain_only_registered_name() {
    for extension in [
        Extension {
            fail: true,
            ..Extension::default()
        },
        Extension {
            score: f64::NAN,
            ..Extension::default()
        },
        Extension {
            score: 1.5,
            ..Extension::default()
        },
    ] {
        let expected = if extension.fail {
            MatchError::MatcherFailed {
                name: "test-extension".into(),
            }
        } else {
            MatchError::InvalidMatcherScore {
                name: "test-extension".into(),
            }
        };
        let engine = MatchEngine::with_matchers(
            Config::default(),
            vec![WeightedMatcher::new(1.0, extension)],
        )
        .unwrap();
        let error = engine.match_schemas(&schema(), &schema()).unwrap_err();
        assert!(error.source().is_none());
        assert_error(error, expected);
    }
}

#[test]
fn human_messages_and_error_sources_remain_useful() {
    let error = MatchError::BudgetExceeded(BudgetKind::Pairs);
    assert_eq!(error.to_string(), "source-target pair budget exceeded");
    let source = error.source().unwrap();
    assert_eq!(source.to_string(), error.to_string());
    assert_eq!(
        source.downcast_ref::<BudgetKind>(),
        Some(&BudgetKind::Pairs)
    );

    let error = MatchError::MatcherFailed {
        name: "test-extension".into(),
    };
    assert_eq!(
        error.to_string(),
        "matcher 'test-extension' failed (details suppressed to protect sample values)"
    );
    assert_eq!(
        MatchError::InvalidMatcherScore {
            name: "test-extension".into()
        }
        .to_string(),
        "matcher 'test-extension' returned a score outside [0, 1]"
    );
}
