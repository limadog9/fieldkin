//! Prepared built-ins must preserve the complete public pairwise result.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use fieldkin::{
    Config, DataType, Evidence, Field, MatchEngine, Matcher, NameMatcher, SampleMatcher,
    SampleValue, Schema, TypeMatcher, WeightedMatcher,
};

// The wrapper keeps the custom Matcher path while deliberately retaining the
// built-in signal name. It delegates to the public, uncached pairwise API.
struct Pairwise<M>(M);

impl<M: Matcher> Matcher for Pairwise<M> {
    fn name(&self) -> &str {
        self.0.name()
    }

    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        self.0.evaluate(source, target)
    }
}

fn engine(config: Config, names: NameMatcher, samples: SampleMatcher, cached: bool) -> MatchEngine {
    let matchers = if cached {
        vec![
            WeightedMatcher::new(0.65, names),
            WeightedMatcher::new(0.20, TypeMatcher),
            WeightedMatcher::new(0.15, samples),
        ]
    } else {
        vec![
            WeightedMatcher::new(0.65, Pairwise(names)),
            WeightedMatcher::new(0.20, Pairwise(TypeMatcher)),
            WeightedMatcher::new(0.15, Pairwise(samples)),
        ]
    };
    MatchEngine::with_matchers(config, matchers).unwrap()
}

fn assert_equivalent(config: Config, source: &Schema, target: &Schema) {
    assert_eq!(
        engine(
            config.clone(),
            NameMatcher::default(),
            SampleMatcher::default(),
            true,
        )
        .match_schemas(source, target),
        engine(
            config,
            NameMatcher::default(),
            SampleMatcher::default(),
            false,
        )
        .match_schemas(source, target),
    );
}

fn fixture(prefix: &str) -> Schema {
    use DataType::*;
    let descriptions = [
        ("TransDate", Date),
        ("transaction_date", Timestamp),
        ("GrossAmt", Decimal),
        ("gross_amount", Float),
        ("value", Boolean),
        ("value", Integer),
        ("value", Text),
        ("value", Unknown),
        ("", Binary),
        ("---_\u{301}", Text),
        ("CAFÉCode", Text),
        ("Cafe\u{301}_Code", Text),
        ("Ｆｉｅｌｄ＿Ｃｏｄｅ", Integer),
        ("XMLHttpRequest2026", Text),
        ("same_same_token", Text),
    ];
    let fields = descriptions
        .iter()
        .enumerate()
        .map(|(index, (name, data_type))| {
            let field = Field::new(format!("{prefix}-{index:02}"), *name, *data_type);
            match index % 7 {
                0 => field,
                1 => field.with_samples(vec![]),
                2 => field.with_samples(vec![SampleValue::Null; 10]),
                3 => field.with_samples(vec![SampleValue::Number(1.0); 2]),
                4 => field.with_samples(vec![SampleValue::Boolean(true); 3]),
                5 => field.with_samples(vec![
                    SampleValue::Number(-0.0),
                    SampleValue::Number(0.0),
                    SampleValue::Number(1.0),
                    SampleValue::Text("0".into()),
                    SampleValue::Null,
                ]),
                _ => field.with_samples(vec![
                    SampleValue::Text("SensitiveCase".into()),
                    SampleValue::Text("sensitivecase".into()),
                    SampleValue::Text("SensitiveCase ".into()),
                    SampleValue::Text("SensitiveCase".into()),
                    SampleValue::Null,
                    SampleValue::Null,
                ]),
            }
        })
        .collect();
    Schema::new(fields)
}

#[test]
fn full_reports_match_pairwise_across_modes_truncation_and_missing_evidence() {
    let mut source = fixture("source");
    let mut target = fixture("target");
    target.fields.remove(3);
    target.fields.reverse();
    source.fields.rotate_left(4);
    for one_to_one in [false, true] {
        for abstain_on_ambiguity in [false, true] {
            for max_candidates in [1, 3, 32] {
                for min_score in [0.0, 0.70, 0.95] {
                    assert_equivalent(
                        Config {
                            one_to_one,
                            abstain_on_ambiguity,
                            max_candidates,
                            min_score,
                            ..Config::default()
                        },
                        &source,
                        &target,
                    );
                }
            }
        }
    }
    for (source, target) in [
        (source, Schema::default()),
        (Schema::default(), target),
        (Schema::default(), Schema::default()),
    ] {
        assert_equivalent(Config::default(), &source, &target);
    }
}

#[test]
fn custom_aliases_and_sample_minimum_preserve_explanations_and_errors() {
    let source = fixture("source");
    let target = fixture("target");
    for aliases in [
        BTreeMap::new(),
        BTreeMap::from([
            ("amt".into(), "amount".into()),
            ("trans".into(), "transaction".into()),
            ("value".into(), "value".into()),
        ]),
        BTreeMap::from([("unused".into(), "invalid alias".into())]),
        BTreeMap::from([("amt".into(), "invalid alias".into())]),
        BTreeMap::from([("value".into(), "x".repeat(257))]),
    ] {
        for min_non_null in [0, 1, 3, 20] {
            let names = NameMatcher {
                aliases: aliases.clone(),
            };
            let samples = SampleMatcher {
                min_non_null,
                ..SampleMatcher::default()
            };
            assert_eq!(
                engine(Config::default(), names.clone(), samples, true)
                    .match_schemas(&source, &target),
                engine(Config::default(), names, samples, false).match_schemas(&source, &target),
            );
        }
    }
}

#[test]
fn empty_products_and_disabled_builtins_do_not_validate_unused_evidence() {
    let invalid_names = NameMatcher {
        aliases: BTreeMap::from([("amt".into(), "invalid alias".into())]),
    };
    let schema = Schema::new(vec![Field::new("id", "amt", DataType::Text)]);
    for cached in [false, true] {
        let invalid = engine(
            Config::default(),
            invalid_names.clone(),
            SampleMatcher {
                min_non_null: 0,
                ..SampleMatcher::default()
            },
            cached,
        );
        assert!(invalid.match_schemas(&schema, &Schema::default()).is_ok());
        assert!(invalid.match_schemas(&Schema::default(), &schema).is_ok());
    }
    let disabled = MatchEngine::with_matchers(
        Config::default(),
        vec![
            WeightedMatcher::new(0.0, invalid_names),
            WeightedMatcher::new(
                0.0,
                SampleMatcher {
                    min_non_null: 0,
                    ..SampleMatcher::default()
                },
            ),
            WeightedMatcher::new(1.0, TypeMatcher),
        ],
    )
    .unwrap();
    let report = disabled.match_schemas(&schema, &schema).unwrap();
    assert_eq!(report.fields[0].candidates[0].signals.len(), 1);
}

#[test]
fn resource_limits_and_invalid_inputs_have_identical_failures() {
    let source = fixture("source");
    let target = fixture("target");
    let mut configurations = vec![];
    for budget in [0, 1, 256, 16_384] {
        let mut config = Config::default();
        config.limits.max_explanation_bytes = budget;
        configurations.push(config);
    }
    let mut config = Config::default();
    config.limits.max_pairs = 1;
    configurations.push(config);
    let mut config = Config::default();
    config.limits.max_signal_evaluations = 1;
    configurations.push(config);
    for config in configurations {
        assert_equivalent(config, &source, &target);
    }
    for invalid in [
        Field::new("", "name", DataType::Text),
        Field::new("id", "x".repeat(257), DataType::Text),
        Field::new("id", "name", DataType::Float).with_samples(vec![SampleValue::Number(f64::NAN)]),
        Field::new("id", "name", DataType::Float)
            .with_samples(vec![SampleValue::Number(f64::INFINITY)]),
        Field::new("id", "name", DataType::Text)
            .with_samples(vec![SampleValue::Text("x".repeat(1025))]),
    ] {
        assert_equivalent(Config::default(), &Schema::new(vec![invalid]), &target);
    }
}

type Calls = Arc<Mutex<Vec<(String, String)>>>;

struct Custom {
    name: &'static str,
    calls: Calls,
    fail: bool,
}

impl Matcher for Custom {
    fn name(&self) -> &str {
        self.name
    }

    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        self.calls
            .lock()
            .unwrap()
            .push((source.id.0.clone(), target.id.0.clone()));
        if self.fail {
            return Err("private custom error detail".into());
        }
        Ok(Evidence {
            score: Some(0.9),
            explanation: "custom signal evaluated".into(),
        })
    }
}

#[test]
fn public_box_replacement_and_builtin_signal_names_keep_custom_dispatch() {
    let calls = Calls::default();
    let mut weighted = WeightedMatcher::new(1.0, NameMatcher::default());
    weighted.matcher = Box::new(Custom {
        name: "name",
        calls: calls.clone(),
        fail: false,
    });
    let engine = MatchEngine::with_matchers(Config::default(), vec![weighted]).unwrap();
    let source = Schema::new(vec![Field::new("source", "left", DataType::Text)]);
    let target = Schema::new(vec![Field::new("target", "unrelated", DataType::Text)]);
    let report = engine.match_schemas(&source, &target).unwrap();
    let candidate = &report.fields[0].candidates[0];
    assert_eq!(candidate.score, 0.9);
    assert_eq!(
        candidate.signals[0].evidence.explanation,
        "custom signal evaluated"
    );
    assert_eq!(calls.lock().unwrap().len(), 1);
}

#[test]
fn custom_callbacks_remain_per_pair_sorted_and_zero_weight_disabled() {
    let source = Schema::new(fixture("s").fields[..2].to_vec());
    let mut target = Schema::new(fixture("t").fields[..3].to_vec());
    target.fields.reverse();
    let calls = Calls::default();
    let disabled_calls = Calls::default();
    let engine = MatchEngine::with_matchers(
        Config::default(),
        vec![
            WeightedMatcher::new(0.5, NameMatcher::default()),
            WeightedMatcher::new(
                0.5,
                Custom {
                    name: "counting",
                    calls: calls.clone(),
                    fail: false,
                },
            ),
            WeightedMatcher::new(
                0.0,
                Custom {
                    name: "disabled",
                    calls: disabled_calls.clone(),
                    fail: true,
                },
            ),
        ],
    )
    .unwrap();
    engine.match_schemas(&source, &target).unwrap();
    let expected: Vec<_> = (0..2)
        .flat_map(|s| (0..3).map(move |t| (format!("s-{s:02}"), format!("t-{t:02}"))))
        .collect();
    assert_eq!(*calls.lock().unwrap(), expected);
    assert!(disabled_calls.lock().unwrap().is_empty());
}

#[test]
fn validation_and_signal_order_remain_observable_to_custom_matchers() {
    let source = Schema::new(vec![Field::new("s", "amt", DataType::Text)]);
    let target = Schema::new(vec![Field::new("t", "amount", DataType::Text)]);
    for cached in [false, true] {
        let calls = Calls::default();
        let names = NameMatcher {
            aliases: BTreeMap::from([("amt".into(), "invalid alias".into())]),
        };
        let custom = WeightedMatcher::new(
            0.5,
            Custom {
                name: "first",
                calls: calls.clone(),
                fail: true,
            },
        );
        let names = if cached {
            WeightedMatcher::new(0.5, names)
        } else {
            WeightedMatcher::new(0.5, Pairwise(names))
        };
        let engine = MatchEngine::with_matchers(Config::default(), vec![custom, names]).unwrap();
        let error = engine.match_schemas(&source, &target).unwrap_err();
        assert!(error.to_string().contains("matcher 'first' failed"));
        assert!(!error.to_string().contains("private"));
        assert_eq!(calls.lock().unwrap().len(), 1);
        let invalid_source = Schema::new(vec![Field::new("", "amt", DataType::Text)]);
        engine.match_schemas(&invalid_source, &target).unwrap_err();
        assert_eq!(calls.lock().unwrap().len(), 1);
    }
}

#[test]
fn deferred_preparation_errors_preserve_calls_from_earlier_pairs() {
    let source = Schema::new(vec![Field::new("s", "amount", DataType::Text)]);
    let target = Schema::new(vec![
        Field::new("z-invalid", "amt", DataType::Text),
        Field::new("a-valid", "amount", DataType::Text),
    ]);
    for cached in [false, true] {
        let calls = Calls::default();
        let names = NameMatcher {
            aliases: BTreeMap::from([("amt".into(), "invalid alias".into())]),
        };
        let names = if cached {
            WeightedMatcher::new(0.5, names)
        } else {
            WeightedMatcher::new(0.5, Pairwise(names))
        };
        let engine = MatchEngine::with_matchers(
            Config::default(),
            vec![
                names,
                WeightedMatcher::new(
                    0.5,
                    Custom {
                        name: "after_names",
                        calls: calls.clone(),
                        fail: false,
                    },
                ),
            ],
        )
        .unwrap();
        let error = engine.match_schemas(&source, &target).unwrap_err();
        assert!(error.to_string().contains("matcher 'name' failed"));
        assert_eq!(
            *calls.lock().unwrap(),
            vec![("s".to_owned(), "a-valid".to_owned())],
        );
    }
}
