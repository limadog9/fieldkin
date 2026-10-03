//! Deterministic dense-contest diagnostics cost measurement.
//!
//! Run `cargo run --locked --release --example diagnostics_cost`. Each mode has
//! one warmup and three measured calls. CSV is written to stdout. This measures
//! complete public API calls, including report construction, not solver-only
//! latency. It is a cost observation, not a stable CI timing gate.

use std::error::Error;
use std::time::Instant;

use fieldkin::{
    AssignmentDiagnosticStatus, Config, DataType, Evidence, Field, FieldId,
    GlobalDiagnosticsConfig, MatchEngine, MatchReport, Matcher, Schema, WeightedMatcher,
};

struct EqualScores;

impl Matcher for EqualScores {
    fn name(&self) -> &str {
        "fixed"
    }

    fn evaluate(&self, _source: &Field, _target: &Field) -> Result<Evidence, String> {
        Ok(Evidence {
            score: Some(0.9),
            explanation: "Fixed synthetic score".into(),
        })
    }
}

fn schema(prefix: &str, count: usize) -> Schema {
    Schema::new(
        (0..count)
            .map(|index| Field::new(format!("{prefix}{index:03}"), "same_name", DataType::Text))
            .collect(),
    )
}

fn selected(report: &MatchReport) -> Vec<(&FieldId, Option<&FieldId>)> {
    report
        .fields
        .iter()
        .map(|field| {
            (
                &field.source,
                field.selected.as_ref().map(|candidate| &candidate.target),
            )
        })
        .collect()
}

fn engine(diagnostics: GlobalDiagnosticsConfig) -> Result<MatchEngine, fieldkin::MatchError> {
    MatchEngine::with_matchers(
        Config {
            one_to_one: true,
            abstain_on_ambiguity: false,
            global_diagnostics: diagnostics,
            ..Config::default()
        },
        vec![WeightedMatcher::new(1.0, EqualScores)],
    )
}

fn main() -> Result<(), Box<dyn Error>> {
    println!("fixture,source_count,target_count,mode,max_solves,max_work,status,solves_used,work_used,witnesses,base_objective,iteration,elapsed_ns,warmup_calls,measured_calls,debug_assertions,crate_version,target_os,target_arch");
    for (rows, columns) in [(16, 16), (64, 64), (128, 128), (128, 64), (64, 128)] {
        let source = schema("s", rows);
        let target = schema("t", columns);
        let baseline =
            engine(GlobalDiagnosticsConfig::default())?.match_schemas(&source, &target)?;
        let per_solve = rows * rows * (columns + rows);
        let modes = [
            ("disabled", GlobalDiagnosticsConfig::default()),
            (
                "one_probe_budget",
                GlobalDiagnosticsConfig {
                    max_solves: rows,
                    max_work: per_solve,
                    ..GlobalDiagnosticsConfig::default()
                },
            ),
            (
                "default_work_budget",
                GlobalDiagnosticsConfig {
                    max_solves: rows,
                    ..GlobalDiagnosticsConfig::default()
                },
            ),
            (
                "complete",
                GlobalDiagnosticsConfig {
                    max_solves: rows,
                    max_work: per_solve * rows.min(columns),
                    ..GlobalDiagnosticsConfig::default()
                },
            ),
        ];
        for (mode, diagnostics) in modes {
            let max_solves = diagnostics.max_solves;
            let max_work = diagnostics.max_work;
            let engine = engine(diagnostics)?;
            for iteration in 0..4 {
                let start = Instant::now();
                let report = engine.match_schemas(&source, &target)?;
                let elapsed = start.elapsed().as_nanos();
                assert_eq!(selected(&report), selected(&baseline));
                assert_eq!(report.unmatched_sources, baseline.unmatched_sources);
                assert_eq!(report.unmatched_targets, baseline.unmatched_targets);
                assert_eq!(report.fields, baseline.fields);
                let diagnostic = &report.assignment_diagnostics;
                if mode == "one_probe_budget" {
                    assert_eq!(
                        diagnostic.status,
                        AssignmentDiagnosticStatus::BudgetExhausted
                    );
                }
                if mode == "complete" {
                    assert_eq!(diagnostic.status, AssignmentDiagnosticStatus::Complete);
                    assert_eq!(diagnostic.solves_used, rows.min(columns));
                }
                if iteration != 0 {
                    println!(
                        "dense_equal,{rows},{columns},{mode},{max_solves},{max_work},{:?},{},{},{},{},{iteration},{elapsed},1,3,{},{},{},{}",
                        diagnostic.status,
                        diagnostic.solves_used,
                        diagnostic.work_used,
                        diagnostic.alternatives.len(),
                        diagnostic.base_objective.map(|value| value.to_string()).unwrap_or_default(),
                        cfg!(debug_assertions),
                        env!("CARGO_PKG_VERSION"),
                        std::env::consts::OS,
                        std::env::consts::ARCH,
                    );
                }
            }
        }
    }
    Ok(())
}
