//! One reproducible workload per process: run with `--release` and `--help`.
//! Timings exclude input generation, output hashing, and result disposal.
//! Matching includes preparation; selection/metric modes time those public calls.
//! Linux VmHWM is a process lifetime peak, including inputs and the warmup; it is
//! not incremental allocation usage. Compare fresh processes on the same machine.

use fieldkin::{
    ColumnPair, DataType, Field, MatchDetails, MatchOptions, Matcher, MatcherResults, Table,
    WorkLimits,
    algorithms::{
        Coma, Cupid, DistributionBased, JaccardConfig, JaccardDistanceMatcher, SimilarityFlooding,
        StringDistanceFunction,
    },
    match_tables,
    metrics::{GroundTruth, OneToOneMethod, get_metrics, precision_increasing_n},
    valentine_match,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, error::Error, fmt::Write, hint::black_box, time::Instant};

#[path = "support/performance.rs"]
mod performance;
use performance::{ordered_results_digest, results_digest};

type BenchResult<T> = Result<T, Box<dyn Error>>;

#[derive(Debug, Serialize)]
struct Args {
    algorithm: String,
    values: String,
    columns: usize,
    target_columns: Option<usize>,
    workers: usize,
    rows: usize,
    distinct: usize,
    tables: usize,
    overlap: usize,
    schema_overlap: usize,
    repetitions: usize,
    sample_size: Option<usize>,
    result_scores: String,
    #[serde(skip)]
    dump_input: Option<String>,
}

impl Default for Args {
    fn default() -> Self {
        Self {
            algorithm: "jaccard".into(),
            values: "mixed".into(),
            columns: 8,
            target_columns: None,
            workers: 1,
            rows: 128,
            distinct: 32,
            tables: 2,
            overlap: 50,
            schema_overlap: 100,
            repetitions: 5,
            sample_size: None,
            result_scores: "mixed".into(),
            dump_input: None,
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
            "--target-columns" => parsed.target_columns = Some(value.parse()?),
            "--workers" => parsed.workers = value.parse()?,
            "--rows" => parsed.rows = value.parse()?,
            "--distinct" => parsed.distinct = value.parse()?,
            "--tables" => parsed.tables = value.parse()?,
            "--overlap" => parsed.overlap = value.parse()?,
            "--schema-overlap" => parsed.schema_overlap = value.parse()?,
            "--repetitions" => parsed.repetitions = value.parse()?,
            "--sample-size" => parsed.sample_size = Some(value.parse()?),
            "--result-scores" => parsed.result_scores = value,
            "--dump-input" => parsed.dump_input = Some(value),
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
        "jaccard-exact",
        "top0",
        "top10",
        "filter",
        "per-source",
        "metrics",
        "curve",
        "hungarian",
        "hungarian-aware",
        "hungarian-guarded",
        "hungarian-aware-guarded",
        "jaccard-direct",
        "jaccard-guarded",
        "jaccard-exact-direct",
        "jaccard-exact-guarded",
        "distribution-direct",
        "distribution-guarded",
    ]
    .contains(&parsed.algorithm.as_str())
    {
        return Err("unknown algorithm (see --help)".into());
    }
    if !["mixed", "codes", "hashes", "interleaved", "ring"].contains(&parsed.values.as_str()) {
        return Err("values must be mixed, codes, hashes, interleaved, or ring".into());
    }
    if !["mixed", "tied"].contains(&parsed.result_scores.as_str()) {
        return Err("result-scores must be mixed or tied".into());
    }
    if parsed.columns == 0
        || parsed.distinct == 0
        || parsed.repetitions == 0
        || parsed.workers == 0
        || parsed.target_columns == Some(0)
        || parsed.tables < 2
        || parsed.overlap > 100
        || parsed.schema_overlap > 100
    {
        return Err(
            "columns, target-columns, distinct, repetitions, workers must be positive; tables >= 2; overlap and schema-overlap <= 100".into(),
        );
    }
    parsed
        .tables
        .checked_mul(parsed.columns.max(parsed.target_columns.unwrap_or(0)))
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
    } else if args.values == "interleaved" {
        // Similar distributions with disjoint, interleaved column value sets.
        (index * args.columns * args.tables + group * args.columns + column).to_string()
    } else if args.values == "ring" {
        // Adjacent columns share half a window on a circular vocabulary. Hashing
        // scatters those intersections across value ranks instead of intervals.
        let stride = distinct.div_ceil(2);
        let position = (column * stride + index) % (args.columns * stride);
        mix((group * args.columns * stride + position) as u64).to_string()
    } else if args.values == "mixed" && column.is_multiple_of(2) {
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
                (0..if table == 0 {
                    args.columns
                } else {
                    args.target_columns.unwrap_or(args.columns)
                })
                    .map(|column| Field {
                        name: column_name(args, table, column),
                        data_type: if (args.values == "mixed" && column.is_multiple_of(2))
                            || ["interleaved", "ring"].contains(&args.values.as_str())
                        {
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

fn matcher(args: &Args) -> BenchResult<Box<dyn Matcher>> {
    // Default configurations, apart from explicit Jaccard worker/exact controls.
    Ok(match args.algorithm.as_str() {
        "coma" => Box::new(Coma::default()),
        "cupid" => Box::new(Cupid::default()),
        "distribution" => Box::new(DistributionBased::default()),
        "jaccard" | "jaccard-exact" => Box::new(JaccardDistanceMatcher::new(JaccardConfig {
            process_num: args.workers,
            distance_fun: if args.algorithm == "jaccard-exact" {
                StringDistanceFunction::Exact
            } else {
                StringDistanceFunction::Levenshtein
            },
            ..JaccardConfig::default()
        })?),
        "flooding" => Box::new(SimilarityFlooding::default()),
        _ => unreachable!("validated matching algorithm"),
    })
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

fn result_operation(algorithm: &str) -> bool {
    matches!(
        algorithm,
        "top0"
            | "top10"
            | "filter"
            | "per-source"
            | "metrics"
            | "curve"
            | "hungarian"
            | "hungarian-aware"
            | "hungarian-guarded"
            | "hungarian-aware-guarded"
    )
}

// Detailed, deterministic candidates with ties and both signs of zero. Subset
// modes use rows entries; metric/assignment modes use the full rectangular grid.
fn scored_results(args: &Args) -> BenchResult<MatcherResults> {
    let count = if matches!(
        args.algorithm.as_str(),
        "top0" | "top10" | "filter" | "per-source"
    ) {
        args.rows
    } else {
        args.columns
            .checked_mul(args.target_columns.unwrap_or(args.columns))
            .ok_or("result dimensions overflow usize")?
    };
    let mut details = MatchDetails::new();
    let entries = (0..count)
        .map(|index| {
            let pair = ColumnPair::new(
                "source",
                format!("column_{:05}", index % args.columns),
                "target",
                format!("column_{:05}", index / args.columns),
            );
            let score = match (args.result_scores.as_str(), index) {
                ("tied", _) => 0.75,
                (_, 0) => -0.0,
                (_, 1) => 0.0,
                _ => (mix(index as u64) % 4096) as f64 / 4096.0,
            };
            details.insert(
                pair.clone(),
                BTreeMap::from([
                    ("similarity".into(), score),
                    ("component".into(), score * 0.5),
                ]),
            );
            (pair, score)
        })
        .collect();
    Ok(MatcherResults::with_details(entries, details)?)
}

fn measure_result_operation(args: &Args) -> BenchResult<(Vec<f64>, Option<u64>, usize, String)> {
    let input = scored_results(args)?;
    let (timings, peak, count, output_hash) =
        if matches!(args.algorithm.as_str(), "metrics" | "curve") {
            let truth = GroundTruth::Columns(
                (0..args
                    .columns
                    .min(args.target_columns.unwrap_or(args.columns)))
                    .map(|column| {
                        ColumnPair::new(
                            "source",
                            format!("column_{column:05}"),
                            "target",
                            format!("column_{column:05}"),
                        )
                    })
                    .collect(),
            );
            let (timings, output) = measure(args.repetitions, || {
                Ok(if args.algorithm == "metrics" {
                    get_metrics(&input, &truth)?
                } else {
                    precision_increasing_n(&input, &truth, OneToOneMethod::Hungarian)?
                })
            })?;
            let peak = peak_rss_kib();
            let bits: Vec<_> = output
                .iter()
                .map(|(name, value)| (name, value.to_bits()))
                .collect();
            (
                timings,
                peak,
                output.len(),
                format!("{:x}", Sha256::digest(serde_json::to_vec(&bits)?)),
            )
        } else {
            let (timings, output) = measure(args.repetitions, || {
                Ok(match args.algorithm.as_str() {
                    "top0" => input.take_top_n(0),
                    "top10" => input.take_top_n(10),
                    "filter" => input.filter(0.9)?,
                    "per-source" => input.take_top_n_per_source(1),
                    "hungarian" => input.one_to_one_hungarian(Some(0.5))?,
                    "hungarian-aware" => input.one_to_one_hungarian_threshold_aware(Some(0.5))?,
                    "hungarian-guarded" => {
                        input.one_to_one_hungarian_with_limits(Some(0.5), &WorkLimits::default())?
                    }
                    "hungarian-aware-guarded" => input
                        .one_to_one_hungarian_threshold_aware_with_limits(
                            Some(0.5),
                            &WorkLimits::default(),
                        )?,
                    _ => unreachable!("validated result operation"),
                })
            })?;
            let peak = peak_rss_kib();
            (
                timings,
                peak,
                output.len(),
                ordered_results_digest(&output)?,
            )
        };
    let input_hash = ordered_results_digest(&input)?;
    Ok((
        timings,
        peak,
        count,
        format!(
            "{:x}",
            Sha256::digest(format!("{input_hash}:{output_hash}"))
        ),
    ))
}

// Guard overhead uses identical direct batch APIs, without the high-level
// table preparation included in the historical matching modes above.
fn measure_direct_matching(
    args: &Args,
    tables: &[Table],
) -> BenchResult<(Vec<f64>, MatcherResults)> {
    let limits = WorkLimits::default();
    let distribution = DistributionBased {
        process_num: args.workers,
        ..DistributionBased::default()
    };
    let jaccard = JaccardDistanceMatcher::new(JaccardConfig {
        process_num: args.workers,
        distance_fun: if args.algorithm.starts_with("jaccard-exact-") {
            StringDistanceFunction::Exact
        } else {
            StringDistanceFunction::Levenshtein
        },
        ..JaccardConfig::default()
    })?;
    measure(args.repetitions, || {
        Ok(match args.algorithm.as_str() {
            "jaccard-direct" | "jaccard-exact-direct" => jaccard.get_matches_batch(tables)?,
            "jaccard-guarded" | "jaccard-exact-guarded" => {
                jaccard.get_matches_batch_with_limits(tables, &limits)?
            }
            "distribution-direct" => distribution.get_matches_batch(tables)?,
            "distribution-guarded" => {
                distribution.get_matches_batch_with_limits(tables, &limits)?
            }
            _ => unreachable!("validated direct matching mode"),
        })
    })
}

fn main() -> BenchResult<()> {
    let arguments: Vec<_> = std::env::args().skip(1).collect();
    if arguments.iter().any(|argument| argument == "--help") {
        println!(
            "Run in release mode. One workload per process; JSON output.\n\
            --algorithm coma|cupid|distribution|jaccard|jaccard-exact|flooding|csv|json (jaccard)\n\
            Result operations: top0|top10|filter|per-source|metrics|curve|hungarian|hungarian-aware\n\
            Guard controls: jaccard[-exact]|distribution with -direct or -guarded suffix;\n\
            hungarian[-aware]-guarded. Guard controls use unlimited checked work counts.\n\
            --values mixed|codes|hashes|interleaved|ring (mixed); codes creates dense fuzzy hits;\n\
            hashes uses all high-entropy text; interleaved/ring stress distribution clustering\n\
            --columns N (8) --rows N (128) --distinct N (32) --tables N (2)\n\
            --target-columns N (default: columns) --workers N (1; Jaccard/direct Distribution)\n\
            --overlap PERCENT (50) --schema-overlap PERCENT (100) --repetitions N (5) --sample-size N (default: all)\n\
            CSV/JSON ingest one table; --tables and overlap settings do not affect ingestion.\n\
            Actual distinct count is min(rows, distinct); zero rows are supported.\n\
            Subset modes use rows detailed candidates; metrics/assignment use columns x target-columns.\n\
            --result-scores mixed|tied (mixed) chooses varied or uniform 0.75 result-operation scores.\n\
            Filter keeps score >= 0.9; per-source keeps one; assignment uses threshold 0.5.\n\
            Peak RSS includes generated inputs and warmup; output hashing follows measurement.\n\
            --dump-input PATH exports an unsampled two-table matching workload without executing it."
        );
        return Ok(());
    }
    let args = parse(arguments)?;
    if let Some(path) = &args.dump_input {
        if args.tables != 2
            || args.sample_size.is_some()
            || result_operation(&args.algorithm)
            || matches!(args.algorithm.as_str(), "csv" | "json")
        {
            return Err("--dump-input requires unsampled matching with two tables".into());
        }
        let tables = tables(&args)?;
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "name": format!("performance_{}_{}", args.values, args.columns),
                "source": {"fields": tables[0].columns},
                "target": {"fields": tables[1].columns},
            }))?,
        )?;
        return Ok(());
    }
    let (timings, peak_rss, count, fingerprint) = if result_operation(&args.algorithm) {
        measure_result_operation(&args)?
    } else if matches!(args.algorithm.as_str(), "csv" | "json") {
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
        if args.algorithm.ends_with("-direct") || args.algorithm.ends_with("-guarded") {
            if args.sample_size.is_some() {
                return Err("direct/guarded matching controls require unsampled inputs".into());
            }
            let (timings, results) = measure_direct_matching(&args, &tables)?;
            (
                timings,
                peak_rss_kib(),
                results.len(),
                results_digest(&results)?,
            )
        } else {
            let matcher = matcher(&args)?;
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
        }
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
            ["--target-columns", "0"],
            ["--workers", "0"],
            ["--algorithm", "unknown"],
            ["--values", "unknown"],
            ["--result-scores", "unknown"],
        ] {
            assert!(parse(args.map(str::to_owned)).is_err());
        }
        assert!(parse(["--rows".to_owned()]).is_err());
    }

    #[test]
    fn operation_inputs_have_unique_detailed_pairs_and_both_zero_signs() {
        let args = Args {
            algorithm: "top10".into(),
            columns: 3,
            rows: 20,
            ..Args::default()
        };
        let results = scored_results(&args).unwrap();
        assert_eq!(results.len(), 20);
        assert_eq!(results.details().len(), 20);
        assert_eq!(
            ordered_results_digest(&results).unwrap(),
            ordered_results_digest(&scored_results(&args).unwrap()).unwrap()
        );
        for (source, expected) in [(0, -0.0_f64), (1, 0.0_f64)] {
            let pair = ColumnPair::new(
                "source",
                format!("column_{source:05}"),
                "target",
                "column_00000",
            );
            assert_eq!(results.get(&pair).unwrap().to_bits(), expected.to_bits());
        }
        let args = Args {
            algorithm: "hungarian".into(),
            target_columns: Some(5),
            ..args
        };
        assert_eq!(scored_results(&args).unwrap().len(), 15);
        let generated = tables(&args).unwrap();
        assert_eq!(generated[0].columns.len(), 3);
        assert_eq!(generated[1].columns.len(), 5);
        let tied = scored_results(&Args {
            result_scores: "tied".into(),
            ..args
        })
        .unwrap();
        assert_eq!(tied.len(), 15);
        assert!(tied.iter().all(|(_, score)| *score == 0.75));
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
    fn clustering_stress_inputs_have_the_documented_intersections() {
        use std::collections::BTreeSet;

        let mut args = Args {
            values: "interleaved".into(),
            columns: 8,
            rows: 128,
            distinct: 128,
            overlap: 100,
            ..Args::default()
        };
        for mode in ["interleaved", "ring", "hashes"] {
            args.values = mode.into();
            let generated = tables(&args).unwrap();
            assert_eq!(generated, tables(&args).unwrap());
            for column in 0..args.columns {
                assert_eq!(
                    generated[0].columns[column].samples,
                    generated[1].columns[column].samples
                );
            }
            let samples = |column: usize| {
                generated[0].columns[column]
                    .samples
                    .iter()
                    .collect::<BTreeSet<_>>()
            };
            assert_eq!(samples(0).len(), 128);
            assert_eq!(samples(0).intersection(&samples(2)).count(), 0);
            assert_eq!(
                samples(0).intersection(&samples(1)).count(),
                if mode == "ring" { 64 } else { 0 }
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
