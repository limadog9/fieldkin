//! Caller review decisions constrain selection without manufacturing evidence.

use std::collections::BTreeSet;
use std::error::Error;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use fieldkin::{
    AssignmentDiagnosticStatus, BudgetKind, CandidateIssue, Config, ConstraintError, Corroboration,
    DataType, Decision, Evidence, Field, FieldDiagnostic, FieldId, FieldPair,
    GlobalDiagnosticsConfig, InputError, MatchConstraints, MatchEngine, MatchError, MatchReport,
    Matcher, SampleValue, Schema, SemanticAxis, SemanticHints, WeightedMatcher,
};
use proptest::prelude::*;

struct MatrixMatcher {
    scores: Vec<Vec<Option<f64>>>,
    calls: Arc<AtomicUsize>,
}

impl Matcher for MatrixMatcher {
    fn name(&self) -> &str {
        "synthetic-matrix"
    }

    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let source_index = source.id.0[1..].parse::<usize>().unwrap();
        let target_index = target.id.0[1..].parse::<usize>().unwrap();
        Ok(Evidence {
            score: self.scores[source_index][target_index],
            explanation: "Synthetic test evidence".into(),
        })
    }
}

fn schema(prefix: &str, count: usize) -> Schema {
    Schema::new(
        (0..count)
            .map(|index| Field::new(format!("{prefix}{index}"), "duplicate", DataType::Text))
            .collect(),
    )
}

fn config() -> Config {
    Config {
        min_score: 0.1,
        ambiguity_margin: 0.0,
        abstain_on_ambiguity: false,
        ..Config::default()
    }
}

fn matrix_engine(scores: &[Vec<Option<f64>>], config: Config) -> (MatchEngine, Arc<AtomicUsize>) {
    let calls = Arc::new(AtomicUsize::new(0));
    let engine = MatchEngine::with_matchers(
        config,
        vec![WeightedMatcher::new(
            1.0,
            MatrixMatcher {
                scores: scores.to_vec(),
                calls: calls.clone(),
            },
        )],
    )
    .unwrap();
    (engine, calls)
}

fn run(scores: &[Vec<Option<f64>>], config: Config, constraints: &MatchConstraints) -> MatchReport {
    matrix_engine(scores, config)
        .0
        .match_schemas_with_constraints(
            &schema("s", scores.len()),
            &schema("t", scores.first().map_or(0, Vec::len)),
            constraints,
        )
        .unwrap()
}

fn confirmed(source: &str, target: &str) -> MatchConstraints {
    MatchConstraints {
        confirmed: vec![FieldPair::new(source, target)],
        ..MatchConstraints::default()
    }
}

fn selected(report: &MatchReport, source: usize) -> Option<&str> {
    report.fields[source]
        .selected
        .as_ref()
        .map(|candidate| candidate.target.0.as_str())
}

fn diagnostics() -> GlobalDiagnosticsConfig {
    GlobalDiagnosticsConfig {
        max_solves: 128,
        objective_margin: 4.0,
        ..GlobalDiagnosticsConfig::default()
    }
}

#[test]
fn empty_constraints_preserve_the_entire_existing_report() {
    for one_to_one in [false, true] {
        for abstain_on_ambiguity in [false, true] {
            for scores in [
                vec![vec![Some(0.9), Some(0.9)], vec![Some(0.8), None]],
                vec![vec![None; 2]; 3],
                vec![vec![]; 2],
                vec![],
            ] {
                let (engine, _) = matrix_engine(
                    &scores,
                    Config {
                        one_to_one,
                        abstain_on_ambiguity,
                        global_diagnostics: diagnostics(),
                        ..config()
                    },
                );
                let source = schema("s", scores.len());
                let target = schema("t", scores.first().map_or(0, Vec::len));
                assert_eq!(
                    engine.match_schemas(&source, &target).unwrap(),
                    engine
                        .match_schemas_with_constraints(
                            &source,
                            &target,
                            &MatchConstraints::default(),
                        )
                        .unwrap()
                );
            }
        }
    }
}

#[test]
fn confirmation_selects_beyond_top_k_without_changing_scores_or_evidence() {
    let scores = vec![vec![Some(0.9), Some(0.8), Some(0.01)]];
    for one_to_one in [false, true] {
        let config = Config {
            one_to_one,
            max_candidates: 1,
            ..config()
        };
        let original = run(&scores, config.clone(), &MatchConstraints::default());
        let report = run(&scores, config, &confirmed("s0", "t2"));
        let field = &report.fields[0];
        assert_eq!(field.decision, Decision::Confirmed);
        assert_eq!(field.candidates, original.fields[0].candidates);
        assert_eq!(field.candidates[0].target.0, "t0");
        let selection = field.selected.as_ref().unwrap();
        assert_eq!(selection.target.0, "t2");
        assert_eq!(selection.score, 0.01);
        assert_eq!(selection.signals[0].evidence.score, Some(0.01));
        assert!(!selection.eligible);
        assert!(selection
            .issues
            .contains(&CandidateIssue::InsufficientScore));
        assert!(field
            .diagnostics
            .contains(&FieldDiagnostic::ConfirmedByCaller));
        assert_eq!(
            report.unmatched_targets,
            vec![FieldId::from("t0"), "t1".into()]
        );
        assert!(report.unmatched_sources.is_empty());
    }
}

#[test]
fn confirmation_retains_local_ambiguity_as_evidence() {
    let scores = vec![vec![Some(0.9), Some(0.9)]];
    for one_to_one in [false, true] {
        let report = run(
            &scores,
            Config {
                one_to_one,
                abstain_on_ambiguity: true,
                ..config()
            },
            &confirmed("s0", "t1"),
        );
        assert_eq!(selected(&report, 0), Some("t1"));
        assert_eq!(report.fields[0].decision, Decision::Confirmed);
        assert_eq!(
            report.fields[0].alternatives,
            vec!["t0".into(), "t1".into()]
        );
        assert!(report.fields[0]
            .diagnostics
            .contains(&FieldDiagnostic::LocalAmbiguity));
    }
}

#[test]
fn confirmation_can_record_zero_and_missing_evidence_without_fabricating_a_score() {
    for score in [None, Some(0.0)] {
        let report = run(&[vec![score]], config(), &confirmed("s0", "t0"));
        let candidate = report.fields[0].selected.as_ref().unwrap();
        assert_eq!(candidate.score, 0.0);
        assert_eq!(candidate.signals[0].evidence.score, score);
        assert!(!candidate.eligible);
        assert_eq!(report.fields[0].decision, Decision::Confirmed);
    }
}

#[test]
fn confirmation_records_the_callers_choice_even_when_corroboration_is_missing() {
    let engine = MatchEngine::new(Config {
        corroboration: Some(Corroboration::default()),
        ..Config::default()
    })
    .unwrap();
    let source = Schema::new(vec![Field::new("s0", "amount", DataType::Decimal)]);
    let target = Schema::new(vec![Field::new("t0", "amount", DataType::Decimal)]);
    let original = engine.match_schemas(&source, &target).unwrap();
    let report = engine
        .match_schemas_with_constraints(&source, &target, &confirmed("s0", "t0"))
        .unwrap();
    assert!(original.fields[0].selected.is_none());
    let selection = report.fields[0].selected.as_ref().unwrap();
    assert_eq!(selection.score, original.fields[0].candidates[0].score);
    assert_eq!(selection.signals, original.fields[0].candidates[0].signals);
    assert_eq!(
        selection.eligible,
        original.fields[0].candidates[0].eligible
    );
    assert!(selection
        .issues
        .contains(&CandidateIssue::InsufficientSampleSupport));
}

#[test]
fn confirmed_targets_are_reserved_only_in_one_to_one_mode() {
    let scores = vec![vec![Some(0.1), Some(0.2)], vec![Some(0.9), Some(0.8)]];
    for one_to_one in [false, true] {
        let report = run(
            &scores,
            Config {
                one_to_one,
                ..config()
            },
            &confirmed("s0", "t0"),
        );
        assert_eq!(selected(&report, 0), Some("t0"));
        assert_eq!(
            selected(&report, 1),
            Some(if one_to_one { "t1" } else { "t0" })
        );
        let candidate = &report.fields[1].candidates[0];
        assert_eq!(candidate.target.0, "t0");
        assert_eq!(candidate.eligible, !one_to_one);
        assert_eq!(
            candidate
                .issues
                .contains(&CandidateIssue::TargetConfirmedByCaller),
            one_to_one
        );
        assert!(report.target_competition.is_empty());
    }
}

#[test]
fn duplicate_names_are_addressed_by_stable_ids_and_independent_confirmations_can_share_targets() {
    let constraints = MatchConstraints {
        confirmed: vec![FieldPair::new("s0", "t0"), FieldPair::new("s1", "t0")],
        ..MatchConstraints::default()
    };
    let report = run(&[vec![Some(0.1)], vec![Some(0.9)]], config(), &constraints);
    assert_eq!(selected(&report, 0), Some("t0"));
    assert_eq!(selected(&report, 1), Some("t0"));
    assert!(report
        .fields
        .iter()
        .all(|field| field.decision == Decision::Confirmed));
    assert!(report.unmatched_targets.is_empty());
}

#[test]
fn reserving_the_only_target_leaves_other_sources_unmatched() {
    let report = run(
        &[vec![Some(0.1)], vec![Some(0.9)]],
        Config {
            one_to_one: true,
            ..config()
        },
        &confirmed("s0", "t0"),
    );
    assert_eq!(selected(&report, 0), Some("t0"));
    assert_eq!(selected(&report, 1), None);
    assert_eq!(report.unmatched_sources, vec![FieldId::from("s1")]);
    assert!(report.target_competition.is_empty());
}

#[test]
fn forbidding_a_tied_pair_resolves_ambiguity_and_preserves_its_ranked_evidence() {
    let scores = vec![vec![Some(0.9), Some(0.9)]];
    let constraints = MatchConstraints {
        forbidden: vec![FieldPair::new("s0", "t0")],
        ..MatchConstraints::default()
    };
    for one_to_one in [false, true] {
        let config = Config {
            one_to_one,
            abstain_on_ambiguity: true,
            ..config()
        };
        let original = run(&scores, config.clone(), &MatchConstraints::default());
        let report = run(&scores, config, &constraints);
        assert_eq!(original.fields[0].decision, Decision::Ambiguous);
        assert_eq!(report.fields[0].decision, Decision::Proposed);
        assert_eq!(selected(&report, 0), Some("t1"));
        assert_eq!(report.fields[0].alternatives, vec![FieldId::from("t1")]);
        let candidate = &report.fields[0].candidates[0];
        assert_eq!(candidate.target.0, "t0");
        assert_eq!(candidate.score, 0.9);
        assert_eq!(candidate.signals, original.fields[0].candidates[0].signals);
        assert!(!candidate.eligible);
        assert!(candidate
            .issues
            .contains(&CandidateIssue::ForbiddenByCaller));
    }
}

#[test]
fn explicit_unmatched_sources_are_excluded_without_deleting_evidence() {
    let scores = vec![vec![Some(1.0); 2], vec![Some(0.9), Some(0.8)]];
    let constraints = MatchConstraints {
        unmatched_sources: vec!["s0".into()],
        ..MatchConstraints::default()
    };
    for one_to_one in [false, true] {
        let report = run(
            &scores,
            Config {
                one_to_one,
                ..config()
            },
            &constraints,
        );
        let excluded = &report.fields[0];
        assert_eq!(excluded.decision, Decision::ExcludedByCaller);
        assert!(excluded.selected.is_none());
        assert!(excluded.alternatives.is_empty());
        assert!(excluded
            .diagnostics
            .contains(&FieldDiagnostic::ExcludedByCaller));
        assert_eq!(excluded.candidates.len(), 2);
        assert!(excluded
            .candidates
            .iter()
            .all(|candidate| candidate.score == 1.0
                && !candidate.eligible
                && candidate
                    .issues
                    .contains(&CandidateIssue::SourceExcludedByCaller)));
        assert_eq!(selected(&report, 1), Some("t0"));
        assert_eq!(report.unmatched_sources, vec![FieldId::from("s0")]);
        assert!(report.target_competition.is_empty());
    }
}

#[test]
fn identical_directives_and_input_order_are_idempotent() {
    let scores = vec![vec![Some(0.9); 3]; 3];
    let mut constraints = MatchConstraints {
        confirmed: vec![FieldPair::new("s0", "t1")],
        forbidden: vec![FieldPair::new("s1", "t0"), FieldPair::new("s2", "t1")],
        unmatched_sources: vec!["s2".into()],
    };
    for one_to_one in [false, true] {
        let (engine, _) = matrix_engine(
            &scores,
            Config {
                one_to_one,
                ..config()
            },
        );
        let mut source = schema("s", 3);
        let mut target = schema("t", 3);
        let first = engine
            .match_schemas_with_constraints(&source, &target, &constraints)
            .unwrap();
        constraints.confirmed.extend(constraints.confirmed.clone());
        constraints.forbidden.extend(constraints.forbidden.clone());
        constraints
            .unmatched_sources
            .extend(constraints.unmatched_sources.clone());
        constraints.forbidden.reverse();
        source.fields.reverse();
        target.fields.rotate_left(1);
        assert_eq!(
            first,
            engine
                .match_schemas_with_constraints(&source, &target, &constraints)
                .unwrap()
        );
    }
}

#[test]
fn unknown_ids_and_contradictory_directives_fail_before_callbacks_with_private_errors() {
    let cases = [
        (
            confirmed("private-source-id", "t0"),
            ConstraintError::UnknownSource,
        ),
        (
            confirmed("s0", "private-target-id"),
            ConstraintError::UnknownTarget,
        ),
        (
            MatchConstraints {
                forbidden: vec![FieldPair::new("private-source-id", "t0")],
                ..MatchConstraints::default()
            },
            ConstraintError::UnknownSource,
        ),
        (
            MatchConstraints {
                forbidden: vec![FieldPair::new("s0", "private-target-id")],
                ..MatchConstraints::default()
            },
            ConstraintError::UnknownTarget,
        ),
        (
            MatchConstraints {
                unmatched_sources: vec!["private-source-id".into()],
                ..MatchConstraints::default()
            },
            ConstraintError::UnknownSource,
        ),
        (
            MatchConstraints {
                confirmed: vec![FieldPair::new("s0", "t0"), FieldPair::new("s0", "t1")],
                ..MatchConstraints::default()
            },
            ConstraintError::ConflictingSource,
        ),
        (
            MatchConstraints {
                confirmed: vec![FieldPair::new("s0", "t0"), FieldPair::new("s1", "t0")],
                ..MatchConstraints::default()
            },
            ConstraintError::ConflictingTarget,
        ),
        (
            MatchConstraints {
                forbidden: vec![FieldPair::new("s0", "t0")],
                ..confirmed("s0", "t0")
            },
            ConstraintError::ConfirmedForbidden,
        ),
        (
            MatchConstraints {
                unmatched_sources: vec!["s0".into()],
                ..confirmed("s0", "t0")
            },
            ConstraintError::ConfirmedUnmatched,
        ),
    ];
    for (constraints, reason) in cases {
        let (engine, calls) = matrix_engine(
            &vec![vec![Some(1.0); 2]; 2],
            Config {
                one_to_one: true,
                ..config()
            },
        );
        let error = engine
            .match_schemas_with_constraints(&schema("s", 2), &schema("t", 2), &constraints)
            .unwrap_err();
        assert_eq!(error, MatchError::InvalidConstraints(reason));
        assert!(error.source().is_some());
        for rendering in [
            error.to_string(),
            format!("{error:?}"),
            error.source().unwrap().to_string(),
        ] {
            assert!(!rendering.contains("private-source-id"));
            assert!(!rendering.contains("private-target-id"));
        }
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn confirmation_respects_the_enabled_type_veto() {
    let source = Schema::new(vec![Field::new("s0", "value", DataType::Boolean)]);
    let target = Schema::new(vec![Field::new("t0", "value", DataType::Date)]);
    for reject_incompatible_types in [true, false] {
        let (engine, calls) = matrix_engine(
            &[vec![Some(0.9)]],
            Config {
                reject_incompatible_types,
                ..config()
            },
        );
        let result =
            engine.match_schemas_with_constraints(&source, &target, &confirmed("s0", "t0"));
        if reject_incompatible_types {
            assert_eq!(
                result.unwrap_err(),
                MatchError::InvalidConstraints(ConstraintError::IncompatibleTypes)
            );
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        } else {
            let report = result.unwrap();
            assert_eq!(report.fields[0].decision, Decision::Confirmed);
            assert!(report.fields[0]
                .selected
                .as_ref()
                .unwrap()
                .issues
                .contains(&CandidateIssue::IncompatibleTypes));
            assert_eq!(calls.load(Ordering::SeqCst), 1);
        }
    }
}

#[test]
fn confirmation_cannot_override_any_supplied_semantic_conflict() {
    for axis in [
        SemanticAxis::Unit,
        SemanticAxis::Currency,
        SemanticAxis::IdentifierScope,
    ] {
        let hints = |value: &str| match axis {
            SemanticAxis::Unit => SemanticHints {
                unit: Some(value.into()),
                ..SemanticHints::default()
            },
            SemanticAxis::Currency => SemanticHints {
                currency: Some(value.into()),
                ..SemanticHints::default()
            },
            SemanticAxis::IdentifierScope => SemanticHints {
                identifier_scope: Some(value.into()),
                ..SemanticHints::default()
            },
            _ => unreachable!(),
        };
        let source =
            Schema::new(vec![Field::new("s0", "duplicate", DataType::Text)
                .with_hints(hints("private-source-hint"))]);
        let target =
            Schema::new(vec![Field::new("t0", "duplicate", DataType::Text)
                .with_hints(hints("private-target-hint"))]);
        let (engine, calls) = matrix_engine(
            &[vec![Some(1.0)]],
            Config {
                reject_incompatible_types: false,
                ..config()
            },
        );
        let error = engine
            .match_schemas_with_constraints(&source, &target, &confirmed("s0", "t0"))
            .unwrap_err();
        assert_eq!(
            error,
            MatchError::InvalidConstraints(ConstraintError::SemanticConflict(axis))
        );
        assert!(!format!("{error} {error:?}").contains("private-"));
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn excluded_rows_still_receive_full_schema_validation() {
    let mut source = schema("s", 1);
    source.fields[0].samples = Some(vec![SampleValue::Number(f64::NAN)]);
    let constraints = MatchConstraints {
        unmatched_sources: vec!["s0".into()],
        ..MatchConstraints::default()
    };
    let (engine, calls) = matrix_engine(&[vec![Some(1.0)]], config());
    assert_eq!(
        engine
            .match_schemas_with_constraints(&source, &schema("t", 1), &constraints)
            .unwrap_err(),
        MatchError::InvalidInput(InputError::NonFiniteSample)
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn all_pairs_are_scored_even_if_confirmation_exclusion_or_forbidding_removes_selection_work() {
    let constraints = MatchConstraints {
        confirmed: vec![FieldPair::new("s0", "t0")],
        forbidden: vec![FieldPair::new("s1", "t1")],
        unmatched_sources: vec!["s2".into()],
    };
    for one_to_one in [false, true] {
        let (engine, calls) = matrix_engine(
            &vec![vec![Some(1.0); 2]; 3],
            Config {
                one_to_one,
                ..config()
            },
        );
        engine
            .match_schemas_with_constraints(&schema("s", 3), &schema("t", 2), &constraints)
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 6);
    }
}

#[test]
fn caller_directives_cannot_bypass_pair_or_signal_budgets() {
    let constraints = MatchConstraints {
        unmatched_sources: vec!["s0".into(), "s1".into()],
        ..MatchConstraints::default()
    };
    for budget in [BudgetKind::Pairs, BudgetKind::SignalEvaluations] {
        let mut config = config();
        if budget == BudgetKind::Pairs {
            config.limits.max_pairs = 3;
        } else {
            config.limits.max_signal_evaluations = 3;
        }
        let (engine, calls) = matrix_engine(&vec![vec![Some(1.0); 2]; 2], config);
        assert_eq!(
            engine
                .match_schemas_with_constraints(&schema("s", 2), &schema("t", 2), &constraints)
                .unwrap_err(),
            MatchError::BudgetExceeded(budget)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn raw_duplicate_constraint_counts_are_bounded_before_deduplication() {
    let constraints = [
        MatchConstraints {
            confirmed: vec![FieldPair::new("s0", "t0"); 2],
            ..MatchConstraints::default()
        },
        MatchConstraints {
            unmatched_sources: vec!["s0".into(); 2],
            ..MatchConstraints::default()
        },
        MatchConstraints {
            forbidden: vec![FieldPair::new("s0", "t0"); 3],
            ..MatchConstraints::default()
        },
    ];
    for constraints in constraints {
        let mut config = config();
        config.limits.max_fields = 1;
        config.limits.max_pairs = 2;
        let (engine, calls) = matrix_engine(&[vec![Some(1.0)]], config);
        assert_eq!(
            engine
                .match_schemas_with_constraints(&schema("s", 1), &schema("t", 1), &constraints)
                .unwrap_err(),
            MatchError::BudgetExceeded(BudgetKind::Constraints)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn constraint_id_byte_limits_apply_before_unknown_id_lookup() {
    for constraints in [
        confirmed("source-id-too-long", "t0"),
        confirmed("s0", "target-id-too-long"),
        MatchConstraints {
            forbidden: vec![FieldPair::new("s0", "target-id-too-long")],
            ..MatchConstraints::default()
        },
        MatchConstraints {
            unmatched_sources: vec!["source-id-too-long".into()],
            ..MatchConstraints::default()
        },
    ] {
        let mut config = config();
        config.limits.max_name_bytes = 10;
        let (engine, calls) = matrix_engine(&[vec![Some(1.0)]], config);
        assert_eq!(
            engine
                .match_schemas_with_constraints(&schema("s", 1), &schema("t", 1), &constraints)
                .unwrap_err(),
            MatchError::BudgetExceeded(BudgetKind::NameBytes)
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

#[test]
fn assignment_diagnostics_only_probe_the_residual_automatic_problem() {
    let scores = vec![vec![Some(0.9); 4]; 4];
    let constraints = MatchConstraints {
        unmatched_sources: vec!["s3".into()],
        ..confirmed("s0", "t0")
    };
    let report = run(
        &scores,
        Config {
            one_to_one: true,
            global_diagnostics: diagnostics(),
            ..config()
        },
        &constraints,
    );
    let diagnostics = &report.assignment_diagnostics;
    assert_eq!(diagnostics.status, AssignmentDiagnosticStatus::Complete);
    assert_eq!(diagnostics.base_objective, Some(1.8));
    assert_eq!(diagnostics.solves_used, 2);
    // Fixed edges are absent, while conservative work retains input dimensions.
    assert_eq!(diagnostics.work_used, 256); // Two probes, each 4 * 4 * (4 + 4).
    assert!(!diagnostics.alternatives.is_empty());
    for alternative in &diagnostics.alternatives {
        assert_eq!(alternative.objective, 1.8);
        assert_eq!(alternative.gap, 0.0);
        for change in &alternative.changes {
            assert!(matches!(change.source.0.as_str(), "s1" | "s2"));
            assert!(change
                .alternative_target
                .as_ref()
                .is_none_or(|id| id.0 != "t0"));
        }
    }
}

#[test]
fn diagnostic_work_budget_retains_original_dimensions_before_each_solve() {
    let constraints = MatchConstraints {
        unmatched_sources: vec!["s3".into()],
        ..confirmed("s0", "t0")
    };
    for (max_work, solves, work, status) in [
        (127, 0, 0, AssignmentDiagnosticStatus::BudgetExhausted),
        (128, 1, 128, AssignmentDiagnosticStatus::BudgetExhausted),
        (256, 2, 256, AssignmentDiagnosticStatus::Complete),
    ] {
        let report = run(
            &vec![vec![Some(0.9); 4]; 4],
            Config {
                one_to_one: true,
                global_diagnostics: GlobalDiagnosticsConfig {
                    max_work,
                    ..diagnostics()
                },
                ..config()
            },
            &constraints,
        );
        assert_eq!(report.assignment_diagnostics.status, status);
        assert_eq!(report.assignment_diagnostics.solves_used, solves);
        assert_eq!(report.assignment_diagnostics.work_used, work);
        assert_eq!(report.assignment_diagnostics.base_objective, Some(1.8));
        assert_eq!(selected(&report, 0), Some("t0"));
    }
}

#[test]
fn fixed_only_problems_have_zero_residual_objective_and_no_probes() {
    let constraints = MatchConstraints {
        unmatched_sources: vec!["s1".into()],
        ..confirmed("s0", "t0")
    };
    let report = run(
        &vec![vec![Some(0.9); 2]; 2],
        Config {
            one_to_one: true,
            global_diagnostics: GlobalDiagnosticsConfig {
                max_work: 0,
                ..diagnostics()
            },
            ..config()
        },
        &constraints,
    );
    assert_eq!(
        report.assignment_diagnostics.status,
        AssignmentDiagnosticStatus::Complete
    );
    assert_eq!(report.assignment_diagnostics.base_objective, Some(0.0));
    assert_eq!(report.assignment_diagnostics.solves_used, 0);
    assert_eq!(report.assignment_diagnostics.work_used, 0);
    assert!(report.assignment_diagnostics.alternatives.is_empty());
    assert_eq!(selected(&report, 0), Some("t0"));
}

fn brute_force_residual(matrix: &[Vec<Option<f64>>]) -> f64 {
    fn visit(matrix: &[Vec<Option<f64>>], row: usize, used: &mut BTreeSet<usize>) -> f64 {
        let Some(scores) = matrix.get(row) else {
            return 0.0;
        };
        let mut best = visit(matrix, row + 1, used);
        for (column, score) in scores.iter().enumerate() {
            if let Some(score) = score {
                if used.insert(column) {
                    best = best.max(score + visit(matrix, row + 1, used));
                    used.remove(&column);
                }
            }
        }
        best
    }
    visit(matrix, 0, &mut BTreeSet::new())
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    #[test]
    fn constrained_assignment_matches_a_brute_force_oracle_and_is_reproducible(
        rows in 1usize..5,
        columns in 1usize..5,
        raw in prop::collection::vec(0u8..11, 16),
        forbidden_mask in prop::collection::vec(any::<bool>(), 16),
        fix_first in any::<bool>(),
        exclude_last in any::<bool>(),
    ) {
        let matrix: Vec<Vec<_>> = (0..rows).map(|row| (0..columns).map(|column|
            Some(f64::from(raw[row * 4 + column]) / 10.0)).collect()).collect();
        let excluded = exclude_last && !(fix_first && rows == 1);
        let mut constraints = MatchConstraints::default();
        if fix_first { constraints.confirmed.push(FieldPair::new("s0", "t0")); }
        if excluded { constraints.unmatched_sources.push(format!("s{}", rows - 1).into()); }
        for row in 0..rows {
            for column in 0..columns {
                if forbidden_mask[row * 4 + column] && !(fix_first && row == 0 && column == 0) {
                    constraints.forbidden.push(FieldPair::new(format!("s{row}"), format!("t{column}")));
                }
            }
        }
        let residual: Vec<Vec<_>> = (0..rows)
            .filter(|row| !(fix_first && *row == 0 || excluded && *row == rows - 1))
            .map(|row| (0..columns).filter(|column| !(fix_first && *column == 0))
                .map(|column| matrix[row][column].filter(|score|
                    *score >= 0.1 && !forbidden_mask[row * 4 + column])).collect()).collect();
        let optimum = brute_force_residual(&residual);
        let (engine, _) = matrix_engine(&matrix, Config {
            one_to_one: true,
            max_candidates: 1,
            global_diagnostics: diagnostics(),
            ..config()
        });
        let mut source = schema("s", rows);
        let mut target = schema("t", columns);
        let report = engine.match_schemas_with_constraints(&source, &target, &constraints).unwrap();
        let mut objective = 0.0;
        let mut used = BTreeSet::new();
        for (row, field) in report.fields.iter().enumerate() {
            if fix_first && row == 0 {
                prop_assert_eq!(&field.decision, &Decision::Confirmed);
                prop_assert_eq!(selected(&report, row), Some("t0"));
            } else if excluded && row == rows - 1 {
                prop_assert_eq!(&field.decision, &Decision::ExcludedByCaller);
                prop_assert!(field.selected.is_none());
            }
            if let Some(selection) = &field.selected {
                let column = selection.target.0[1..].parse::<usize>().unwrap();
                prop_assert!(used.insert(column));
                prop_assert!(!forbidden_mask[row * 4 + column] || fix_first && row == 0 && column == 0);
                if !(fix_first && row == 0) {
                    prop_assert!(selection.eligible);
                    objective += selection.score;
                }
            }
        }
        prop_assert!((objective - optimum).abs() < 1e-12);
        prop_assert!((report.assignment_diagnostics.base_objective.unwrap() - optimum).abs() < 1e-12);
        prop_assert_eq!(report.assignment_diagnostics.status, AssignmentDiagnosticStatus::Complete);
        for alternative in &report.assignment_diagnostics.alternatives {
            let mut choices: Vec<_> = report.fields.iter().map(|field|
                field.selected.as_ref().map(|candidate| candidate.target.clone())).collect();
            for change in &alternative.changes {
                let row = change.source.0[1..].parse::<usize>().unwrap();
                prop_assert!(!(fix_first && row == 0));
                prop_assert!(!(excluded && row == rows - 1));
                prop_assert_eq!(&choices[row], &change.selected_target);
                choices[row] = change.alternative_target.clone();
            }
            let mut alternative_objective = 0.0;
            let mut used = BTreeSet::new();
            for (row, choice) in choices.iter().enumerate() {
                if let Some(choice) = choice {
                    let column = choice.0[1..].parse::<usize>().unwrap();
                    prop_assert!(used.insert(column));
                    if !(fix_first && row == 0) {
                        prop_assert!(!forbidden_mask[row * 4 + column]);
                        prop_assert!(matrix[row][column].unwrap() >= 0.1);
                        alternative_objective += matrix[row][column].unwrap();
                    }
                }
            }
            prop_assert!((alternative_objective - alternative.objective).abs() < 1e-12);
            prop_assert!((optimum - alternative_objective - alternative.gap).abs() < 1e-12);
        }
        source.fields.reverse();
        target.fields.reverse();
        constraints.forbidden.reverse();
        prop_assert_eq!(report, engine.match_schemas_with_constraints(&source, &target, &constraints).unwrap());
    }
}
