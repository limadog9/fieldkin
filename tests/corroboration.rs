//! Corroboration constrains eligibility without retuning scores or trusting names.

use std::sync::{Arc, Mutex};

use fieldkin::{
    CandidateIssue, Config, ConfigurationError, Corroboration, DataType, Decision, Evidence,
    ExactDecimal, Field, MatchEngine, MatchError, MatchReport, Matcher, NameMatcher, SampleMatcher,
    SampleProfileMatcher, SampleReliability, SampleValue, Schema, SemanticHints, TypeMatcher,
    WeightedMatcher,
};
use proptest::prelude::*;

fn samples(values: &[i128]) -> Vec<SampleValue> {
    values.iter().copied().map(SampleValue::Integer).collect()
}

fn field(id: &str, name: &str, values: &[i128]) -> Field {
    Field::new(id, name, DataType::Text).with_samples(samples(values))
}

fn enabled() -> Config {
    Config {
        corroboration: Some(Corroboration::default()),
        ..Config::default()
    }
}

fn pair(source: Field, target: Field, config: Config) -> MatchReport {
    MatchEngine::new(config)
        .unwrap()
        .match_schemas(&Schema::new(vec![source]), &Schema::new(vec![target]))
        .unwrap()
}

#[test]
fn ordinary_defaults_remain_ungated_and_the_opt_in_preset_is_explicit() {
    assert!(Config::default().corroboration.is_none());
    assert_eq!(Corroboration::default().min_name_score, 0.0);
    assert_eq!(Corroboration::default().min_sample_score, 0.5);
    let source = Field::new("s", "amount", DataType::Text);
    let target = Field::new("t", "amount", DataType::Text);
    let old = pair(source.clone(), target.clone(), Config::default());
    let gated = pair(source, target, enabled());
    assert!(old.fields[0].selected.is_some());
    assert!(gated.fields[0].selected.is_none());
    assert_eq!(
        old.fields[0].candidates[0].score,
        gated.fields[0].candidates[0].score
    );
    assert_eq!(
        old.fields[0].candidates[0].signals,
        gated.fields[0].candidates[0].signals
    );
    assert!(gated.fields[0].candidates[0]
        .issues
        .contains(&CandidateIssue::InsufficientSampleSupport));
}

#[test]
fn useful_distinct_overlap_and_lexical_support_allow_a_proposal() {
    let result = pair(
        field("s", "TransDate", &[1, 2, 3]),
        field("t", "transaction_date", &[1, 2, 3]),
        enabled(),
    );
    let selected = result.fields[0].selected.as_ref().unwrap();
    assert_eq!(selected.score, 1.0);
    assert!(!selected
        .issues
        .contains(&CandidateIssue::InsufficientNameSupport));
    assert!(!selected
        .issues
        .contains(&CandidateIssue::InsufficientSampleSupport));
}

#[test]
fn missing_empty_insufficient_constant_and_null_heavy_samples_abstain() {
    let mut sparse = samples(&[1, 2, 3]);
    sparse.extend(vec![SampleValue::Null; 9]);
    for source_samples in [
        None,
        Some(Vec::new()),
        Some(vec![SampleValue::Null; 12]),
        Some(samples(&[1, 2])),
        Some(samples(&[1, 1, 1, 1, 1])),
        Some(sparse),
    ] {
        let mut source = Field::new("s", "value", DataType::Text);
        source.samples = source_samples;
        let result = pair(source, field("t", "value", &[1, 2, 3]), enabled());
        assert!(result.fields[0].selected.is_none());
        assert!(result.fields[0].candidates[0]
            .issues
            .contains(&CandidateIssue::InsufficientSampleSupport));
        assert!(!result.fields[0].candidates[0]
            .issues
            .contains(&CandidateIssue::InsufficientNameSupport));
    }
}

#[test]
fn two_value_columns_can_meet_the_half_support_boundary() {
    // This is an intentional limitation: two repeated values are not semantic
    // proof, even though full coverage yields the chosen 0.5 support floor.
    let values = vec![
        SampleValue::Boolean(true),
        SampleValue::Boolean(false),
        SampleValue::Boolean(true),
    ];
    let source = Field::new("s", "flag", DataType::Boolean).with_samples(values.clone());
    let target = Field::new("t", "flag", DataType::Boolean).with_samples(values);
    assert!(pair(source.clone(), target.clone(), enabled()).fields[0]
        .selected
        .is_some());
    let stricter = pair(
        source,
        target,
        Config {
            corroboration: Some(Corroboration {
                min_sample_score: f64::from_bits(0.5_f64.to_bits() + 1),
                ..Corroboration::default()
            }),
            ..Config::default()
        },
    );
    assert!(stricter.fields[0].selected.is_none());
}

#[test]
fn exact_numeric_kinds_are_not_silently_coerced() {
    for values in [
        vec![
            SampleValue::Number(1.0),
            SampleValue::Number(2.0),
            SampleValue::Number(3.0),
        ],
        (1..=3)
            .map(|value| SampleValue::Decimal(ExactDecimal::new(value, 0).unwrap()))
            .collect(),
    ] {
        let target = Field::new("t", "value", DataType::Text).with_samples(values);
        let result = pair(field("s", "value", &[1, 2, 3]), target, enabled());
        assert!(result.fields[0].selected.is_none());
        assert!(result.fields[0].candidates[0]
            .issues
            .contains(&CandidateIssue::InsufficientSampleSupport));
    }
    let decimal_field = |id, scale| {
        Field::new(id, "value", DataType::Decimal).with_samples(
            (1..=3)
                .map(|value| {
                    SampleValue::Decimal(
                        ExactDecimal::new(value * 10_i128.pow(scale), scale).unwrap(),
                    )
                })
                .collect(),
        )
    };
    assert!(
        pair(decimal_field("s", 0), decimal_field("t", 2), enabled()).fields[0]
            .selected
            .is_some()
    );
}

#[test]
fn strict_name_support_is_separate_from_the_opt_in_default() {
    let source = field("s", "GrossAmt", &[1, 2, 3]);
    let target = field("t", "amount", &[1, 2, 3]);
    // Isolate the name floor from the separate total-score threshold.
    assert!(pair(
        source.clone(),
        target.clone(),
        Config {
            min_score: 0.0,
            ..enabled()
        }
    )
    .fields[0]
        .selected
        .is_some());
    let result = pair(
        source,
        target,
        Config {
            min_score: 0.0,
            corroboration: Some(Corroboration {
                min_name_score: 0.8,
                ..Corroboration::default()
            }),
            ..Config::default()
        },
    );
    assert!(result.fields[0].selected.is_none());
    assert!(result.fields[0].candidates[0]
        .issues
        .contains(&CandidateIssue::InsufficientNameSupport));
    assert!(!result.fields[0].candidates[0]
        .issues
        .contains(&CandidateIssue::InsufficientSampleSupport));
}

#[test]
fn name_floor_is_inclusive_but_always_requires_positive_evidence() {
    let source = field("s", "GrossAmt", &[1, 2, 3]);
    let target = field("t", "amount", &[1, 2, 3]);
    let boundary = NameMatcher::default()
        .evaluate(&source, &target)
        .unwrap()
        .score
        .unwrap();
    for (floor, passes) in [
        (boundary, true),
        (f64::from_bits(boundary.to_bits() + 1), false),
    ] {
        let result = pair(
            source.clone(),
            target.clone(),
            Config {
                min_score: 0.0,
                corroboration: Some(Corroboration {
                    min_name_score: floor,
                    ..Corroboration::default()
                }),
                ..Config::default()
            },
        );
        assert_eq!(result.fields[0].selected.is_some(), passes);
    }
    for (source_name, target_name) in [("", "---"), ("x", "y")] {
        let no_name = pair(
            field("s", source_name, &[1, 2, 3]),
            field("t", target_name, &[1, 2, 3]),
            Config {
                min_score: 0.0,
                ..enabled()
            },
        );
        assert!(no_name.fields[0].selected.is_none());
        assert!(no_name.fields[0].candidates[0]
            .issues
            .contains(&CandidateIssue::InsufficientNameSupport));
    }
}

#[test]
fn hard_type_and_hint_constraints_still_win_over_perfect_support() {
    for one_to_one in [false, true] {
        let source = field("s", "value", &[1, 2, 3]);
        let mut target = field("t", "value", &[1, 2, 3]);
        target.data_type = DataType::Binary;
        let incompatible = pair(
            source.clone(),
            target,
            Config {
                one_to_one,
                ..enabled()
            },
        );
        assert!(incompatible.fields[0].selected.is_none());
        assert!(incompatible.fields[0].candidates[0]
            .issues
            .contains(&CandidateIssue::IncompatibleTypes));
        let source = source.with_hints(SemanticHints {
            currency: Some("PRIVATE_A".into()),
            ..Default::default()
        });
        let target = field("t", "value", &[1, 2, 3]).with_hints(SemanticHints {
            currency: Some("PRIVATE_B".into()),
            ..Default::default()
        });
        let conflict = pair(
            source,
            target,
            Config {
                one_to_one,
                ..enabled()
            },
        );
        assert!(conflict.fields[0].selected.is_none());
        assert!(!format!("{conflict:?}").contains("PRIVATE"));
    }
}

#[test]
fn agreeing_hints_do_not_replace_missing_samples_or_names() {
    let hints = SemanticHints {
        unit: Some("same".into()),
        currency: Some("same".into()),
        identifier_scope: Some("same".into()),
    };
    let source = Field::new("s", "value", DataType::Text).with_hints(hints.clone());
    let target = Field::new("t", "value", DataType::Text).with_hints(hints.clone());
    assert!(pair(source, target, enabled()).fields[0].selected.is_none());
    let source = field("s", "", &[1, 2, 3]).with_hints(hints.clone());
    let target = field("t", "", &[1, 2, 3]).with_hints(hints);
    assert!(pair(
        source,
        target,
        Config {
            min_score: 0.0,
            ..enabled()
        }
    )
    .fields[0]
        .selected
        .is_none());
}

struct Spoof {
    name: &'static str,
    calls: Arc<Mutex<Vec<(String, String)>>>,
}

impl Matcher for Spoof {
    fn name(&self) -> &str {
        self.name
    }

    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        self.calls
            .lock()
            .unwrap()
            .push((source.id.0.clone(), target.id.0.clone()));
        Ok(Evidence {
            score: Some(1.0),
            explanation: "Custom evidence".into(),
        })
    }
}

#[test]
fn custom_matcher_names_do_not_qualify_and_all_callbacks_still_run() {
    for fake_name in ["name", "samples"] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let custom = WeightedMatcher::new(
            1.0,
            Spoof {
                name: fake_name,
                calls: Arc::clone(&calls),
            },
        );
        let matchers = if fake_name == "name" {
            vec![custom, WeightedMatcher::new(1.0, SampleMatcher::default())]
        } else {
            vec![WeightedMatcher::new(1.0, NameMatcher::default()), custom]
        };
        let source = Schema::new(vec![
            field("s1", "value", &[1, 2, 3]),
            field("s0", "value", &[1, 2, 3]),
        ]);
        let target = Schema::new(vec![
            field("t1", "value", &[1, 2, 3]),
            field("t0", "value", &[1, 2, 3]),
        ]);
        let report = MatchEngine::with_matchers(
            Config {
                abstain_on_ambiguity: false,
                ..enabled()
            },
            matchers,
        )
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap();
        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                ("s0".into(), "t0".into()),
                ("s0".into(), "t1".into()),
                ("s1".into(), "t0".into()),
                ("s1".into(), "t1".into())
            ]
        );
        let issue = if fake_name == "name" {
            CandidateIssue::InsufficientNameSupport
        } else {
            CandidateIssue::InsufficientSampleSupport
        };
        for result in report.fields {
            assert!(result.selected.is_none());
            assert!(result
                .candidates
                .iter()
                .all(|candidate| candidate.issues.contains(&issue)));
        }
    }
}

struct WrappedSamples(SampleMatcher);
impl Matcher for WrappedSamples {
    fn name(&self) -> &str {
        self.0.name()
    }
    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        self.0.evaluate(source, target)
    }
}

#[test]
fn legacy_profile_wrapped_and_disabled_samples_cannot_supply_support() {
    let source = Schema::new(vec![field("s", "value", &[1, 2, 3, 4, 5, 6, 7, 8])]);
    let target = Schema::new(vec![field("t", "value", &[1, 2, 3, 4, 5, 6, 7, 8])]);
    for signal in [
        WeightedMatcher::new(
            1.0,
            SampleMatcher {
                reliability: SampleReliability::Legacy,
                ..Default::default()
            },
        ),
        WeightedMatcher::new(1.0, SampleProfileMatcher::default()),
        WeightedMatcher::new(1.0, WrappedSamples(SampleMatcher::default())),
        WeightedMatcher::new(0.0, SampleMatcher::default()),
    ] {
        let engine = MatchEngine::with_matchers(
            enabled(),
            vec![WeightedMatcher::new(1.0, NameMatcher::default()), signal],
        )
        .unwrap();
        let report = engine.match_schemas(&source, &target).unwrap();
        assert!(report.fields[0].selected.is_none());
        assert!(report.fields[0].candidates[0]
            .issues
            .contains(&CandidateIssue::InsufficientSampleSupport));
    }
    let engine = MatchEngine::with_matchers(
        enabled(),
        vec![
            WeightedMatcher::new(0.0, NameMatcher::default()),
            WeightedMatcher::new(1.0, SampleMatcher::default()),
        ],
    )
    .unwrap();
    let report = engine.match_schemas(&source, &target).unwrap();
    assert!(report.fields[0].selected.is_none());
    assert!(report.fields[0].candidates[0]
        .issues
        .contains(&CandidateIssue::InsufficientNameSupport));
}

#[test]
fn removing_an_unsupported_tie_can_create_a_previously_absent_proposal() {
    let source = Schema::new(vec![field("s", "amount", &[1, 2, 3])]);
    let target = Schema::new(vec![
        field("supported", "amount", &[1, 2, 3]),
        field("unsupported", "amount", &[8, 9, 10]),
    ]);
    for one_to_one in [false, true] {
        let base = Config {
            one_to_one,
            ambiguity_margin: 0.2,
            max_candidates: 1,
            ..Config::default()
        };
        let original = MatchEngine::new(base.clone())
            .unwrap()
            .match_schemas(&source, &target)
            .unwrap();
        assert_eq!(original.fields[0].decision, Decision::Ambiguous);
        let gated = MatchEngine::new(Config {
            corroboration: Some(Corroboration::default()),
            ..base
        })
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap();
        assert_eq!(gated.fields[0].decision, Decision::Proposed);
        assert_eq!(
            gated.fields[0].selected.as_ref().unwrap().target.0,
            "supported"
        );
        assert_eq!(gated.fields[0].alternatives.len(), 1);
    }
}

#[test]
fn unsupported_pairs_stay_out_of_global_assignment_and_competition() {
    let mut source = Schema::new(vec![
        field("s0", "amount", &[1, 2, 3]),
        field("s1", "amount", &[4, 5, 6]),
    ]);
    let mut target = Schema::new(vec![
        field("t0", "amount", &[4, 5, 6]),
        field("t1", "amount", &[1, 2, 3]),
    ]);
    let config = Config {
        one_to_one: true,
        ..enabled()
    };
    let full = MatchEngine::new(config.clone())
        .unwrap()
        .match_schemas(&source, &target)
        .unwrap();
    assert_eq!(full.fields[0].selected.as_ref().unwrap().target.0, "t1");
    assert_eq!(full.fields[1].selected.as_ref().unwrap().target.0, "t0");
    assert!(full.target_competition.is_empty());
    source.fields.reverse();
    target.fields.reverse();
    assert_eq!(
        full,
        MatchEngine::new(config.clone())
            .unwrap()
            .match_schemas(&source, &target)
            .unwrap()
    );
    let clipped = MatchEngine::new(Config {
        max_candidates: 1,
        ..config
    })
    .unwrap()
    .match_schemas(&source, &target)
    .unwrap();
    for (full, clipped) in full.fields.iter().zip(&clipped.fields) {
        assert_eq!(full.selected, clipped.selected);
        assert_eq!(full.alternatives, clipped.alternatives);
        assert_eq!(full.diagnostics, clipped.diagnostics);
    }
}

#[test]
fn perfect_support_is_not_a_semantic_equivalence_guarantee() {
    // An application may know these equally named, equally valued columns have
    // different meanings. That absent metadata is not invented by this policy.
    let result = pair(
        field("unrelated_source_fact", "code", &[1, 2, 3]),
        field("unrelated_target_fact", "code", &[1, 2, 3]),
        enabled(),
    );
    assert!(result.fields[0].selected.is_some());
}

#[test]
fn invalid_corroboration_floors_are_structured_configuration_errors() {
    for (name, sample) in [
        (f64::NAN, 0.5),
        (f64::INFINITY, 0.5),
        (-0.01, 0.5),
        (1.01, 0.5),
        (0.0, 0.0),
        (0.0, -0.01),
        (0.0, 1.01),
        (0.0, f64::NAN),
        (0.0, f64::INFINITY),
    ] {
        assert!(matches!(
            MatchEngine::new(Config {
                corroboration: Some(Corroboration {
                    min_name_score: name,
                    min_sample_score: sample
                }),
                ..Config::default()
            }),
            Err(MatchError::InvalidConfiguration(
                ConfigurationError::Corroboration
            ))
        ));
    }
}

#[test]
fn gate_does_not_bypass_signal_work_budgets() {
    let source = Schema::new(vec![field("s", "", &[1, 2, 3])]);
    let target = Schema::new(vec![field("t", "", &[1, 2, 3])]);
    let mut config = enabled();
    config.limits.max_signal_evaluations = 2;
    assert!(MatchEngine::new(config)
        .unwrap()
        .match_schemas(&source, &target)
        .is_err());
    // Even without the required built-ins, active custom signals retain their
    // normal work/validation behavior rather than an early eligibility shortcut.
    let custom =
        MatchEngine::with_matchers(enabled(), vec![WeightedMatcher::new(1.0, TypeMatcher)])
            .unwrap();
    assert!(custom.match_schemas(&source, &target).unwrap().fields[0]
        .selected
        .is_none());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]
    #[test]
    fn pair_scores_are_unchanged_and_eligibility_can_only_be_removed(
        source_name in "[a-zA-Z0-9_-]{0,20}",
        target_name in "[a-zA-Z0-9_-]{0,20}",
        source_values in proptest::collection::vec(0_i128..8, 0..12),
        target_values in proptest::collection::vec(0_i128..8, 0..12),
        strict in any::<bool>(),
    ) {
        let source = field("s", &source_name, &source_values);
        let target = field("t", &target_name, &target_values);
        let plain = pair(source.clone(), target.clone(), Config { min_score: 0.0, ..Config::default() });
        let gated = pair(source, target, Config {
            min_score: 0.0,
            corroboration: Some(Corroboration { min_name_score: if strict { 0.8 } else { 0.0 }, ..Corroboration::default() }),
            ..Config::default()
        });
        let plain = &plain.fields[0].candidates[0];
        let gated = &gated.fields[0].candidates[0];
        prop_assert_eq!(plain.score, gated.score);
        prop_assert_eq!(&plain.signals, &gated.signals);
        prop_assert!(!gated.eligible || plain.eligible);
        prop_assert!(plain.issues.iter().all(|issue| gated.issues.contains(issue)));
    }
}
