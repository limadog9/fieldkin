//! Structured reasons distinguish evidence rejection, local ties and competition.

use fieldkin::{
    CandidateIssue, Config, DataType, Decision, Evidence, Field, FieldDiagnostic, FieldId,
    MatchEngine, MatchReport, Matcher, Schema, SemanticAxis, SemanticHints, TargetCompetition,
    WeightedMatcher,
};

struct FixedMatrix(Vec<Vec<Option<f64>>>);

impl Matcher for FixedMatrix {
    fn name(&self) -> &str {
        "fixed"
    }

    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        let source_index = source.id.0[1..].parse::<usize>().unwrap();
        let target_index = target.id.0[1..].parse::<usize>().unwrap();
        Ok(Evidence {
            score: self.0[source_index][target_index],
            explanation: "Synthetic pair score".into(),
        })
    }
}

fn schema(prefix: &str, count: usize) -> Schema {
    Schema::new(
        (0..count)
            .map(|index| Field::new(format!("{prefix}{index}"), "value", DataType::Text))
            .collect(),
    )
}

fn config() -> Config {
    Config {
        one_to_one: true,
        abstain_on_ambiguity: false,
        min_score: 0.1,
        ambiguity_margin: 0.01,
        ..Config::default()
    }
}

fn engine(matrix: Vec<Vec<Option<f64>>>, config: Config) -> MatchEngine {
    MatchEngine::with_matchers(config, vec![WeightedMatcher::new(1.0, FixedMatrix(matrix))])
        .unwrap()
}

fn report(matrix: Vec<Vec<Option<f64>>>, config: Config) -> MatchReport {
    let sources = schema("s", matrix.len());
    let targets = schema("t", matrix.first().map_or(0, Vec::len));
    engine(matrix, config)
        .match_schemas(&sources, &targets)
        .unwrap()
}

#[test]
fn no_eligible_target_includes_empty_target_schema() {
    let report = report(vec![vec![]], config());
    assert_eq!(report.fields[0].decision, Decision::BelowThreshold);
    assert_eq!(
        report.fields[0].diagnostics,
        vec![FieldDiagnostic::NoEligibleTarget]
    );
    assert!(report.fields[0].candidates.is_empty());
    assert!(report.target_competition.is_empty());
}

#[test]
fn type_veto_has_a_distinct_reason_even_for_a_high_score() {
    let sources = Schema::new(vec![Field::new("s0", "value", DataType::Boolean)]);
    let targets = Schema::new(vec![Field::new("t0", "value", DataType::Timestamp)]);
    for veto in [true, false] {
        let result = engine(
            vec![vec![Some(1.0)]],
            Config {
                reject_incompatible_types: veto,
                ..config()
            },
        )
        .match_schemas(&sources, &targets)
        .unwrap();
        let field = &result.fields[0];
        assert_eq!(
            field.candidates[0].issues,
            vec![CandidateIssue::IncompatibleTypes]
        );
        assert_eq!(field.candidates[0].eligible, !veto);
        assert_eq!(field.selected.is_none(), veto);
        assert_eq!(
            field
                .diagnostics
                .contains(&FieldDiagnostic::NoEligibleTarget),
            veto
        );
        assert!(!field.candidates[0]
            .issues
            .contains(&CandidateIssue::InsufficientScore));
    }
}

#[test]
fn semantic_axes_are_structured_without_exposing_supplied_values() {
    let sources = Schema::new(vec![Field::new("s0", "value", DataType::Text).with_hints(
        SemanticHints {
            unit: Some("PRIVATE-unit-alpha".into()),
            currency: Some("PRIVATE-currency-alpha".into()),
            identifier_scope: Some("PRIVATE-scope-alpha".into()),
        },
    )]);
    let targets = Schema::new(vec![Field::new("t0", "value", DataType::Text).with_hints(
        SemanticHints {
            unit: Some("PRIVATE-unit-beta".into()),
            currency: Some("PRIVATE-currency-beta".into()),
            identifier_scope: Some("PRIVATE-scope-beta".into()),
        },
    )]);
    let result = engine(
        vec![vec![Some(1.0)]],
        Config {
            reject_incompatible_types: false,
            ..config()
        },
    )
    .match_schemas(&sources, &targets)
    .unwrap();
    assert_eq!(
        result.fields[0].candidates[0].issues,
        vec![
            CandidateIssue::SemanticConflict(SemanticAxis::Unit),
            CandidateIssue::SemanticConflict(SemanticAxis::Currency),
            CandidateIssue::SemanticConflict(SemanticAxis::IdentifierScope),
        ]
    );
    assert!(!result.fields[0].candidates[0].eligible);
    assert_eq!(
        result.fields[0].diagnostics,
        vec![FieldDiagnostic::NoEligibleTarget]
    );
    assert!(!format!("{result:?}").contains("PRIVATE"));
    assert!(result.target_competition.is_empty());
}

#[test]
fn agreeing_missing_and_absent_hints_are_distinct_and_do_not_boost_scores() {
    let hints = SemanticHints {
        unit: Some("PRIVATE-unit".into()),
        currency: Some("PRIVATE-currency".into()),
        identifier_scope: Some("PRIVATE-scope".into()),
    };
    let cases = [
        (
            hints.clone(),
            hints.clone(),
            vec![
                CandidateIssue::SemanticAgreement(SemanticAxis::Unit),
                CandidateIssue::SemanticAgreement(SemanticAxis::Currency),
                CandidateIssue::SemanticAgreement(SemanticAxis::IdentifierScope),
            ],
        ),
        (
            hints.clone(),
            SemanticHints::default(),
            vec![
                CandidateIssue::SemanticMissing(SemanticAxis::Unit),
                CandidateIssue::SemanticMissing(SemanticAxis::Currency),
                CandidateIssue::SemanticMissing(SemanticAxis::IdentifierScope),
            ],
        ),
        (
            SemanticHints::default(),
            hints,
            vec![
                CandidateIssue::SemanticMissing(SemanticAxis::Unit),
                CandidateIssue::SemanticMissing(SemanticAxis::Currency),
                CandidateIssue::SemanticMissing(SemanticAxis::IdentifierScope),
            ],
        ),
        (SemanticHints::default(), SemanticHints::default(), vec![]),
    ];
    for (source_hints, target_hints, expected) in cases {
        let sources = Schema::new(vec![
            Field::new("s0", "value", DataType::Text).with_hints(source_hints)
        ]);
        let targets = Schema::new(vec![
            Field::new("t0", "value", DataType::Text).with_hints(target_hints)
        ]);
        let result = engine(vec![vec![Some(0.75)]], config())
            .match_schemas(&sources, &targets)
            .unwrap();
        let candidate = result.fields[0].selected.as_ref().unwrap();
        assert_eq!(candidate.score, 0.75);
        assert_eq!(candidate.issues, expected);
        assert!(!format!("{result:?}").contains("PRIVATE"));
    }
}

#[test]
fn missing_evidence_and_insufficient_score_are_separate_reasons() {
    let sources = schema("s", 1);
    let targets = schema("t", 1);
    let builtin = MatchEngine::new(Config::default())
        .unwrap()
        .match_schemas(&sources, &targets)
        .unwrap();
    let candidate = builtin.fields[0].selected.as_ref().unwrap();
    assert!(candidate.issues.contains(&CandidateIssue::MissingEvidence));
    assert!(!candidate
        .issues
        .contains(&CandidateIssue::InsufficientScore));
    let absent = report(
        vec![vec![None]],
        Config {
            min_score: 0.0,
            ..config()
        },
    );
    assert_eq!(
        absent.fields[0].candidates[0].issues,
        vec![
            CandidateIssue::MissingEvidence,
            CandidateIssue::InsufficientScore
        ]
    );
    assert_eq!(
        absent.fields[0].diagnostics,
        vec![FieldDiagnostic::NoEligibleTarget]
    );
    assert!(absent.fields[0].selected.is_none());
}

#[test]
fn displacement_identifies_preferred_target_while_other_target_is_selected() {
    let result = report(
        vec![vec![Some(0.9), Some(0.8)], vec![Some(0.85), None]],
        config(),
    );
    assert_eq!(
        result.target_competition,
        vec![TargetCompetition {
            target: "t0".into(),
            sources: vec!["s0".into(), "s1".into()]
        }]
    );
    assert_eq!(
        result.fields[0].diagnostics,
        vec![
            FieldDiagnostic::TargetCompetition("t0".into()),
            FieldDiagnostic::Displaced {
                preferred_target: "t0".into()
            }
        ]
    );
    assert_eq!(
        result.fields[0].selected.as_ref().unwrap().target,
        FieldId::from("t1")
    );
    assert_eq!(result.fields[0].decision, Decision::Proposed);
    assert!(!result.fields[0]
        .selected
        .as_ref()
        .unwrap()
        .issues
        .contains(&CandidateIssue::TargetCompetition));
    assert!(result.fields[1]
        .selected
        .as_ref()
        .unwrap()
        .issues
        .contains(&CandidateIssue::TargetCompetition));
}

#[test]
fn globally_unassigned_is_separate_from_ineligible_and_local_ambiguity() {
    let result = report(vec![vec![Some(0.9)], vec![Some(0.8)]], config());
    assert_eq!(result.fields[1].decision, Decision::AssignmentConflict);
    assert_eq!(
        result.fields[1].diagnostics,
        vec![
            FieldDiagnostic::TargetCompetition("t0".into()),
            FieldDiagnostic::UnassignedByGlobalConstraint
        ]
    );
    assert!(result.fields[1].candidates[0].eligible);
    assert!(result.fields[1].selected.is_none());
}

#[test]
fn local_ambiguity_is_reported_with_or_without_abstention() {
    for abstain in [false, true] {
        for one_to_one in [false, true] {
            let result = report(
                vec![vec![Some(0.9), Some(0.9)]],
                Config {
                    abstain_on_ambiguity: abstain,
                    one_to_one,
                    ..config()
                },
            );
            assert_eq!(
                result.fields[0].diagnostics,
                vec![FieldDiagnostic::LocalAmbiguity]
            );
            assert_eq!(result.fields[0].selected.is_none(), abstain);
            assert_eq!(
                result.fields[0].decision,
                if abstain {
                    Decision::Ambiguous
                } else {
                    Decision::Proposed
                }
            );
            assert!(result.target_competition.is_empty());
        }
    }
}

#[test]
fn locally_abstaining_sources_do_not_create_global_competition() {
    let matrix = vec![vec![Some(0.9), Some(0.9)], vec![Some(0.8), None]];
    let abstaining = report(
        matrix.clone(),
        Config {
            abstain_on_ambiguity: true,
            ..config()
        },
    );
    assert!(abstaining.target_competition.is_empty());
    assert_eq!(
        abstaining.fields[0].diagnostics,
        vec![FieldDiagnostic::LocalAmbiguity]
    );
    assert!(abstaining.fields[1].diagnostics.is_empty());
    assert!(!abstaining.fields[1]
        .selected
        .as_ref()
        .unwrap()
        .issues
        .contains(&CandidateIssue::TargetCompetition));
    let allowed = report(matrix, config());
    assert_eq!(allowed.target_competition.len(), 1);
    assert_eq!(allowed.target_competition[0].sources.len(), 2);
}

#[test]
fn structured_reasons_ignore_top_k_and_preserve_stable_order() {
    let matrix = vec![vec![Some(0.9), Some(0.8)], vec![Some(0.85), None]];
    let original = report(matrix.clone(), config());
    let mut sources = schema("s", 2);
    let mut targets = schema("t", 2);
    sources.fields.reverse();
    targets.fields.reverse();
    let reordered = engine(
        matrix,
        Config {
            max_candidates: 1,
            ..config()
        },
    )
    .match_schemas(&sources, &targets)
    .unwrap();
    assert_eq!(original.target_competition, reordered.target_competition);
    for (original, reordered) in original.fields.iter().zip(&reordered.fields) {
        assert_eq!(original.source, reordered.source);
        assert_eq!(original.diagnostics, reordered.diagnostics);
        assert_eq!(original.selected, reordered.selected);
    }
}
