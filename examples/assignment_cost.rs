//! Fixed assignment-cost workloads for `performance/assignment.py`.
//! No fixture values are logged. `--smoke` performs only one timed call per case.

use std::hint::black_box;
use std::time::Instant;

use fieldkin::{
    Config, DataType, Decision, Evidence, Field, FieldPair, GlobalDiagnosticsConfig,
    MatchConstraints, MatchEngine, MatchReport, Matcher, Schema, WeightedMatcher,
};

const FAMILIES: [&str; 9] = [
    "dense-disabled",
    "dense-bounded",
    "sparse-complete",
    "half-excluded-disabled",
    "all-excluded-complete",
    "mixed-bounded",
    "reserved-complete",
    "wide-complete",
    "tall-bounded",
];

struct GraphMatcher {
    dense: bool,
    live_diagonals: usize,
}

impl Matcher for GraphMatcher {
    fn name(&self) -> &str {
        "fixed-assignment-graph"
    }

    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        let index = |field: &Field| {
            field
                .id
                .0
                .get(2..)
                .and_then(|value| value.parse::<usize>().ok())
                .ok_or_else(|| "invalid synthetic field identifier".to_string())
        };
        let source = index(source)?;
        let target = index(target)?;
        Ok(Evidence {
            score: Some(
                if self.dense || (source == target && source < self.live_diagonals) {
                    0.85
                } else {
                    0.0
                },
            ),
            explanation: "Fixed synthetic graph; no semantic accuracy claim".to_string(),
        })
    }
}

fn schema(prefix: &str, count: usize) -> Schema {
    Schema::new(
        (0..count)
            .map(|index| Field::new(format!("{prefix}-{index:04}"), "metric", DataType::Integer))
            .collect(),
    )
}

fn constraints(family: &str, count: usize) -> MatchConstraints {
    let mut constraints = MatchConstraints::default();
    let (confirmed, excluded) = match family {
        "half-excluded-disabled" => (0, count / 2),
        "all-excluded-complete" => (0, count),
        "mixed-bounded" => (count / 4, count / 4),
        "reserved-complete" => (count - 4, 0),
        _ => (0, 0),
    };
    constraints.confirmed = (0..confirmed)
        .map(|index| FieldPair::new(format!("s-{index:04}"), format!("t-{index:04}")))
        .collect();
    constraints.unmatched_sources = (confirmed..confirmed + excluded)
        .map(|index| format!("s-{index:04}").into())
        .collect();
    if family == "mixed-bounded" {
        // Exclude one wrong cyclic pair per remaining source. Its original score is zero.
        constraints.forbidden = (count / 2..count)
            .map(|index| {
                let wrong = count / 2 + (index - count / 2 + 1) % (count / 2);
                FieldPair::new(format!("s-{index:04}"), format!("t-{wrong:04}"))
            })
            .collect();
    }
    constraints
}

fn behavior(report: &MatchReport) -> String {
    let count = |decision| {
        report
            .fields
            .iter()
            .filter(|field| field.decision == decision)
            .count()
    };
    let diagnostics = &report.assignment_diagnostics;
    let objective = diagnostics
        .base_objective
        .map_or_else(|| "none".to_string(), |value| format!("{value:.17}"));
    format!(
        "{},{},{},{},{},{:?},{},{},{},{}",
        count(Decision::Proposed),
        count(Decision::Confirmed),
        count(Decision::ExcludedByCaller),
        report.unmatched_sources.len(),
        report.unmatched_targets.len(),
        diagnostics.status,
        diagnostics.solves_used,
        diagnostics.work_used,
        diagnostics.alternatives.len(),
        objective,
    )
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let smoke = match args.as_slice() {
        [] => false,
        [option] if option == "--smoke" => true,
        _ => return Err("expected no arguments or --smoke".into()),
    };
    println!("fieldkin fixed assignment comparison; no samples; all original pairs scored");
    println!("one warm-up; construction excluded; matching and report destruction timed");
    println!("family,size,sources,targets,iterations,total_ms,us_per_match,proposed,confirmed,excluded,unmatched_sources,unmatched_targets,status,solves,work,alternatives,objective");
    for size in [16, 64, 128] {
        for family in FAMILIES {
            let sources = if family == "wide-complete" {
                size / 4
            } else {
                size
            };
            let targets = if family == "tall-bounded" {
                size / 4
            } else {
                size
            };
            let source = schema("s", sources);
            let mut target = schema("t", targets);
            target.fields.reverse();
            let constraints = constraints(family, size);
            let max_solves = if family.ends_with("disabled") {
                0
            } else if family.ends_with("bounded") {
                2
            } else {
                128
            };
            let config = Config {
                one_to_one: true,
                abstain_on_ambiguity: false,
                global_diagnostics: GlobalDiagnosticsConfig {
                    max_solves,
                    // Full original-dimension charge for up to 128 probes at the largest shape.
                    max_work: 536_870_912,
                    objective_margin: 0.08,
                },
                ..Config::default()
            };
            let engine = if family == "dense-disabled" {
                // Retain a built-in scoring control with no caller review constraints.
                MatchEngine::new(config)?
            } else {
                MatchEngine::with_matchers(
                    config,
                    vec![WeightedMatcher::new(
                        1.0,
                        GraphMatcher {
                            dense: matches!(
                                family,
                                "dense-bounded"
                                    | "half-excluded-disabled"
                                    | "all-excluded-complete"
                            ),
                            live_diagonals: if matches!(
                                family,
                                "mixed-bounded" | "reserved-complete"
                            ) {
                                size
                            } else {
                                4
                            },
                        },
                    )],
                )?
            };
            let evaluate = || {
                if constraints == MatchConstraints::default() {
                    engine.match_schemas(black_box(&source), black_box(&target))
                } else {
                    engine.match_schemas_with_constraints(
                        black_box(&source),
                        black_box(&target),
                        black_box(&constraints),
                    )
                }
            };
            let warmup = evaluate()?;
            let expected = behavior(&warmup);
            drop(black_box(warmup));
            let iterations = if smoke {
                1
            } else {
                match size {
                    16 => 10,
                    64 => 4,
                    _ => 2,
                }
            };
            let started = Instant::now();
            for _ in 0..iterations {
                drop(black_box(evaluate()?));
            }
            let elapsed = started.elapsed().as_secs_f64();
            // Behavior collection is outside the timed region; the driver checks both revisions.
            if behavior(&evaluate()?) != expected {
                return Err("benchmark behavior changed between calls".into());
            }
            println!(
                "{family},{size},{sources},{targets},{iterations},{:.6},{:.6},{expected}",
                elapsed * 1000.0,
                elapsed * 1_000_000.0 / f64::from(iterations),
            );
        }
    }
    Ok(())
}
