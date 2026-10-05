//! Native performance protocols. Build and measurement remain separate phases.
//! Synthetic benchmark behavior is validated before any success record is written.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::common;

const RUNS: usize = 5;
const SIZES: [usize; 3] = [16, 64, 128];
const SCALE_SIZES: [usize; 3] = [128, 512, 1000];
const SCALE_FAMILIES: [&str; 3] = ["diagonal", "partial", "builtin"];
const ASSIGNMENT_FAMILIES: [&str; 9] = [
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
const ASSIGNMENT_PREAMBLE: [&str; 2] = [
    "fieldkin fixed assignment comparison; no samples; all original pairs scored",
    "one warm-up; construction excluded; matching and report destruction timed",
];
const ASSIGNMENT_HEADER: [&str; 17] = [
    "family",
    "size",
    "sources",
    "targets",
    "iterations",
    "total_ms",
    "us_per_match",
    "proposed",
    "confirmed",
    "excluded",
    "unmatched_sources",
    "unmatched_targets",
    "status",
    "solves",
    "work",
    "alternatives",
    "objective",
];
const REVIEW_PREAMBLE: [&str; 2] = [
    "fieldkin fixed caller-review benchmark; 16 samples/field when enabled",
    "one warm-up; input/engine/constraint construction excluded; report destruction included",
];
const MATCHING_PREAMBLE: [&str; 2] = [
    "fieldkin fixed synthetic benchmark; 16 samples/field when enabled",
    "one warm-up; engine/input construction excluded; no network or randomness",
];
const DIAGNOSTICS_HEADER: [&str; 19] = [
    "fixture",
    "source_count",
    "target_count",
    "mode",
    "max_solves",
    "max_work",
    "status",
    "solves_used",
    "work_used",
    "witnesses",
    "base_objective",
    "iteration",
    "elapsed_ns",
    "warmup_calls",
    "measured_calls",
    "debug_assertions",
    "crate_version",
    "target_os",
    "target_arch",
];
const SCALE_KEYS: [&str; 11] = [
    "family",
    "size",
    "one_to_one",
    "iterations",
    "elapsed_ns",
    "selected",
    "unmatched",
    "pairs",
    "signal_evaluations",
    "diagnostic_solves",
    "diagnostic_work",
];
const ALLOCATION_KEYS: [&str; 6] = [
    "allocations",
    "reallocations",
    "deallocations",
    "bytes_allocated",
    "bytes_deallocated",
    "bytes_reallocated",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Kind {
    Run,
    Context,
    Assignment,
    Scale,
    Review,
    Diagnostics,
    Corroboration,
}

impl Kind {
    fn parse(command: &str) -> Result<Self, String> {
        match command {
            "perf-run" => Ok(Self::Run),
            "perf-context" => Ok(Self::Context),
            "perf-assignment" => Ok(Self::Assignment),
            "perf-scale" => Ok(Self::Scale),
            "perf-review" => Ok(Self::Review),
            "perf-diagnostics" => Ok(Self::Diagnostics),
            "perf-corroboration" => Ok(Self::Corroboration),
            _ => Err(format!("unknown performance command: {command}")),
        }
    }
    fn protocol(self) -> &'static str {
        match self {
            Self::Run => "fieldkin-stage2-native-v1",
            Self::Context => "fieldkin-contextual-cost-native-v1",
            Self::Assignment => "assignment-compaction-native-v1",
            Self::Scale => "fieldkin-scale-native-v1",
            Self::Review => "review-cost-native-v1",
            Self::Diagnostics => "diagnostics-cost-native-v1",
            Self::Corroboration => "corroboration-cost-native-v1",
        }
    }
    fn paired(self) -> bool {
        matches!(self, Self::Run | Self::Context | Self::Assignment)
    }
    fn harness(self) -> &'static [&'static str] {
        match self {
            Self::Run | Self::Context => &[
                "performance/Cargo.toml",
                "performance/Cargo.lock",
                "performance/src/main.rs",
            ],
            Self::Assignment => &["examples/assignment_cost.rs"],
            Self::Scale => &["examples/scale_cost.rs"],
            Self::Review => &["examples/review_cost.rs"],
            Self::Diagnostics => &["examples/diagnostics_cost.rs"],
            Self::Corroboration => &["benches/matching.rs"],
        }
    }
    fn target(self) -> (&'static str, &'static str) {
        match self {
            Self::Run | Self::Context => ("bin", "fieldkin-performance"),
            Self::Assignment => ("example", "assignment_cost"),
            Self::Scale => ("example", "scale_cost"),
            Self::Review => ("example", "review_cost"),
            Self::Diagnostics => ("example", "diagnostics_cost"),
            Self::Corroboration => ("bench", "matching"),
        }
    }
    fn normalized_sources(self) -> bool {
        !matches!(self, Self::Assignment | Self::Scale)
    }
}

struct Options {
    action: String,
    output: PathBuf,
    baseline: Option<PathBuf>,
    candidate: Option<PathBuf>,
}

fn absolute(path: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path);
    let path = if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map_err(|error| error.to_string())?
            .join(path)
    };
    common::checked_path(&path)?;
    Ok(path)
}

fn parse_options(kind: Kind, args: &[String]) -> Result<Options, String> {
    let Some(action) = args.first() else {
        return Err("choose build or run (perf-run also supports summarize)".into());
    };
    if !matches!(action.as_str(), "build" | "run")
        && !(matches!(kind, Kind::Run | Kind::Context) && action == "summarize")
    {
        return Err(format!("unsupported performance action: {action}"));
    }
    let mut values = BTreeMap::new();
    for pair in args[1..].chunks(2) {
        if pair.len() != 2 || !matches!(pair[0].as_str(), "--output" | "--baseline" | "--candidate")
        {
            return Err(
                "use --output PATH, and paired build may use --baseline PATH --candidate PATH"
                    .into(),
            );
        }
        if values.insert(pair[0].as_str(), pair[1].as_str()).is_some() {
            return Err("duplicate performance option".into());
        }
    }
    let output = absolute(values.get("--output").ok_or("--output is required")?)?;
    let baseline = values
        .get("--baseline")
        .map(|path| absolute(path))
        .transpose()?;
    let candidate = values
        .get("--candidate")
        .map(|path| absolute(path))
        .transpose()?;
    if action == "build" && kind.paired() && baseline.is_none() {
        return Err("paired build requires --baseline".into());
    }
    if (action != "build" || !kind.paired()) && (baseline.is_some() || candidate.is_some()) {
        return Err("this phase uses the recorded checkout, not --baseline or --candidate".into());
    }
    Ok(Options {
        action: action.clone(),
        output,
        baseline,
        candidate,
    })
}

/// Six native commands, with the former build/run options and no interpreter dependency.
pub fn dispatch(command: &str, args: Vec<String>) -> Result<(), String> {
    let kind = Kind::parse(command)?;
    if args.iter().any(|arg| arg == "--help") {
        println!("{command} build --output FRESH_DIRECTORY{}\n{command} run --output BUILT_DIRECTORY\nUses the installed stable Rust channel, freezes the actual compiler and runner, and measures five independent processes. Build and run are separate; stop concurrent builds/tests before run. Native records have a distinct protocol ID; historical records stay unchanged.", if kind.paired() { " --baseline CHECKOUT [--candidate CHECKOUT]" } else { "" });
        if matches!(kind, Kind::Run | Kind::Context) {
            println!("{command} summarize --output MEASURED_DIRECTORY");
        }
        return Ok(());
    }
    let options = parse_options(kind, &args)?;
    match options.action.as_str() {
        "build" => build(kind, options),
        "run" => measure(kind, &options.output),
        "summarize" => summarize_stage(kind, &options.output),
        _ => unreachable!(),
    }
}

fn now() -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    // UTC Gregorian conversion, avoiding an external process or local timezone.
    let seconds = elapsed.as_secs();
    let shifted = (seconds / 86400) as i64 + 719468;
    let era = shifted / 146097;
    let day_of_era = shifted - era * 146097;
    let year_of_era =
        (day_of_era - day_of_era / 1460 + day_of_era / 36524 - day_of_era / 146096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02}T{:02}:{:02}:{:02}.{:09}Z",
        seconds / 3600 % 24,
        seconds / 60 % 60,
        seconds % 60,
        elapsed.subsec_nanos()
    )
}

fn environment() -> Value {
    let names = [
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTC",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "CARGO_BUILD_TARGET",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "RUSTUP_TOOLCHAIN",
        "RUSTDOCFLAGS",
        "CARGO_ENCODED_RUSTDOCFLAGS",
    ];
    let values: BTreeMap<_, _> = std::env::vars()
        .filter(|(key, _)| {
            names.contains(&key.as_str())
                || [
                    "CARGO_PROFILE_RELEASE_",
                    "CARGO_PROFILE_BENCH_",
                    "CARGO_TARGET_",
                    "CARGO_BUILD_",
                ]
                .iter()
                .any(|prefix| key.starts_with(prefix))
        })
        .collect();
    json!(values)
}

fn machine() -> Value {
    json!({"platform":std::env::consts::OS, "machine":std::env::consts::ARCH,
        "processor":std::env::var("PROCESSOR_IDENTIFIER").unwrap_or_else(|_| "unspecified".into()),
        "logical_cpus":std::thread::available_parallelism().map(|count| count.get()).ok(),
        "runner":"native Rust", "runner_crate_version":env!("CARGO_PKG_VERSION")})
}

fn capture(program: &str, args: &[String], root: &Path) -> Result<String, String> {
    let bytes = common::command(program, args, root)?.stdout;
    String::from_utf8(bytes)
        .map(|text| text.trim().replace("\r\n", "\n"))
        .map_err(|_| "tool emitted non-UTF8 output".into())
}

fn compiler() -> Result<Value, String> {
    reject_compiler_overrides(
        &std::env::vars().collect(),
        &toml::Value::Table(Default::default()),
    )?;
    let root = common::root();
    let rustc = capture("rustc", &["+stable".into(), "-Vv".into()], &root)?;
    if !rustc.lines().any(|line| {
        line.starts_with("release: ") && !line.contains("nightly") && !line.contains("beta")
    }) {
        return Err("performance tooling requires a stable Rust compiler".into());
    }
    Ok(
        json!({"channel":"stable", "rustc":rustc, "cargo":capture("cargo", &["+stable".into(), "-Vv".into()], &root)?}),
    )
}

fn file_digest(path: &Path, normalized: bool) -> Result<String, String> {
    let bytes = common::read(path)?;
    if normalized {
        let mut result = Vec::with_capacity(bytes.len());
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
                index += 1;
            }
            result.push(bytes[index]);
            index += 1;
        }
        Ok(common::digest(&result))
    } else {
        Ok(common::digest(&bytes))
    }
}

fn all_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<(), String> {
    common::checked_path(directory)?;
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        common::checked_path(&path)?;
        if path.is_dir() {
            all_files(&path, files)?;
        } else if path.is_file() {
            files.push(path);
        }
        if files.len() > 4096 {
            return Err("performance source inventory file budget exceeded".into());
        }
    }
    Ok(())
}

fn inventory(root: &Path, kind: Kind) -> Result<Value, String> {
    let mut files = Vec::new();
    all_files(&root.join("src"), &mut files)?;
    files.extend(["Cargo.toml", "Cargo.lock", "README.md"].map(|name| root.join(name)));
    files.extend(kind.harness().iter().map(|name| root.join(name)));
    for directory in [
        root.to_owned(),
        root.join("evaluation"),
        root.join("performance"),
    ] {
        for name in ["build.rs", "rust-toolchain", "rust-toolchain.toml"] {
            let path = directory.join(name);
            if path.exists() {
                files.push(path);
            }
        }
    }
    let evaluation_manifest = root.join("evaluation/Cargo.toml");
    if evaluation_manifest.exists() {
        files.push(evaluation_manifest);
    }
    let mut result = BTreeMap::new();
    for path in files {
        result.insert(
            path.strip_prefix(root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/"),
            file_digest(&path, kind.normalized_sources())?,
        );
    }
    Ok(json!(result))
}

fn runner_record() -> Result<Value, String> {
    let root = common::root();
    let mut files = Vec::new();
    all_files(&root.join("tooling/src"), &mut files)?;
    files.extend([
        root.join("tooling/Cargo.toml"),
        root.join("Cargo.toml"),
        root.join("Cargo.lock"),
        root.join("rust-toolchain.toml"),
    ]);
    let mut sources = BTreeMap::new();
    for path in files {
        sources.insert(
            path.strip_prefix(&root)
                .map_err(|error| error.to_string())?
                .to_string_lossy()
                .replace('\\', "/"),
            file_digest(&path, false)?,
        );
    }
    let executable = std::env::current_exe().map_err(|error| error.to_string())?;
    Ok(
        json!({"source_sha256":sources,"binary_path":executable,"binary_sha256":file_digest(&executable,false)?}),
    )
}

fn cargo_configs(root: &Path) -> Result<Value, String> {
    let home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                .map(|home| PathBuf::from(home).join(".cargo"))
        })
        .ok_or("cannot resolve Cargo home")?;
    let home = if home.is_absolute() {
        home
    } else {
        root.join(home)
    };
    let mut directories: BTreeSet<_> = root
        .ancestors()
        .map(|ancestor| ancestor.join(".cargo"))
        .collect();
    directories.insert(home);
    let mut entries = BTreeMap::new();
    for directory in directories {
        for name in ["config", "config.toml"] {
            let path = directory.join(name);
            if path.exists() {
                let bytes = common::read(&path)?;
                let config: toml::Value = toml::from_str(
                    std::str::from_utf8(&bytes).map_err(|_| "Cargo configuration is not UTF8")?,
                )
                .map_err(|error| error.to_string())?;
                reject_compiler_overrides(&std::env::vars().collect(), &config)?;
                entries.insert(
                    path.to_string_lossy().into_owned(),
                    file_digest(&path, false)?,
                );
            }
        }
    }
    Ok(json!(entries))
}

fn compiler_override_name(name: &str) -> bool {
    matches!(
        name.to_ascii_uppercase().as_str(),
        "RUSTC"
            | "RUSTDOC"
            | "RUSTC_WRAPPER"
            | "RUSTC_WORKSPACE_WRAPPER"
            | "CARGO_BUILD_TARGET"
            | "CARGO_BUILD_RUSTC"
            | "CARGO_BUILD_RUSTDOC"
            | "CARGO_BUILD_RUSTC_WRAPPER"
            | "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"
    )
}

fn reject_compiler_overrides(
    environment: &BTreeMap<String, String>,
    config: &toml::Value,
) -> Result<(), String> {
    if environment.keys().any(|key| compiler_override_name(key)) {
        return Err("native performance records require the stable host compiler; compiler wrappers and cross-target overrides are unsupported".into());
    }
    if config.get("include").is_some()
        || config.get("source").is_some()
        || config.get("target").is_some()
        || config.get("build").is_some_and(|build| {
            [
                "target",
                "rustc",
                "rustdoc",
                "rustc-wrapper",
                "rustc-workspace-wrapper",
            ]
            .iter()
            .any(|key| build.get(key).is_some())
        })
        || config
            .get("env")
            .and_then(toml::Value::as_table)
            .is_some_and(|table| table.keys().any(|key| compiler_override_name(key)))
    {
        return Err("Cargo includes, source/target tables and compiler overrides are unsupported in native performance records".into());
    }
    Ok(())
}

fn write_text_new(path: &Path, text: &str) -> Result<(), String> {
    common::checked_path(path)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("{}: {error}", path.display()))?;
    file.write_all(text.as_bytes())
        .map_err(|error| error.to_string())
}

fn ensure_fresh(directory: &Path) -> Result<(), String> {
    common::checked_path(directory)?;
    if directory.exists()
        && fs::read_dir(directory)
            .map_err(|error| error.to_string())?
            .next()
            .is_some()
    {
        return Err("use a fresh empty performance output directory".into());
    }
    fs::create_dir_all(directory).map_err(|error| error.to_string())
}

fn only_build(directory: &Path) -> Result<(), String> {
    common::checked_path(directory)?;
    let names: BTreeSet<_> = fs::read_dir(directory)
        .map_err(|error| error.to_string())?
        .map(|entry| {
            entry
                .map(|entry| entry.file_name())
                .map_err(|error| error.to_string())
        })
        .collect::<Result<_, _>>()?;
    if names != BTreeSet::from([std::ffi::OsString::from("build.json")]) {
        return Err("measurement destination must contain only build.json".into());
    }
    Ok(())
}

fn close(actual: f64, expected: f64, relative: f64, absolute: f64) -> bool {
    actual.is_finite()
        && expected.is_finite()
        && (actual - expected).abs() <= absolute.max(relative * actual.abs().max(expected.abs()))
}

fn specification(kind: Kind) -> Value {
    match kind {
        Kind::Run => {
            json!({"workloads":50,"warmup_calls":3,"allocations_warmup_calls":1,"timing_instrumented":false,"iterations_by_core_size":{"16":30,"64":10,"128":5},"extended_iterations":5,"budget_iterations":500,"allocation_iterations":1,"order":"timing then allocations; baseline,candidate on odd runs; candidate,baseline on even runs","timing_scope":"matching and report destruction; input and engine setup excluded"})
        }
        Kind::Context => {
            json!({"workloads":24,"baseline_policy":"ContextualEvidence::default() with no configured conflict rules","candidate_policy":"Config::contextual_quality()","harness_feature":"quality-policy enabled only for candidate","max_explanation_bytes":33554432,"report_rejection_max_explanation_bytes":1,"sizes":SIZES,"sampled": [false,true],"extra_cases":["ambiguous","disjoint"],"budget_cases":["input","pairs","signals","report"],"assignments":["independent","one_to_one"],"runs":RUNS,"warmup_calls":3,"allocations_warmup_calls":1,"iterations_by_core_size":{"16":30,"64":10,"128":5},"extended_iterations":5,"budget_iterations":500,"allocation_iterations":1,"timing_scope":"matching and report destruction; input and engine construction excluded","interpretation":"Policies can produce different decisions; this is their actual runtime cost on identical schemas, not a calibrated confidence comparison"})
        }
        Kind::Assignment => {
            json!({"families":ASSIGNMENT_FAMILIES,"sizes":SIZES,"iterations":{"16":10,"64":4,"128":2},"warmup_calls":1,"post_timing_validation_calls":1,"config":{"one_to_one":true,"abstain_on_ambiguity":false,"min_score":0.70,"ambiguity_margin":0.08,"max_candidates":5,"samples":false,"max_work":536870912,"objective_margin":0.08,"diagnostic_solves":{"disabled":0,"bounded":2,"complete":128}},"order":"fixed workload order; alternating revision order","timing_scope":"matching and report destruction; input, engine, constraints and behavior summary excluded"})
        }
        Kind::Scale => {
            json!({"sizes":SCALE_SIZES,"families":SCALE_FAMILIES,"iterations":{"128":3,"512":1,"1000":1},"warmup_calls":1,"max_candidates":1,"diagnostic_solves":0,"max_explanation_bytes":536870912,"limits":"max_fields=size; max_pairs=size*size; max_signal_evaluations=size*size*(3 for builtin, otherwise 1)","builtin":"default builtins; ideal disjoint 16-Integer samples","custom":"0.9 diagonal; partial removes every eighth diagonal; zero otherwise","timing_scope":"matching and report destruction; fixture and engine creation excluded"})
        }
        Kind::Review => {
            json!({"workloads":36,"policies":["none","empty","mixed-review"],"sizes":SIZES,"iterations":{"16":30,"64":10,"128":5},"warmup_calls":1,"mixed_review":"confirm first quarter, exclude second quarter, forbid one wrong cyclic target per remaining source","config":"default builtins; threshold .7; diagnostics disabled; assignment per workload","order":"size, samples, assignment, policy","timing_scope":"matching and report destruction; input, engine and constraint setup excluded"})
        }
        Kind::Diagnostics => {
            json!({"workloads":20,"measurements":60,"warmup_calls":1,"measured_calls":3,"default_work_budget":8388608,"dimensions":[[16,16],[64,64],[128,128],[128,64],[64,128]],"modes":["disabled","one_probe_budget","default_work_budget","complete"],"timing_scope":"match_schemas only; excludes setup, assertions and returned-report destruction"})
        }
        Kind::Corroboration => {
            json!({"workloads":36,"models":["combined","sample-supported","name-only"],"sizes":SIZES,"iterations":{"16":30,"64":10,"128":5},"warmup_calls":1,"config":"default Corroboration when sample-supported; 16 values with nulls","order":"size, samples, assignment, model","timing_scope":"matching and report destruction; input and engine setup excluded"})
        }
    }
}

#[derive(Debug)]
struct Validated {
    text: String,
    rows: Vec<Value>,
    behavior: Vec<Value>,
}

fn csv_rows(raw: &str, preamble: &[&str], header: &[&str]) -> Result<(String, Vec<Value>), String> {
    let lines: Vec<_> = raw.lines().collect();
    if lines.len() <= preamble.len()
        || !lines
            .iter()
            .take(preamble.len())
            .copied()
            .eq(preamble.iter().copied())
    {
        return Err("unexpected benchmark preamble".into());
    }
    let text = lines[preamble.len()..].join("\n") + "\n";
    let mut reader = csv::ReaderBuilder::new()
        .flexible(false)
        .from_reader(text.as_bytes());
    if !reader
        .headers()
        .map_err(|error| error.to_string())?
        .iter()
        .eq(header.iter().copied())
    {
        return Err("unexpected benchmark CSV columns".into());
    }
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|error| error.to_string())?;
        let row: serde_json::Map<_, _> = header
            .iter()
            .zip(record.iter())
            .map(|(key, value)| ((*key).into(), json!(value)))
            .collect();
        rows.push(Value::Object(row));
    }
    Ok((text, rows))
}

fn text<'a>(row: &'a Value, key: &str) -> Result<&'a str, String> {
    row[key]
        .as_str()
        .ok_or_else(|| format!("missing text column {key}"))
}
fn integer(row: &Value, key: &str) -> Result<usize, String> {
    text(row, key)?
        .parse()
        .map_err(|_| format!("invalid integer column {key}"))
}
fn duration(row: &Value, key: &str) -> Result<f64, String> {
    let value: f64 = text(row, key)?
        .parse()
        .map_err(|_| format!("invalid duration column {key}"))?;
    if !value.is_finite() || value < 0.0 {
        return Err("invalid benchmark duration".into());
    }
    Ok(value)
}
fn json_integer(row: &Value, key: &str) -> Result<u64, String> {
    row[key]
        .as_u64()
        .ok_or_else(|| format!("invalid nonnegative JSON integer {key}"))
}

fn assignment_counts(size: usize, family: &str, smoke: bool) -> Value {
    let n = if family == "wide-complete" {
        size / 4
    } else {
        size
    };
    let m = if family == "tall-bounded" {
        size / 4
    } else {
        size
    };
    let confirmed = match family {
        "mixed-bounded" => size / 4,
        "reserved-complete" => size - 4,
        _ => 0,
    };
    let excluded = match family {
        "all-excluded-complete" => size,
        "half-excluded-disabled" => size / 2,
        "mixed-bounded" => size / 4,
        _ => 0,
    };
    let proposed = match family {
        "dense-disabled" | "dense-bounded" => size,
        "all-excluded-complete" => 0,
        "half-excluded-disabled" | "mixed-bounded" => size / 2,
        _ => 4,
    };
    let solves = if family.ends_with("disabled") {
        0
    } else if family.ends_with("bounded") {
        2
    } else {
        proposed
    };
    let status = if family.ends_with("disabled") {
        "Disabled"
    } else if family.ends_with("bounded") {
        "BudgetExhausted"
    } else {
        "Complete"
    };
    json!({"sources":n,"targets":m,"iterations":if smoke {1} else {match size {16=>10,64=>4,_=>2}},"proposed":proposed,"confirmed":confirmed,"excluded":excluded,"unmatched_sources":n-proposed-confirmed,"unmatched_targets":m-proposed-confirmed,"solves":solves,"work":solves*n*n*(m+n),"status":status})
}

fn validate_assignment(raw: &str, smoke: bool) -> Result<Validated, String> {
    let (csv_text, rows) = csv_rows(raw, &ASSIGNMENT_PREAMBLE, &ASSIGNMENT_HEADER)?;
    let mut keys = Vec::new();
    let mut behavior = Vec::new();
    for row in &rows {
        let size = integer(row, "size")?;
        let family = text(row, "family")?;
        if !SIZES.contains(&size) || !ASSIGNMENT_FAMILIES.contains(&family) {
            return Err("unknown assignment workload".into());
        }
        keys.push((size, family));
        let expected = assignment_counts(size, family, smoke);
        for (key, value) in expected.as_object().unwrap() {
            if key == "status" {
                if row[key] != *value {
                    return Err("unexpected assignment diagnostic status".into());
                }
            } else if integer(row, key)? as u64 != value.as_u64().unwrap() {
                return Err(
                    "unexpected assignment decision counts or original-dimension work charge"
                        .into(),
                );
            }
        }
        if integer(row, "alternatives")? > integer(row, "solves")? {
            return Err("invalid assignment alternatives count".into());
        }
        if row["status"] == "Disabled" {
            if row["objective"] != "none" {
                return Err("disabled objective must be absent".into());
            }
        } else {
            let value = text(row, "objective")?
                .parse()
                .map_err(|_| "invalid assignment objective")?;
            if !close(value, 0.85 * integer(row, "proposed")? as f64, 1e-12, 1e-12) {
                return Err("unexpected automatic-only objective".into());
            }
        }
        let total = duration(row, "total_ms")?;
        let mean = duration(row, "us_per_match")?;
        if !close(
            total * 1000.0 / integer(row, "iterations")? as f64,
            mean,
            1e-6,
            0.001,
        ) {
            return Err("inconsistent assignment durations".into());
        }
        let mut stable = row.clone();
        stable.as_object_mut().unwrap().remove("total_ms");
        stable.as_object_mut().unwrap().remove("us_per_match");
        behavior.push(stable);
    }
    let expected: Vec<_> = SIZES
        .into_iter()
        .flat_map(|size| {
            ASSIGNMENT_FAMILIES
                .into_iter()
                .map(move |family| (size, family))
        })
        .collect();
    if keys != expected {
        return Err("missing, duplicate or reordered assignment workloads".into());
    }
    Ok(Validated {
        text: csv_text,
        rows,
        behavior,
    })
}

fn validate_scale(raw: &str, small: bool) -> Result<Validated, String> {
    let mut rows = Vec::new();
    let mut keys = Vec::new();
    for line in raw.lines() {
        let row: Value = serde_json::from_str(line).map_err(|error| error.to_string())?;
        let object = row.as_object().ok_or("scale row is not an object")?;
        if object.len() != SCALE_KEYS.len()
            || SCALE_KEYS.iter().any(|key| !object.contains_key(*key))
        {
            return Err("unexpected scale output fields".into());
        }
        let size = json_integer(&row, "size")? as usize;
        let family = text(&row, "family")?;
        let policy = row["one_to_one"]
            .as_bool()
            .ok_or("invalid scale assignment policy")?;
        if !SCALE_SIZES.contains(&size) || !SCALE_FAMILIES.contains(&family) {
            return Err("unknown scale workload".into());
        }
        keys.push((size, family.to_owned(), policy));
        let selected = size - if family == "partial" { size / 8 } else { 0 };
        let expected = [
            ("iterations", if size == 128 { 3 } else { 1 }),
            ("selected", selected),
            ("unmatched", size - selected),
            ("pairs", size * size),
            (
                "signal_evaluations",
                size * size * if family == "builtin" { 3 } else { 1 },
            ),
            ("diagnostic_solves", 0),
            ("diagnostic_work", 0),
        ];
        for (key, value) in expected {
            if json_integer(&row, key)? as usize != value {
                return Err("scale decisions, pair counts or diagnostic work changed".into());
            }
        }
        json_integer(&row, "elapsed_ns")?;
        rows.push(row);
    }
    let sizes: &[usize] = if small { &[128] } else { &SCALE_SIZES };
    let expected: Vec<_> = sizes
        .iter()
        .flat_map(|size| {
            SCALE_FAMILIES.into_iter().flat_map(move |family| {
                [false, true].map(|policy| (*size, family.to_owned(), policy))
            })
        })
        .collect();
    if keys != expected {
        return Err("missing, duplicate or reordered scale workloads".into());
    }
    let behavior = rows
        .iter()
        .map(|row| {
            let mut row = row.clone();
            row.as_object_mut().unwrap().remove("elapsed_ns");
            row
        })
        .collect();
    Ok(Validated {
        text: raw.trim_end().to_owned() + "\n",
        rows,
        behavior,
    })
}

fn validate_matching(raw: &str, review: bool, smoke: bool) -> Result<Validated, String> {
    let models: &[&str] = if review {
        &["none", "empty", "mixed-review"]
    } else {
        &["combined", "sample-supported", "name-only"]
    };
    let model_key = if review { "policy" } else { "engine" };
    let header = [
        model_key,
        "fields_per_side",
        "samples",
        "one_to_one",
        "iterations",
        "total_ms",
        "us_per_match",
    ];
    let (csv_text, rows) = csv_rows(
        raw,
        if review {
            &REVIEW_PREAMBLE
        } else {
            &MATCHING_PREAMBLE
        },
        &header,
    )?;
    let mut keys = Vec::new();
    let mut behavior = Vec::new();
    for row in &rows {
        let size = integer(row, "fields_per_side")?;
        let model = text(row, model_key)?;
        let samples = text(row, "samples")?;
        let assignment = text(row, "one_to_one")?;
        if !SIZES.contains(&size)
            || !models.contains(&model)
            || !["false", "true"].contains(&samples)
            || !["false", "true"].contains(&assignment)
        {
            return Err("unknown matching workload".into());
        }
        keys.push((size, samples, assignment, model));
        let calls = if smoke {
            1
        } else {
            match size {
                16 => 30,
                64 => 10,
                _ => 5,
            }
        };
        if integer(row, "iterations")? != calls {
            return Err("unexpected matching iteration count".into());
        }
        let total = duration(row, "total_ms")?;
        let mean = duration(row, "us_per_match")?;
        // Both are independently rounded by the public benchmark to 3 decimals.
        if !close(total * 1000.0 / calls as f64, mean, 1e-6, 0.501) {
            return Err("inconsistent matching durations".into());
        }
        let mut row = row.clone();
        row.as_object_mut().unwrap().remove("total_ms");
        row.as_object_mut().unwrap().remove("us_per_match");
        behavior.push(row);
    }
    let expected: Vec<_> = SIZES
        .into_iter()
        .flat_map(|size| {
            ["false", "true"].into_iter().flat_map(move |samples| {
                ["false", "true"].into_iter().flat_map(move |assignment| {
                    models
                        .iter()
                        .map(move |model| (size, samples, assignment, *model))
                })
            })
        })
        .collect();
    if keys != expected {
        return Err("missing, duplicate or reordered matching workloads".into());
    }
    Ok(Validated {
        text: csv_text,
        rows,
        behavior,
    })
}

fn validate_diagnostics(raw: &str) -> Result<Validated, String> {
    let (csv_text, rows) = csv_rows(raw, &[], &DIAGNOSTICS_HEADER)?;
    let mut keys = Vec::new();
    let mut behavior = Vec::new();
    for row in &rows {
        let n = integer(row, "source_count")?;
        let m = integer(row, "target_count")?;
        let mode = text(row, "mode")?;
        let iteration = integer(row, "iteration")?;
        if ![(16, 16), (64, 64), (128, 128), (128, 64), (64, 128)].contains(&(n, m))
            || !(1..=3).contains(&iteration)
        {
            return Err("unknown diagnostics dimensions or iteration".into());
        }
        keys.push((n, m, mode, iteration));
        if row["fixture"] != "dense_equal"
            || row["debug_assertions"] != "false"
            || integer(row, "warmup_calls")? != 1
            || integer(row, "measured_calls")? != 3
        {
            return Err("unexpected diagnostics fixture, call count or debug binary".into());
        }
        let per_solve = n * n * (m + n);
        let solves = integer(row, "solves_used")?;
        let work = integer(row, "work_used")?;
        if solves > n.min(m) || work != solves * per_solve || integer(row, "witnesses")? > solves {
            return Err("invalid diagnostics solves, work or witnesses".into());
        }
        match mode {
            "disabled" => {
                if row["status"] != "Disabled"
                    || solves != 0
                    || integer(row, "max_solves")? != 0
                    || !text(row, "base_objective")?.is_empty()
                {
                    return Err("invalid disabled diagnostics".into());
                }
            }
            "one_probe_budget" => {
                if row["status"] != "BudgetExhausted"
                    || solves != 1
                    || integer(row, "max_work")? != per_solve
                    || integer(row, "max_solves")? != n
                {
                    return Err("invalid one-probe diagnostic charge".into());
                }
            }
            "default_work_budget" => {
                let expected = (8388608 / per_solve).min(n.min(m));
                let status = if expected == n.min(m) {
                    "Complete"
                } else {
                    "BudgetExhausted"
                };
                if integer(row, "max_solves")? != n
                    || integer(row, "max_work")? != 8388608
                    || solves != expected
                    || row["status"] != status
                {
                    return Err("invalid default diagnostic budget".into());
                }
            }
            "complete" => {
                if row["status"] != "Complete"
                    || solves != n.min(m)
                    || integer(row, "max_solves")? != n
                    || integer(row, "max_work")? != per_solve * n.min(m)
                {
                    return Err("incomplete diagnostic workload".into());
                }
            }
            _ => return Err("unknown diagnostics mode".into()),
        }
        if mode != "disabled" {
            let objective: f64 = text(row, "base_objective")?
                .parse()
                .map_err(|_| "invalid diagnostic objective")?;
            if !close(objective, 0.9 * n.min(m) as f64, 1e-12, 1e-12) {
                return Err("unexpected diagnostic objective".into());
            }
        }
        integer(row, "elapsed_ns")?;
        let mut row = row.clone();
        row.as_object_mut().unwrap().remove("elapsed_ns");
        behavior.push(row);
    }
    let expected: Vec<_> = [(16, 16), (64, 64), (128, 128), (128, 64), (64, 128)]
        .into_iter()
        .flat_map(|(n, m)| {
            [
                "disabled",
                "one_probe_budget",
                "default_work_budget",
                "complete",
            ]
            .into_iter()
            .flat_map(move |mode| (1..=3).map(move |iteration| (n, m, mode, iteration)))
        })
        .collect();
    if keys != expected {
        return Err("missing, duplicate or reordered diagnostics measurements".into());
    }
    Ok(Validated {
        text: csv_text,
        rows,
        behavior,
    })
}

fn stage_workloads(smoke: bool) -> Vec<(String, usize, usize, usize, bool)> {
    let mut expected = Vec::new();
    for size in SIZES {
        for samples in ["empty", "sampled"] {
            for assignment in ["independent", "assignment"] {
                for model in ["combined", "name-only"] {
                    expected.push((
                        format!("core-{size}-{samples}-{assignment}-{model}"),
                        size,
                        size,
                        if smoke {
                            1
                        } else {
                            match size {
                                16 => 30,
                                64 => 10,
                                _ => 5,
                            }
                        },
                        false,
                    ));
                }
            }
        }
    }
    for (style, n, m) in [
        ("short", 64, 64),
        ("long", 64, 64),
        ("sparse", 64, 64),
        ("dense", 64, 64),
        ("rejected", 64, 64),
        ("competition", 64, 64),
        ("unequal-wide", 16, 128),
        ("unequal-tall", 128, 16),
        ("single-pair", 1, 1),
        ("empty-source", 0, 128),
        ("empty-target", 128, 0),
    ] {
        for assignment in ["independent", "assignment"] {
            expected.push((
                format!("extended-{style}-{assignment}"),
                n,
                m,
                if smoke { 1 } else { 5 },
                false,
            ));
        }
    }
    for budget in ["input", "pairs", "signals", "report"] {
        expected.push((
            format!("budget-{budget}"),
            16,
            16,
            if smoke { 1 } else { 500 },
            true,
        ));
    }
    expected
}

fn validate_stage(raw: &str, allocations: bool, smoke: bool) -> Result<Validated, String> {
    validate_workloads(raw, allocations, stage_workloads(smoke))
}

fn context_workloads(smoke: bool) -> Vec<(String, usize, usize, usize, bool)> {
    let mut expected = Vec::new();
    for size in SIZES {
        for samples in ["empty", "sampled"] {
            for assignment in ["independent", "assignment"] {
                expected.push((
                    format!("context-{size}-{samples}-{assignment}"),
                    size,
                    size,
                    if smoke {
                        1
                    } else {
                        match size {
                            16 => 30,
                            64 => 10,
                            _ => 5,
                        }
                    },
                    false,
                ));
            }
        }
    }
    for style in ["ambiguous", "disjoint"] {
        for assignment in ["independent", "assignment"] {
            expected.push((
                format!("context-{style}-{assignment}"),
                64,
                64,
                if smoke { 1 } else { 5 },
                false,
            ));
        }
    }
    for budget in ["input", "pairs", "signals", "report"] {
        for assignment in ["independent", "assignment"] {
            expected.push((
                format!("context-budget-{budget}-{assignment}"),
                16,
                16,
                if smoke { 1 } else { 500 },
                true,
            ));
        }
    }
    expected
}

fn validate_workloads(
    raw: &str,
    allocations: bool,
    expected: Vec<(String, usize, usize, usize, bool)>,
) -> Result<Validated, String> {
    let mut rows = Vec::new();
    let mut behavior = Vec::new();
    for (index, line) in raw.lines().enumerate() {
        let row: Value = serde_json::from_str(line).map_err(|error| error.to_string())?;
        let object = row.as_object().ok_or("benchmark row is not an object")?;
        let (id, n, m, calls, error) = expected.get(index).ok_or("excess performance workload")?;
        let keys = [
            "workload",
            "iterations",
            "elapsed_ns",
            "sources",
            "targets",
            "expects_error",
        ];
        if object.len()
            != keys.len()
                + if allocations {
                    ALLOCATION_KEYS.len()
                } else {
                    0
                }
            || keys.iter().any(|key| !object.contains_key(*key))
            || (allocations && ALLOCATION_KEYS.iter().any(|key| !object.contains_key(*key)))
        {
            return Err("unexpected timing/allocation output fields".into());
        }
        if row["workload"] != *id
            || json_integer(&row, "sources")? as usize != *n
            || json_integer(&row, "targets")? as usize != *m
            || json_integer(&row, "iterations")? as usize != *calls
            || row["expects_error"].as_bool() != Some(*error)
        {
            return Err("changed, missing or reordered performance workloads".into());
        }
        json_integer(&row, "elapsed_ns")?;
        if allocations {
            for key in ALLOCATION_KEYS {
                if key == "bytes_reallocated" {
                    row[key]
                        .as_i64()
                        .ok_or("invalid signed reallocation byte count")?;
                } else {
                    json_integer(&row, key)?;
                }
            }
        }
        let stable: serde_json::Map<_, _> = keys
            .iter()
            .filter(|key| **key != "elapsed_ns")
            .map(|key| ((*key).to_owned(), row[*key].clone()))
            .collect();
        behavior.push(Value::Object(stable));
        rows.push(row);
    }
    if rows.len() != expected.len() {
        return Err("missing performance workloads".into());
    }
    Ok(Validated {
        text: raw.trim_end().to_owned() + "\n",
        rows,
        behavior,
    })
}

fn validate(kind: Kind, raw: &str, mode: &str, smoke: bool) -> Result<Validated, String> {
    match kind {
        Kind::Run => validate_stage(raw, mode == "allocations", smoke),
        Kind::Context => validate_workloads(raw, mode == "allocations", context_workloads(smoke)),
        Kind::Assignment => validate_assignment(raw, smoke),
        Kind::Scale => validate_scale(raw, smoke),
        Kind::Review => validate_matching(raw, true, smoke),
        Kind::Corroboration => validate_matching(raw, false, smoke),
        Kind::Diagnostics => validate_diagnostics(raw),
    }
}

fn copy_harness(kind: Kind, baseline: &Path, candidate: &Path) -> Result<Value, String> {
    let mut hashes = BTreeMap::new();
    for relative in kind.harness() {
        let source = candidate.join(relative);
        let destination = baseline.join(relative);
        common::checked_path(&source)?;
        common::checked_path(&destination)?;
        let bytes = common::read(&source)?;
        if destination.exists() {
            if common::read(&destination)? != bytes {
                return Err(format!("baseline contains a different harness at {relative}; use a fresh baseline checkout"));
            }
        } else {
            fs::create_dir_all(destination.parent().ok_or("harness has no parent")?)
                .map_err(|error| error.to_string())?;
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&destination)
                .map_err(|error| error.to_string())?;
            file.write_all(&bytes).map_err(|error| error.to_string())?;
        }
        hashes.insert(*relative, common::digest(&bytes));
    }
    Ok(json!(hashes))
}

fn cargo_artifact(raw: &str, name: &str, kind: &str) -> Result<PathBuf, String> {
    let mut paths = Vec::new();
    for line in raw.lines() {
        let record: Value = serde_json::from_str(line).map_err(|error| error.to_string())?;
        if record["reason"] == "compiler-artifact"
            && record["target"]["name"] == name
            && record["target"]["kind"]
                .as_array()
                .is_some_and(|kinds| kinds.iter().any(|value| value == kind))
        {
            if let Some(path) = record["executable"].as_str() {
                paths.push(PathBuf::from(path));
            }
        }
    }
    if paths.len() != 1 {
        return Err("Cargo did not report exactly one benchmark executable".into());
    }
    Ok(paths.remove(0))
}

fn canonical_checked(path: &Path) -> Result<PathBuf, String> {
    // Inspect the caller-supplied ancestors before resolving links away.
    common::checked_path(path)?;
    let canonical = path.canonicalize().map_err(|error| error.to_string())?;
    common::checked_path(&canonical)?;
    Ok(canonical)
}

fn checked_executable(path: &Path, target: &Path) -> Result<PathBuf, String> {
    let binary = canonical_checked(path)?;
    let target = canonical_checked(target)?;
    if !binary.starts_with(&target) || !binary.is_file() {
        return Err("Cargo-reported benchmark executable is outside its isolated target".into());
    }
    Ok(binary)
}

fn build(kind: Kind, options: Options) -> Result<(), String> {
    // Refuse existing output before compiler, Git or benchmark execution.
    ensure_fresh(&options.output)?;
    let candidate = canonical_checked(&options.candidate.unwrap_or_else(common::root))?;
    let mut checkouts = Vec::new();
    let mut copied = Value::Null;
    if let Some(baseline) = options.baseline {
        let baseline = canonical_checked(&baseline)?;
        if baseline == candidate {
            return Err("baseline and candidate must be separate checkouts".into());
        }
        copied = copy_harness(kind, &baseline, &candidate)?;
        checkouts.push(("baseline", baseline));
    }
    checkouts.push(("candidate", candidate));
    let mut meta = json!({"protocol":kind.protocol(),"specification":specification(kind),"compiler":compiler()?,
        "build_environment":environment(),"machine":machine(),"runner":runner_record()?,"runs":RUNS,
        "built_utc":now(),"copied_harness_sha256":copied,"cli":std::env::args().collect::<Vec<_>>(),"checkouts":{}});
    for (revision, root) in checkouts {
        let before = inventory(&root, kind)?;
        let configs = cargo_configs(&root)?;
        let nonce = &common::digest(options.output.to_string_lossy().as_bytes())[..16];
        let target = root
            .join("target")
            .join("native-performance")
            .join(kind.protocol())
            .join(nonce);
        common::checked_path(&target)?;
        if target.exists() {
            return Err(
                "isolated benchmark target already exists; choose a fresh output directory".into(),
            );
        }
        fs::create_dir_all(&target).map_err(|error| error.to_string())?;
        let modes: &[&str] = if matches!(kind, Kind::Run | Kind::Context) {
            &["timing", "allocations"]
        } else {
            &["timing"]
        };
        let mut binaries = BTreeMap::new();
        for mode in modes {
            let (target_kind, name) = kind.target();
            let mut args: Vec<String> = if kind == Kind::Corroboration {
                vec![
                    "+stable", "bench", "--locked", "-p", "fieldkin", "--bench", name, "--no-run",
                ]
            } else {
                vec!["+stable", "build", "--release", "--locked"]
            }
            .into_iter()
            .map(str::to_owned)
            .collect();
            if matches!(kind, Kind::Run | Kind::Context) {
                args.extend([
                    "--manifest-path".into(),
                    root.join("performance/Cargo.toml")
                        .to_string_lossy()
                        .into_owned(),
                ]);
            } else if kind != Kind::Corroboration {
                args.extend([
                    "-p".into(),
                    "fieldkin".into(),
                    "--example".into(),
                    name.into(),
                ]);
            }
            args.extend([
                "--target-dir".into(),
                target.join(mode).to_string_lossy().into_owned(),
                "--message-format=json".into(),
            ]);
            let mut features = Vec::new();
            if *mode == "allocations" {
                features.push("allocations");
            }
            if kind == Kind::Context && revision == "candidate" {
                features.push("quality-policy");
            }
            if !features.is_empty() {
                args.extend(["--features".into(), features.join(",")]);
            }
            println!("Building {revision}/{mode}; measurement will run separately");
            let raw = capture("cargo", &args, &root)?;
            let binary = checked_executable(&cargo_artifact(&raw, name, target_kind)?, &target)?;
            if before != inventory(&root, kind)? || configs != cargo_configs(&root)? {
                return Err("build inputs or Cargo configuration changed during build".into());
            }
            let smoke_supported = matches!(
                kind,
                Kind::Run | Kind::Context | Kind::Assignment | Kind::Review
            );
            let smoke_behavior = if smoke_supported {
                let mut smoke_args = vec!["--smoke".into()];
                if kind == Kind::Context {
                    smoke_args.push("--contextual".into());
                }
                let raw = capture(&binary.to_string_lossy(), &smoke_args, &root)?;
                json!(validate(kind, &raw, mode, true)?.behavior)
            } else {
                Value::Null
            };
            binaries.insert(*mode,json!({"path":binary,"sha256":file_digest(&binary,false)?,"build_command":([vec!["cargo".to_owned()],args].concat()),"smoke_behavior":smoke_behavior}));
        }
        meta["checkouts"][revision] = json!({"path":root,"commit":capture("git",&["rev-parse".into(),"HEAD".into()],&root)?,
            "git_status":capture("git",&["status".into(),"--short".into()],&root)?,"source_sha256":before,
            "cargo_config_sha256":configs,"binaries":binaries});
    }
    if kind.paired() {
        for mode in ["timing", "allocations"] {
            let baseline = &meta["checkouts"]["baseline"]["binaries"][mode]["smoke_behavior"];
            let candidate = &meta["checkouts"]["candidate"]["binaries"][mode]["smoke_behavior"];
            if baseline != candidate {
                return Err("baseline and candidate smoke workload behavior differs".into());
            }
        }
    }
    verify(kind, &meta)?;
    common::write_new(&options.output.join("build.json"), &meta)?;
    println!(
        "Build and checks recorded. Stop concurrent builds/tests before the measurement phase."
    );
    Ok(())
}

fn verify(kind: Kind, meta: &Value) -> Result<(), String> {
    if meta["protocol"] != kind.protocol()
        || meta["runs"] != RUNS
        || meta["specification"] != specification(kind)
    {
        return Err("native performance protocol changed; historical records require their recorded toolchain and runner".into());
    }
    if meta["compiler"] != compiler()? || meta["build_environment"] != environment() {
        return Err("compiler or build environment changed after build".into());
    }
    if meta["runner"] != runner_record()? {
        return Err("executing native runner or its sources changed after build".into());
    }
    let checkouts = meta["checkouts"]
        .as_object()
        .ok_or("missing recorded checkouts")?;
    let expected: BTreeSet<_> = if kind.paired() {
        BTreeSet::from(["baseline", "candidate"])
    } else {
        BTreeSet::from(["candidate"])
    };
    if checkouts
        .keys()
        .map(String::as_str)
        .collect::<BTreeSet<_>>()
        != expected
    {
        return Err("unexpected performance revision inventory".into());
    }
    for entry in checkouts.values() {
        let root = PathBuf::from(entry["path"].as_str().ok_or("missing checkout path")?);
        common::checked_path(&root)?;
        if entry["source_sha256"] != inventory(&root, kind)?
            || entry["cargo_config_sha256"] != cargo_configs(&root)?
        {
            return Err(
                "benchmark source inventory or Cargo configuration changed after build".into(),
            );
        }
        let binaries = entry["binaries"]
            .as_object()
            .ok_or("missing benchmark binaries")?;
        let modes: BTreeSet<_> = if matches!(kind, Kind::Run | Kind::Context) {
            BTreeSet::from(["timing", "allocations"])
        } else {
            BTreeSet::from(["timing"])
        };
        if binaries.keys().map(String::as_str).collect::<BTreeSet<_>>() != modes {
            return Err("unexpected timing/instrumentation binary inventory".into());
        }
        for binary in binaries.values() {
            if binary["sha256"]
                != file_digest(
                    Path::new(binary["path"].as_str().ok_or("missing binary path")?),
                    false,
                )?
            {
                return Err("benchmark executable changed after build".into());
            }
        }
    }
    Ok(())
}

fn normalized_smoke_behavior(rows: &[Value]) -> Value {
    json!(rows
        .iter()
        .map(|row| {
            let mut row = row.clone();
            if let Some(iterations) = row.get_mut("iterations") {
                *iterations = if iterations.is_string() {
                    json!("1")
                } else {
                    json!(1)
                };
            }
            row
        })
        .collect::<Vec<_>>())
}

fn measure(kind: Kind, output: &Path) -> Result<(), String> {
    only_build(output)?;
    let meta = common::json(&output.join("build.json"))?;
    verify(kind, &meta)?;
    let started = now();
    let mut execution = Vec::new();
    let mut reference: BTreeMap<String, Vec<Value>> = BTreeMap::new();
    let mut all_rows = Vec::new();
    let modes: &[&str] = if matches!(kind, Kind::Run | Kind::Context) {
        &["timing", "allocations"]
    } else {
        &["timing"]
    };
    for mode in modes {
        for run in 1..=RUNS {
            let revisions: &[&str] = if !kind.paired() {
                &["candidate"]
            } else if run % 2 == 1 {
                &["baseline", "candidate"]
            } else {
                &["candidate", "baseline"]
            };
            for revision in revisions {
                let entry = &meta["checkouts"][revision];
                let binary = &entry["binaries"][mode];
                let path = text(binary, "path")?;
                let root = Path::new(text(entry, "path")?);
                let mut args = if *mode == "allocations" {
                    vec!["--smoke".into()]
                } else {
                    Vec::new()
                };
                if kind == Kind::Context {
                    args.push("--contextual".into());
                }
                println!("Measuring {mode} process {run}/{RUNS}, {revision}");
                let stamp = now();
                let raw = capture(path, &args, root)?;
                let measured = validate(kind, &raw, mode, *mode == "allocations")?;
                if !binary["smoke_behavior"].is_null()
                    && normalized_smoke_behavior(&measured.behavior) != binary["smoke_behavior"]
                {
                    return Err("workload behavior changed since build smoke check".into());
                }
                if let Some(reference) = reference.get(*mode) {
                    if reference != &measured.behavior {
                        return Err(
                            "workload behavior changed across revisions or processes".into()
                        );
                    }
                } else {
                    reference.insert((*mode).into(), measured.behavior.clone());
                }
                verify(kind, &meta)?;
                let extension = if matches!(kind, Kind::Run | Kind::Context | Kind::Scale) {
                    "jsonl"
                } else {
                    "csv"
                };
                let filename = if matches!(kind, Kind::Run | Kind::Context) {
                    format!("{mode}-{revision}-run-{run}.{extension}")
                } else if kind.paired() {
                    format!("{revision}-run-{run}.{extension}")
                } else {
                    format!("run-{run}.{extension}")
                };
                let artifact = output.join(&filename);
                write_text_new(&artifact, &measured.text)?;
                execution.push(json!({"run":run,"revision":revision,"mode":mode,"command":([vec![path.to_owned()],args].concat()),"started_utc":stamp,"file":filename,"sha256":file_digest(&artifact,false)?}));
                for row in measured.rows {
                    all_rows.push(((*mode).to_owned(), (*revision).to_owned(), row));
                }
            }
        }
    }
    verify(kind, &meta)?;
    common::write_new(
        &output.join("run.json"),
        &json!({"protocol":kind.protocol(),"started_utc":started,"finished_utc":now(),"machine":machine(),"execution":execution}),
    )?;
    if matches!(kind, Kind::Run | Kind::Context) {
        return summarize_stage(kind, output);
    }
    let rows = summarize_rows(kind, &all_rows)?;
    common::write_new(
        &output.join("summary.json"),
        &json!({"protocol":kind.protocol(),"execution":execution,"rows":rows,"behavior":reference.get("timing"),"machine":machine(),"cli":std::env::args().collect::<Vec<_>>(),"interpretation":"Five independent process means, fixed workload order, alternating revisions for paired comparisons. No CPU affinity, confidence interval, semantic accuracy, worst-case latency or peak-memory claim. Matching/report timing scope is recorded in build.json; local hashes detect drift, not a trusted signature or hermetic build."}),
    )?;
    Ok(())
}

fn median(values: &[f64]) -> Result<f64, String> {
    if values.is_empty() || values.iter().any(|value| !value.is_finite()) {
        return Err("invalid or empty process measurements".into());
    }
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    Ok(if sorted.len() % 2 == 1 {
        sorted[sorted.len() / 2]
    } else {
        (sorted[sorted.len() / 2 - 1] + sorted[sorted.len() / 2]) / 2.0
    })
}

fn statistics(values: &[f64], unit: &str) -> Result<Value, String> {
    let middle = median(values)?;
    let deviations: Vec<_> = values.iter().map(|value| (value - middle).abs()).collect();
    Ok(
        json!({format!("process_means_{unit}"):values,format!("median_{unit}"):middle,
        format!("min_{unit}"):values.iter().copied().min_by(f64::total_cmp),
        format!("max_{unit}"):values.iter().copied().max_by(f64::total_cmp),
        format!("median_absolute_deviation_{unit}"):median(&deviations)?}),
    )
}

fn summarize_rows(kind: Kind, all_rows: &[(String, String, Value)]) -> Result<Vec<Value>, String> {
    let mut grouped: BTreeMap<String, (Value, BTreeMap<String, Vec<f64>>)> = BTreeMap::new();
    for (_, revision, row) in all_rows {
        let identity = match kind {
            Kind::Assignment => json!({"size":integer(row,"size")?,"family":text(row,"family")?}),
            Kind::Scale => {
                json!({"size":json_integer(row,"size")?,"family":text(row,"family")?,"one_to_one":row["one_to_one"]})
            }
            Kind::Review | Kind::Corroboration => {
                json!({if kind==Kind::Review{"policy"}else{"model"}:text(row,if kind==Kind::Review{"policy"}else{"engine"})?,"fields_per_side":integer(row,"fields_per_side")?,"samples":text(row,"samples")?=="true","one_to_one":text(row,"one_to_one")?=="true"})
            }
            Kind::Diagnostics => {
                json!({"source_count":integer(row,"source_count")?,"target_count":integer(row,"target_count")?,"mode":text(row,"mode")?})
            }
            Kind::Run | Kind::Context => {
                return Err("stage summaries use the allocation-aware protocol".into())
            }
        };
        let value = match kind {
            Kind::Scale => {
                json_integer(row, "elapsed_ns")? as f64
                    / json_integer(row, "iterations")? as f64
                    / 1_000_000.0
            }
            Kind::Diagnostics => integer(row, "elapsed_ns")? as f64,
            _ => duration(row, "us_per_match")?,
        };
        grouped
            .entry(identity.to_string())
            .or_insert_with(|| (identity, BTreeMap::new()))
            .1
            .entry(revision.clone())
            .or_default()
            .push(value);
    }
    let mut results = Vec::new();
    for (identity, mut revisions) in grouped.into_values() {
        let mut result = identity;
        if kind == Kind::Diagnostics {
            for values in revisions.values_mut() {
                if values.len() != RUNS * 3 {
                    return Err("incomplete diagnostic process measurements".into());
                }
                *values = values
                    .as_chunks::<3>()
                    .0
                    .iter()
                    .map(|calls| calls.iter().sum::<f64>() / 3.0)
                    .collect();
            }
        }
        for values in revisions.values() {
            if values.len() != RUNS {
                return Err("incomplete independent process runs".into());
            }
        }
        if kind == Kind::Assignment {
            if revisions.len() != 2 {
                return Err("missing paired process results".into());
            }
            for (revision, values) in &revisions {
                result[revision] = statistics(values, "us")?;
            }
            let before = result["baseline"]["median_us"].as_f64().unwrap();
            let after = result["candidate"]["median_us"].as_f64().unwrap();
            result["median_change_percent"] = if before > 0.0 {
                json!(100.0 * (after / before - 1.0))
            } else {
                Value::Null
            };
        } else {
            let values = revisions
                .get("candidate")
                .ok_or("missing native candidate process means")?;
            let stats = statistics(
                values,
                if kind == Kind::Scale {
                    "ms"
                } else if kind == Kind::Diagnostics {
                    "ns"
                } else {
                    "us"
                },
            )?;
            result
                .as_object_mut()
                .unwrap()
                .extend(stats.as_object().unwrap().clone());
        }
        results.push(result);
    }
    Ok(results)
}

fn summarize_stage(kind: Kind, output: &Path) -> Result<(), String> {
    // Summaries also use exclusive files; rerunning never replaces a published result.
    if output.join("summary.json").exists() || output.join("summary.md").exists() {
        return Err("summary already exists; existing measurements are retained".into());
    }
    let meta = common::json(&output.join("build.json"))?;
    verify(kind, &meta)?;
    let run = common::json(&output.join("run.json"))?;
    if run["protocol"] != kind.protocol() {
        return Err("unexpected native execution protocol".into());
    }
    let execution = run["execution"]
        .as_array()
        .ok_or("missing execution manifest")?;
    if execution.len() != RUNS * 4 {
        return Err("incomplete stage execution manifest".into());
    }
    let mut grouped: BTreeMap<String, BTreeMap<(String, String), Vec<Value>>> = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for entry in execution {
        let mode = text(entry, "mode")?;
        let revision = text(entry, "revision")?;
        let process = json_integer(entry, "run")?;
        if !["timing", "allocations"].contains(&mode)
            || !["baseline", "candidate"].contains(&revision)
            || !(1..=RUNS as u64).contains(&process)
        {
            return Err("unexpected stage execution identity".into());
        }
        let filename = format!("{mode}-{revision}-run-{process}.jsonl");
        if entry["file"] != filename || !seen.insert(filename.clone()) {
            return Err("duplicate or malformed stage execution manifest".into());
        }
        let path = output.join(filename);
        if entry["sha256"] != file_digest(&path, false)? {
            return Err("measurement artifact differs from its execution hash".into());
        }
        let raw =
            String::from_utf8(common::read(&path)?).map_err(|_| "non-UTF8 measurement artifact")?;
        for row in validate(kind, &raw, mode, mode == "allocations")?.rows {
            grouped
                .entry(text(&row, "workload")?.to_owned())
                .or_default()
                .entry((mode.to_owned(), revision.to_owned()))
                .or_default()
                .push(row);
        }
    }
    let mut summary = Vec::new();
    let mut markdown=vec!["# Native repeated cost measurements".to_owned(),String::new(),
        "Five independent processes per revision and mode. Timing uses the normal allocator; separate instrumented processes measure allocation requests. Spread is min–max of process means, without confidence intervals or peak-memory claims.".to_owned(),String::new(),
        "| Workload | Baseline median µs [min–max] | Candidate median µs [min–max] | Speedup | Allocations baseline → candidate | Allocated bytes baseline → candidate |".to_owned(),
        "| --- | ---: | ---: | ---: | ---: | ---: |".to_owned()];
    if grouped.len() != if kind == Kind::Context { 24 } else { 50 } {
        return Err("incomplete stage workload summaries".into());
    }
    for (workload, measurements) in grouped {
        let mut result = json!({"workload":workload});
        for revision in ["baseline", "candidate"] {
            let timings = measurements
                .get(&("timing".into(), revision.into()))
                .ok_or("missing timing runs")?;
            let allocations = measurements
                .get(&("allocations".into(), revision.into()))
                .ok_or("missing allocation runs")?;
            if timings.len() != RUNS || allocations.len() != RUNS {
                return Err("incomplete stage process runs".into());
            }
            let values: Vec<_> = timings
                .iter()
                .map(|row| {
                    Ok(json_integer(row, "elapsed_ns")? as f64
                        / json_integer(row, "iterations")? as f64
                        / 1000.0)
                })
                .collect::<Result<_, String>>()?;
            let mut stats = statistics(&values, "us")?;
            for key in ALLOCATION_KEYS {
                let values: Vec<_> = allocations
                    .iter()
                    .map(|row| {
                        Ok(row[key].as_f64().ok_or("invalid allocation count")?
                            / json_integer(row, "iterations")? as f64)
                    })
                    .collect::<Result<_, String>>()?;
                stats[format!("{key}_per_call")] = json!(median(&values)?);
                stats[format!("{key}_range")] = json!([
                    values.iter().copied().min_by(f64::total_cmp),
                    values.iter().copied().max_by(f64::total_cmp)
                ]);
            }
            result[revision] = stats;
        }
        let before = result["baseline"]["median_us"].as_f64().unwrap();
        let after = result["candidate"]["median_us"].as_f64().unwrap();
        result["median_speedup"] = if after > 0.0 {
            json!(before / after)
        } else {
            Value::Null
        };
        result["median_latency_change_percent"] = if before > 0.0 {
            json!(100.0 * (after / before - 1.0))
        } else {
            Value::Null
        };
        let b = &result["baseline"];
        let a = &result["candidate"];
        markdown.push(format!("| {workload} | {:.2} [{:.2}–{:.2}] | {:.2} [{:.2}–{:.2}] | {} | {:.0} → {:.0} | {:.0} → {:.0} |",
            before,b["min_us"].as_f64().unwrap(),b["max_us"].as_f64().unwrap(),after,a["min_us"].as_f64().unwrap(),a["max_us"].as_f64().unwrap(),
            result["median_speedup"].as_f64().map_or_else(||"undefined".into(),|speedup|format!("{speedup:.2}x")),b["allocations_per_call"].as_f64().unwrap(),a["allocations_per_call"].as_f64().unwrap(),b["bytes_allocated_per_call"].as_f64().unwrap(),a["bytes_allocated_per_call"].as_f64().unwrap()));
        summary.push(result);
    }
    common::write_new(&output.join("summary.json"), &json!(summary))?;
    write_text_new(&output.join("summary.md"), &(markdown.join("\n") + "\n"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Temporary(PathBuf, PathBuf);
    impl Temporary {
        fn new() -> Self {
            Self::new_in(&std::env::temp_dir())
        }
        fn new_in(root: &Path) -> Self {
            let root = common::canonical_test_temp_root(root);
            let path = root.join(format!(
                "fieldkin-native-performance-{}-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_nanos(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&path).unwrap();
            Self(path, root)
        }
        fn put(&self, name: &str, bytes: &[u8]) {
            let path = self.0.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, bytes).unwrap();
        }
    }
    impl Drop for Temporary {
        fn drop(&mut self) {
            if self.0.parent() == Some(self.1.as_path())
                && self.0.file_name().is_some_and(|name| {
                    name.to_string_lossy()
                        .starts_with("fieldkin-native-performance-")
                })
            {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
    }

    fn assignment_fixture(smoke: bool) -> String {
        let mut writer = csv::Writer::from_writer(Vec::new());
        writer.write_record(ASSIGNMENT_HEADER).unwrap();
        for size in SIZES {
            for family in ASSIGNMENT_FAMILIES {
                let expected = assignment_counts(size, family, smoke);
                let calls = expected["iterations"].as_u64().unwrap();
                let row: Vec<String> = ASSIGNMENT_HEADER
                    .iter()
                    .map(|key| match *key {
                        "family" => family.to_owned(),
                        "size" => size.to_string(),
                        "total_ms" => "1.0".into(),
                        "us_per_match" => (1000.0 / calls as f64).to_string(),
                        "alternatives" => "0".into(),
                        "objective" => {
                            if expected["status"] == "Disabled" {
                                "none".into()
                            } else {
                                (0.85 * expected["proposed"].as_f64().unwrap()).to_string()
                            }
                        }
                        "status" => expected["status"].as_str().unwrap().into(),
                        key => expected[key].to_string(),
                    })
                    .collect();
                writer.write_record(row).unwrap();
            }
        }
        ASSIGNMENT_PREAMBLE.join("\n")
            + "\n"
            + &String::from_utf8(writer.into_inner().unwrap()).unwrap()
    }

    fn scale_fixture(small: bool) -> Vec<Value> {
        let mut rows = Vec::new();
        for size in if small {
            vec![128]
        } else {
            SCALE_SIZES.to_vec()
        } {
            for family in SCALE_FAMILIES {
                for policy in [false, true] {
                    let unmatched = if family == "partial" { size / 8 } else { 0 };
                    rows.push(json!({"family":family,"size":size,"one_to_one":policy,"iterations":if size==128{3}else{1},"elapsed_ns":123456,"selected":size-unmatched,"unmatched":unmatched,"pairs":size*size,"signal_evaluations":size*size*if family=="builtin"{3}else{1},"diagnostic_solves":0,"diagnostic_work":0}));
                }
            }
        }
        rows
    }
    fn encode(rows: &[Value]) -> String {
        rows.iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn contextual_protocol_requires_every_assignment_sample_and_budget_workload() {
        for smoke in [false, true] {
            let rows: Vec<_> = context_workloads(smoke).into_iter().map(|(id,n,m,calls,error)|
                json!({"workload":id,"sources":n,"targets":m,"iterations":calls,"expects_error":error,"elapsed_ns":1000})).collect();
            assert_eq!(
                validate(Kind::Context, &encode(&rows), "timing", smoke)
                    .unwrap()
                    .rows
                    .len(),
                24
            );
            let mut reversed = rows.clone();
            reversed.reverse();
            let mut duplicate = rows.clone();
            duplicate.push(rows[0].clone());
            for invalid in [rows[..23].to_vec(), reversed, duplicate] {
                assert!(validate(Kind::Context, &encode(&invalid), "timing", smoke).is_err());
            }
            assert!(validate(Kind::Run, &encode(&rows), "timing", smoke).is_err());
        }
    }

    #[test]
    fn assignment_fixed_workload_inventory_and_smoke() {
        for smoke in [false, true] {
            let rows = validate_assignment(&assignment_fixture(smoke), smoke).unwrap();
            assert_eq!(rows.rows.len(), 27);
            assert_eq!(rows.behavior.len(), 27);
        }
        assert!(validate_assignment(&assignment_fixture(true), false).is_err());
    }

    #[test]
    fn assignment_rejects_missing_duplicate_reordered_and_malformed_workloads() {
        let raw = assignment_fixture(false);
        let lines: Vec<_> = raw.lines().collect();
        let mut duplicate = lines.clone();
        duplicate.push(lines.last().unwrap());
        let mut reversed = lines[..3].to_vec();
        reversed.extend(lines[3..].iter().rev());
        let mut preamble = lines.clone();
        preamble[0] = "wrong preamble";
        for changed in [
            lines[..lines.len() - 1].to_vec(),
            duplicate,
            reversed,
            preamble,
        ] {
            assert!(validate_assignment(&changed.join("\n"), false).is_err());
        }
        let malformed = raw.replacen(lines[3], &(lines[3].to_owned() + ",unexpected"), 1);
        assert!(validate_assignment(&malformed, false).is_err());
    }

    #[test]
    fn assignment_rejects_wrong_decisions_status_work_objective_and_duration() {
        let raw = assignment_fixture(false);
        let original: Vec<_> = raw.lines().map(str::to_owned).collect();
        for (key, value) in [
            ("proposed", "0"),
            ("solves", "1"),
            ("work", "128"),
            ("status", "Complete"),
            ("objective", "nan"),
            ("alternatives", "3"),
            ("us_per_match", "inf"),
            ("total_ms", "-1"),
            ("us_per_match", "100000"),
        ] {
            let mut lines = original.clone();
            let mut row: Vec<_> = lines[4].split(',').map(str::to_owned).collect();
            row[ASSIGNMENT_HEADER
                .iter()
                .position(|column| *column == key)
                .unwrap()] = value.into();
            lines[4] = row.join(",");
            assert!(
                validate_assignment(&lines.join("\n"), false).is_err(),
                "{key}"
            );
        }
    }

    #[test]
    fn recursive_inventory_detects_data_changes_additions_and_optional_build_inputs() {
        let temporary = Temporary::new();
        for name in [
            "Cargo.toml",
            "Cargo.lock",
            "README.md",
            "evaluation/Cargo.toml",
            "examples/assignment_cost.rs",
            "src/lib.rs",
            "src/private/data.txt",
        ] {
            temporary.put(name, b"original");
        }
        let before = inventory(&temporary.0, Kind::Assignment).unwrap();
        temporary.put("src/private/data.txt", b"changed");
        assert_ne!(before, inventory(&temporary.0, Kind::Assignment).unwrap());
        temporary.put("src/private/data.txt", b"original");
        assert_eq!(before, inventory(&temporary.0, Kind::Assignment).unwrap());
        temporary.put("src/private/new.txt", b"new");
        assert_ne!(before, inventory(&temporary.0, Kind::Assignment).unwrap());
        fs::remove_file(temporary.0.join("src/private/new.txt")).unwrap();
        temporary.put("build.rs", b"fn main() {}");
        assert_ne!(before, inventory(&temporary.0, Kind::Assignment).unwrap());
    }

    #[test]
    fn existing_outputs_are_refused_before_any_compiler_or_benchmark_command() {
        for kind in [
            Kind::Run,
            Kind::Assignment,
            Kind::Scale,
            Kind::Review,
            Kind::Diagnostics,
            Kind::Corroboration,
        ] {
            let temporary = Temporary::new();
            temporary.put("build.json", b"{}");
            temporary.put("run-1.csv", b"retained");
            assert!(measure(kind, &temporary.0).is_err());
            assert_eq!(
                fs::read(temporary.0.join("run-1.csv")).unwrap(),
                b"retained"
            );
            assert!(ensure_fresh(&temporary.0).is_err());
            assert!(write_text_new(&temporary.0.join("run-1.csv"), "replace").is_err());
        }
    }

    #[test]
    fn scale_exact_full_and_small_inventory() {
        assert_eq!(
            validate_scale(&encode(&scale_fixture(false)), false)
                .unwrap()
                .rows
                .len(),
            18
        );
        assert_eq!(
            validate_scale(&encode(&scale_fixture(true)), true)
                .unwrap()
                .rows
                .len(),
            6
        );
        assert!(validate_scale(&encode(&scale_fixture(true)), false).is_err());
    }

    #[test]
    fn scale_rejects_missing_duplicate_reordered_rows() {
        let rows = scale_fixture(false);
        let mut duplicate = rows.clone();
        duplicate.push(rows.last().unwrap().clone());
        let mut reversed = rows.clone();
        reversed.reverse();
        for changed in [rows[..rows.len() - 1].to_vec(), duplicate, reversed] {
            assert!(validate_scale(&encode(&changed), false).is_err());
        }
    }

    #[test]
    fn scale_rejects_wrong_decisions_budgets_types_duration_and_extra_fields() {
        for (key, value) in [
            ("selected", json!(0)),
            ("unmatched", json!(1)),
            ("pairs", json!(10)),
            ("signal_evaluations", json!(100)),
            ("diagnostic_solves", json!(1)),
            ("diagnostic_work", json!(1)),
            ("one_to_one", json!(1)),
            ("elapsed_ns", json!(-1)),
            ("elapsed_ns", json!(true)),
            ("size", json!(1024)),
            ("sample_values", json!([])),
        ] {
            let mut rows = scale_fixture(false);
            rows[0][key] = value;
            assert!(validate_scale(&encode(&rows), false).is_err(), "{key}");
        }
    }

    #[test]
    fn archived_measurement_shapes_validate_without_rewriting_or_retiming() {
        let root = common::root();
        for (kind, path, preamble, mode, smoke) in [
            (
                Kind::Run,
                "performance/results/rc-v1/timing-candidate-run-1.jsonl",
                &[][..],
                "timing",
                false,
            ),
            (
                Kind::Run,
                "performance/results/rc-v1/allocations-candidate-run-1.jsonl",
                &[][..],
                "allocations",
                true,
            ),
            (
                Kind::Assignment,
                "performance/results/assignment-v1/solver-cost/candidate-run-1.csv",
                &ASSIGNMENT_PREAMBLE[..],
                "timing",
                false,
            ),
            (
                Kind::Review,
                "performance/results/review-v1/review-cost/run-1.csv",
                &REVIEW_PREAMBLE[..],
                "timing",
                false,
            ),
            (
                Kind::Diagnostics,
                "performance/results/diagnostics-v1/run-1.csv",
                &[][..],
                "timing",
                false,
            ),
            (
                Kind::Corroboration,
                "performance/results/corrective-v1/gate-cost/run-1.csv",
                &MATCHING_PREAMBLE[..],
                "timing",
                false,
            ),
        ] {
            let bytes = common::read(&root.join(path)).unwrap();
            let before = common::digest(&bytes);
            let raw = String::from_utf8(bytes).unwrap();
            let raw = if preamble.is_empty() {
                raw
            } else {
                preamble.join("\n") + "\n" + &raw
            };
            validate(kind, &raw, mode, smoke).unwrap_or_else(|error| panic!("{path}: {error}"));
            assert_eq!(before, file_digest(&root.join(path), false).unwrap());
        }
    }

    #[test]
    fn native_options_and_protocol_fail_closed() {
        assert!(parse_options(
            Kind::Assignment,
            &["build".into(), "--output".into(), "target/example".into()]
        )
        .is_err());
        assert!(parse_options(
            Kind::Scale,
            &[
                "run".into(),
                "--output".into(),
                "target/example".into(),
                "--baseline".into(),
                ".".into()
            ]
        )
        .is_err());
        assert!(parse_options(
            Kind::Run,
            &[
                "build".into(),
                "--output".into(),
                "a".into(),
                "--output".into(),
                "b".into()
            ]
        )
        .is_err());
        assert!(verify(
            Kind::Assignment,
            &json!({"protocol":"assignment-compaction-v1"})
        )
        .is_err());
        assert_ne!(Kind::Assignment.protocol(), "assignment-compaction-v1");
    }

    #[test]
    fn compiler_identity_rejects_environment_and_cargo_override_routes() {
        let empty = toml::Value::Table(Default::default());
        for key in [
            "RUSTC",
            "RUSTDOC",
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
            "CARGO_BUILD_TARGET",
            "CARGO_BUILD_RUSTC",
            "CARGO_BUILD_RUSTDOC",
            "CARGO_BUILD_RUSTC_WRAPPER",
            "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
            "rustc",
        ] {
            assert!(
                reject_compiler_overrides(
                    &BTreeMap::from([(key.to_owned(), "override".to_owned())]),
                    &empty
                )
                .is_err(),
                "{key}"
            );
            let config: toml::Value =
                toml::from_str(&format!("[env]\n{key}=\"override\"\n")).unwrap();
            assert!(
                reject_compiler_overrides(&BTreeMap::new(), &config).is_err(),
                "{key}"
            );
        }
        for source in [
            "[build]\nrustc=\"override\"",
            "[build]\nrustdoc=\"override\"",
            "[build]\nrustc-wrapper=\"override\"",
            "[build]\nrustc-workspace-wrapper=\"override\"",
            "[build]\ntarget=\"other\"",
            "include=\"external.toml\"",
            "[source.registry]\nreplace-with=\"other\"",
            "[target.host]\nlinker=\"other\"",
        ] {
            let config: toml::Value = toml::from_str(source).unwrap();
            assert!(
                reject_compiler_overrides(&BTreeMap::new(), &config).is_err(),
                "{source}"
            );
        }
        assert!(reject_compiler_overrides(
            &BTreeMap::from([("RUSTFLAGS".into(), "-C debuginfo=1".into())]),
            &toml::from_str("[build]\nrustflags=[\"-C\",\"debuginfo=1\"]").unwrap()
        )
        .is_ok());
    }

    #[test]
    fn cargo_executable_must_be_a_checked_file_within_the_fresh_target() {
        let temporary = Temporary::new();
        temporary.put("target/release/benchmark", b"binary");
        temporary.put("outside", b"other");
        let target = temporary.0.join("target");
        assert!(checked_executable(&temporary.0.join("target/release/benchmark"), &target).is_ok());
        assert!(checked_executable(&temporary.0.join("outside"), &target).is_err());
        assert!(checked_executable(&target, &target).is_err());
        assert!(cargo_artifact("{\"reason\":\"compiler-artifact\",\"target\":{\"name\":\"cost\",\"kind\":[\"example\"]},\"executable\":\"one\"}\n{\"reason\":\"compiler-artifact\",\"target\":{\"name\":\"cost\",\"kind\":[\"example\"]},\"executable\":\"two\"}","cost","example").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn trusted_symlinked_temp_root_supports_fixtures_and_cleanup() {
        let temporary = Temporary::new();
        temporary.put("private/var/cache/benchmark", b"binary");
        std::os::unix::fs::symlink(temporary.0.join("private/var"), temporary.0.join("var"))
            .unwrap();
        let raw_root = temporary.0.join("var/cache");
        assert!(checked_executable(&raw_root.join("benchmark"), &raw_root).is_err());

        let fixture = Temporary::new_in(&raw_root);
        assert_eq!(fixture.1, temporary.0.join("private/var/cache"));
        fixture.put("target/benchmark", b"binary");
        assert!(checked_executable(
            &fixture.0.join("target/benchmark"),
            &fixture.0.join("target")
        )
        .is_ok());
        let fixture_path = fixture.0.clone();
        drop(fixture);
        assert!(!fixture_path.exists());
        assert!(raw_root.join("benchmark").exists());
    }

    #[cfg(unix)]
    #[test]
    fn raw_symlink_paths_are_rejected_before_canonicalization() {
        let temporary = Temporary::new();
        temporary.put("real/benchmark", b"binary");
        std::os::unix::fs::symlink(temporary.0.join("real"), temporary.0.join("link")).unwrap();
        assert!(canonical_checked(&temporary.0.join("link")).is_err());
        assert!(checked_executable(
            &temporary.0.join("link/benchmark"),
            &temporary.0.join("real")
        )
        .is_err());
    }

    #[test]
    fn statistics_use_process_means_and_median_not_pooled_call_timings() {
        let stats = statistics(&[1.0, 2.0, 30.0, 4.0, 5.0], "us").unwrap();
        assert_eq!(stats["median_us"], 4.0);
        assert_eq!(stats["min_us"], 1.0);
        assert_eq!(stats["max_us"], 30.0);
        assert_eq!(stats["median_absolute_deviation_us"], 2.0);
        assert!(statistics(&[], "us").is_err());
        assert!(statistics(&[f64::NAN], "us").is_err());
    }
}
