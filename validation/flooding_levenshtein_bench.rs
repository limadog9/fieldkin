use fieldkin::algorithms::SimilarityFlooding;
use fieldkin::algorithms::flooding::StringMatcher;
use fieldkin::{DataType, Field, Matcher, MatcherResults, Table};
use std::{hint::black_box, time::Instant};

fn schema(side: usize, case: &str) -> Table {
    let columns = (0..if side == 0 { 8 } else { 12 })
        .map(|i| {
            let name = match case {
                "short" => format!("{}_{}", if side == 0 { "measure" } else { "attribute" }, i),
                "unicode" => format!(
                    "{}_{i}_{}",
                    if side == 0 {
                        "生年月日🙂e\u{301}"
                    } else {
                        "生年月日😀é"
                    },
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
                _ => panic!("unknown case"),
            };
            Field {
                name,
                data_type: if i % 2 == 0 {
                    DataType::Integer
                } else {
                    DataType::Text
                },
                samples: vec![],
            }
        })
        .collect();
    Table::new(if side == 0 { "source" } else { "target" }, columns).unwrap()
}

fn main() {
    let args: Vec<_> = std::env::args().collect();
    let matcher = SimilarityFlooding {
        string_matcher: match args[1].as_str() {
            "levenshtein" => StringMatcher::Levenshtein,
            "default" => StringMatcher::PrefixSuffix,
            _ => panic!("unknown matcher"),
        },
        ..Default::default()
    };
    let source = schema(0, &args[2]);
    let target = schema(1, &args[2]);
    let repetitions: usize = args[3].parse().unwrap();
    drop(matcher.get_matches(&source, &target).unwrap());
    let mut times = Vec::new();
    let mut output: Option<MatcherResults> = None;
    for _ in 0..repetitions {
        let start = Instant::now();
        let matched = black_box(
            matcher
                .get_matches(black_box(&source), black_box(&target))
                .unwrap(),
        );
        times.push(start.elapsed().as_secs_f64() * 1000.0);
        if let Some(previous) = &output {
            assert_eq!(previous, &matched);
        }
        output = Some(matched);
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
        output
            .unwrap()
            .iter()
            .map(|(p, s)| (p.clone(), s.to_bits()))
            .collect::<Vec<_>>()
    );
}
