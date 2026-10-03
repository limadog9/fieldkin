//! Fixed synthetic inputs; run with `cargo bench --bench matching`.
//! Timings are wall-clock observations, not statistical performance claims.

use std::hint::black_box;
use std::time::Instant;

use fieldkin::{
    Config, DataType, Field, FieldId, MatchEngine, NameMatcher, SampleValue, Schema,
    WeightedMatcher,
};

fn fixture(count: usize, samples: bool) -> (Schema, Schema) {
    let build = |target: bool| {
        let fields = (0..count)
            .map(|i| {
                let name = if target {
                    format!("metric_{i:04}_count")
                } else {
                    format!("Metric{i:04}Count")
                };
                let field = Field::new(
                    FieldId(format!("{}-{i:04}", if target { "t" } else { "s" })),
                    name,
                    DataType::Integer,
                );
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
            .collect();
        Schema::new(fields)
    };
    let source = build(false);
    let mut target = build(true);
    target.fields.reverse();
    (source, target)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("fieldkin fixed synthetic benchmark; 16 samples/field when enabled");
    println!("one warm-up; engine/input construction excluded; no network or randomness");
    println!("engine,fields_per_side,samples,one_to_one,iterations,total_ms,us_per_match");
    for count in [16, 64, 128] {
        let iterations = match count {
            16 => 30,
            64 => 10,
            _ => 5,
        };
        for samples in [false, true] {
            let (source, target) = fixture(count, samples);
            for one_to_one in [false, true] {
                for baseline in [false, true] {
                    let config = Config {
                        one_to_one,
                        reject_incompatible_types: !baseline,
                        ..Config::default()
                    };
                    let engine = if baseline {
                        MatchEngine::with_matchers(
                            config,
                            vec![WeightedMatcher::new(1.0, NameMatcher::default())],
                        )?
                    } else {
                        MatchEngine::new(config)?
                    };
                    black_box(engine.match_schemas(black_box(&source), black_box(&target))?);
                    let started = Instant::now();
                    for _ in 0..iterations {
                        black_box(engine.match_schemas(black_box(&source), black_box(&target))?);
                    }
                    let elapsed = started.elapsed().as_secs_f64();
                    println!(
                        "{},{count},{samples},{one_to_one},{iterations},{:.3},{:.3}",
                        if baseline { "name-only" } else { "combined" },
                        elapsed * 1000.0,
                        elapsed * 1_000_000.0 / f64::from(iterations),
                    );
                }
            }
        }
    }
    Ok(())
}
