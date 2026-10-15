use fieldkin::algorithms::{
    Coma, Cupid, DistributionBased, JaccardDistanceMatcher, SimilarityFlooding,
};
use fieldkin::{
    CheckedMatcher, MatchOptions, Matcher, MatcherResults, Table, WorkLimits,
    match_tables_with_limits, valentine_match,
};
use std::{
    collections::BTreeSet,
    ffi::{OsStr, OsString},
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

const BOUNDED_FLAGS: &[&str] = &[
    "--bounded",
    "--max-input-bytes",
    "--sample-size",
    "--max-assignment-dimension",
    "--max-matrix-cells",
    "--max-clustering-variables",
    "--max-clustering-constraints",
    "--max-jaccard-comparisons",
    "--assignment",
    "--threshold",
];

const HELP: &str = "Usage: fieldkin <coma|cupid|distribution|jaccard|flooding> <source.csv|json> <target.csv|json> [source_name [target_name]] [options]
       fieldkin <algorithm> <source.csv|json> <target.csv|json> -- [source_name [target_name]]
Outputs ranked table-aware matches and component scores as JSON.
Without --bounded, all instance values and the existing unchecked matching path are retained.
Before the -- separator, optional names starting with -- are options; unknown or malformed options
fail before inputs are opened. Option values must be separate tokens (--flag=value is unsupported).
The separator makes remaining positional arguments literal; at most two table names are allowed in total.
It never disables --bounded specified earlier. Compatibility: table names starting with -- now require it.
Examples:
  fieldkin jaccard source.csv target.csv -- --bounded --target
  fieldkin --bounded jaccard source.csv target.csv --max-input-bytes 8388608 -- --source --target

Bounded mode (jaccard and distribution only; options may precede or interleave positionals before --):
  --bounded                              Enable byte-bounded loading and checked matching.
  --max-input-bytes N                     Serialized bytes per input file (default: 8388608).
  --sample-size N                         Evenly spaced nonempty values per column (default: 1000; 0 clears samples).
  --max-assignment-dimension N            Unique source/target dimension (default: 1024; requires assignment).
  --max-matrix-cells N                    Cells per assignment/distribution matrix, or Jaccard batch column pairs (default: 1048576).
  --max-clustering-variables N            Directed variables per conflicting distribution model (default: 4096; distribution only).
  --max-clustering-constraints N          Triangle constraints per distribution model (default: 262144; distribution only).
  --max-jaccard-comparisons N             Batch-wide distinct-value Cartesian comparisons (default: 10000000; jaccard only).
  --assignment MODE                      none (default), rectangular, or rectangular-threshold-aware.
  --threshold FLOAT                      Finite assignment cutoff; otherwise the existing distinct-score cutoff is used.
  --                                     End options, allowing literal table names and bounded positional paths beginning with --.

Bounded defaults are illustrative conservative budgets, not universal safe settings.
The byte cap is per file, not combined. Parsing and complete-data type inference precede sampling;
parsed structures and allocations can exceed serialized size. Sampling alone does not bound loading memory.
WorkLimits retain their per-matrix, per-model and batch-wide scopes, not cumulative budgets.
Preparation, string processing and embedding work are outside these work counts.
These are input-byte/work-count limits, not an RSS guarantee, deadline or hard memory sandbox.
Use external process memory and time limits when hard containment is required.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Assignment {
    None,
    Rectangular,
    RectangularThresholdAware,
}

#[derive(Debug)]
struct BoundedOptions {
    max_input_bytes: usize,
    sample_size: usize,
    limits: WorkLimits,
    assignment: Assignment,
    threshold: Option<f64>,
}

impl Default for BoundedOptions {
    fn default() -> Self {
        Self {
            max_input_bytes: 8_388_608,
            sample_size: 1_000,
            limits: WorkLimits {
                max_assignment_dimension: 1_024,
                max_matrix_cells: 1_048_576,
                max_clustering_variables: 4_096,
                max_clustering_constraints: 262_144,
                max_jaccard_comparisons: 10_000_000,
            },
            assignment: Assignment::None,
            threshold: None,
        }
    }
}

fn read_table(path: &Path, name: &str) -> Result<Table, fieldkin::Error> {
    parse_table(path, name, File::open(path)?)
}

fn parse_table(path: &Path, name: &str, reader: impl Read) -> Result<Table, fieldkin::Error> {
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("json"))
    {
        Table::from_json(name, reader)
    } else {
        Table::from_csv(name, reader)
    }
}

fn byte_read_limit(cap: usize) -> Result<u64, String> {
    cap.checked_add(1)
        .and_then(|n| u64::try_from(n).ok())
        .ok_or_else(|| {
            format!("--max-input-bytes {cap} plus one overflows the supported byte count")
        })
}

fn read_bounded(reader: impl Read, path: &Path, cap: usize) -> Result<Vec<u8>, fieldkin::Error> {
    let read_limit = byte_read_limit(cap).map_err(|message| {
        fieldkin::Error::InvalidConfig(format!("{}: {message}", path.display()))
    })?;
    // Grow with actual input, rather than reserving an arbitrarily large configured cap.
    let mut bytes = Vec::new();
    reader.take(read_limit).read_to_end(&mut bytes)?;
    if bytes.len() > cap {
        return Err(fieldkin::Error::InvalidInput(format!(
            "input {} exceeds --max-input-bytes {cap} bytes (per file)",
            path.display()
        )));
    }
    Ok(bytes)
}

fn read_table_bounded(path: &Path, name: &str, cap: usize) -> Result<Table, fieldkin::Error> {
    let bytes = read_bounded(File::open(path)?, path, cap)?;
    parse_table(path, name, bytes.as_slice())
}

fn parse_count(option: &str, value: &OsStr) -> Result<usize, String> {
    let value = value
        .to_str()
        .ok_or_else(|| format!("{option} requires a nonnegative integer"))?;
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(format!("{option} requires a nonnegative integer: {value}"));
    }
    value
        .parse()
        .map_err(|_| format!("{option} integer is too large: {value}"))
}

fn parse_bounded(args: &[OsString]) -> Result<(Vec<OsString>, BoundedOptions), String> {
    let mut options = BoundedOptions::default();
    let mut seen = BTreeSet::new();
    let mut positional = Vec::new();
    let mut args = args.iter();
    let mut positional_only = false;
    while let Some(arg) = args.next() {
        if !positional_only && arg == "--" {
            positional_only = true;
            continue;
        }
        let Some(option) = arg.to_str().filter(|s| s.starts_with('-') && *s != "-") else {
            positional.push(arg.clone());
            continue;
        };
        if positional_only {
            positional.push(arg.clone());
            continue;
        }
        if !BOUNDED_FLAGS.contains(&option) {
            return Err(format!("unknown bounded option: {option}"));
        }
        if !seen.insert(option) {
            return Err(format!("duplicate option: {option}"));
        }
        if option == "--bounded" {
            continue;
        }
        let value = args
            .next()
            .ok_or_else(|| format!("{option} requires a value"))?;
        match option {
            "--assignment" => {
                options.assignment = match value.to_str() {
                    Some("none") => Assignment::None,
                    Some("rectangular") => Assignment::Rectangular,
                    Some("rectangular-threshold-aware") => Assignment::RectangularThresholdAware,
                    _ => {
                        return Err("--assignment requires none, rectangular, or rectangular-threshold-aware".into());
                    }
                };
            }
            "--threshold" => {
                options.threshold = Some(
                    value
                        .to_str()
                        .and_then(|value| value.parse::<f64>().ok())
                        .filter(|value| value.is_finite())
                        .ok_or("--threshold requires a finite floating-point value")?,
                );
            }
            _ => {
                let count = parse_count(option, value)?;
                match option {
                    "--max-input-bytes" => options.max_input_bytes = count,
                    "--sample-size" => options.sample_size = count,
                    "--max-assignment-dimension" => options.limits.max_assignment_dimension = count,
                    "--max-matrix-cells" => options.limits.max_matrix_cells = count,
                    "--max-clustering-variables" => options.limits.max_clustering_variables = count,
                    "--max-clustering-constraints" => {
                        options.limits.max_clustering_constraints = count
                    }
                    "--max-jaccard-comparisons" => options.limits.max_jaccard_comparisons = count,
                    _ => unreachable!("all bounded flags are handled above"),
                }
            }
        }
    }
    if !seen.contains("--bounded") {
        return Err("new options require --bounded".into());
    }
    if !(3..=5).contains(&positional.len()) {
        return Err("expected an algorithm and two input files; use --help".into());
    }
    let algorithm = positional[0]
        .to_str()
        .ok_or("algorithm must be valid UTF-8")?;
    match algorithm {
        "jaccard" => {
            for flag in ["--max-clustering-variables", "--max-clustering-constraints"] {
                if seen.contains(flag) {
                    return Err(format!(
                        "{flag} applies only to bounded distribution matching"
                    ));
                }
            }
        }
        "distribution" => {
            if seen.contains("--max-jaccard-comparisons") {
                return Err(
                    "--max-jaccard-comparisons applies only to bounded jaccard matching".into(),
                );
            }
        }
        _ => return Err("bounded mode supports only jaccard and distribution".into()),
    }
    if options.assignment == Assignment::None {
        for flag in ["--threshold", "--max-assignment-dimension"] {
            if seen.contains(flag) {
                return Err(format!("{flag} requires --assignment other than none"));
            }
        }
    }
    byte_read_limit(options.max_input_bytes)?;
    Ok((positional, options))
}

fn table_names(args: &[OsString]) -> Result<(&str, &str), &'static str> {
    let source_name = args.get(3).map_or(Ok("aaa"), |name| {
        name.to_str().ok_or("source table name must be valid UTF-8")
    })?;
    let target_name = args.get(4).map_or(Ok("bbb"), |name| {
        name.to_str().ok_or("target table name must be valid UTF-8")
    })?;
    Ok((source_name, target_name))
}

fn run_legacy(args: &[OsString]) -> Result<MatcherResults, Box<dyn std::error::Error>> {
    if !(3..=5).contains(&args.len()) {
        return Err("expected an algorithm and two input files; use --help".into());
    }
    let algorithm = args[0].to_str().ok_or("algorithm must be valid UTF-8")?;
    let matcher: Box<dyn Matcher> = match algorithm {
        "coma" => Box::new(Coma::default()),
        "cupid" => Box::new(Cupid::default()),
        "distribution" => Box::new(DistributionBased::default()),
        "jaccard" => Box::new(JaccardDistanceMatcher::default()),
        "flooding" => Box::new(SimilarityFlooding::default()),
        _ => return Err(format!("unknown algorithm: {algorithm}").into()),
    };
    let (source_name, target_name) = table_names(args)?;
    let source = read_table(Path::new(&args[1]), source_name)?;
    let target = read_table(Path::new(&args[2]), target_name)?;
    Ok(valentine_match(&[source, target], matcher.as_ref())?)
}

fn run_bounded(args: &[OsString]) -> Result<MatcherResults, Box<dyn std::error::Error>> {
    let (args, options) = parse_bounded(args)?;
    let (source_name, target_name) = table_names(&args)?;
    let source = read_table_bounded(Path::new(&args[1]), source_name, options.max_input_bytes)?;
    let target = read_table_bounded(Path::new(&args[2]), target_name, options.max_input_bytes)?;
    let jaccard = JaccardDistanceMatcher::default();
    let distribution = DistributionBased::default();
    let matcher = match args[0].to_str() {
        Some("jaccard") => CheckedMatcher::Jaccard(&jaccard),
        Some("distribution") => CheckedMatcher::DistributionBased(&distribution),
        _ => unreachable!("bounded algorithms were validated before loading input"),
    };
    let matches = match_tables_with_limits(
        &[source, target],
        matcher,
        MatchOptions {
            instance_sample_size: Some(options.sample_size),
        },
        &options.limits,
    )?;
    Ok(match options.assignment {
        Assignment::None => matches,
        Assignment::Rectangular => {
            matches.one_to_one_rectangular_with_limits(options.threshold, &options.limits)?
        }
        Assignment::RectangularThresholdAware => matches
            .one_to_one_rectangular_threshold_aware_with_limits(
                options.threshold,
                &options.limits,
            )?,
    })
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let mut args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        writeln!(io::stdout().lock(), "{HELP}")?;
        return Ok(());
    }
    let has_bounded_options = args
        .iter()
        .take_while(|arg| *arg != "--")
        .any(|arg| BOUNDED_FLAGS.iter().any(|flag| arg == flag));
    let matches = if has_bounded_options {
        run_bounded(&args)?
    } else {
        // Keep required legacy paths positional, but never treat unescaped
        // option-like names as permission to run unchecked matching.
        let separator = args
            .iter()
            .skip(3)
            .position(|arg| arg == "--")
            .map(|i| i + 3);
        let names_end = separator.unwrap_or(args.len());
        if let Some(option) = args
            .get(3..names_end)
            .unwrap_or_default()
            .iter()
            .find(|arg| arg.as_encoded_bytes().starts_with(b"--"))
        {
            return Err(format!(
                "unknown option: {}; use -- before literal table names",
                option.to_string_lossy()
            )
            .into());
        }
        if let Some(index) = separator {
            args.remove(index);
        }
        run_legacy(&args)?
    };
    writeln!(
        io::stdout().lock(),
        "{}",
        serde_json::to_string_pretty(&matches)?
    )?;
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        let _ = writeln!(io::stderr().lock(), "fieldkin: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    fn args(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn bounded_options_have_explicit_finite_defaults_and_accept_zero_counts() {
        let (positional, options) = parse_bounded(&args(&[
            "--bounded",
            "jaccard",
            "source.csv",
            "target.json",
        ]))
        .unwrap();
        assert_eq!(positional, args(&["jaccard", "source.csv", "target.json"]));
        assert_eq!(options.max_input_bytes, 8_388_608);
        assert_eq!(options.sample_size, 1_000);
        assert_eq!(options.limits.max_assignment_dimension, 1_024);
        assert_eq!(options.limits.max_matrix_cells, 1_048_576);
        assert_eq!(options.limits.max_clustering_variables, 4_096);
        assert_eq!(options.limits.max_clustering_constraints, 262_144);
        assert_eq!(options.limits.max_jaccard_comparisons, 10_000_000);
        assert_eq!(options.assignment, Assignment::None);
        assert_eq!(options.threshold, None);

        let (_, options) = parse_bounded(&args(&[
            "jaccard",
            "--bounded",
            "source",
            "target",
            "--sample-size",
            "0",
            "--max-input-bytes",
            "0",
            "--max-matrix-cells",
            "0",
            "--max-jaccard-comparisons",
            "0",
            "--assignment",
            "rectangular",
            "--max-assignment-dimension",
            "0",
            "--threshold",
            "-0.5",
        ]))
        .unwrap();
        assert_eq!(options.sample_size, 0);
        assert_eq!(options.max_input_bytes, 0);
        assert_eq!(options.threshold, Some(-0.5));
        assert_eq!(options.limits.max_assignment_dimension, 0);
        assert_eq!(options.limits.max_matrix_cells, 0);
        assert_eq!(options.limits.max_jaccard_comparisons, 0);
    }

    #[test]
    fn bounded_parser_rejects_ambiguous_invalid_and_inapplicable_controls() {
        for (extra, message) in [
            (vec!["--bounded"], "duplicate"),
            (
                vec!["--sample-size", "1", "--sample-size", "2"],
                "duplicate",
            ),
            (vec!["--unknown", "1"], "unknown"),
            (vec!["-x"], "unknown"),
            (vec!["--sample-size"], "requires a value"),
            (vec!["--sample-size", "-1"], "nonnegative integer"),
            (vec!["--sample-size", "1.5"], "nonnegative integer"),
            (
                vec!["--sample-size", "9999999999999999999999999999999999"],
                "too large",
            ),
            (vec!["--threshold", "0.5"], "requires --assignment"),
            (
                vec!["--max-assignment-dimension", "1"],
                "requires --assignment",
            ),
            (vec!["--max-clustering-variables", "1"], "distribution"),
            (vec!["--max-clustering-constraints", "1"], "distribution"),
            (vec!["--assignment", "greedy"], "--assignment"),
            (
                vec!["--assignment", "rectangular", "--threshold", "NaN"],
                "finite",
            ),
            (
                vec!["--assignment", "rectangular", "--threshold", "inf"],
                "finite",
            ),
        ] {
            let mut values = args(&["jaccard", "source", "target", "--bounded"]);
            values.extend(args(&extra));
            let error = parse_bounded(&values).unwrap_err();
            assert!(error.contains(message), "{extra:?}: {error}");
        }
        for values in [
            vec!["jaccard", "source", "target", "--sample-size", "1"],
            vec![
                "distribution",
                "source",
                "target",
                "--bounded",
                "--max-jaccard-comparisons",
                "1",
            ],
            vec!["coma", "missing-source", "missing-target", "--bounded"],
        ] {
            assert!(parse_bounded(&args(&values)).is_err());
        }
        let mut overflowing = args(&[
            "jaccard",
            "source",
            "target",
            "--bounded",
            "--max-input-bytes",
        ]);
        overflowing.push(usize::MAX.to_string().into());
        assert!(
            parse_bounded(&overflowing)
                .unwrap_err()
                .contains("overflows")
        );
    }

    #[test]
    fn bounded_parser_preserves_option_like_and_native_paths_after_separator() {
        let values = args(&[
            "--bounded",
            "jaccard",
            "--",
            "--source.csv",
            "--target.json",
        ]);
        assert_eq!(
            parse_bounded(&values).unwrap().0,
            args(&["jaccard", "--source.csv", "--target.json"])
        );
        #[cfg(unix)]
        {
            use std::os::unix::ffi::OsStringExt;
            let path = OsString::from_vec(b"native-\xff.csv".to_vec());
            let values = vec![
                "--bounded".into(),
                "jaccard".into(),
                path.clone(),
                "target".into(),
            ];
            assert_eq!(parse_bounded(&values).unwrap().0[1], path);
        }
    }

    #[test]
    fn bounded_reader_distinguishes_exact_eof_from_an_extra_byte() {
        let path = Path::new("input with spaces.csv");
        for bytes in [b"".as_slice(), b"x", b"abc"] {
            assert_eq!(read_bounded(bytes, path, bytes.len()).unwrap(), bytes);
            assert_eq!(read_bounded(bytes, path, bytes.len() + 1).unwrap(), bytes);
            if !bytes.is_empty() {
                let cap = bytes.len() - 1;
                let error = read_bounded(bytes, path, cap).unwrap_err().to_string();
                for expected in [
                    "input with spaces.csv",
                    "--max-input-bytes",
                    &cap.to_string(),
                ] {
                    assert!(error.contains(expected), "{error}");
                }
            }
        }
        // An otherwise complete, valid CSV/JSON prefix must not be accepted.
        for prefix in [b"value\n1\n".as_slice(), b"[{\"value\":1}]".as_slice()] {
            let mut bytes = prefix.to_vec();
            bytes.push(b' ');
            assert!(read_bounded(bytes.as_slice(), path, prefix.len()).is_err());
        }
    }

    #[test]
    fn bounded_reader_retries_interruptions_and_short_reads_without_exceeding_cap_plus_one() {
        struct ShortReader<'a> {
            bytes: &'a [u8],
            calls: &'a Cell<usize>,
            bytes_read: &'a Cell<usize>,
        }
        impl Read for ShortReader<'_> {
            fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
                let calls = self.calls.get();
                self.calls.set(calls + 1);
                if calls.is_multiple_of(2) {
                    return Err(io::ErrorKind::Interrupted.into());
                }
                let count = self.bytes.len().min(out.len()).min(1);
                out[..count].copy_from_slice(&self.bytes[..count]);
                self.bytes = &self.bytes[count..];
                self.bytes_read.set(self.bytes_read.get() + count);
                Ok(count)
            }
        }
        for cap in [0, 2, 3, 4] {
            let calls = Cell::new(0);
            let bytes_read = Cell::new(0);
            let result = read_bounded(
                ShortReader {
                    bytes: b"abc",
                    calls: &calls,
                    bytes_read: &bytes_read,
                },
                Path::new("source.csv"),
                cap,
            );
            if cap >= 3 {
                assert_eq!(result.unwrap(), b"abc");
            } else {
                assert!(result.is_err());
            }
            assert_eq!(bytes_read.get(), 3.min(cap + 1));
            assert!(calls.get() > bytes_read.get());
        }
    }

    #[test]
    fn bounded_reader_propagates_errors_and_checks_overflow_before_reading() {
        struct Broken;
        impl Read for Broken {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "injected read failure",
                ))
            }
        }
        assert!(matches!(
            read_bounded(Broken, Path::new("input.csv"), 10),
            Err(fieldkin::Error::Io(error)) if error.kind() == io::ErrorKind::PermissionDenied
        ));
        struct MustNotRead;
        impl Read for MustNotRead {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
                panic!("overflow must be rejected before reading")
            }
        }
        assert!(read_bounded(MustNotRead, Path::new("input.csv"), usize::MAX).is_err());
        // A huge cap is not a request to preallocate it.
        assert_eq!(
            read_bounded(b"small".as_slice(), Path::new("input.csv"), usize::MAX - 1).unwrap(),
            b"small"
        );
    }
}
