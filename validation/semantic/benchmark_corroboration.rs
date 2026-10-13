#![allow(dead_code)]
#[path = "support/labeled.rs"]
mod labeled;
use fieldkin::{
    Config, DataType, Field, MatchReport, Schema, try_match_schemas, try_match_schemas_corroborated,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use std::{hint::black_box, path::Path, time::Instant};

type Cases = Vec<(String, Schema, Schema)>;
fn inputs(workload: &str) -> Cases {
    if workload == "exposed597" {
        return labeled::load_cases(
            Path::new(env!("CARGO_MANIFEST_DIR")),
            &[
                "eval",
                "eval_realworld",
                "eval_independent",
                "eval_reliability",
                "eval_reliability_new",
            ],
        )
        .unwrap()
        .into_iter()
        .map(|(id, case)| (id, case.source, case.target))
        .collect();
    }
    let (columns, rows, distinct, ambiguous) = match workload {
        "aligned_small" => (8, 32, 8, false),
        "aligned_medium" => (24, 256, 64, false),
        "aligned_large" => (64, 1024, 256, false),
        "ambiguous_small" => (8, 64, 16, true),
        "ambiguous_medium" => (16, 256, 64, true),
        "ambiguous_large" => (32, 512, 128, true),
        _ => panic!("unknown workload"),
    };
    let mut source = Vec::new();
    let mut target = Vec::new();
    for column in 0..columns {
        let values = (0..rows)
            .map(|row| {
                format!(
                    "{:x}",
                    Sha256::digest(format!("fieldkin-fixed-{column}-{}", row % distinct))
                )[..16]
                    .to_owned()
            })
            .collect::<Vec<_>>();
        let suffix = format!("{:x}", Sha256::digest(format!("fieldkin-name-{column}")))[..16]
            .chars()
            .map(|c| (b'a' + c.to_digit(16).unwrap() as u8) as char)
            .collect::<String>();
        let leaf = format!("field{suffix}");
        let name = if ambiguous {
            format!("records[].details[].{leaf}")
        } else {
            format!("records[].amount{column}")
        };
        source.push(Field {
            name: name.clone(),
            data_type: DataType::Text,
            samples: values.clone(),
        });
        target.push(Field {
            name,
            data_type: DataType::Text,
            samples: values.clone(),
        });
        if ambiguous {
            let values = values
                .into_iter()
                .enumerate()
                .map(|(row, value)| {
                    if row % distinct < distinct * 4 / 5 {
                        value
                    } else {
                        format!(
                            "{:x}",
                            Sha256::digest(format!("fieldkin-other-{column}-{}", row % distinct))
                        )[..16]
                            .to_owned()
                    }
                })
                .collect();
            target.push(Field {
                name: leaf,
                data_type: DataType::Text,
                samples: values,
            });
        }
    }
    vec![(
        workload.into(),
        Schema { fields: source },
        Schema { fields: target },
    )]
}
fn matching(cases: &Cases, proposed: bool) -> Vec<MatchReport> {
    cases
        .iter()
        .map(|(_, source, target)| {
            if proposed {
                try_match_schemas_corroborated(source, target, Config::default()).unwrap()
            } else {
                try_match_schemas(source, target, Config::default()).unwrap()
            }
        })
        .collect()
}
fn fingerprint(reports: &[MatchReport]) -> String {
    format!("{:x}", Sha256::digest(format!("{reports:?}")))
}
fn peak_rss() -> Option<usize> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find(|line| line.starts_with("VmHWM:"))?
        .split_whitespace()
        .nth(1)?
        .parse()
        .ok()
}
fn main() {
    let args = std::env::args().collect::<Vec<_>>();
    let mode = &args[1];
    let workload = &args[2];
    let cases = inputs(workload);
    if mode == "verify" {
        let before = matching(&cases, false);
        let after = matching(&cases, true);
        let mut changed = 0;
        for (a, b) in before.iter().zip(&after) {
            for (a, b) in a.fields.iter().zip(&b.fields) {
                assert_eq!(a.source, b.source);
                assert_eq!(a.candidates, b.candidates);
                for (a, b) in a.candidates.iter().zip(&b.candidates) {
                    assert_eq!(a.score.to_bits(), b.score.to_bits());
                    assert_eq!(a.name_score.to_bits(), b.name_score.to_bits());
                    assert_eq!(a.type_score.to_bits(), b.type_score.to_bits());
                    assert_eq!(
                        a.sample_score.map(f64::to_bits),
                        b.sample_score.map(f64::to_bits)
                    );
                }
                changed += usize::from(a.decision != b.decision);
            }
        }
        println!(
            "{}",
            json!({"workload":workload,"original_digest":fingerprint(&before),"proposed_digest":fingerprint(&after),"changed_decisions":changed,"candidate_bits_identical":true})
        );
        return;
    }
    let proposed = match mode.as_str() {
        "original" => false,
        "proposed" => true,
        _ => panic!("unknown mode"),
    };
    let warmup = matching(&cases, proposed);
    let digest = fingerprint(&warmup);
    drop(warmup);
    let mut times = Vec::new();
    for _ in 0..5 {
        let start = Instant::now();
        let reports = matching(&cases, proposed);
        black_box(&reports);
        drop(reports);
        times.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    println!(
        "{}",
        json!({"mode":mode,"workload":workload,"cases":cases.len(),"source_columns":cases.iter().map(|(_,s,_)|s.fields.len()).sum::<usize>(),"target_columns":cases.iter().map(|(_,_,t)|t.fields.len()).sum::<usize>(),"source_values":cases.iter().flat_map(|(_,s,_)|&s.fields).map(|f|f.samples.len()).sum::<usize>(),"target_values":cases.iter().flat_map(|(_,_,t)|&t.fields).map(|f|f.samples.len()).sum::<usize>(),"times_ms":times,"warmups":1,"peak_rss_kib":peak_rss(),"result_digest":digest})
    );
}
