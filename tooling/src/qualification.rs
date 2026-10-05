//! Native generated-case qualification and dependency review records.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::common::{
    capture, checked_path, command, digest, executable, json as load_json, read, root,
    source_inventory, write_new,
};

const PROTOCOL: &str = "fieldkin-native-qualification-v4";
const DEFAULT_CASES: u64 = 1_000_000;
const DEFAULT_SEED: u64 = 20_261_003;
const TRUST: &str = "Local reproducibility record, not a signature or hermetic build. Trust the local wrapper, compiler, Cargo, processes and locked dependencies. Before/after checks cannot detect a change restored between checks. Subprocess and pipe completion deadlines do not sandbox descendant processes.";
const COMPILER_OVERRIDES: [&str; 9] = [
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

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn normalized_hash(path: &Path) -> Result<String, String> {
    let bytes = read(path)?;
    let mut normalized = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n') {
            normalized.push(b'\n');
            index += 2;
        } else {
            normalized.push(bytes[index]);
            index += 1;
        }
    }
    Ok(digest(&normalized))
}

fn source_hashes(base: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();
    for directory in ["src", "qualification/src", "tooling/src"] {
        for (name, _) in source_inventory(&base.join(directory))? {
            let relative = format!("{directory}/{name}");
            result.insert(relative.clone(), normalized_hash(&base.join(relative))?);
        }
    }
    for name in [
        "Cargo.toml",
        "Cargo.lock",
        "README.md",
        "qualification/Cargo.toml",
        "qualification/Cargo.lock",
        "qualification/README.md",
        "tooling/Cargo.toml",
        "rust-toolchain.toml",
        "rust-toolchain",
    ] {
        let path = base.join(name);
        if path.exists() {
            result.insert(name.into(), normalized_hash(&path)?);
        }
    }
    for directory in [
        base.to_owned(),
        base.join("qualification"),
        base.join("tooling"),
    ] {
        let path = directory.join("build.rs");
        if path.exists() {
            result.insert(
                path.strip_prefix(base)
                    .map_err(|error| error.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/"),
                normalized_hash(&path)?,
            );
        }
    }
    Ok(result)
}

fn build_environment() -> Result<BTreeMap<String, String>, String> {
    if COMPILER_OVERRIDES
        .iter()
        .any(|key| std::env::var_os(key).is_some_and(|value| !value.is_empty()))
    {
        return Err(
            "cross targets and compiler overrides are unsupported in qualification records".into(),
        );
    }
    let keys = [
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
        "SOURCE_DATE_EPOCH",
        "TEMP",
        "TMP",
        "TMPDIR",
    ];
    Ok(std::env::vars()
        .filter(|(key, _)| {
            (keys.contains(&key.as_str())
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

fn validate_cargo_config(value: &toml::Value) -> Result<(), String> {
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
        || value
            .get("env")
            .is_some_and(|env| COMPILER_OVERRIDES.iter().any(|key| env.get(key).is_some()))
    {
        return Err("Cargo include/source/path, target or compiler overrides are unsupported in qualification records".into());
    }
    Ok(())
}

fn cargo_config(base: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut directories: BTreeSet<PathBuf> = base
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
            validate_cargo_config(&value)?;
            result.insert(path.to_string_lossy().into_owned(), digest(&bytes));
        }
    }
    Ok(result)
}

fn output_text(program: &str, args: Vec<String>, base: &Path) -> Result<String, String> {
    String::from_utf8(command(program, &args, base)?.stdout)
        .map(|text| text.trim().to_owned())
        .map_err(|error| error.to_string())
}

fn context(base: &Path) -> Result<Value, String> {
    let environment = build_environment()?;
    let configurations = cargo_config(base)?;
    let compiler = output_text("rustc", vec!["+stable".into(), "-Vv".into()], base)?;
    let cargo = output_text("cargo", vec!["+stable".into(), "-Vv".into()], base)?;
    let release = compiler
        .lines()
        .find_map(|line| line.strip_prefix("release: "))
        .ok_or("rustc release missing")?;
    if !cargo
        .lines()
        .next()
        .is_some_and(|line| line.starts_with(&format!("cargo {release} ")))
    {
        return Err("stable rustc and Cargo versions differ".into());
    }
    Ok(
        json!({"source_sha256":source_hashes(base)?, "compiler":compiler, "compiler_release":release,
        "cargo":cargo, "build_environment_sha256":environment, "cargo_config_sha256":configurations}),
    )
}

fn destination(base: &Path, path: &Path) -> Result<PathBuf, String> {
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        return Err("output cannot contain parent/current directory components".into());
    }
    let path = if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    };
    checked_path(&path)?;
    if path == base {
        return Err("output must be a dedicated directory or file".into());
    }
    Ok(path)
}

fn seal(value: &Value) -> Result<String, String> {
    let mut copy = value.clone();
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
        return Err("build record exceeds its byte limit".into());
    }
    let value = load_json(path)?;
    if value["record_sha256"] != seal(&value)? {
        return Err("record checksum differs".into());
    }
    Ok(value)
}

#[derive(Debug)]
struct Options {
    output: PathBuf,
    cases: u64,
    seed: u64,
}

fn options(args: Vec<String>) -> Result<Options, String> {
    let mut args = args.into_iter();
    let mut output = None;
    let mut cases = DEFAULT_CASES;
    let mut seed = DEFAULT_SEED;
    let mut seen = BTreeSet::new();
    while let Some(arg) = args.next() {
        if !seen.insert(arg.clone()) {
            return Err(format!("duplicate option {arg}"));
        }
        let value = args.next().ok_or_else(|| format!("{arg} needs a value"))?;
        match arg.as_str() {
            "--output" => output = Some(PathBuf::from(value)),
            "--cases" => {
                cases = value
                    .parse::<u64>()
                    .map_err(|_| "cases must be an unsigned integer")?
            }
            "--seed" => seed = value.parse::<u64>().map_err(|_| "seed must fit u64")?,
            _ => return Err(format!("unsupported qualification option {arg}")),
        }
    }
    if !(1..=10_000_000).contains(&cases) {
        return Err("cases must be 1..=10000000".into());
    }
    Ok(Options {
        output: output.ok_or("--output is required")?,
        cases,
        seed,
    })
}

fn build_command(directory: &Path) -> Vec<String> {
    vec![
        "+stable".into(),
        "build".into(),
        "--release".into(),
        "--locked".into(),
        "--offline".into(),
        "--manifest-path".into(),
        "qualification/Cargo.toml".into(),
        "--target-dir".into(),
        directory
            .join("cargo-target")
            .to_string_lossy()
            .into_owned(),
        "--message-format=json-render-diagnostics".into(),
    ]
}

pub fn build(args: Vec<String>) -> Result<(), String> {
    let options = options(args)?;
    let base = root();
    let directory = destination(&base, &options.output)?;
    if directory.exists() {
        return Err("use a fresh qualification output directory".into());
    }
    let before = context(&base)?;
    let started = now();
    fs::create_dir_all(&directory).map_err(|error| error.to_string())?;
    let arguments = build_command(&directory);
    let compiled = command("cargo", &arguments, &base)?;
    let mut artifacts = Vec::new();
    for line in String::from_utf8(compiled.stdout)
        .map_err(|error| error.to_string())?
        .lines()
        .filter(|line| !line.trim().is_empty())
    {
        let value: Value = serde_json::from_str(line).map_err(|error| error.to_string())?;
        if value["reason"] == "compiler-artifact"
            && value["target"]["name"] == "fieldkin-qualification"
        {
            if let Some(path) = value["executable"].as_str() {
                artifacts.push(PathBuf::from(path));
            }
        }
    }
    if artifacts.len() != 1 {
        return Err("Cargo must report exactly one qualification executable".into());
    }
    let original = &artifacts[0];
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
        return Err("Cargo executable escaped the fresh target directory".into());
    }
    let binary_dir = directory.join("bin");
    fs::create_dir(&binary_dir).map_err(|error| error.to_string())?;
    let binary = executable(&binary_dir, "fieldkin-qualification");
    let binary_sha = digest(&read(original)?);
    fs::copy(original, &binary).map_err(|error| error.to_string())?;
    if digest(&read(&binary)?) != binary_sha || digest(&read(original)?) != binary_sha {
        return Err("qualification executable changed during copy".into());
    }
    let after = context(&base)?;
    if before != after {
        return Err(
            "qualification sources/compiler/configuration/environment changed during compilation"
                .into(),
        );
    }
    record(
        &directory.join("build.json"),
        json!({"protocol":PROTOCOL, "repository_root":base,
        "built_unix_seconds":now(), "started_unix_seconds":started, "toolchain":"stable", "commit":output_text("git",vec!["rev-parse".into(),"HEAD".into()],&base)?,
        "git_status":output_text("git",vec!["status".into(),"--short".into()],&base)?, "context_before":before, "context_after":after,
        "build_command":arguments, "cargo_executable":original, "binary_path":binary.strip_prefix(&directory).unwrap().to_string_lossy().replace('\\',"/"),
        "binary_sha256":binary_sha, "machine":{"platform":std::env::consts::OS,"architecture":std::env::consts::ARCH,
        "logical_cpus":std::thread::available_parallelism().map(|count|count.get()).ok(),"runner":"native Rust"},"trust_boundary":TRUST}),
    )?;
    println!(
        "Qualification binary built with stable Rust. Freeze sources before running the campaign."
    );
    Ok(())
}

fn verified_build(base: &Path, directory: &Path) -> Result<(Value, PathBuf), String> {
    let value = load_record(&directory.join("build.json"))?;
    if value["protocol"] != PROTOCOL
        || value["repository_root"] != json!(base)
        || value["toolchain"] != "stable"
        || value["trust_boundary"] != TRUST
        || !matches!(
            value["binary_path"].as_str(),
            Some("bin/fieldkin-qualification" | "bin/fieldkin-qualification.exe")
        )
    {
        return Err("unexpected qualification build record identity or executable path".into());
    }
    if value["build_command"] != json!(build_command(directory)) {
        return Err("qualification build command differs".into());
    }
    let binary = directory.join(value["binary_path"].as_str().unwrap());
    if digest(&read(&binary)?) != value["binary_sha256"] {
        return Err("qualification binary changed after build".into());
    }
    let current = context(base)?;
    if value["context_before"] != value["context_after"] || value["context_before"] != current {
        return Err(
            "qualification sources/compiler/configuration/environment changed after build".into(),
        );
    }
    Ok((value, binary))
}

fn check_campaign(campaign: &Value, requested: u64) -> Result<(), String> {
    let categories = campaign["categories"]
        .as_object()
        .ok_or("campaign categories missing")?;
    let total = categories.values().try_fold(0_u64, |sum, count| {
        sum.checked_add(count.as_u64().ok_or("invalid category count")?)
            .ok_or("category total overflow")
    })?;
    if campaign["cases"].as_u64() != Some(requested)
        || total != requested
        || campaign["passed"] != true
    {
        return Err("campaign counts/status inconsistent".into());
    }
    Ok(())
}

pub fn run(args: Vec<String>) -> Result<(), String> {
    let options = options(args)?;
    let base = root();
    let directory = destination(&base, &options.output)?;
    if directory.join("run.json").exists() {
        return Err("run.json exists; choose a new qualification output directory".into());
    }
    let build_path = directory.join("build.json");
    let build_sha = digest(&read(&build_path)?);
    let (before, binary) = verified_build(&base, &directory)?;
    let started = now();
    let clock = Instant::now();
    let arguments = vec![
        "--cases".into(),
        options.cases.to_string(),
        "--seed".into(),
        options.seed.to_string(),
    ];
    let completed = capture(
        binary.to_str().ok_or("non-UTF8 executable path")?,
        &arguments,
        &base,
        Duration::from_secs(600),
    )?;
    let elapsed = clock.elapsed().as_secs_f64();
    let (after, checked) = verified_build(&base, &directory)?;
    if before != after || checked != binary || digest(&read(&build_path)?) != build_sha {
        return Err("qualification inputs, binary or build record changed during campaign".into());
    }
    let mut result = json!({"protocol":PROTOCOL,"started_unix_seconds":started,"wall_seconds":elapsed,
        "exit_code":completed.status.code(),"requested_cases":options.cases,"seed":options.seed,
        "stderr":String::from_utf8_lossy(&completed.stderr),"build_sha256":build_sha,"trust_boundary":TRUST});
    if completed.status.success() {
        let campaign: Value =
            serde_json::from_slice(&completed.stdout).map_err(|error| error.to_string())?;
        check_campaign(&campaign, options.cases)?;
        result["campaign"] = campaign;
    } else {
        result["stdout"] = json!(String::from_utf8_lossy(&completed.stdout));
    }
    record(&directory.join("run.json"), result.clone())?;
    println!(
        "{}",
        serde_json::to_string_pretty(result.get("campaign").unwrap_or(&result))
            .map_err(|error| error.to_string())?
    );
    if !completed.status.success() {
        return Err("qualification campaign failed; inspect the saved run record".into());
    }
    Ok(())
}

fn fetch(url: &str, base: &Path) -> Result<Value, String> {
    let args = vec![
        "--fail".into(),
        "--silent".into(),
        "--show-error".into(),
        "--location".into(),
        "--proto".into(),
        "=https".into(),
        "--proto-redir".into(),
        "=https".into(),
        "--max-time".into(),
        "30".into(),
        "--user-agent".into(),
        "fieldkin-dependency-review/0.2 (https://github.com/limadog9/fieldkin)".into(),
        "--header".into(),
        "Accept: application/json".into(),
        url.into(),
    ];
    let output = capture(
        if cfg!(windows) { "curl.exe" } else { "curl" },
        &args,
        base,
        Duration::from_secs(40),
    )?;
    if !output.status.success() {
        return Err(format!(
            "HTTPS metadata request failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())
}

fn relative(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

fn github_repository(url: &str) -> Option<&str> {
    let path = url
        .strip_prefix("https://github.com/")?
        .trim_end_matches('/')
        .trim_end_matches(".git");
    let parts: Vec<_> = path.split('/').collect();
    (parts.len() == 2
        && parts.iter().all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
        }))
    .then_some(path)
}

pub fn dependencies(args: Vec<String>) -> Result<(), String> {
    let mut args = args.into_iter();
    let mut output = None;
    let mut audit = None;
    let mut database = None;
    let mut seen = BTreeSet::new();
    while let Some(arg) = args.next() {
        if !seen.insert(arg.clone()) {
            return Err(format!("duplicate option {arg}"));
        }
        let path = PathBuf::from(args.next().ok_or_else(|| format!("{arg} needs a path"))?);
        match arg.as_str() {
            "--output" => output = Some(path),
            "--audit" => audit = Some(path),
            "--database" => database = Some(path),
            _ => return Err(format!("unsupported dependency review option {arg}")),
        }
    }
    let base = root();
    let output = destination(&base, &output.ok_or("--output is required")?)?;
    if output.exists() {
        return Err("dependency output already exists".into());
    }
    let audit = destination(&base, &audit.ok_or("--audit is required")?)?;
    let database = destination(&base, &database.ok_or("--database is required")?)?;
    let audit_sha = digest(&read(&audit)?);
    let database_commit = output_text(
        "git",
        vec![
            "-C".into(),
            database.to_string_lossy().into_owned(),
            "rev-parse".into(),
            "HEAD".into(),
        ],
        &base,
    )?;
    if !output_text(
        "git",
        vec![
            "-C".into(),
            database.to_string_lossy().into_owned(),
            "status".into(),
            "--porcelain".into(),
        ],
        &base,
    )?
    .is_empty()
    {
        return Err("advisory database checkout must be clean".into());
    }
    let mut packages = BTreeMap::<String, Value>::new();
    let mut upstream_names = BTreeSet::<String>::new();
    let mut locks = BTreeMap::<String, String>::new();
    let mut manifests = BTreeMap::<String, String>::new();
    for manifest in [
        "Cargo.toml",
        "evaluation/Cargo.toml",
        "tooling/Cargo.toml",
        "performance/Cargo.toml",
        "qualification/Cargo.toml",
    ] {
        manifests.insert(manifest.into(), normalized_hash(&base.join(manifest))?);
        let metadata: Value = serde_json::from_str(&output_text(
            "cargo",
            vec![
                "+stable".into(),
                "metadata".into(),
                "--locked".into(),
                "--offline".into(),
                "--format-version".into(),
                "1".into(),
                "--manifest-path".into(),
                manifest.into(),
                "--all-features".into(),
            ],
            &base,
        )?)
        .map_err(|error| error.to_string())?;
        let lock = PathBuf::from(
            metadata["workspace_root"]
                .as_str()
                .ok_or("workspace root missing")?,
        )
        .join("Cargo.lock");
        locks.insert(relative(&base, &lock), normalized_hash(&lock)?);
        for package in metadata["packages"]
            .as_array()
            .ok_or("metadata packages missing")?
        {
            if package["source"].is_null() {
                for dependency in package["dependencies"]
                    .as_array()
                    .ok_or("metadata dependencies missing")?
                {
                    if !dependency["source"].is_null() {
                        upstream_names.insert(
                            dependency["name"]
                                .as_str()
                                .ok_or("dependency name missing")?
                                .into(),
                        );
                    }
                }
                continue;
            }
            let name = package["name"].as_str().ok_or("package name missing")?;
            let version = package["version"]
                .as_str()
                .ok_or("package version missing")?;
            let key = format!("{name}@{version}");
            if !packages.contains_key(&key) {
                let manifest_path = PathBuf::from(
                    package["manifest_path"]
                        .as_str()
                        .ok_or("package manifest missing")?,
                );
                let directory = manifest_path
                    .parent()
                    .ok_or("package manifest has no parent")?;
                let mut notices = BTreeMap::new();
                for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
                    let path = entry.map_err(|error| error.to_string())?.path();
                    let notice_name = path.file_name().unwrap().to_string_lossy().into_owned();
                    let upper = notice_name.to_uppercase();
                    if path.is_file()
                        && ["LICENSE", "COPYING", "COPYRIGHT"]
                            .iter()
                            .any(|prefix| upper.starts_with(prefix))
                    {
                        notices.insert(notice_name, normalized_hash(&path)?);
                    }
                }
                packages.insert(key.clone(),json!({"name":name,"version":version,"license":package["license"],"repository":package["repository"],
                    "source":package["source"],"graphs":[],"license_file":package["license_file"],"notice_sha256":notices,"crate_manifest_sha256":normalized_hash(&manifest_path)?}));
            }
            let graphs = packages.get_mut(&key).unwrap()["graphs"]
                .as_array_mut()
                .ok_or("package graphs invalid")?;
            if !graphs.contains(&json!(manifest)) {
                graphs.push(json!(manifest));
            }
        }
    }
    if packages
        .values()
        .any(|package| package["name"] == "tinyvec")
    {
        upstream_names.insert("tinyvec".into());
    }
    let mut upstream = BTreeMap::new();
    for name in upstream_names {
        if !name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_".contains(&byte))
        {
            return Err("invalid upstream crate name".into());
        }
        let response = fetch(&format!("https://crates.io/api/v1/crates/{name}"), &base)?;
        let crate_info = &response["crate"];
        let versions = response["versions"]
            .as_array()
            .ok_or("upstream version list missing")?;
        let mut locked = Vec::new();
        for package in packages.values().filter(|package| package["name"] == name) {
            let version = versions
                .iter()
                .find(|version| version["num"] == package["version"])
                .ok_or("locked version missing from upstream metadata")?;
            locked.push(json!({"num":version["num"],"created_at":version["created_at"],"updated_at":version["updated_at"],"yanked":version["yanked"],"license":version["license"]}));
        }
        let mut value = json!({"crate_url":format!("https://crates.io/crates/{name}"),"latest_version":crate_info["max_version"],
            "last_updated":crate_info["updated_at"],"locked_versions":locked,"repository":crate_info["repository"]});
        if let Some(path) = crate_info["repository"]
            .as_str()
            .and_then(github_repository)
        {
            match fetch(&format!("https://api.github.com/repos/{path}"), &base) {
                Ok(repository) => {
                    value["github"] = json!({"html_url":repository["html_url"],"archived":repository["archived"],"disabled":repository["disabled"],"pushed_at":repository["pushed_at"],"updated_at":repository["updated_at"]})
                }
                Err(error) => value["github_error"] = json!(error),
            }
        }
        upstream.insert(name, value);
    }
    let mut audits = BTreeMap::new();
    let mut passed = true;
    for (lock, hash) in &locks {
        let arguments = vec![
            "audit".into(),
            "--json".into(),
            "--db".into(),
            database.to_string_lossy().into_owned(),
            "--no-fetch".into(),
            "--file".into(),
            lock.clone(),
            "--deny".into(),
            "warnings".into(),
        ];
        let scanned = capture(
            audit.to_str().ok_or("non-UTF8 audit path")?,
            &arguments,
            &base,
            Duration::from_secs(600),
        )?;
        passed &= scanned.status.success();
        let report: Value =
            serde_json::from_slice(&scanned.stdout).map_err(|error| error.to_string())?;
        audits.insert(lock.clone(),json!({"command":["cargo-audit","audit","--json","--db","<advisory-db-checkout>","--no-fetch","--file",lock,"--deny","warnings"],
            "exit_code":scanned.status.code(),"lock_sha256":hash,"stderr":String::from_utf8_lossy(&scanned.stderr),"report":report}));
    }
    if digest(&read(&audit)?) != audit_sha
        || output_text(
            "git",
            vec![
                "-C".into(),
                database.to_string_lossy().into_owned(),
                "rev-parse".into(),
                "HEAD".into(),
            ],
            &base,
        )? != database_commit
        || !output_text(
            "git",
            vec![
                "-C".into(),
                database.to_string_lossy().into_owned(),
                "status".into(),
                "--porcelain".into(),
            ],
            &base,
        )?
        .is_empty()
    {
        return Err("advisory tool/database changed during dependency review".into());
    }
    for (path, hash) in manifests.iter().chain(locks.iter()) {
        if normalized_hash(&base.join(path))? != *hash {
            return Err("dependency manifest or lockfile changed during review".into());
        }
    }
    let count = packages.len();
    record(
        &output,
        json!({"protocol":"fieldkin-native-dependency-review-v2","reviewed_unix_seconds":now(),
        "git_commit":output_text("git",vec!["rev-parse".into(),"HEAD".into()],&base)?,"compiler":output_text("rustc",vec!["+stable".into(),"-Vv".into()],&base)?,
        "tool":output_text(audit.to_str().ok_or("non-UTF8 audit path")?,vec!["audit".into(),"--version".into()],&base)?,"tool_binary_sha256":audit_sha,
        "advisory_database":{"url":"https://github.com/rustsec/advisory-db","commit":database_commit,"commit_time":output_text("git",vec!["-C".into(),database.to_string_lossy().into_owned(),"log".into(),"-1".into(),"--format=%cI".into()],&base)?},
        "manifest_sha256":manifests,"lock_sha256":locks,"packages":packages,"upstream":upstream,"audits":audits,"passed":passed,"transport":"curl HTTPS with certificate verification"}),
    )?;
    println!(
        "Recorded {count} third-party package versions across {} lockfiles",
        locks.len()
    );
    if !passed {
        return Err("at least one advisory audit did not pass; inspect the saved report".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cargo_config_rejects_compiler_and_target_overrides_without_environment_mutation() {
        for key in [
            "target",
            "rustc",
            "rustdoc",
            "rustc-wrapper",
            "rustc-workspace-wrapper",
        ] {
            let value: toml::Value =
                toml::from_str(&format!("[build]\n{key} = 'override'\n")).unwrap();
            assert!(validate_cargo_config(&value).is_err(), "[build].{key}");
        }
        for key in COMPILER_OVERRIDES {
            for specification in ["'override'", "{ value = 'override', force = true }"] {
                let value: toml::Value =
                    toml::from_str(&format!("[env]\n{key} = {specification}\n")).unwrap();
                assert!(validate_cargo_config(&value).is_err(), "[env].{key}");
            }
        }
        for text in [
            "include = ['other.toml']",
            "[source.crates-io]\nreplace-with = 'other'",
            "paths = ['outside']",
            "[patch.crates-io]\ncrate = { path = 'outside' }",
            "[replace]\n'crate:1.0.0' = { path = 'outside' }",
        ] {
            let value: toml::Value = toml::from_str(text).unwrap();
            assert!(validate_cargo_config(&value).is_err());
        }
        let ordinary: toml::Value = toml::from_str("[build]\njobs = 2\nrustflags = ['-C', 'opt-level=2']\n[env]\nRUSTFLAGS = '-C opt-level=2'\n").unwrap();
        assert!(validate_cargo_config(&ordinary).is_ok());
    }

    #[test]
    fn defaults_and_numeric_bounds_preserve_campaign_contract() {
        let value = options(vec![
            "--output".into(),
            "target/native-qualification".into(),
        ])
        .unwrap();
        assert_eq!(value.cases, 1_000_000);
        assert_eq!(value.seed, 20_261_003);
        for args in [
            vec!["--cases", "0"],
            vec!["--cases", "10000001"],
            vec!["--cases", "-1"],
            vec!["--seed", "18446744073709551616"],
            vec!["--command", "ignored"],
            vec!["--output", "first", "--output", "second"],
        ] {
            let mut input = vec!["--output".into(), "target/native-qualification".into()];
            input.extend(args.into_iter().map(String::from));
            assert!(options(input).is_err());
        }
    }

    #[test]
    fn result_validation_rejects_false_status_and_wrong_counts() {
        let good = json!({"cases":5,"passed":true,"categories":{"a":2,"b":3}});
        assert!(check_campaign(&good, 5).is_ok());
        for bad in [
            json!({"cases":5,"passed":false,"categories":{"a":5}}),
            json!({"cases":5,"passed":true,"categories":{"a":4}}),
            json!({"cases":4,"passed":true,"categories":{"a":5}}),
            json!({"cases":5,"passed":true,"categories":{"a":-1}}),
        ] {
            assert!(check_campaign(&bad, 5).is_err());
        }
    }

    #[test]
    fn checksums_cover_build_inputs_and_binary_and_paths_are_fixed() {
        let original =
            json!({"binary_sha256":"a","context_before":{"source_sha256":{"source":"b"}}});
        let before = seal(&original).unwrap();
        let mut changed = original.clone();
        changed["binary_sha256"] = json!("changed");
        assert_ne!(seal(&changed).unwrap(), before);
        changed = original;
        changed["context_before"]["source_sha256"]["source"] = json!("changed");
        assert_ne!(seal(&changed).unwrap(), before);
        assert!(destination(&root(), Path::new("target/../escaped")).is_err());
        assert!(destination(&root(), Path::new("target/qualification-check")).is_ok());
    }

    #[test]
    fn github_urls_are_limited_to_repository_endpoints() {
        assert_eq!(
            github_repository("https://github.com/owner/project.git"),
            Some("owner/project")
        );
        for url in [
            "http://github.com/a/b",
            "https://github.com/a/b/issues",
            "https://github.com/a/b?query=1",
            "https://example.invalid/a/b",
        ] {
            assert!(github_repository(url).is_none());
        }
    }

    #[test]
    fn persisted_record_mutation_and_existing_outputs_are_rejected() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let temporary_root = crate::common::canonical_test_temp_root(&std::env::temp_dir());
        let path = temporary_root.join(format!(
            "fieldkin-native-qualification-{}-{nonce}.json",
            std::process::id()
        ));
        let value = json!({"protocol":PROTOCOL,"binary_sha256":"pinned","context_before":{"source_sha256":{"input":"fixed"}}});
        record(&path, value).unwrap();
        assert!(load_record(&path).is_ok());
        assert!(record(&path, json!({"unexpected":"overwrite"})).is_err());
        let mut changed = load_json(&path).unwrap();
        changed["context_before"]["source_sha256"]["input"] = json!("changed");
        fs::write(&path, serde_json::to_vec(&changed).unwrap()).unwrap();
        assert!(load_record(&path).is_err());
        assert_eq!(path.parent(), Some(temporary_root.as_path()));
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn native_capture_preserves_failed_exit_for_failure_records() {
        let output = capture(
            "rustc",
            &[
                "+stable".into(),
                "--fieldkin-invalid-native-test-option".into(),
            ],
            &root(),
            Duration::from_secs(30),
        )
        .unwrap();
        assert!(!output.status.success());
        assert!(!output.stderr.is_empty());
    }
}
