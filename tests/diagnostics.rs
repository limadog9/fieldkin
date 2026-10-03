//! Global diagnostic witnesses must explain selections without changing them.

use fieldkin::{
    AssignmentDiagnosticStatus, Config, DataType, Evidence, Field, GlobalDiagnosticsConfig,
    MatchEngine, MatchReport, Matcher, Schema, WeightedMatcher,
};

struct MatrixMatcher(Vec<Vec<Option<f64>>>);

impl Matcher for MatrixMatcher {
    fn name(&self) -> &str {
        "matrix"
    }

    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        let source_index = source.id.0[1..].parse::<usize>().unwrap();
        let target_index = target.id.0[1..].parse::<usize>().unwrap();
        Ok(Evidence {
            score: self.0[source_index][target_index],
            explanation: "Fixed synthetic score".into(),
        })
    }
}

fn schema(prefix: &str, count: usize) -> Schema {
    Schema::new(
        (0..count)
            .map(|index| Field::new(format!("{prefix}{index}"), "duplicated", DataType::Text))
            .collect(),
    )
}

fn config() -> Config {
    Config {
        one_to_one: true,
        abstain_on_ambiguity: false,
        min_score: 0.1,
        global_diagnostics: GlobalDiagnosticsConfig {
            max_solves: 128,
            objective_margin: 0.1,
            ..GlobalDiagnosticsConfig::default()
        },
        ..Config::default()
    }
}

fn engine(matrix: &[Vec<Option<f64>>], config: Config) -> MatchEngine {
    MatchEngine::with_matchers(
        config,
        vec![WeightedMatcher::new(1.0, MatrixMatcher(matrix.to_vec()))],
    )
    .unwrap()
}

fn result(matrix: &[Vec<Option<f64>>], config: Config) -> MatchReport {
    engine(matrix, config)
        .match_schemas(
            &schema("s", matrix.len()),
            &schema("t", matrix.first().map_or(0, Vec::len)),
        )
        .unwrap()
}

#[test]
fn diagnostics_preserve_every_selection_and_original_decision() {
    for matrix in [
        vec![vec![Some(0.9), Some(0.8)], vec![Some(0.85), None]],
        vec![vec![Some(0.9); 2]; 3],
        vec![vec![None; 2]; 3],
    ] {
        for abstain_on_ambiguity in [false, true] {
            let enabled = result(
                &matrix,
                Config {
                    abstain_on_ambiguity,
                    ..config()
                },
            );
            let disabled = result(
                &matrix,
                Config {
                    abstain_on_ambiguity,
                    global_diagnostics: GlobalDiagnosticsConfig::default(),
                    ..config()
                },
            );
            assert_eq!(enabled.fields, disabled.fields);
            assert_eq!(enabled.unmatched_sources, disabled.unmatched_sources);
            assert_eq!(enabled.unmatched_targets, disabled.unmatched_targets);
            assert_eq!(
                enabled.assignment_diagnostics.status,
                AssignmentDiagnosticStatus::Complete
            );
            assert_eq!(
                disabled.assignment_diagnostics.status,
                AssignmentDiagnosticStatus::Disabled
            );
        }
    }
}

#[test]
fn input_reordering_preserves_full_report_and_witnesses() {
    let matrix = vec![vec![Some(0.9); 3]; 4];
    let engine = engine(&matrix, config());
    let mut source = schema("s", 4);
    let mut target = schema("t", 3);
    let original = engine.match_schemas(&source, &target).unwrap();
    source.fields.reverse();
    target.fields.rotate_left(1);
    assert_eq!(original, engine.match_schemas(&source, &target).unwrap());
    assert!(!original.assignment_diagnostics.alternatives.is_empty());
}

#[test]
fn presentation_top_k_does_not_limit_assignment_probes() {
    let matrix = vec![vec![Some(0.9), Some(0.85)], vec![Some(0.86), None]];
    let full = result(&matrix, config());
    let truncated = result(
        &matrix,
        Config {
            max_candidates: 1,
            ..config()
        },
    );
    assert_eq!(
        full.assignment_diagnostics,
        truncated.assignment_diagnostics
    );
    assert_eq!(full.fields[0].selected, truncated.fields[0].selected);
    assert_eq!(
        truncated.fields[0].selected.as_ref().unwrap().target.0,
        "t1"
    );
    assert_eq!(truncated.fields[0].candidates[0].target.0, "t0");
}

#[test]
fn independent_mode_marks_diagnostics_not_applicable() {
    let report = result(
        &[vec![Some(0.9)]],
        Config {
            one_to_one: false,
            ..config()
        },
    );
    assert_eq!(
        report.assignment_diagnostics.status,
        AssignmentDiagnosticStatus::NotApplicable
    );
    assert_eq!(report.assignment_diagnostics.base_objective, None);
    assert_eq!(report.assignment_diagnostics.solves_used, 0);
    assert_eq!(report.assignment_diagnostics.work_used, 0);
    assert!(report.assignment_diagnostics.alternatives.is_empty());
}

#[test]
fn local_abstention_remains_a_constraint_on_global_probes() {
    let report = result(
        &[vec![Some(0.9), Some(0.9)]],
        Config {
            abstain_on_ambiguity: true,
            ..config()
        },
    );
    assert_eq!(
        report.assignment_diagnostics.status,
        AssignmentDiagnosticStatus::Complete
    );
    assert_eq!(report.assignment_diagnostics.base_objective, Some(0.0));
    assert_eq!(report.assignment_diagnostics.solves_used, 0);
    assert!(report.assignment_diagnostics.alternatives.is_empty());
    assert!(report.fields[0].selected.is_none());
}

#[test]
fn budget_exhaustion_is_explicit_and_does_not_hide_found_witnesses() {
    let matrix = vec![vec![Some(0.9); 2]; 2];
    let partial = result(
        &matrix,
        Config {
            global_diagnostics: GlobalDiagnosticsConfig {
                max_solves: 1,
                objective_margin: 0.0,
                ..GlobalDiagnosticsConfig::default()
            },
            ..config()
        },
    );
    assert_eq!(
        partial.assignment_diagnostics.status,
        AssignmentDiagnosticStatus::BudgetExhausted
    );
    assert_eq!(partial.assignment_diagnostics.solves_used, 1);
    assert_eq!(partial.assignment_diagnostics.work_used, 16);
    assert_eq!(partial.assignment_diagnostics.alternatives.len(), 1);
    assert_eq!(partial.fields, result(&matrix, config()).fields);
}

#[test]
fn invalid_budgets_and_nonfinite_margins_fail_engine_construction() {
    for diagnostics in [
        GlobalDiagnosticsConfig {
            max_solves: 1_025,
            ..GlobalDiagnosticsConfig::default()
        },
        GlobalDiagnosticsConfig {
            objective_margin: f64::NAN,
            ..GlobalDiagnosticsConfig::default()
        },
        GlobalDiagnosticsConfig {
            objective_margin: f64::INFINITY,
            ..GlobalDiagnosticsConfig::default()
        },
        GlobalDiagnosticsConfig {
            objective_margin: -0.1,
            ..GlobalDiagnosticsConfig::default()
        },
    ] {
        assert!(MatchEngine::new(Config {
            global_diagnostics: diagnostics,
            ..Config::default()
        })
        .is_err());
    }
}
