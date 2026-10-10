//! One reproducible workload per process: run with `--release` and `--help`.
//! Timings include table preparation, but exclude generation, hashing, and selection.
//! Linux VmHWM is a process lifetime peak, including inputs and the warmup; it is
//! not incremental allocation usage. Compare fresh processes on the same machine.

use fieldkin::{
    DataType, Field, MatchOptions, Matcher, MatcherResults, Table,
    algorithms::{Coma, Cupid, DistributionBased, JaccardDistanceMatcher, SimilarityFlooding},
    match_tables, valentine_match,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{error::Error, fmt::Write, hint::black_box, time::Instant};

type BenchResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug, Serialize)]
struct Args {
    algorithm: String,
    values: String,
    columns: usize,
    rows: usize,
    distinct: usize,
    tables: usize,
    overlap: usize,
    schema_overlap: usize,
    repetitions: usize,
    sample_size: Option<usize>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            algorithm: "jaccard".into(),
            values: "mixed".into(),
            columns: 8,
            rows: 128,
            distinct: 32,
            tables: 2,
            overlap: 50,
            schema_overlap: 100,
            repetitions: 5,
            sample_size: None,
        }
    }
}

fn parse(args: impl IntoIterator<Item = String>) -> BenchResult<Args> {
    let mut parsed = Args::default();
    let mut args = args.into_iter();
    while let Some(flag) = args.next() {
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--algorithm" => parsed.algorithm = value,
            "--values" => parsed.values = value,
            "--columns" => parsed.columns = value.parse()?,
            "--rows" => parsed.rows = value.parse()?,
            "--distinct" => parsed.distinct = value.parse()?,
            "--tables" => parsed.tables = value.parse()?,
            "--overlap" => parsed.overlap = value.parse()?,
            "--schema-overlap" => parsed.schema_overlap = value.parse()?,
            "--repetitions" => parsed.repetitions = value.parse()?,
            "--sample-size" => parsed.sample_size = Some(value.parse()?),
            _ => return Err(format!("unknown option {flag}").into()),
        }
    }
    if ![
        "coma",
        "cupid",
        "distribution",
        "jaccard",
        "flooding",
        "csv",
        "json",
    ]
    .contains(&parsed.algorithm.as_str())
    {
        return Err("unknown algorithm (see --help)".into());
    }
    if !["mixed", "codes"].contains(&parsed.values.as_str()) {
        return Err("values must be mixed or codes".into());
    }
    if parsed.columns == 0
        || parsed.distinct == 0
        || parsed.repetitions == 0
        || parsed.tables < 2
        || parsed.overlap > 100
        || parsed.schema_overlap > 100
    {
        return Err(
            "columns, distinct, repetitions must be positive; tables >= 2; overlap and schema-overlap <= 100".into(),
        );
    }
    parsed
        .tables
        .checked_mul(parsed.columns)
        .and_then(|n| n.checked_mul(parsed.rows.max(parsed.distinct)))
        .ok_or("workload dimensions overflow usize")?;
    if matches!(parsed.algorithm.as_str(), "csv" | "json") && parsed.sample_size.is_some() {
        return Err("--sample-size applies only to matching".into());
    }
    Ok(parsed)
}

// Name overlap is independent of value overlap; neither asserts ground truth.
fn column_name(args: &Args, table: usize, column: usize) -> String {
    let shared = (args.columns as u128 * args.schema_overlap as u128 / 100) as usize;
    if table > 0 && column >= shared {
        return format!("other_{:016x}", mix((table * args.columns + column) as u64));
    }
    let prefix = if table > 0 && column.is_multiple_of(3) {
        "attribute"
    } else {
        "measure"
    };
    format!("{prefix}_{column:04}")
}

fn mix(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

// Numeric and text columns alternate. Distinct values repeat in input order;
// overlap is the percentage shared with table zero, rounded down. Remaining
// values differ by table. Mixed text keys have no common long prefix/suffix;
// codes intentionally create dense fuzzy hits, without asserting semantic truth.
fn value(args: &Args, table: usize, column: usize, row: usize) -> String {
    let distinct = args.distinct.min(args.rows).max(1);
    let index = row % distinct;
    let shared = (distinct as u128 * args.overlap as u128 / 100) as usize;
    let group = if index < shared { 0 } else { table };
    let key = (group * args.columns + column) * distinct + index;
    if args.values == "codes" {
        format!("customer-account-{key:010}")
    } else if column.is_multiple_of(2) {
        key.to_string()
    } else {
        format!(
            "{:016x}{:016x}",
            mix(key as u64),
            mix((key as u64).wrapping_add(0x9e37_79b9_7f4a_7c15))
        )
    }
}

fn tables(args: &Args) -> BenchResult<Vec<Table>> {
    (0..args.tables)
        .map(|table| {
            Ok(Table::new(
                format!("table_{table:04}"),
                (0..args.columns)
                    .map(|column| Field {
                        name: column_name(args, table, column),
                        data_type: if args.values == "mixed" && column.is_multiple_of(2) {
                            DataType::Integer
                        } else {
                            DataType::Text
                        },
                        samples: (0..args.rows)
                            .map(|row| value(args, table, column, row))
                            .collect(),
                    })
                    .collect(),
            )?)
        })
        .collect()
}

// Write records directly into the encoded input: no staging row matrix or Table.
// JSON values are strings to retain exactly the same textual samples as CSV.
fn encoded_input(args: &Args) -> String {
    let json = args.algorithm == "json";
    let mut input = String::new();
    if json {
        input.push('[');
    } else {
        for column in 0..args.columns {
            if column > 0 {
                input.push(',');
            }
            input.push_str(&column_name(args, 0, column));
        }
        input.push('\n');
    }
    for row in 0..args.rows {
        if json {
            if row > 0 {
                input.push(',');
            }
            input.push('{');
        }
        for column in 0..args.columns {
            if column > 0 {
                input.push(',');
            }
            let value = value(args, 0, column, row);
            if json {
                write!(input, "\"{}\":\"{}\"", column_name(args, 0, column), value).unwrap();
            } else {
                input.push_str(&value);
            }
        }
        input.push(if json { '}' } else { '\n' });
    }
    if json {
        input.push(']');
    }
    input
}

fn matcher(algorithm: &str) -> Box<dyn Matcher> {
    // No threshold/accuracy tuning: use each algorithm's existing default config.
    match algorithm {
        "coma" => Box::new(Coma::default()),
        "cupid" => Box::new(Cupid::default()),
        "distribution" => Box::new(DistributionBased::default()),
        "jaccard" => Box::new(JaccardDistanceMatcher::default()),
        "flooding" => Box::new(SimilarityFlooding::default()),
        _ => unreachable!("validated matching algorithm"),
    }
}

fn measure<T>(
    repetitions: usize,
    mut run: impl FnMut() -> BenchResult<T>,
) -> BenchResult<(Vec<f64>, T)> {
    drop(black_box(run()?)); // Warm caches; omit this run from timing.
    let mut timings = Vec::with_capacity(repetitions);
    let mut last = None;
    for _ in 0..repetitions {
        drop(last.take());
        let start = Instant::now();
        let output = black_box(run()?);
        timings.push(start.elapsed().as_secs_f64() * 1000.0);
        last = Some(output);
    }
    Ok((timings, last.expect("positive repetitions")))
}

fn peak_rss_kib() -> Option<u64> {
    std::fs::read_to_string("/proc/self/status")
        .ok()?
        .lines()
        .find_map(|line| {
            line.strip_prefix("VmHWM:")?
                .split_whitespace()
                .next()?
                .parse()
                .ok()
        })
}

fn results_digest(results: &MatcherResults) -> BenchResult<String> {
    let mut hash = Sha256::new();
    for selected in [
        results.clone(),
        results.filter(0.5)?,
        results.take_top_n(10),
        results.one_to_one_greedy(Some(0.5))?,
        results.one_to_one_hungarian(Some(0.5))?,
        results.one_to_one_hungarian_threshold_aware(Some(0.5))?,
    ] {
        // Encode IEEE-754 bits to detect even signed-zero or last-bit changes.
        let entries: Vec<_> = selected
            .iter()
            .map(|(pair, score)| {
                (
                    pair,
                    score.to_bits(),
                    selected.get_details(pair).map(|details| {
                        details
                            .iter()
                            .map(|(name, value)| (name, value.to_bits()))
                            .collect::<Vec<_>>()
                    }),
                )
            })
            .collect();
        let bytes = serde_json::to_vec(&entries)?;
        hash.update((bytes.len() as u64).to_le_bytes());
        hash.update(bytes);
    }
    Ok(format!("{:x}", hash.finalize()))
}

fn main() -> BenchResult<()> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.iter().any(|argument| argument == "--help") {
        println!(
            "Run in release mode. One workload per process; JSON output.\n\
            --algorithm coma|cupid|distribution|jaccard|flooding|csv|json (jaccard)\n\
            --values mixed|codes (mixed); codes intentionally creates dense fuzzy hits\n\
            --columns N (8) --rows N (128) --distinct N (32) --tables N (2)\n\
            --overlap PERCENT (50) --schema-overlap PERCENT (100) --repetitions N (5) --sample-size N (default: all)\n\
            CSV/JSON ingest one table; --tables and overlap settings do not affect ingestion.\n\
            Actual distinct count is min(rows, distinct); zero rows are supported.\n\
            Peak RSS includes generated inputs and warmup; hashing/selection follow measurement."
        );
        return Ok(());
    }
    let args = parse(arguments)?;
    let (timings, peak_rss, count, fingerprint) =
        if matches!(args.algorithm.as_str(), "csv" | "json") {
            let input = encoded_input(&args);
            let (timings, table) = measure(args.repetitions, || {
                Ok(if args.algorithm == "csv" {
                    Table::from_csv("input", input.as_bytes())?
                } else {
                    Table::from_json("input", input.as_bytes())?
                })
            })?;
            let peak = peak_rss_kib();
            let fingerprint = format!("{:x}", Sha256::digest(serde_json::to_vec(&table)?));
            (timings, peak, table.columns.len(), fingerprint)
        } else {
            let tables = tables(&args)?;
            let matcher = matcher(&args.algorithm);
            let (timings, results) = measure(args.repetitions, || {
                Ok(if args.sample_size.is_some() {
                    match_tables(
                        &tables,
                        matcher.as_ref(),
                        MatchOptions {
                            instance_sample_size: args.sample_size,
                        },
                    )?
                } else {
                    valentine_match(&tables, matcher.as_ref())?
                })
            })?;
            let peak = peak_rss_kib();
            (timings, peak, results.len(), results_digest(&results)?)
        };
    let mut sorted = timings.clone();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    let median = if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    };
    println!(
        "{}",
        serde_json::to_string(&serde_json::json!({
            "parameters": args, "milliseconds": timings, "median_ms": median,
            "peak_rss_kib": peak_rss, "result_count": count, "output_sha256": fingerprint,
        }))?
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_parameters() {
        for args in [
            ["--columns", "0"],
            ["--distinct", "0"],
            ["--tables", "1"],
            ["--overlap", "101"],
            ["--schema-overlap", "101"],
            ["--repetitions", "0"],
            ["--algorithm", "unknown"],
            ["--values", "unknown"],
        ] {
            assert!(parse(args.map(str::to_owned)).is_err());
        }
        assert!(parse(["--rows".to_owned()]).is_err());
    }

    #[test]
    fn generator_is_deterministic_and_ingestion_preserves_samples() {
        let mut args = Args {
            columns: 4,
            rows: 12,
            distinct: 8,
            ..Args::default()
        };
        let generated = tables(&args).unwrap();
        assert_eq!(generated, tables(&args).unwrap());
        for column in 0..args.columns {
            assert_eq!(
                generated[0].columns[column].samples[0],
                generated[1].columns[column].samples[0]
            );
            assert_ne!(
                generated[0].columns[column].samples[7],
                generated[1].columns[column].samples[7]
            );
        }
        args.algorithm = "csv".into();
        let csv = Table::from_csv("table_0000", encoded_input(&args).as_bytes()).unwrap();
        args.algorithm = "json".into();
        let json = Table::from_json("table_0000", encoded_input(&args).as_bytes()).unwrap();
        assert_eq!(generated[0], csv);
        assert_eq!(csv, json);
        args.schema_overlap = 50;
        let renamed = tables(&args).unwrap();
        assert_eq!(generated[0], renamed[0]);
        for column in 0..args.columns {
            assert_eq!(
                renamed[1].columns[column].samples,
                generated[1].columns[column].samples
            );
            assert_eq!(
                renamed[1].columns[column].name.starts_with("other_"),
                column >= 2
            );
        }
    }

    #[test]
    fn codes_are_text_and_preserved_by_ingestion() {
        let mut args = Args {
            values: "codes".into(),
            columns: 4,
            rows: 12,
            distinct: 8,
            ..Args::default()
        };
        let generated = tables(&args).unwrap();
        assert!(
            generated
                .iter()
                .flat_map(|table| &table.columns)
                .all(|column| {
                    column.data_type == DataType::Text
                        && column
                            .samples
                            .iter()
                            .all(|value| value.starts_with("customer-account-"))
                })
        );
        assert_eq!(
            generated[0].columns[0].samples[0],
            "customer-account-0000000000"
        );
        args.algorithm = "csv".into();
        let csv = Table::from_csv("table_0000", encoded_input(&args).as_bytes()).unwrap();
        args.algorithm = "json".into();
        let json = Table::from_json("table_0000", encoded_input(&args).as_bytes()).unwrap();
        assert_eq!(generated[0], csv);
        assert_eq!(csv, json);
    }
}
