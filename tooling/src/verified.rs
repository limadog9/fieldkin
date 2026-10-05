//! Fresh native builds with before/after source, environment and binary records.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::common::{
    checked_path, command, digest, executable, json as load_json, read, root, write_new,
};

const PROTOCOL: &str = "fieldkin-native-verified-v1";
const TRUST: &str = "Local reproducibility record, not a signature or hermetic build. Trust local processes, the wrapper, compiler, Cargo and locked cache. Before/after guards cannot detect a change restored between checks.";

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn all_files(
    base: &Path,
    directory: &Path,
    result: &mut BTreeMap<String, String>,
) -> Result<(), String> {
    checked_path(directory)?;
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        checked_path(&path)?;
        if path.is_dir() {
            all_files(base, &path, result)?;
        } else {
            result.insert(
                path.strip_prefix(base)
                    .map_err(|error| error.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/"),
                digest(&read(&path)?),
            );
        }
    }
    Ok(())
}

fn manifests(base: &Path) -> Result<(), String> {
    for name in ["Cargo.toml", "evaluation/Cargo.toml", "tooling/Cargo.toml"] {
        let text = String::from_utf8(read(&base.join(name))?).map_err(|error| error.to_string())?;
        let value: toml::Value = toml::from_str(&text).map_err(|error| error.to_string())?;
        if value.get("patch").is_some() || value.get("replace").is_some() {
            return Err("dependency overrides are unsupported".into());
        }
        if let Some(script) = value
            .get("package")
            .and_then(|package| package.get("build"))
        {
            if script.as_str() != Some("build.rs") && script.as_bool() != Some(false) {
                return Err("unscoped build script is unsupported".into());
            }
        }
        if name == "Cargo.toml" {
            let members = value
                .get("workspace")
                .and_then(|workspace| workspace.get("members"))
                .and_then(toml::Value::as_array)
                .ok_or("workspace members missing")?;
            if members
                .iter()
                .filter_map(toml::Value::as_str)
                .collect::<Vec<_>>()
                != ["evaluation", "tooling"]
            {
                return Err("verified builds require the scoped native workspace".into());
            }
        }
        fn visit(value: &toml::Value, name: &str) -> Result<(), String> {
            if let Some(table) = value.as_table() {
                for (key, child) in table {
                    if ["dependencies", "dev-dependencies", "build-dependencies"]
                        .contains(&key.as_str())
                    {
                        for (dependency, spec) in
                            child.as_table().ok_or("dependency table invalid")?
                        {
                            if let Some(path) = spec.get("path") {
                                if !(name == "evaluation/Cargo.toml"
                                    && dependency == "fieldkin"
                                    && path.as_str() == Some(".."))
                                {
                                    return Err("unscoped path dependency is unsupported".into());
                                }
                            }
                        }
                    }
                    visit(child, name)?;
                }
            }
            Ok(())
        }
        visit(&value, name)?;
        for kind in ["lib", "bin"] {
            let targets: Vec<_> = match value.get(kind) {
                Some(toml::Value::Array(values)) => values.iter().collect(),
                Some(value) => vec![value],
                None => Vec::new(),
            };
            for target in targets {
                if let Some(path) = target.get("path").and_then(toml::Value::as_str) {
                    if !path.starts_with("src/")
                        || Path::new(path)
                            .components()
                            .any(|part| matches!(part, Component::ParentDir))
                    {
                        return Err("source targets must remain inside inventoried src".into());
                    }
                }
            }
        }
    }
    Ok(())
}

pub fn inputs(base: &Path) -> Result<BTreeMap<String, String>, String> {
    manifests(base)?;
    let mut files = BTreeMap::new();
    for directory in [
        "src",
        "evaluation/src",
        "evaluation/corpus",
        "evaluation/fixtures",
        "evaluation/external/t2d-v1",
        "evaluation/external/northix-v1",
        "tooling/src",
    ] {
        all_files(base, &base.join(directory), &mut files)?;
    }
    for path in [
        "Cargo.toml",
        "Cargo.lock",
        "README.md",
        "evaluation/Cargo.toml",
        "tooling/Cargo.toml",
    ] {
        files.insert(path.to_owned(), digest(&read(&base.join(path))?));
    }
    for directory in [
        base.to_path_buf(),
        base.join("evaluation"),
        base.join("tooling"),
    ] {
        for name in [
            "build.rs",
            "Cargo.lock",
            "rust-toolchain",
            "rust-toolchain.toml",
        ] {
            let path = directory.join(name);
            if path.exists() {
                files.insert(
                    path.strip_prefix(base)
                        .unwrap()
                        .to_string_lossy()
                        .replace('\\', "/"),
                    digest(&read(&path)?),
                );
            }
        }
    }
    for entry in fs::read_dir(base.join("evaluation")).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path
            .file_name()
            .is_some_and(|name| name.to_string_lossy().ends_with("protocol.json"))
        {
            files.insert(
                path.strip_prefix(base)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/"),
                digest(&read(&path)?),
            );
        }
    }
    Ok(files)
}

fn environment() -> Result<BTreeMap<String, String>, String> {
    let forbidden = [
        "RUSTC",
        "RUSTDOC",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "CARGO_BUILD_TARGET",
        "CARGO_BUILD_RUSTC",
        "CARGO_BUILD_RUSTDOC",
        "CARGO_BUILD_RUSTC_WRAPPER",
        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
    ];
    if forbidden
        .iter()
        .any(|key| std::env::var_os(key).is_some_and(|value| !value.is_empty()))
    {
        return Err("cross targets and compiler overrides are unsupported".into());
    }
    let exact = [
        "PATH",
        "PATHEXT",
        "HOME",
        "USERPROFILE",
        "CARGO_HOME",
        "RUSTUP_HOME",
        "CC",
        "CXX",
        "AR",
        "RANLIB",
        "LD",
        "CFLAGS",
        "CXXFLAGS",
        "LDFLAGS",
        "LIB",
        "LIBPATH",
        "INCLUDE",
        "SDKROOT",
        "MACOSX_DEPLOYMENT_TARGET",
        "SOURCE_DATE_EPOCH",
        "TEMP",
        "TMP",
        "TMPDIR",
    ];
    Ok(std::env::vars()
        .filter(|(key, _)| {
            (exact.contains(&key.as_str())
                || [
                    "CARGO_",
                    "RUST",
                    "CC_",
                    "CXX_",
                    "AR_",
                    "CFLAGS_",
                    "CXXFLAGS_",
                ]
                .iter()
                .any(|prefix| key.starts_with(prefix)))
                && ![
                    "TOKEN",
                    "PASSWORD",
                    "SECRET",
                    "CREDENTIAL",
                    "AUTHORIZATION",
                    "PRIVATE_KEY",
                ]
                .iter()
                .any(|secret| key.to_uppercase().contains(secret))
        })
        .map(|(key, value)| (key, digest(value.as_bytes())))
        .collect())
}

fn configs(base: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut directories: BTreeSet<_> = base
        .ancestors()
        .map(|directory| directory.join(".cargo"))
        .collect();
    let cargo_home = std::env::var_os("CARGO_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(
                std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" })
                    .unwrap_or_default(),
            )
            .join(".cargo")
        });
    directories.insert(if cargo_home.is_absolute() {
        cargo_home
    } else {
        base.join(cargo_home)
    });
    let mut result = BTreeMap::new();
    for directory in directories {
        for name in ["config", "config.toml"] {
            let path = directory.join(name);
            if !path.exists() {
                continue;
            }
            let bytes = read(&path)?;
            let value: toml::Value =
                toml::from_str(std::str::from_utf8(&bytes).map_err(|error| error.to_string())?)
                    .map_err(|error| error.to_string())?;
            if ["include", "source", "paths", "patch", "replace"]
                .iter()
                .any(|key| value.get(key).is_some())
                || value.get("build").is_some_and(|build| {
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
                || value.get("env").is_some_and(|env| {
                    [
                        "RUSTC",
                        "RUSTDOC",
                        "RUSTC_WRAPPER",
                        "RUSTC_WORKSPACE_WRAPPER",
                        "CARGO_BUILD_TARGET",
                        "CARGO_BUILD_RUSTC",
                        "CARGO_BUILD_RUSTDOC",
                        "CARGO_BUILD_RUSTC_WRAPPER",
                        "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
                    ]
                    .iter()
                    .any(|key| env.get(key).is_some())
                })
            {
                return Err(
                    "Cargo includes, source/target or compiler overrides are unsupported".into(),
                );
            }
            result.insert(path.to_string_lossy().into_owned(), digest(&bytes));
        }
    }
    for directory in base.ancestors() {
        for name in ["rust-toolchain", "rust-toolchain.toml"] {
            let path = directory.join(name);
            if path.exists() {
                result.insert(path.to_string_lossy().into_owned(), digest(&read(&path)?));
            }
        }
    }
    Ok(result)
}

fn valid_toolchain(toolchain: &str) -> bool {
    let parts: Vec<_> = toolchain.split('.').collect();
    toolchain == "stable"
        || parts.len() == 3
            && parts
                .iter()
                .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn context(base: &Path, toolchain: &str) -> Result<Value, String> {
    if !valid_toolchain(toolchain) {
        return Err("use stable or an explicit numeric Rust toolchain".into());
    }
    let args = vec![format!("+{toolchain}"), "-Vv".into()];
    let compiler = String::from_utf8(command("rustc", &args, base)?.stdout)
        .map_err(|error| error.to_string())?;
    let cargo = String::from_utf8(command("cargo", &args, base)?.stdout)
        .map_err(|error| error.to_string())?;
    let actual = compiler
        .lines()
        .find_map(|line| line.strip_prefix("release: "))
        .ok_or("rustc release missing")?;
    if toolchain != "stable" && actual != toolchain
        || !cargo
            .lines()
            .next()
            .is_some_and(|line| line.starts_with(&format!("cargo {actual} ")))
    {
        return Err("actual toolchain differs from the requested version".into());
    }
    Ok(
        json!({"input_sha256":inputs(base)?,"cargo_config_sha256":configs(base)?,
        "build_environment_sha256":environment()?,"compiler":compiler.trim(),"cargo":cargo.trim()}),
    )
}

fn destination(base: &Path, path: &Path, build: bool) -> Result<PathBuf, String> {
    if path
        .components()
        .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err("destination cannot contain parent/current directory components".into());
    }
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    };
    checked_path(&path)?;
    let target = base.join("target");
    let results = base.join("evaluation/results");
    if !(path.starts_with(&target) && path != target
        || !build && path.starts_with(&results) && path != results)
    {
        return Err("destination must be below target or evaluation/results".into());
    }
    Ok(path)
}

fn seal(record: &Value) -> Result<String, String> {
    let mut copy = record.clone();
    copy.as_object_mut()
        .ok_or("record must be an object")?
        .remove("record_sha256");
    Ok(digest(
        &serde_json::to_vec(&copy).map_err(|error| error.to_string())?,
    ))
}

fn record(path: &Path, mut value: Value) -> Result<(), String> {
    value["record_sha256"] = json!(seal(&value)?);
    write_new(path, &value)
}

fn load_record(path: &Path) -> Result<Value, String> {
    if fs::metadata(path).map_err(|error| error.to_string())?.len() > 4 * 1024 * 1024 {
        return Err("record exceeds its byte limit".into());
    }
    let value = load_json(path)?;
    if value["record_sha256"] != seal(&value)? {
        return Err("record checksum differs".into());
    }
    Ok(value)
}

fn build_command(toolchain: &str, directory: &Path) -> Vec<String> {
    vec![
        format!("+{toolchain}"),
        "build".into(),
        "--locked".into(),
        "--offline".into(),
        "--release".into(),
        "-p".into(),
        "fieldkin-eval".into(),
        "--bin".into(),
        "fieldkin-eval".into(),
        "--target-dir".into(),
        directory
            .join("cargo-target")
            .to_string_lossy()
            .into_owned(),
        "--message-format=json".into(),
    ]
}

pub fn build(args: Vec<String>) -> Result<(), String> {
    let (directory, _, toolchain, scores) = options(args)?;
    if !scores.is_empty() {
        return Err("build accepts no score files".into());
    }
    let base = root();
    let directory = destination(&base, &directory, true)?;
    if directory.exists() {
        return Err("use a fresh build directory".into());
    }
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let started = now();
    let before = context(&base, &toolchain)?;
    let arguments = build_command(&toolchain, &directory);
    let output = command("cargo", &arguments, &base)?;
    let binaries: Vec<_> = String::from_utf8(output.stdout)
        .map_err(|error| error.to_string())?
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).map_err(|error| error.to_string()))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .filter(|value| {
            value["reason"] == "compiler-artifact"
                && value["target"]["name"] == "fieldkin-eval"
                && value["executable"].is_string()
        })
        .map(|value| PathBuf::from(value["executable"].as_str().unwrap()))
        .collect();
    if binaries.len() != 1 {
        return Err("Cargo must report exactly one evaluator executable".into());
    }
    let original = &binaries[0];
    // Canonicalize only after rejecting symlink/junction components.
    checked_path(original)?;
    if !original
        .canonicalize()
        .map_err(|error| error.to_string())?
        .starts_with(
            directory
                .join("cargo-target")
                .canonicalize()
                .map_err(|error| error.to_string())?,
        )
    {
        return Err("Cargo executable escaped the fresh target".into());
    }
    let binary_dir = directory.join("bin");
    fs::create_dir(&binary_dir).map_err(|error| error.to_string())?;
    let binary = executable(&binary_dir, "fieldkin-eval");
    let sha = digest(&read(original)?);
    fs::copy(original, &binary).map_err(|error| error.to_string())?;
    if digest(&read(&binary)?) != sha || digest(&read(original)?) != sha {
        return Err("executable changed during copy".into());
    }
    let after = context(&base, &toolchain)?;
    if before != after {
        return Err("verified context changed during build".into());
    }
    record(
        &directory.join("build.json"),
        json!({"protocol":PROTOCOL,"repository_root":base,"toolchain":toolchain,
        "command":arguments,"context_before":before,"context_after":after,"cargo_executable":original,
        "binary_path":binary.strip_prefix(&directory).unwrap().to_string_lossy().replace('\\',"/"),"binary_sha256":sha,
        "started_unix_seconds":started,"completed_unix_seconds":now(),"trust_boundary":TRUST}),
    )?;
    println!("Fresh native evaluator build recorded.");
    Ok(())
}

fn verified_build(base: &Path, directory: &Path) -> Result<(Value, PathBuf), String> {
    let value = load_record(&directory.join("build.json"))?;
    if value["protocol"] != PROTOCOL
        || value["repository_root"] != json!(base)
        || value["trust_boundary"] != TRUST
        || !matches!(
            value["binary_path"].as_str(),
            Some("bin/fieldkin-eval" | "bin/fieldkin-eval.exe")
        )
    {
        return Err("unexpected native build record shape or identity".into());
    }
    let toolchain = value["toolchain"].as_str().ok_or("missing toolchain")?;
    if value["command"] != json!(build_command(toolchain, directory)) {
        return Err("build command differs".into());
    }
    let binary = directory.join(value["binary_path"].as_str().unwrap());
    if digest(&read(&binary)?) != value["binary_sha256"] {
        return Err("copied executable changed".into());
    }
    let current = context(base, toolchain)?;
    if value["context_before"] != value["context_after"] || value["context_before"] != current {
        return Err("verified source, compiler, data, configuration or environment changed".into());
    }
    Ok((value, binary))
}

fn scores(paths: &[PathBuf], base: &Path) -> Result<Vec<Value>, String> {
    if !matches!(paths.len(), 0 | 2) {
        return Err("Northix needs zero or two distinct score files".into());
    }
    let mut seen = BTreeSet::new();
    let mut result = Vec::new();
    for path in paths {
        let path = if path.is_absolute() {
            path.to_owned()
        } else {
            base.join(path)
        };
        checked_path(&path)?;
        let path = path.canonicalize().map_err(|error| error.to_string())?;
        let bytes = read(&path)?;
        if !seen.insert(path.clone()) || bytes.len() > 16 * 1024 * 1024 {
            return Err("duplicate or oversized score file".into());
        }
        result.push(json!({"path":path,"bytes":bytes.len(),"sha256":digest(&bytes)}));
    }
    Ok(result)
}

pub fn run(args: Vec<String>, northix: bool) -> Result<(), String> {
    run_task(
        args,
        if northix {
            Task::Northix
        } else {
            Task::External
        },
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Task {
    External,
    Northix,
    ContextQualification,
}

pub fn record_context(mut args: Vec<String>) -> Result<(), String> {
    if args
        .iter()
        .filter(|arg| arg.as_str() == "--acknowledge-new-holdout")
        .count()
        != 1
    {
        return Err("record-context requires explicit --acknowledge-new-holdout".into());
    }
    args.retain(|arg| arg != "--acknowledge-new-holdout");
    run_task(args, Task::ContextQualification)
}

fn run_task(args: Vec<String>, task: Task) -> Result<(), String> {
    let (directory, output, _, paths) = options(args)?;
    if task != Task::Northix && !paths.is_empty() {
        return Err("this fixed task accepts no score files".into());
    }
    let base = root();
    let directory = destination(&base, &directory, true)?;
    let output = destination(&base, &output.ok_or("--output is required")?, false)?;
    if output.starts_with(&directory) || directory.starts_with(&output) {
        return Err("build and output directories overlap".into());
    }
    if output.exists() {
        return Err("use a fresh output directory".into());
    }
    let record_path = directory.join("build.json");
    let record_sha = digest(&read(&record_path)?);
    let (before, binary) = verified_build(&base, &directory)?;
    let before_scores = scores(&paths, &base)?;
    fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    let route = match task {
        Task::External => "--external",
        Task::Northix => "--northix",
        Task::ContextQualification => "--context-qualification",
    };
    let mut arguments = vec![
        route.into(),
        "--output".into(),
        output.join("artifacts").to_string_lossy().into_owned(),
    ];
    for score in &before_scores {
        arguments.extend([
            "--scores".into(),
            score["path"].as_str().ok_or("score path invalid")?.into(),
        ]);
    }
    if task == Task::ContextQualification {
        arguments.extend([
            "--acknowledge-new-holdout".into(),
            "--frozen-binary-sha256".into(),
            before["binary_sha256"]
                .as_str()
                .ok_or("missing frozen executable hash")?
                .into(),
            "--frozen-build-record-sha256".into(),
            record_sha.clone(),
        ]);
        // Persist before the process starts. Even a failed run is an examined
        // holdout, so a retry must never silently become another prospective run.
        write_new(
            &base.join("target/context-qualification-v1.started.json"),
            &json!({
            "protocol":"context-qualification-v1","build_record_sha256":record_sha,
            "binary_sha256":before["binary_sha256"],"output":output,
            "corpus_sha256":before["context_before"]["input_sha256"]["evaluation/corpus/qualification-20261004.json"],
            "started_unix_seconds":now()}),
        )?;
    }
    command(
        binary.to_str().ok_or("non-UTF8 executable path")?,
        &arguments,
        &base,
    )?;
    let (after, checked) = verified_build(&base, &directory)?;
    if before != after
        || binary != checked
        || digest(&read(&record_path)?) != record_sha
        || scores(&paths, &base)? != before_scores
    {
        return Err("record, executable or scores changed during evaluation".into());
    }
    let expected = match task {
        Task::Northix => ["results.json", "predictions.jsonl", "results.md"],
        Task::External => [
            "development.json",
            "development-predictions.jsonl",
            "development.md",
        ],
        Task::ContextQualification => [
            "qualification.json",
            "predictions.jsonl",
            "qualification.md",
        ],
    };
    let artifacts = output.join("artifacts");
    let actual: BTreeSet<_> = fs::read_dir(&artifacts)
        .map_err(|error| error.to_string())?
        .map(|entry| {
            entry
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .map_err(|error| error.to_string())
        })
        .collect::<Result<_, _>>()?;
    if actual != expected.into_iter().map(str::to_owned).collect() {
        return Err("unexpected or missing evaluator artifacts".into());
    }
    if fs::read_dir(&output)
        .map_err(|error| error.to_string())?
        .count()
        != 1
    {
        return Err("unexpected output files".into());
    }
    let mut hashes = BTreeMap::new();
    for name in expected {
        let bytes = read(&artifacts.join(name))?;
        if bytes.len() > 64 * 1024 * 1024 {
            return Err("artifact exceeds byte limit".into());
        }
        hashes.insert(name, digest(&bytes));
    }
    if verified_build(&base, &directory)?.0 != before
        || digest(&read(&record_path)?) != record_sha
        || scores(&paths, &base)? != before_scores
    {
        return Err("verified context changed during artifact inspection".into());
    }
    let partition = match task {
        Task::External => "development",
        Task::Northix => "fixed-external-diagnostic",
        Task::ContextQualification => "new-reserved-synthetic-holdout",
    };
    record(
        &output.join("run.json"),
        json!({"protocol":PROTOCOL,"status":"success","partition":partition,
        "reserved_holdouts_scored":task==Task::ContextQualification,"other_reserved_holdouts_scored":false,"build_record_sha256":record_sha,"binary_sha256":before["binary_sha256"],
        "input_sha256":before["context_before"]["input_sha256"],"command":arguments,"artifact_sha256":hashes,"score_files":before_scores,
        "completed_unix_seconds":now(),"trust_boundary":TRUST}),
    )?;
    println!("Verified native evaluation recorded for {partition}.");
    Ok(())
}

type Options = (PathBuf, Option<PathBuf>, String, Vec<PathBuf>);
fn options(args: Vec<String>) -> Result<Options, String> {
    let mut args = args.into_iter();
    let mut directory = None;
    let mut output = None;
    let mut toolchain = "stable".to_owned();
    let mut scores = Vec::new();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--build-dir" => {
                directory = Some(PathBuf::from(
                    args.next().ok_or("--build-dir needs a directory")?,
                ))
            }
            "--output" => {
                output = Some(PathBuf::from(
                    args.next().ok_or("--output needs a directory")?,
                ))
            }
            "--toolchain" => toolchain = args.next().ok_or("--toolchain needs a version")?,
            "--scores" => scores.push(PathBuf::from(args.next().ok_or("--scores needs a file")?)),
            _ => return Err("verified tooling accepts only documented fixed-task options".into()),
        }
    }
    Ok((
        directory.ok_or("--build-dir is required")?,
        output,
        toolchain,
        scores,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_routes_reject_holdout_and_arbitrary_matcher_arguments() {
        for flag in [
            "--holdout",
            "--acknowledge-holdout",
            "--min-score",
            "--command",
        ] {
            assert!(options(vec![
                "--build-dir".into(),
                "target/test".into(),
                flag.into()
            ])
            .is_err());
        }
        assert!(record_context(vec!["--build-dir".into(), "target/test".into()]).is_err());
    }
    #[test]
    fn recorded_destinations_stay_in_repository() {
        let base = root();
        for path in [
            "target/../outside",
            "evaluation/results/../outside",
            "src/output",
            "target",
            ".",
        ] {
            assert!(destination(&base, Path::new(path), false).is_err());
        }
        assert!(destination(&base, Path::new("target/native/run"), false).is_ok());
        assert!(destination(&base, Path::new("evaluation/results/native/run"), false).is_ok());
        assert!(destination(&base, Path::new("evaluation/results/native/run"), true).is_err());
    }
    #[test]
    fn exact_numeric_toolchain_and_record_tampering_are_detected() {
        assert!(valid_toolchain("1.99.0"));
        assert!(valid_toolchain("stable"));
        for value in ["1.99", "nightly", "1.99.0 --arg"] {
            assert!(!valid_toolchain(value));
        }
        let value = json!({"binary_sha256":"original","source":{"a":"one"}});
        let hash = seal(&value).unwrap();
        let mut changed = value.clone();
        changed["source"]["a"] = json!("two");
        assert_ne!(seal(&changed).unwrap(), hash);
        changed = value;
        changed["binary_sha256"] = json!("different");
        assert_ne!(seal(&changed).unwrap(), hash);
    }
}
