//! Fixed, public-API workloads. See ../README.md for the measurement protocol.

use std::hint::black_box;
use std::time::Instant;

use fieldkin::{
    Config, DataType, Field, FieldId, MatchEngine, NameMatcher, SampleValue, Schema,
    WeightedMatcher,
};

#[cfg(feature = "allocations")]
#[global_allocator]
static GLOBAL: &stats_alloc::StatsAlloc<std::alloc::System> = &stats_alloc::INSTRUMENTED_SYSTEM;

struct Workload {
    id: String,
    source: Schema,
    target: Schema,
    config: Config,
    name_only: bool,
    iterations: u32,
    expected_error: Option<&'static str>,
}

impl Workload {
    fn engine(&self) -> Result<MatchEngine, fieldkin::MatchError> {
        if self.name_only {
            MatchEngine::with_matchers(
                self.config.clone(),
                vec![WeightedMatcher::new(1.0, NameMatcher::default())],
            )
        } else {
            MatchEngine::new(self.config.clone())
        }
    }
}

fn fixture(sources: usize, targets: usize, samples: usize, style: &str) -> (Schema, Schema) {
    let build = |count, target| {
        Schema::new(
            (0..count)
                .map(|i| {
                    let name = match style {
                        "short" => format!("x{i:03}"),
                        "long" => format!(
                            "{}_Metric{i:04}_Count_{}",
                            "regional_transaction_item".repeat(4),
                            "subdepartment_aggregate".repeat(4)
                        ),
                        "competition" => "transaction_amount".to_owned(),
                        _ if target => format!("metric_{i:04}_count"),
                        _ => format!("Metric{i:04}Count"),
                    };
                    let field = Field::new(
                        FieldId(format!("{}-{i:04}", if target { "t" } else { "s" })),
                        name,
                        if style == "rejected" && target {
                            DataType::Boolean
                        } else {
                            DataType::Integer
                        },
                    );
                    if samples == 0 {
                        field
                    } else {
                        field.with_samples(
                            (0..samples)
                                .map(|j| {
                                    if j % 5 == 0 || (style == "sparse" && j % 16 != 1) {
                                        SampleValue::Null
                                    } else {
                                        SampleValue::Number((i * 1000 + j) as f64)
                                    }
                                })
                                .collect(),
                        )
                    }
                })
                .collect(),
        )
    };
    let source = build(sources, false);
    let mut target = build(targets, true);
    target.fields.reverse();
    (source, target)
}

fn workloads() -> Vec<Workload> {
    let mut workloads = Vec::new();
    // Preserve the existing 24 workloads, including its exact sample formula.
    for count in [16, 64, 128] {
        for sampled in [false, true] {
            for one_to_one in [false, true] {
                for name_only in [false, true] {
                    let (mut source, mut target) =
                        fixture(count, count, if sampled { 16 } else { 0 }, "base");
                    if sampled {
                        for schema in [&mut source, &mut target] {
                            for field in &mut schema.fields {
                                for sample in field.samples.iter_mut().flatten() {
                                    if let SampleValue::Number(value) = sample {
                                        let i = (*value as usize) / 1000;
                                        let j = (*value as usize) % 1000;
                                        *value = (i * 100 + j) as f64;
                                    }
                                }
                            }
                        }
                    }
                    workloads.push(Workload {
                        id: format!(
                            "core-{count}-{}-{}-{}",
                            if sampled { "sampled" } else { "empty" },
                            if one_to_one {
                                "assignment"
                            } else {
                                "independent"
                            },
                            if name_only { "name-only" } else { "combined" }
                        ),
                        source,
                        target,
                        config: Config {
                            one_to_one,
                            reject_incompatible_types: !name_only,
                            ..Config::default()
                        },
                        name_only,
                        iterations: match count {
                            16 => 30,
                            64 => 10,
                            _ => 5,
                        },
                        expected_error: None,
                    });
                }
            }
        }
    }
    for (style, sources, targets, samples) in [
        ("short", 64, 64, 0),
        ("long", 64, 64, 0),
        ("sparse", 64, 64, 64),
        ("dense", 64, 64, 256),
        ("rejected", 64, 64, 16),
        ("competition", 64, 64, 0),
        ("unequal-wide", 16, 128, 16),
        ("unequal-tall", 128, 16, 16),
        ("single-pair", 1, 1, 16),
        ("empty-source", 0, 128, 16),
        ("empty-target", 128, 0, 16),
    ] {
        for one_to_one in [false, true] {
            let (source, target) = fixture(sources, targets, samples, style);
            workloads.push(Workload {
                id: format!(
                    "extended-{style}-{}",
                    if one_to_one {
                        "assignment"
                    } else {
                        "independent"
                    }
                ),
                source,
                target,
                config: Config {
                    one_to_one,
                    abstain_on_ambiguity: style != "competition",
                    ..Config::default()
                },
                name_only: false,
                iterations: 5,
                expected_error: None,
            });
        }
    }
    for budget in ["input", "pairs", "signals", "report"] {
        let (source, target) = fixture(16, 16, 16, "base");
        let mut config = Config::default();
        let error = match budget {
            "input" => {
                config.limits.max_fields = 15;
                "field budget exceeded"
            }
            "pairs" => {
                config.limits.max_pairs = 255;
                "source-target pair budget exceeded"
            }
            "signals" => {
                config.limits.max_signal_evaluations = 767;
                "signal evaluation budget exceeded"
            }
            _ => {
                config.limits.max_explanation_bytes = 1;
                "aggregate explanation byte budget exceeded"
            }
        };
        workloads.push(Workload {
            id: format!("budget-{budget}"),
            source,
            target,
            config,
            name_only: false,
            iterations: 500,
            expected_error: Some(error),
        });
    }
    workloads
}

fn invoke(engine: &MatchEngine, workload: &Workload) -> Result<(), Box<dyn std::error::Error>> {
    let result =
        black_box(engine.match_schemas(black_box(&workload.source), black_box(&workload.target)));
    match (result, workload.expected_error) {
        (Ok(report), None) => drop(black_box(report)),
        (Err(error), Some(expected)) if error.0 == expected => drop(black_box(error)),
        (Err(error), _) => return Err(error.into()),
        (Ok(_), Some(_)) => return Err("expected budget rejection".into()),
    }
    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let smoke = std::env::args().skip(1).any(|arg| arg == "--smoke");
    for workload in workloads() {
        let engine = workload.engine()?;
        for _ in 0..if smoke { 1 } else { 3 } {
            invoke(&engine, &workload)?;
        }
        let iterations = if smoke { 1 } else { workload.iterations };
        #[cfg(feature = "allocations")]
        let region = stats_alloc::Region::new(GLOBAL);
        let started = Instant::now();
        for _ in 0..iterations {
            invoke(&engine, &workload)?;
        }
        let elapsed = started.elapsed().as_nanos();
        #[cfg(feature = "allocations")]
        let stats = region.change();
        print!(
            "{{\"workload\":\"{}\",\"iterations\":{iterations},\"elapsed_ns\":{elapsed},\"sources\":{},\"targets\":{},\"expects_error\":{}",
            workload.id,
            workload.source.fields.len(),
            workload.target.fields.len(),
            workload.expected_error.is_some()
        );
        #[cfg(feature = "allocations")]
        print!(
            ",\"allocations\":{},\"reallocations\":{},\"deallocations\":{},\"bytes_allocated\":{},\"bytes_deallocated\":{},\"bytes_reallocated\":{}",
            stats.allocations,
            stats.reallocations,
            stats.deallocations,
            stats.bytes_allocated,
            stats.bytes_deallocated,
            stats.bytes_reallocated
        );
        println!("}}");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn frozen_workloads_are_unique_and_follow_expected_success_or_budget_paths() {
        let cases = workloads();
        assert_eq!(cases.len(), 50);
        let mut ids = BTreeSet::new();
        let mut rejections = 0;
        for case in cases {
            assert!(ids.insert(case.id.clone()), "duplicate workload ID");
            rejections += usize::from(case.expected_error.is_some());
            invoke(&case.engine().expect("valid configuration"), &case)
                .unwrap_or_else(|error| panic!("{}: {error}", case.id));
        }
        assert_eq!(rejections, 4);
    }
}
