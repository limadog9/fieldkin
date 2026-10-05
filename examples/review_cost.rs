//! Reproducible caller-review cost workloads; use `fieldkin-tools perf-review` to measure.
//! `--smoke` executes one timed call per workload and is only a functional check.

use std::hint::black_box;
use std::time::Instant;

use fieldkin::{
    Config, DataType, Field, FieldPair, MatchConstraints, MatchEngine, MatchError, MatchReport,
    SampleValue, Schema,
};

fn fixture(count: usize, samples: bool) -> (Schema, Schema) {
    let build = |target: bool| {
        Schema::new(
            (0..count)
                .map(|i| {
                    let prefix = if target { "t" } else { "s" };
                    let name = if target {
                        format!("metric_{i:04}_count")
                    } else {
                        format!("Metric{i:04}Count")
                    };
                    let field = Field::new(format!("{prefix}-{i:04}"), name, DataType::Integer);
                    if samples {
                        field.with_samples(
                            (0..16)
                                .map(|j| {
                                    if j % 5 == 0 {
                                        SampleValue::Null
                                    } else {
                                        SampleValue::Number((i * 100 + j) as f64)
                                    }
                                })
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

fn mixed_review(count: usize) -> MatchConstraints {
    let quarter = count / 4;
    let half = count / 2;
    MatchConstraints {
        confirmed: (0..quarter)
            .map(|i| FieldPair::new(format!("s-{i:04}"), format!("t-{i:04}")))
            .collect(),
        unmatched_sources: (quarter..half)
            .map(|i| format!("s-{i:04}").into())
            .collect(),
        forbidden: (half..count)
            .map(|i| {
                // Cycle within the unreviewed half, always excluding a wrong pair.
                let wrong = half + (i - half + 1) % half;
                FieldPair::new(format!("s-{i:04}"), format!("t-{wrong:04}"))
            })
            .collect(),
    }
}

fn evaluate(
    engine: &MatchEngine,
    source: &Schema,
    target: &Schema,
    constraints: Option<&MatchConstraints>,
) -> Result<MatchReport, MatchError> {
    match constraints {
        None => engine.match_schemas(black_box(source), black_box(target)),
        Some(constraints) => engine.match_schemas_with_constraints(
            black_box(source),
            black_box(target),
            black_box(constraints),
        ),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    let smoke = match args.as_slice() {
        [] => false,
        [option] if option == "--smoke" => true,
        _ => return Err("expected no arguments or --smoke".into()),
    };
    println!("fieldkin fixed caller-review benchmark; 16 samples/field when enabled");
    println!(
        "one warm-up; input/engine/constraint construction excluded; report destruction included"
    );
    println!("policy,fields_per_side,samples,one_to_one,iterations,total_ms,us_per_match");
    for count in [16, 64, 128] {
        let iterations = if smoke {
            1
        } else {
            match count {
                16 => 30,
                64 => 10,
                _ => 5,
            }
        };
        for samples in [false, true] {
            let (source, target) = fixture(count, samples);
            let empty = MatchConstraints::default();
            let mixed = mixed_review(count);
            for one_to_one in [false, true] {
                let engine = MatchEngine::new(Config {
                    one_to_one,
                    ..Config::default()
                })?;
                for (policy, constraints) in [
                    ("none", None),
                    ("empty", Some(&empty)),
                    ("mixed-review", Some(&mixed)),
                ] {
                    drop(black_box(evaluate(&engine, &source, &target, constraints)?));
                    let started = Instant::now();
                    for _ in 0..iterations {
                        drop(black_box(evaluate(&engine, &source, &target, constraints)?));
                    }
                    let elapsed = started.elapsed().as_secs_f64();
                    println!(
                        "{policy},{count},{samples},{one_to_one},{iterations},{:.3},{:.3}",
                        elapsed * 1000.0,
                        elapsed * 1_000_000.0 / f64::from(iterations),
                    );
                }
            }
        }
    }
    Ok(())
}
