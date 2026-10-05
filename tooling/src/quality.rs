//! Numerical quality acceptance is deliberately separate from reproducibility.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{json, Value};

use crate::common::{checked_path, command, digest, json as load_json, read, root, write_new};
use crate::verified;

const VERSION: &str = "fieldkin-quality-v1";
const CANDIDATE: &str = "contextual_quality";
const ASSIGNMENTS: [&str; 2] = ["independent", "one_to_one"];
const VARIANTS: [&str; 5] = [
    "base",
    "reordered",
    "without_samples",
    "null_heavy",
    "separator_noise",
];
const MODELS: [&str; 4] = ["default", "name_only_070", "name_only_exact", CANDIDATE];
const DOMAINS: [&str; 4] = [
    "product_import",
    "financial_records",
    "crm_contacts",
    "operational_telemetry",
];
const DEVELOPMENT: [(&str, u64, u64, u64); 3] = [
    ("original_development", 32, 160, 960),
    ("stage3_extension_without_hints", 12, 12, 32),
    ("corrective_development", 12, 36, 216),
];

fn now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}
fn targets() -> Value {
    json!({"precision":0.95,"unique_coverage":0.60,"candidate_recall_at_5":0.90,"nonzero_proposals":true})
}
fn options(args: Vec<String>, permitted: &[&str]) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if !permitted.contains(&arg.as_str()) {
            return Err(format!("unsupported quality option: {arg}"));
        }
        let value = if arg == "--acknowledge-new-holdout" {
            "true".into()
        } else {
            args.next().ok_or("missing quality option value")?
        };
        if result.insert(arg, value).is_some() {
            return Err("duplicate quality option".into());
        }
    }
    Ok(result)
}
fn path(options: &BTreeMap<String, String>, flag: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(
        options
            .get(flag)
            .ok_or_else(|| format!("{flag} is required"))?,
    );
    if path
        .components()
        .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err("quality paths cannot contain parent/current components".into());
    }
    let path = if path.is_absolute() {
        path
    } else {
        root().join(path)
    };
    checked_path(&path)?;
    Ok(path)
}
fn destination(path: &Path) -> Result<(), String> {
    let base = root();
    if !(path.starts_with(base.join("target")) && path != base.join("target")
        || path.starts_with(base.join("evaluation/results"))
            && path != base.join("evaluation/results"))
    {
        return Err("quality records must remain below target or evaluation/results".into());
    }
    checked_path(path)
}
fn protocol(value: &Value) -> Result<(), String> {
    if value["version"] != VERSION
        || value["candidate"] != CANDIDATE
        || value["models"] != json!(MODELS)
        || value["assignments"] != json!(ASSIGNMENTS)
        || value["variants"] != json!(VARIANTS)
        || value["quality_targets"] != targets()
        || !value["configuration"].is_object()
        || number(&value["inventory"], "families")? < 12
        || number(&value["inventory"], "cases")? < 60
        || number(&value["inventory"], "decisions")? < 360
    {
        return Err(
            "quality protocol weakens or differs from required public candidate contract".into(),
        );
    }
    Ok(())
}
fn number(value: &Value, key: &str) -> Result<u64, String> {
    value[key]
        .as_u64()
        .ok_or_else(|| format!("missing or invalid integer count: {key}"))
}

/// Use integer cross-products: rounded display percentages never qualify a run.
fn numerical(counts: &Value) -> Result<(), String> {
    consistent(counts)?;
    let proposed = number(counts, "proposals")?;
    let unique = number(counts, "unique_fields")?;
    let relevant = number(counts, "candidate_relevant")?;
    if proposed == 0
        || unique == 0
        || relevant == 0
        || u128::from(number(counts, "correct_proposals")?) * 100 < u128::from(proposed) * 95
        || u128::from(number(counts, "unique_proposals")?) * 100 < u128::from(unique) * 60
        || u128::from(number(counts, "candidate_hits")?) * 100 < u128::from(relevant) * 90
    {
        return Err(
            "actual counts fail nonempty/95% precision/60% aggregate unique coverage/90% recall@5"
                .into(),
        );
    }
    Ok(())
}
fn consistent(c: &Value) -> Result<(), String> {
    let fields = number(c, "fields")?;
    let unique = number(c, "unique_fields")?;
    let no_match = number(c, "no_match_fields")?;
    let ambiguous = number(c, "ambiguous_fields")?;
    let proposed = number(c, "proposals")?;
    let correct = number(c, "correct_proposals")?;
    let wrong = number(c, "wrong_unique_proposals")?;
    let unique_proposals = number(c, "unique_proposals")?;
    let no_match_proposals = number(c, "no_match_proposals")?;
    let ambiguous_proposals = number(c, "ambiguous_proposals")?;
    let sum = |values: &[u64]| -> Result<u64, String> {
        values.iter().try_fold(0u64, |sum, value| {
            sum.checked_add(*value).ok_or("count overflow".into())
        })
    };
    if number(c, "feasible_unique_matches")? > unique
        || number(c, "candidate_any_fields")? != sum(&[unique, ambiguous])?
        || number(c, "unique_candidate_relevant")? != unique
        || number(c, "ambiguous_candidate_relevant")?
            < ambiguous.checked_mul(2).ok_or("count overflow")?
        || fields != sum(&[unique, no_match, ambiguous])?
        || fields != sum(&[proposed, number(c, "abstentions")?])?
        || unique_proposals != sum(&[correct, wrong])?
        || proposed != sum(&[unique_proposals, no_match_proposals, ambiguous_proposals])?
        || unique_proposals > unique
        || no_match_proposals > no_match
        || ambiguous_proposals > ambiguous
        || number(c, "expected_abstentions")? != sum(&[no_match, ambiguous])?
        || number(c, "correct_abstentions")?
            != sum(&[
                no_match - no_match_proposals,
                ambiguous - ambiguous_proposals,
            ])?
        || number(c, "candidate_hits")? > number(c, "candidate_relevant")?
        || number(c, "candidate_any_hits")? > number(c, "candidate_any_fields")?
        || number(c, "candidate_relevant")?
            != sum(&[
                number(c, "unique_candidate_relevant")?,
                number(c, "ambiguous_candidate_relevant")?,
            ])?
        || number(c, "candidate_hits")?
            != sum(&[
                number(c, "unique_candidate_hits")?,
                number(c, "ambiguous_candidate_hits")?,
            ])?
        || number(c, "unique_candidate_hits")? > number(c, "unique_candidate_relevant")?
        || number(c, "ambiguous_candidate_hits")? > number(c, "ambiguous_candidate_relevant")?
    {
        return Err("inconsistent quality counts".into());
    }
    Ok(())
}
fn sum_counts<'a>(values: impl Iterator<Item = &'a Value>) -> Result<Value, String> {
    let mut total = BTreeMap::<String, u64>::new();
    let mut shape = None;
    for value in values {
        consistent(value)?;
        let entries = value.as_object().ok_or("counts must be an object")?;
        let keys = entries.keys().cloned().collect::<BTreeSet<_>>();
        if shape.as_ref().is_some_and(|shape| shape != &keys) {
            return Err("count shapes differ".into());
        }
        shape = Some(keys);
        for (key, value) in entries {
            let count = value.as_u64().ok_or("count must be unsigned integer")?;
            let entry = total.entry(key.clone()).or_default();
            *entry = entry.checked_add(count).ok_or("aggregate count overflow")?;
        }
    }
    serde_json::to_value(total).map_err(|error| error.to_string())
}
fn slices(row: &Value, required_variants: &[&str], domains: bool) -> Result<(), String> {
    for key in ["by_family", "by_domain", "by_variant"] {
        let slice = row[key]
            .as_object()
            .ok_or("missing required quality slices")?;
        if slice.is_empty() || sum_counts(slice.values())? != row["counts"] {
            return Err("slice counts do not exactly reconstruct aggregate".into());
        }
        if key == "by_variant"
            && slice.keys().map(String::as_str).collect::<BTreeSet<_>>()
                != required_variants.iter().copied().collect()
        {
            return Err("required variants missing or unexpected".into());
        }
        if key == "by_domain"
            && domains
            && slice.keys().map(String::as_str).collect::<BTreeSet<_>>()
                != DOMAINS.into_iter().collect()
        {
            return Err("required domains missing or unexpected".into());
        }
    }
    Ok(())
}
fn development(report: &Value, protocol: &Value) -> Result<(), String> {
    if report["contract"]["model_configuration"][CANDIDATE] != protocol["configuration"]
        || report["contract"]["quality_targets"] != targets()
    {
        return Err("development candidate/configuration mismatch".into());
    }
    for (name, families, cases, decisions) in DEVELOPMENT {
        let inventory = report["inventory"]
            .as_array()
            .ok_or("missing development inventory")?
            .iter()
            .find(|item| item["corpus"] == name)
            .ok_or("missing required development corpus")?;
        if inventory["families"] != families
            || inventory["cases"] != cases
            || inventory["decisions"] != decisions
        {
            return Err("development inventory differs from required counts".into());
        }
        for assignment in ASSIGNMENTS {
            let rows = report["rows"]
                .as_array()
                .ok_or("missing development rows")?
                .iter()
                .filter(|row| {
                    row["corpus"] == name
                        && row["model"] == CANDIDATE
                        && row["assignment"] == assignment
                })
                .collect::<Vec<_>>();
            if rows.len() != 1 || rows[0]["counts"]["fields"] != decisions {
                return Err("missing, duplicate or incomplete development evidence".into());
            }
            numerical(&rows[0]["counts"])?;
            let variants: &[&str] = if name == "original_development" {
                &VARIANTS
            } else if name == "stage3_extension_without_hints" {
                &["base"]
            } else {
                &["base", "reordered", "null_heavy"]
            };
            slices(rows[0], variants, false)?;
        }
    }
    Ok(())
}
fn current_development(report: &Value) -> Result<(), String> {
    for (path, key) in [
        ("Cargo.toml", "manifest_sha256"),
        ("Cargo.lock", "cargo_lock_sha256"),
    ] {
        let text =
            String::from_utf8(read(&root().join(path))?).map_err(|error| error.to_string())?;
        if report["metadata"][key] != digest(text.replace("\r\n", "\n").as_bytes()) {
            return Err("development manifest/dependency identity is stale".into());
        }
    }
    for name in [
        "readiness-protocol.json",
        "precision-protocol.json",
        "context-qualification-protocol.json",
    ] {
        let text = String::from_utf8(read(&root().join("evaluation").join(name))?)
            .map_err(|error| error.to_string())?;
        if report["contract"]["protocol_sha256"][name]
            != digest(text.replace("\r\n", "\n").as_bytes())
        {
            return Err("development protocol identity is stale".into());
        }
    }
    for (directory, key) in [
        ("src", "engine_source_sha256"),
        ("evaluation/src", "evaluator_source_sha256"),
    ] {
        let mut hashes = BTreeMap::new();
        for entry in fs::read_dir(root().join(directory)).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.extension().is_some_and(|extension| extension == "rs") {
                let text = String::from_utf8(read(&path)?).map_err(|error| error.to_string())?;
                hashes.insert(
                    path.file_name().unwrap().to_string_lossy().into_owned(),
                    digest(text.replace("\r\n", "\n").as_bytes()),
                );
            }
        }
        if report["metadata"][key] != json!(hashes) {
            return Err("development evidence is stale for current engine/evaluator source".into());
        }
    }
    Ok(())
}

fn current_development_inputs(report: &Value, binary: &Path) -> Result<(), String> {
    let output = command(
        binary.to_str().ok_or("invalid evaluator path")?,
        &["--quality-development-identity".into()],
        &root(),
    )?;
    let expected: Value =
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    for key in ["observable_inputs_sha256", "scorer_inputs_sha256"] {
        let expected = expected[key]
            .as_object()
            .ok_or("missing canonical development identity")?;
        if expected
            .iter()
            .any(|(name, value)| report["contract"][key][name] != *value)
        {
            return Err(
                "development observations/labels/slices differ from frozen evaluator inputs".into(),
            );
        }
    }
    Ok(())
}

fn current_candidate_configuration(contract: &Value, binary: &Path) -> Result<(), String> {
    let output = command(
        binary.to_str().ok_or("invalid frozen evaluator path")?,
        &["--quality-configuration".into()],
        &root(),
    )?;
    let actual: Value =
        serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
    if actual != contract["configuration"] {
        return Err(
            "protocol configuration differs from freshly built public runtime preset".into(),
        );
    }
    Ok(())
}

fn normalized_corpus_digest(path: &Path) -> Result<String, String> {
    let text = String::from_utf8(read(path)?).map_err(|error| error.to_string())?;
    Ok(digest(text.replace("\r\n", "\n").as_bytes()))
}

fn verified_development(directory: &Path) -> Result<PathBuf, String> {
    let run_path = directory
        .parent()
        .ok_or("missing guarded development parent")?
        .join("run.json");
    let run = verified::load_record(&run_path)?;
    if run["protocol"] != "fieldkin-native-verified-v1"
        || run["status"] != "success"
        || run["partition"] != "quality-development"
        || run["reserved_holdouts_scored"] != false
        || run["other_reserved_holdouts_scored"] != false
    {
        return Err("guarded native development evidence is required".into());
    }
    let build_dir = Path::new(
        run["build_dir"]
            .as_str()
            .ok_or("missing development executable build identity")?,
    );
    let (build, _) = verified::verified_build(&root(), build_dir)?;
    if run["binary_sha256"] != build["binary_sha256"]
        || run["build_record_sha256"] != digest(&read(&build_dir.join("build.json"))?)
        || run["input_sha256"] != build["context_before"]["input_sha256"]
    {
        return Err("development executable/source/input identity mismatch".into());
    }
    for name in [
        "report.json",
        "report.md",
        "predictions.jsonl",
        "changes.jsonl",
    ] {
        if run["artifact_sha256"][name] != digest(&read(&directory.join(name))?) {
            return Err("guarded development artifacts changed".into());
        }
    }
    Ok(run_path)
}

pub fn freeze(args: Vec<String>) -> Result<(), String> {
    let options = options(
        args,
        &[
            "--protocol",
            "--corpus",
            "--provenance",
            "--development",
            "--build-dir",
            "--toolchain",
        ],
    )?;
    let directory = path(&options, "--build-dir")?;
    destination(&directory)?;
    if !directory.starts_with(root().join("target")) || directory.exists() {
        return Err("quality-freeze requires a fresh build below target".into());
    }
    let protocol_path = path(&options, "--protocol")?;
    let contract = load_json(&protocol_path)?;
    protocol(&contract)?;
    let development_path = path(&options, "--development")?;
    let development_report = load_json(&development_path.join("report.json"))?;
    development(&development_report, &contract)?;
    current_development(&development_report)?;
    let development_run = verified_development(&development_path)?;
    let mut files = BTreeMap::new();
    for (name, file) in [
        ("protocol", protocol_path),
        ("corpus", path(&options, "--corpus")?),
        ("provenance", path(&options, "--provenance")?),
        ("development_report", development_path.join("report.json")),
        (
            "development_predictions",
            development_path.join("predictions.jsonl"),
        ),
        ("development_run", development_run),
    ] {
        // Corpus labels remain opaque at freeze. Only raw bytes are hashed.
        files.insert(name, json!({"path":file,"sha256":digest(&read(&file)?)}));
    }
    let provenance = load_json(Path::new(
        files["provenance"]["path"]
            .as_str()
            .ok_or("invalid provenance path")?,
    ))?;
    validate_authorship(&provenance)?;
    let toolchain = options.get("--toolchain").map_or("stable", String::as_str);
    verified::build(vec![
        "--build-dir".into(),
        directory.to_string_lossy().into_owned(),
        "--toolchain".into(),
        toolchain.into(),
    ])?;
    let (build, binary) = verified::verified_build(&root(), &directory)?;
    current_development_inputs(&development_report, &binary)?;
    current_candidate_configuration(&contract, &binary)?;
    for file in files.values() {
        if digest(&read(Path::new(file["path"].as_str().unwrap()))?) != file["sha256"] {
            return Err("frozen external inputs changed during build".into());
        }
    }
    let corpus_lf_sha = normalized_corpus_digest(Path::new(
        files["corpus"]["path"]
            .as_str()
            .ok_or("invalid corpus path")?,
    ))?;
    if corpus_lf_sha == "8baabaf30a7232a8e1595bf38edc1e33a53505ef8e23680e8aeed6e50cf73a6e" {
        return Err("historical examined qualification cannot be claimed fresh".into());
    }
    verified::record(
        &directory.join("quality-freeze.json"),
        json!({"protocol":VERSION,"candidate":CANDIDATE,"configuration":contract["configuration"],"files":files,"corpus_lf_sha256":corpus_lf_sha,
        "build_record_sha256":digest(&read(&directory.join("build.json"))?),"binary_sha256":build["binary_sha256"],"frozen_unix_seconds":now(),"holdout_scored":false}),
    )?;
    println!("Public candidate, evaluator, protocol, development evidence and opaque fresh inputs frozen.");
    Ok(())
}
fn validate_authorship(value: &Value) -> Result<(), String> {
    for key in [
        "author_isolated_from_implementation",
        "author_isolated_from_results",
        "author_isolated_from_existing_answers",
    ] {
        if value[key] != true {
            return Err("required author isolation is unavailable or unrecorded".into());
        }
    }
    if !value["isolation_limitations"]
        .as_str()
        .is_some_and(|text| !text.trim().is_empty())
    {
        return Err("honest authorship isolation limitations are required".into());
    }
    Ok(())
}
fn frozen(directory: &Path) -> Result<(Value, Value, PathBuf), String> {
    let freeze = verified::load_record(&directory.join("quality-freeze.json"))?;
    let (build, binary) = verified::verified_build(&root(), directory)?;
    if freeze["protocol"] != VERSION
        || freeze["candidate"] != CANDIDATE
        || freeze["holdout_scored"] != false
        || freeze["build_record_sha256"] != digest(&read(&directory.join("build.json"))?)
        || freeze["binary_sha256"] != build["binary_sha256"]
    {
        return Err("freeze/build identity mismatch".into());
    }
    for file in freeze["files"]
        .as_object()
        .ok_or("missing frozen inputs")?
        .values()
    {
        if digest(&read(Path::new(
            file["path"].as_str().ok_or("invalid frozen file path")?,
        ))?) != file["sha256"]
        {
            return Err("stale or mismatched frozen external input".into());
        }
    }
    let contract = load_json(Path::new(
        freeze["files"]["protocol"]["path"]
            .as_str()
            .ok_or("missing protocol")?,
    ))?;
    protocol(&contract)?;
    let report = load_json(Path::new(
        freeze["files"]["development_report"]["path"]
            .as_str()
            .ok_or("missing development report")?,
    ))?;
    development(&report, &contract)?;
    current_development(&report)?;
    current_development_inputs(&report, &binary)?;
    current_candidate_configuration(&contract, &binary)?;
    verified_development(
        Path::new(
            freeze["files"]["development_report"]["path"]
                .as_str()
                .ok_or("missing development path")?,
        )
        .parent()
        .ok_or("missing development artifacts")?,
    )?;
    Ok((freeze, build, binary))
}
pub fn qualify(args: Vec<String>) -> Result<(), String> {
    let options = options(
        args,
        &["--build-dir", "--output", "--acknowledge-new-holdout"],
    )?;
    if options.get("--acknowledge-new-holdout").map(String::as_str) != Some("true") {
        return Err("quality-qualify requires explicit --acknowledge-new-holdout".into());
    }
    let directory = path(&options, "--build-dir")?;
    let output = path(&options, "--output")?;
    destination(&directory)?;
    destination(&output)?;
    if output.exists() || output.starts_with(&directory) || directory.starts_with(&output) {
        return Err("fresh disjoint qualification output is required".into());
    }
    let (freeze, build, binary) = frozen(&directory)?;
    let corpus_sha = freeze["corpus_lf_sha256"]
        .as_str()
        .ok_or("missing corpus identity")?;
    let attempt = root()
        .join("target/quality-attempts")
        .join(format!("{corpus_sha}.started.json"));
    // A start record survives failed/interrupted scoring. A second attempt is
    // examined evidence and cannot silently become another fresh qualification.
    let start = json!({"protocol":VERSION,"authorized":true,"first_attempt":true,"output":output,"corpus_sha256":corpus_sha,"freeze_sha256":digest(&read(&directory.join("quality-freeze.json"))?),"build_record_sha256":freeze["build_record_sha256"],"binary_sha256":freeze["binary_sha256"],"started_unix_seconds":now()});
    write_new(&attempt, &start)?;
    fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    let mut arguments = vec![
        "--quality-qualification".into(),
        "--acknowledge-new-holdout".into(),
        "--output".into(),
        output.join("artifacts").to_string_lossy().into_owned(),
        "--frozen-binary-sha256".into(),
        build["binary_sha256"].as_str().unwrap().into(),
        "--frozen-build-record-sha256".into(),
        freeze["build_record_sha256"].as_str().unwrap().into(),
    ];
    for (name, flag) in [
        ("protocol", "--protocol"),
        ("corpus", "--corpus"),
        ("provenance", "--provenance"),
    ] {
        arguments.extend([
            flag.into(),
            freeze["files"][name]["path"]
                .as_str()
                .ok_or("missing frozen input")?
                .into(),
        ]);
    }
    command(
        binary.to_str().ok_or("invalid binary path")?,
        &arguments,
        &root(),
    )?;
    if frozen(&directory)?.0 != freeze {
        return Err("freeze changed during qualification".into());
    }
    let mut artifacts = BTreeMap::new();
    for name in [
        "qualification.json",
        "qualification.md",
        "predictions.jsonl",
    ] {
        artifacts.insert(name, digest(&read(&output.join("artifacts").join(name))?));
    }
    if fs::read_dir(output.join("artifacts"))
        .map_err(|error| error.to_string())?
        .count()
        != 3
    {
        return Err("unexpected qualification artifacts".into());
    }
    verified::record(
        &output.join("run.json"),
        json!({"protocol":VERSION,"status":"success","partition":"new-reserved-synthetic-holdout","authorized":true,"first_attempt":true,
        "attempt_path":attempt,"attempt_sha256":digest(&read(&attempt)?),"freeze_sha256":digest(&read(&directory.join("quality-freeze.json"))?),"build_record_sha256":freeze["build_record_sha256"],"binary_sha256":build["binary_sha256"],
        "configuration":freeze["configuration"],"files":freeze["files"],"artifact_sha256":artifacts,"command":arguments,"completed_unix_seconds":now(),"other_reserved_holdouts_scored":false}),
    )?;
    println!("Authorized first qualification result preserved; run release-acceptance for numerical acceptance.");
    Ok(())
}

pub fn accept_development(args: Vec<String>) -> Result<(), String> {
    let options = options(args, &["--protocol", "--development"])?;
    let directory = path(&options, "--development")?;
    let run_path = verified_development(&directory)?;
    let run = verified::load_record(&run_path)?;
    let (_, binary) = verified::verified_build(
        &root(),
        Path::new(
            run["build_dir"]
                .as_str()
                .ok_or("missing development build")?,
        ),
    )?;
    let contract = if options.contains_key("--protocol") {
        load_json(&path(&options, "--protocol")?)?
    } else {
        let output = command(
            binary.to_str().ok_or("invalid evaluator path")?,
            &["--quality-configuration".into()],
            &root(),
        )?;
        let configuration: Value =
            serde_json::from_slice(&output.stdout).map_err(|error| error.to_string())?;
        json!({"version":VERSION,"candidate":CANDIDATE,"configuration":configuration,"models":MODELS,"assignments":ASSIGNMENTS,"variants":VARIANTS,"quality_targets":targets(),"inventory":{"families":12,"cases":60,"decisions":360}})
    };
    protocol(&contract)?;
    let report = load_json(&path(&options, "--development")?.join("report.json"))?;
    development(&report, &contract)?;
    current_development(&report)?;
    current_development_inputs(&report, &binary)?;
    current_candidate_configuration(&contract, &binary)?;
    println!("Development numerical targets met separately for all three corpora and both assignments; fresh qualification is still required.");
    Ok(())
}

fn prediction_counts(record: &Value) -> Result<Value, String> {
    const KEYS: [&str; 22] = [
        "fields",
        "unique_fields",
        "no_match_fields",
        "ambiguous_fields",
        "proposals",
        "correct_proposals",
        "wrong_unique_proposals",
        "no_match_proposals",
        "ambiguous_proposals",
        "abstentions",
        "unique_proposals",
        "expected_abstentions",
        "correct_abstentions",
        "candidate_relevant",
        "candidate_hits",
        "unique_candidate_relevant",
        "unique_candidate_hits",
        "ambiguous_candidate_relevant",
        "ambiguous_candidate_hits",
        "candidate_any_fields",
        "candidate_any_hits",
        "feasible_unique_matches",
    ];
    let mut counts = KEYS
        .iter()
        .map(|key| ((*key).to_owned(), 0u64))
        .collect::<BTreeMap<_, _>>();
    let mut unique_targets = BTreeSet::new();
    let mut selected_targets = BTreeSet::new();
    let one_to_one = record["assignment"] == "one_to_one";
    let fields = record["fields"]
        .as_array()
        .ok_or("missing prediction fields")?;
    let mut sources = BTreeSet::new();
    for field in fields {
        if !sources.insert(field["source"].as_str().ok_or("missing predicted source")?) {
            return Err("duplicate predicted source".into());
        }
        let targets = field["gold_targets"]
            .as_array()
            .ok_or("missing scorer-only target annotations")?
            .iter()
            .map(|value| value.as_str().ok_or("invalid gold target"))
            .collect::<Result<BTreeSet<_>, _>>()?;
        if targets.len() != field["gold_targets"].as_array().unwrap().len() {
            return Err("duplicate scorer targets".into());
        }
        let ranked = field["ranked_targets"]
            .as_array()
            .ok_or("missing actual retained candidates")?;
        if ranked.len() > 5 {
            return Err("candidate recall must use first five".into());
        }
        let ranked = ranked
            .iter()
            .map(|value| value.as_str().ok_or("invalid ranked target"))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let selected = if field["selected"].is_null() {
            None
        } else {
            Some(
                field["selected"]
                    .as_str()
                    .ok_or("invalid selected target")?,
            )
        };
        let proposed = selected.is_some();
        if one_to_one && selected.is_some_and(|target| !selected_targets.insert(target)) {
            return Err("duplicate global selected target".into());
        }
        if field["decision"] == "Confirmed" || (proposed && field["decision"] != "Proposed") {
            return Err("caller-confirmed or inconsistent autonomous decision".into());
        }
        counts.entry("fields".into()).and_modify(|n| *n += 1);
        counts
            .entry("proposals".into())
            .and_modify(|n| *n += u64::from(proposed));
        counts
            .entry("abstentions".into())
            .and_modify(|n| *n += u64::from(!proposed));
        let relevant = targets.len() as u64;
        let hits = targets.intersection(&ranked).count() as u64;
        match field["label_kind"]
            .as_str()
            .ok_or("missing scorer label kind")?
        {
            "match" if relevant == 1 => {
                unique_targets.extend(targets.iter().copied());
                let correct = selected.is_some_and(|target| targets.contains(target));
                for (key, value) in [
                    ("unique_fields", 1),
                    ("unique_proposals", u64::from(proposed)),
                    ("correct_proposals", u64::from(correct)),
                    ("wrong_unique_proposals", u64::from(proposed && !correct)),
                    ("unique_candidate_relevant", relevant),
                    ("unique_candidate_hits", hits),
                ] {
                    counts.entry(key.into()).and_modify(|n| *n += value);
                }
            }
            "no_match" if relevant == 0 => {
                for (key, value) in [
                    ("no_match_fields", 1),
                    ("no_match_proposals", u64::from(proposed)),
                    ("expected_abstentions", 1),
                    ("correct_abstentions", u64::from(!proposed)),
                ] {
                    counts.entry(key.into()).and_modify(|n| *n += value);
                }
            }
            "ambiguous" if relevant >= 2 => {
                for (key, value) in [
                    ("ambiguous_fields", 1),
                    ("ambiguous_proposals", u64::from(proposed)),
                    ("expected_abstentions", 1),
                    ("correct_abstentions", u64::from(!proposed)),
                    ("ambiguous_candidate_relevant", relevant),
                    ("ambiguous_candidate_hits", hits),
                ] {
                    counts.entry(key.into()).and_modify(|n| *n += value);
                }
            }
            _ => return Err("invalid scorer label cardinality".into()),
        }
        for (key, value) in [
            ("candidate_relevant", relevant),
            ("candidate_hits", hits),
            ("candidate_any_fields", u64::from(relevant > 0)),
            ("candidate_any_hits", u64::from(hits > 0)),
        ] {
            counts.entry(key.into()).and_modify(|n| *n += value);
        }
    }
    counts.insert(
        "feasible_unique_matches".into(),
        if one_to_one {
            unique_targets.len() as u64
        } else {
            counts["unique_fields"]
        },
    );
    serde_json::to_value(counts).map_err(|error| error.to_string())
}

fn qualification(report: &Value, contract: &Value, predictions: &str) -> Result<(), String> {
    protocol(contract)?;
    if report["metadata"]["protocol"] != *contract
        || report["metadata"]["configuration"] != contract["configuration"]
        || report["inventory"]["partition"] != "all_holdout"
        || report["production_accuracy_certified"] != false
        || report["metadata"]["caller_hints_used"] != false
        || report["metadata"]["caller_confirmations_used"] != false
        || report["metadata"]["new_reserved_holdout_scored"] != true
        || report["metadata"]["other_reserved_holdouts_scored"] != false
    {
        return Err("qualification contract/partition/automatic-input identity differs".into());
    }
    for key in ["families", "cases", "decisions"] {
        if report["inventory"][key] != contract["inventory"][key] {
            return Err("qualification inventory mismatch".into());
        }
    }
    let cases = number(&contract["inventory"], "cases")?;
    let families = number(&contract["inventory"], "families")?;
    let decisions = number(&contract["inventory"], "decisions")?;
    if cases != families.checked_mul(5).ok_or("inventory overflow")?
        || decisions != cases.checked_mul(6).ok_or("inventory overflow")?
    {
        return Err("required qualification variants/source inventory incomplete".into());
    }
    let rows = report["rows"]
        .as_array()
        .ok_or("missing qualification rows")?;
    if rows.len() != 8 {
        return Err("qualification requires every fixed model in both assignments".into());
    }
    let mut seen = BTreeSet::new();
    let mut per_case = BTreeMap::<(String, String), Vec<Value>>::new();
    let mut case_ids = BTreeMap::<(String, String), BTreeSet<String>>::new();
    for line in predictions.lines() {
        let record: Value = serde_json::from_str(line).map_err(|error| error.to_string())?;
        let model = record["model"].as_str().ok_or("invalid prediction model")?;
        let assignment = record["assignment"]
            .as_str()
            .ok_or("invalid prediction assignment")?;
        let id = record["case"].as_str().ok_or("missing prediction case")?;
        let variant = record["variant"]
            .as_str()
            .ok_or("missing prediction variant")?;
        let family = record["family"]
            .as_str()
            .ok_or("missing prediction family")?;
        if !MODELS.contains(&model)
            || !ASSIGNMENTS.contains(&assignment)
            || !VARIANTS.contains(&variant)
            || id != format!("{family}/{variant}")
            || record["counts"]["fields"] != 6
            || !case_ids
                .entry((model.into(), assignment.into()))
                .or_default()
                .insert(id.into())
        {
            return Err(
                "missing, duplicate or unexpected qualification prediction identity".into(),
            );
        }
        let fields = record["fields"]
            .as_array()
            .ok_or("missing prediction fields")?;
        let sources = fields
            .iter()
            .map(|field| field["source"].as_str().ok_or("missing predicted source"))
            .collect::<Result<BTreeSet<_>, _>>()?;
        let proposals = fields
            .iter()
            .filter(|field| field["selected"].is_string())
            .count();
        if fields.len() != 6
            || sources.len() != 6
            || record["counts"]["proposals"] != proposals
            || fields.iter().any(|field| field["decision"] == "Confirmed")
        {
            return Err("prediction field inventory/automatic proposal counts mismatch".into());
        }
        if prediction_counts(&record)? != record["counts"] {
            return Err("scorer annotations and actual selections/rankings do not reconstruct exact prediction counts".into());
        }
        per_case
            .entry((model.into(), assignment.into()))
            .or_default()
            .push(record);
    }
    let mut reference_cases = None;
    for row in rows {
        let model = row["model"].as_str().ok_or("missing model")?;
        let assignment = row["assignment"].as_str().ok_or("missing assignment")?;
        let key = (model.to_owned(), assignment.to_owned());
        if !MODELS.contains(&model)
            || !ASSIGNMENTS.contains(&assignment)
            || !seen.insert(key.clone())
            || row["counts"]["fields"] != decisions
        {
            return Err("missing, duplicate or incomplete qualification model/mode".into());
        }
        consistent(&row["counts"])?;
        slices(row, &VARIANTS, true)?;
        if row["by_family"].as_object().unwrap().len() as u64 != families
            || row["by_variant"]
                .as_object()
                .unwrap()
                .values()
                .any(|counts| counts["fields"] != decisions / 5)
        {
            return Err("families or required variant decision counts incomplete".into());
        }
        let records = per_case
            .get(&key)
            .ok_or("missing predictions for required model/mode")?;
        let ids = case_ids.get(&key).unwrap();
        if records.len() as u64 != cases
            || sum_counts(records.iter().map(|record| &record["counts"]))? != row["counts"]
        {
            return Err("raw prediction counts do not exactly reconstruct report".into());
        }
        if reference_cases
            .as_ref()
            .is_some_and(|reference| reference != ids)
        {
            return Err("models/modes use differing cases".into());
        }
        reference_cases = Some(ids.clone());
        for slice in ["family", "domain", "variant"] {
            let mut groups = BTreeMap::<String, Vec<&Value>>::new();
            for record in records {
                groups
                    .entry(
                        record[slice]
                            .as_str()
                            .ok_or("invalid slice identity")?
                            .into(),
                    )
                    .or_default()
                    .push(&record["counts"]);
            }
            let mut summed = BTreeMap::new();
            for (key, counts) in groups {
                summed.insert(key, sum_counts(counts.into_iter())?);
            }
            if json!(summed) != row[format!("by_{slice}")] {
                return Err("raw prediction slice counts mismatch".into());
            }
        }
        if model == CANDIDATE {
            numerical(&row["counts"])?;
        }
    }
    Ok(())
}

fn provenance(
    freeze: &Value,
    build: &Value,
    run: &Value,
    start: &Value,
    report: &Value,
    output: &Path,
) -> Result<(), String> {
    if run["protocol"] != VERSION
        || run["status"] != "success"
        || run["partition"] != "new-reserved-synthetic-holdout"
        || run["authorized"] != true
        || run["first_attempt"] != true
        || run["other_reserved_holdouts_scored"] != false
        || start["protocol"] != VERSION
        || start["authorized"] != true
        || start["first_attempt"] != true
        || start["output"] != json!(output)
        || run["configuration"] != freeze["configuration"]
        || run["files"] != freeze["files"]
        || run["build_record_sha256"] != freeze["build_record_sha256"]
        || start["build_record_sha256"] != freeze["build_record_sha256"]
        || run["binary_sha256"] != freeze["binary_sha256"]
        || start["binary_sha256"] != freeze["binary_sha256"]
        || build["binary_sha256"] != freeze["binary_sha256"]
        || start["corpus_sha256"] != freeze["corpus_lf_sha256"]
        || report["metadata"]["frozen_binary_sha256"] != freeze["binary_sha256"]
        || report["metadata"]["frozen_build_record_sha256"] != freeze["build_record_sha256"]
    {
        return Err(
            "authorized first-attempt candidate/configuration/executable provenance mismatch"
                .into(),
        );
    }
    for name in ["protocol", "corpus", "provenance"] {
        if report["metadata"][format!("{name}_sha256")] != freeze["files"][name]["sha256"] {
            return Err("qualification input identity mismatch".into());
        }
    }
    let completed = number(run, "completed_unix_seconds")?;
    let started = number(start, "started_unix_seconds")?;
    let frozen = number(freeze, "frozen_unix_seconds")?;
    if frozen < number(build, "completed_unix_seconds")? || started < frozen || completed < started
    {
        return Err("qualification was not frozen before first scoring".into());
    }
    Ok(())
}

pub fn accept(args: Vec<String>) -> Result<(), String> {
    let options = options(args, &["--build-dir", "--output"])?;
    let directory = path(&options, "--build-dir")?;
    let output = path(&options, "--output")?;
    destination(&directory)?;
    destination(&output)?;
    let (freeze, build, _) = frozen(&directory)?;
    let run = verified::load_record(&output.join("run.json"))?;
    let attempt = PathBuf::from(
        run["attempt_path"]
            .as_str()
            .ok_or("missing first-attempt record")?,
    );
    let expected = root().join("target/quality-attempts").join(format!(
        "{}.started.json",
        freeze["corpus_lf_sha256"]
            .as_str()
            .ok_or("missing corpus identity")?
    ));
    if attempt != expected || digest(&read(&attempt)?) != run["attempt_sha256"] {
        return Err("missing or mismatched first-attempt record".into());
    }
    let start = load_json(&attempt)?;
    let freeze_sha = digest(&read(&directory.join("quality-freeze.json"))?);
    if run["freeze_sha256"] != freeze_sha || start["freeze_sha256"] != freeze_sha {
        return Err("stale or mismatched frozen candidate".into());
    }
    for name in [
        "qualification.json",
        "qualification.md",
        "predictions.jsonl",
    ] {
        if digest(&read(&output.join("artifacts").join(name))?) != run["artifact_sha256"][name] {
            return Err("missing or mismatched qualification artifact".into());
        }
    }
    let report = load_json(&output.join("artifacts/qualification.json"))?;
    let contract = load_json(Path::new(
        freeze["files"]["protocol"]["path"]
            .as_str()
            .ok_or("missing frozen protocol")?,
    ))?;
    provenance(&freeze, &build, &run, &start, &report, &output)?;
    let authorship = load_json(Path::new(
        freeze["files"]["provenance"]["path"]
            .as_str()
            .ok_or("missing authorship")?,
    ))?;
    validate_authorship(&authorship)?;
    qualification(
        &report,
        &contract,
        &String::from_utf8(read(&output.join("artifacts/predictions.jsonl"))?)
            .map_err(|error| error.to_string())?,
    )?;
    if frozen(&directory)?.0 != freeze {
        return Err(
            "frozen source/input identities changed while checking release acceptance".into(),
        );
    }
    println!("Release quality acceptance passed: authorized frozen first qualification, every required corpus/mode/variant and exact 95/60/90/nonempty counts. Engineering/platform requirements remain separate.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn counts() -> Value {
        json!({"fields":200,"unique_fields":100,"no_match_fields":50,"ambiguous_fields":50,"proposals":60,"correct_proposals":60,"wrong_unique_proposals":0,"no_match_proposals":0,"ambiguous_proposals":0,"abstentions":140,"unique_proposals":60,"expected_abstentions":100,"correct_abstentions":100,"candidate_relevant":200,"candidate_hits":180,"unique_candidate_relevant":100,"unique_candidate_hits":100,"ambiguous_candidate_relevant":100,"ambiguous_candidate_hits":80,"candidate_any_fields":150,"candidate_any_hits":140,"feasible_unique_matches":100})
    }
    #[test]
    fn numerical_gate_uses_exact_counts_and_keeps_coverage_definition() {
        let valid = counts();
        assert!(numerical(&valid).is_ok());
        let mut wrong = valid.clone();
        wrong["correct_proposals"] = json!(57);
        wrong["wrong_unique_proposals"] = json!(3);
        assert!(numerical(&wrong).is_ok());
        wrong["correct_proposals"] = json!(56);
        wrong["wrong_unique_proposals"] = json!(4);
        wrong["precision"] = json!(0.95);
        assert!(numerical(&wrong).is_err());
        let mut low = valid.clone();
        low["proposals"] = json!(59);
        low["unique_proposals"] = json!(59);
        low["correct_proposals"] = json!(59);
        low["abstentions"] = json!(141);
        assert!(numerical(&low).is_err());
        let mut recall = valid.clone();
        recall["candidate_hits"] = json!(179);
        recall["ambiguous_candidate_hits"] = json!(79);
        assert!(numerical(&recall).is_err());
        let mut empty = valid;
        for key in ["proposals", "correct_proposals", "unique_proposals"] {
            empty[key] = json!(0);
        }
        empty["abstentions"] = json!(200);
        assert!(numerical(&empty).is_err());
    }
    #[test]
    fn inconsistent_and_missing_counts_are_rejected() {
        let valid = counts();
        for key in valid.as_object().unwrap().keys() {
            let mut missing = valid.clone();
            missing.as_object_mut().unwrap().remove(key);
            assert!(numerical(&missing).is_err(), "{key}");
        }
        let mut inflated = valid;
        inflated["unique_proposals"] = json!(61);
        assert!(numerical(&inflated).is_err());
    }
    #[test]
    fn missing_variants_and_mismatched_sums_are_rejected() {
        let c = counts();
        let row =
            json!({"counts":c,"by_family":{"f":c},"by_domain":{"d":c},"by_variant":{"base":c}});
        assert!(slices(&row, &VARIANTS, false).is_err());
        assert!(slices(&row, &["base"], false).is_ok());
        let mut mismatch = row;
        mismatch["by_variant"]["base"]["proposals"] = json!(61);
        assert!(slices(&mismatch, &["base"], false).is_err());
    }
    #[test]
    fn qualification_rejects_missing_modes_corpora_and_stale_configuration() {
        let contract = json!({"version":VERSION,"candidate":CANDIDATE,"models":MODELS,"assignments":ASSIGNMENTS,"variants":VARIANTS,"quality_targets":targets(),"configuration":{},"inventory":{"families":12,"cases":60,"decisions":360}});
        assert!(protocol(&contract).is_ok());
        assert!(development(&json!({}), &contract).is_err());
        assert!(qualification(&json!({}), &contract, "").is_err());
        let mut weaker = contract;
        weaker["quality_targets"]["unique_coverage"] = json!(0.59);
        assert!(protocol(&weaker).is_err());
    }
    #[test]
    fn missing_fresh_author_isolation_and_arbitrary_routes_are_rejected() {
        assert!(validate_authorship(&json!({})).is_err());
        for arg in ["--answers", "--holdout", "--min-score", "--check"] {
            assert!(options(vec![arg.into()], &["--build-dir", "--output"]).is_err());
        }
    }

    fn fixture() -> (Value, Value, String) {
        let contract = json!({"version":VERSION,"candidate":CANDIDATE,"models":MODELS,"assignments":ASSIGNMENTS,"variants":VARIANTS,"quality_targets":targets(),"configuration":{},"inventory":{"families":12,"cases":60,"decisions":360}});
        let mut records = Vec::new();
        let mut rows = Vec::new();
        for model in MODELS {
            for assignment in ASSIGNMENTS {
                let mut group = Vec::new();
                for index in 0..12 {
                    for variant in VARIANTS {
                        let family = format!("f{index}");
                        let fields=(0..6).map(|source|{
                    let (kind,gold,selected)=if source<3 {("match",vec![format!("t{source}")],Some(format!("t{source}")))}else if source<5 {("no_match",Vec::new(),None)}else{("ambiguous",vec!["a".into(),"b".into()],None)};
                    json!({"source":format!("s{source}"),"selected":selected,"decision":if selected.is_some(){"Proposed"}else{"InsufficientEvidence"},"label_kind":kind,"ranked_targets":gold,"gold_targets":gold})
                }).collect::<Vec<_>>();
                        let mut record = json!({"case":format!("{family}/{variant}"),"family":family,"domain":DOMAINS[index/3],"variant":variant,"model":model,"assignment":assignment,"fields":fields});
                        record["counts"] = prediction_counts(&record).unwrap();
                        group.push(record);
                    }
                }
                let total = sum_counts(group.iter().map(|record| &record["counts"])).unwrap();
                let mut row = json!({"model":model,"assignment":assignment,"counts":total});
                for slice in ["family", "domain", "variant"] {
                    let mut groups = BTreeMap::<String, Vec<&Value>>::new();
                    for record in &group {
                        groups
                            .entry(record[slice].as_str().unwrap().into())
                            .or_default()
                            .push(&record["counts"]);
                    }
                    row[format!("by_{slice}")] = json!(groups
                        .into_iter()
                        .map(|(key, counts)| (key, sum_counts(counts.into_iter()).unwrap()))
                        .collect::<BTreeMap<_, _>>());
                }
                rows.push(row);
                records.extend(group);
            }
        }
        let predictions = records
            .iter()
            .map(|record| serde_json::to_string(record).unwrap() + "\n")
            .collect::<String>();
        let report = json!({"metadata":{"protocol":contract,"configuration":{},"caller_hints_used":false,"caller_confirmations_used":false,"new_reserved_holdout_scored":true,"other_reserved_holdouts_scored":false},"inventory":{"families":12,"cases":60,"decisions":360,"partition":"all_holdout"},"rows":rows,"production_accuracy_certified":false});
        (report, contract, predictions)
    }
    #[test]
    fn qualification_rejects_each_incomplete_or_inconsistent_record() {
        let (report, contract, predictions) = fixture();
        qualification(&report, &contract, &predictions).unwrap();
        let mut missing = report.clone();
        missing["rows"].as_array_mut().unwrap().pop();
        assert!(qualification(&missing, &contract, &predictions).is_err());
        let mut duplicate = report.clone();
        duplicate["rows"][1] = duplicate["rows"][0].clone();
        assert!(qualification(&duplicate, &contract, &predictions).is_err());
        let mut variants = report.clone();
        variants["rows"][0]["by_variant"]
            .as_object_mut()
            .unwrap()
            .remove("without_samples");
        assert!(qualification(&variants, &contract, &predictions).is_err());
        let mut counts = report.clone();
        counts["rows"][0]["counts"]["correct_proposals"] = json!(179);
        assert!(qualification(&counts, &contract, &predictions).is_err());
        let mut config = report.clone();
        config["metadata"]["configuration"] = json!({"other":true});
        assert!(qualification(&config, &contract, &predictions).is_err());
        let changed = predictions.replacen("\"selected\":\"t0\"", "\"selected\":\"wrong\"", 1);
        assert!(qualification(&report, &contract, &changed).is_err());
        let changed = predictions.lines().skip(1).collect::<Vec<_>>().join("\n");
        assert!(qualification(&report, &contract, &changed).is_err());
        assert!(qualification(&report, &contract, "").is_err());
    }

    #[test]
    fn aggregate_coverage_allows_missing_sample_variant_abstention() {
        let (mut report, contract, predictions) = fixture();
        let mut records = predictions
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .collect::<Vec<_>>();
        for record in &mut records {
            if record["variant"] == "without_samples" {
                for field in record["fields"].as_array_mut().unwrap() {
                    field["selected"] = Value::Null;
                    field["decision"] = json!("InsufficientEvidence");
                }
                record["counts"] = prediction_counts(record).unwrap();
            }
        }
        for row in report["rows"].as_array_mut().unwrap() {
            let group = records
                .iter()
                .filter(|record| {
                    record["model"] == row["model"] && record["assignment"] == row["assignment"]
                })
                .collect::<Vec<_>>();
            row["counts"] = sum_counts(group.iter().map(|record| &record["counts"])).unwrap();
            for slice in ["family", "domain", "variant"] {
                let mut groups = BTreeMap::<String, Vec<&Value>>::new();
                for record in &group {
                    groups
                        .entry(record[slice].as_str().unwrap().into())
                        .or_default()
                        .push(&record["counts"]);
                }
                row[format!("by_{slice}")] = json!(groups
                    .into_iter()
                    .map(|(key, counts)| (key, sum_counts(counts.into_iter()).unwrap()))
                    .collect::<BTreeMap<_, _>>());
            }
        }
        let predictions = records
            .iter()
            .map(|record| serde_json::to_string(record).unwrap() + "\n")
            .collect::<String>();
        qualification(&report, &contract, &predictions).unwrap();
        assert_eq!(
            report["rows"][0]["by_variant"]["without_samples"]["unique_proposals"],
            0
        );
    }
    #[test]
    fn provenance_rejects_missing_stale_and_mismatched_freeze_and_first_attempt() {
        let files =
            json!({"protocol":{"sha256":"p"},"corpus":{"sha256":"c"},"provenance":{"sha256":"a"}});
        let freeze = json!({"configuration":{},"files":files,"corpus_lf_sha256":"lf","binary_sha256":"b","build_record_sha256":"build","frozen_unix_seconds":20});
        let build = json!({"binary_sha256":"b","completed_unix_seconds":10});
        let run = json!({"protocol":VERSION,"status":"success","partition":"new-reserved-synthetic-holdout","authorized":true,"first_attempt":true,"other_reserved_holdouts_scored":false,"configuration":{},"files":files,"binary_sha256":"b","build_record_sha256":"build","completed_unix_seconds":40});
        let start = json!({"protocol":VERSION,"authorized":true,"first_attempt":true,"output":"target/result","corpus_sha256":"lf","binary_sha256":"b","build_record_sha256":"build","started_unix_seconds":30});
        let report = json!({"metadata":{"frozen_binary_sha256":"b","frozen_build_record_sha256":"build","protocol_sha256":"p","corpus_sha256":"c","provenance_sha256":"a"}});
        let output = Path::new("target/result");
        provenance(&freeze, &build, &run, &start, &report, output).unwrap();
        for (key, value) in [
            ("authorized", json!(false)),
            ("first_attempt", json!(false)),
            ("binary_sha256", json!("different")),
            ("configuration", json!({"other":true})),
            ("build_record_sha256", json!("other")),
            ("completed_unix_seconds", json!(19)),
        ] {
            let mut wrong = run.clone();
            wrong[key] = value;
            assert!(
                provenance(&freeze, &build, &wrong, &start, &report, output).is_err(),
                "{key}"
            );
        }
        let mut early = start.clone();
        early["started_unix_seconds"] = json!(19);
        assert!(provenance(&freeze, &build, &run, &early, &report, output).is_err());
        let mut missing = start.clone();
        missing.as_object_mut().unwrap().remove("first_attempt");
        assert!(provenance(&freeze, &build, &run, &missing, &report, output).is_err());
        let mut mismatched = report;
        mismatched["metadata"]["corpus_sha256"] = json!("other");
        assert!(provenance(&freeze, &build, &run, &start, &mismatched, output).is_err());
    }
}
