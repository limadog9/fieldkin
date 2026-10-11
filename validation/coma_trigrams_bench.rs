//! Standalone COMA allocation-removal benchmark; compile against each library revision.
//! Arguments: CASE COLUMNS REPETITIONS [DATASET_OR_CORPUS_PATH].
//! CASE: short, unicode, long_similar, long_unrelated, mixed, tiny, independent,
//! dataset, dataset_instances. Only dataset_instances enables instance matching.
//! Generation/loading, result checks, and printing
//! are excluded from timing; one warmup precedes timed direct get_matches calls.
//! Output: times in milliseconds, process-lifetime peak RSS, ordered score/detail bits.

use fieldkin::{
    DataType, Field, Matcher, MatcherResults, Schema, Table,
    algorithms::{Coma, ComaConfig},
};
use std::{hint::black_box, path::PathBuf, time::Instant};

fn names(side: usize, case: &str, columns: usize) -> Table {
    let fields = (0..columns)
        .map(|i| {
            let name = match case {
                "short" => format!("{}_{}", if side == 0 { "measure" } else { "attribute" }, i),
                "unicode" => format!(
                    "{}_{}_{}",
                    if side == 0 {
                        "生年月日🙂e\u{301}İ"
                    } else {
                        "生年月日😀éİ"
                    },
                    i,
                    "製品注文".repeat(i % 5 + 1)
                ),
                "long_similar" => format!(
                    "customer_{}_{i}_{}",
                    "account_".repeat(20),
                    if side == 0 { "birth_date" } else { "birthdate" }
                ),
                "long_unrelated" => format!(
                    "{}_{i}",
                    if side == 0 {
                        "north".repeat(40)
                    } else {
                        "xyzpq".repeat(40)
                    }
                ),
                "mixed" => {
                    if (i + side) % 2 == 0 {
                        format!("id_{i}")
                    } else {
                        format!(
                            "{}_{}_{i}",
                            if side == 0 { "海" } else { "🙂" },
                            "x".repeat(160 + i)
                        )
                    }
                }
                "tiny" => format!(
                    "{}{}",
                    (b'a' + (i / 26) as u8) as char,
                    (b'a' + (i % 26) as u8) as char
                ),
                _ => panic!("unknown case"),
            };
            Field {
                name,
                data_type: DataType::Text,
                samples: vec![],
            }
        })
        .collect();
    Table::new(if side == 0 { "source" } else { "target" }, fields).unwrap()
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let case = &args[1];
    let columns: usize = args[2].parse().unwrap();
    let repetitions: usize = args[3].parse().unwrap();
    let mut datasets = Vec::new();
    if matches!(
        case.as_str(),
        "independent" | "dataset" | "dataset_instances"
    ) {
        let mut paths: Vec<_> = if case == "independent" {
            std::fs::read_dir(&args[4])
                .unwrap()
                .map(|e| e.unwrap().path())
                .filter(|p| p.extension().is_some_and(|e| e == "json"))
                .collect()
        } else {
            vec![PathBuf::from(&args[4])]
        };
        paths.sort();
        for path in paths {
            let data: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
            let source: Schema = serde_json::from_value(data["source"].clone()).unwrap();
            let target: Schema = serde_json::from_value(data["target"].clone()).unwrap();
            datasets.push((
                Table::from_schema("source", &source),
                Table::from_schema("target", &target),
            ));
        }
        if case == "independent" {
            assert_eq!(datasets.len(), 17);
        }
    } else {
        datasets.push((names(0, case, columns), names(1, case, columns)));
    }
    let matcher = Coma::new(ComaConfig {
        use_instances: case == "dataset_instances",
        ..Default::default()
    })
    .unwrap();
    let run = || -> Vec<MatcherResults> {
        datasets
            .iter()
            .map(|(s, t)| matcher.get_matches(black_box(s), black_box(t)).unwrap())
            .collect()
    };
    let expected = run();
    let mut times = Vec::new();
    for _ in 0..repetitions {
        let start = Instant::now();
        let matched = black_box(run());
        times.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(matched, expected);
    }
    let rss = std::fs::read_to_string("/proc/self/status")
        .unwrap()
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmHWM:")
                .map(str::trim)
                .map(str::to_owned)
        })
        .unwrap();
    println!("{times:?}");
    println!("{rss}");
    println!(
        "{:?}",
        expected
            .iter()
            .map(|r| (
                r.iter()
                    .map(|(p, s)| (p.clone(), s.to_bits()))
                    .collect::<Vec<_>>(),
                r.details()
                    .iter()
                    .map(|(p, d)| (
                        p,
                        d.iter().map(|(k, s)| (k, s.to_bits())).collect::<Vec<_>>()
                    ))
                    .collect::<Vec<_>>()
            ))
            .collect::<Vec<_>>()
    );
}
