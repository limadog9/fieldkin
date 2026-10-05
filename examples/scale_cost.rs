//! Bounded opt-in scale experiment; use `fieldkin-tools perf-scale` for measurements.
//! This raises caller budgets for fixed synthetic inputs, never library defaults.

use std::hint::black_box;
use std::time::Instant;

use fieldkin::{
    AssignmentDiagnosticStatus, Config, DataType, Decision, Evidence, Field, Limits, MatchEngine,
    MatchReport, Matcher, SampleValue, Schema, WeightedMatcher,
};

struct DiagonalMatcher {
    partial: bool,
}

impl Matcher for DiagonalMatcher {
    fn name(&self) -> &str {
        "scale-diagonal"
    }

    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        let index = |field: &Field| {
            field
                .id
                .0
                .get(2..)
                .and_then(|value| value.parse::<usize>().ok())
                .ok_or_else(|| "invalid synthetic field identity".to_string())
        };
        let source = index(source)?;
        let target = index(target)?;
        Ok(Evidence {
            score: Some(if source == target && (!self.partial || source % 8 != 7) {
                0.9
            } else {
                0.0
            }),
            explanation: "Synthetic graph".to_string(),
        })
    }
}

fn fixture(size: usize, builtin: bool) -> (Schema, Schema) {
    let build = |target| {
        Schema::new(
            (0..size)
                .map(|index| {
                    let prefix = if target { "t" } else { "s" };
                    let name = if target {
                        format!("metric_{index:04}_count")
                    } else {
                        format!("Metric{index:04}Count")
                    };
                    let field = Field::new(format!("{prefix}-{index:04}"), name, DataType::Integer);
                    if builtin {
                        field.with_samples(
                            (0..16)
                                .map(|sample| SampleValue::Integer((index * 100 + sample) as i128))
                                .collect(),
                        )
                    } else {
                        field
                    }
                })
                .collect(),
        )
    };
    let source = build(false);
    let mut target = build(true);
    target.fields.reverse();
    (source, target)
}

fn engine(
    size: usize,
    family: &str,
    one_to_one: bool,
) -> Result<MatchEngine, fieldkin::MatchError> {
    let signals = if family == "builtin" { 3 } else { 1 };
    let config = Config {
        one_to_one,
        max_candidates: 1,
        limits: Limits {
            max_fields: size,
            max_pairs: size * size,
            max_signal_evaluations: size * size * signals,
            max_explanation_bytes: 512 * 1024 * 1024,
            ..Limits::default()
        },
        // Retain ordinary thresholds and abstention. Global diagnostic probes
        // remain disabled, so this is one assignment solve per applicable call.
        ..Config::default()
    };
    if family == "builtin" {
        MatchEngine::new(config)
    } else {
        MatchEngine::with_matchers(
            config,
            vec![WeightedMatcher::new(
                1.0,
                DiagonalMatcher {
                    partial: family == "partial",
                },
            )],
        )
    }
}

fn check_report(report: &MatchReport, size: usize, partial: bool) -> Result<(), &'static str> {
    let selected = size - if partial { size / 8 } else { 0 };
    if report.fields.len() != size
        || report.unmatched_sources.len() != size - selected
        || report.unmatched_targets.len() != size - selected
        || report.assignment_diagnostics.status != AssignmentDiagnosticStatus::Disabled
        || report.assignment_diagnostics.solves_used != 0
        || report.assignment_diagnostics.work_used != 0
    {
        return Err("unexpected scale workload counts or diagnostic work");
    }
    for (index, field) in report.fields.iter().enumerate() {
        if partial && index % 8 == 7 {
            if field.selected.is_some() || field.decision != Decision::BelowThreshold {
                return Err("partial graph selected an excluded edge");
            }
        } else if field.decision != Decision::Proposed
            || field.selected.as_ref().is_none_or(|candidate| {
                candidate.target.0 != format!("t-{index:04}") || !candidate.eligible
            })
        {
            return Err("scale workload did not select the expected diagonal");
        }
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let sizes: &[usize] = match args.as_slice() {
        [] => &[128, 512, 1000],
        [option] if option == "--small" => &[128],
        _ => return Err("expected no arguments or --small".into()),
    };
    for &size in sizes {
        for family in ["diagonal", "partial", "builtin"] {
            let (source, target) = fixture(size, family == "builtin");
            for one_to_one in [false, true] {
                let engine = engine(size, family, one_to_one)?;
                let warmup = engine.match_schemas(&source, &target)?;
                check_report(&warmup, size, family == "partial")?;
                drop(black_box(warmup));
                let iterations = if size == 128 { 3 } else { 1 };
                let started = Instant::now();
                for _ in 0..iterations {
                    drop(black_box(
                        engine.match_schemas(black_box(&source), black_box(&target))?,
                    ));
                }
                let elapsed = started.elapsed().as_nanos();
                let selected = size - if family == "partial" { size / 8 } else { 0 };
                let pairs = size * size;
                let signals = if family == "builtin" { 3 } else { 1 };
                println!(
                    "{{\"family\":\"{family}\",\"size\":{size},\"one_to_one\":{one_to_one},\"iterations\":{iterations},\"elapsed_ns\":{elapsed},\"selected\":{selected},\"unmatched\":{},\"pairs\":{pairs},\"signal_evaluations\":{},\"diagnostic_solves\":0,\"diagnostic_work\":0}}",
                    size - selected,
                    pairs * signals,
                );
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn small_fixtures_have_exact_diagonal_and_unmatched_outcomes() {
        for family in ["diagonal", "partial", "builtin"] {
            let (source, target) = fixture(16, family == "builtin");
            for one_to_one in [false, true] {
                let report = engine(16, family, one_to_one)
                    .unwrap()
                    .match_schemas(&source, &target)
                    .unwrap();
                check_report(&report, 16, family == "partial").unwrap();
            }
        }
    }

    #[test]
    fn default_limits_still_reject_the_large_fixture() {
        let (source, target) = fixture(512, false);
        let error = MatchEngine::new(Config::default())
            .unwrap()
            .match_schemas(&source, &target)
            .unwrap_err();
        assert_eq!(
            error,
            fieldkin::MatchError::BudgetExceeded(fieldkin::BudgetKind::Fields)
        );
    }
}
