//! Prespecified opt-in corroboration experiment with a fresh holdout gate.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use fieldkin::{
    Config, Corroboration, Decision, FieldDiagnostic, FieldMatch, GlobalDiagnosticsConfig, Limits,
    MatchEngine, MatchReport, NameMatcher, SampleMatcher, SampleReliability, SemanticHints,
    TypeMatcher, WeightedMatcher,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::metrics::{score_case, Counts};
use crate::model::{Case, Label, LabelKind};
use crate::{corpus, corrective_corpus};

const PROTOCOL: &str = include_str!("../corrective-protocol.json");
const MODELS: [&str; 4] = ["combined", "sample_gate", "corroborated", "name_only"];
const MODES: [&str; 2] = ["independent", "one_to_one"];
const USAGE: &str = "Corrective evaluation: --output DIR [--check | --check-legacy-decisions | --acknowledge-holdout]\n\
    --check compares current reports exactly (apart from existing source-hash exceptions).\n\
    --check-legacy-decisions checks historical development/regression reports after projecting only\n\
    InsufficientEvidence decisions to BelowThreshold and removing that new diagnostic.\n\
    This explicit compatibility mode preserves historical artifacts and never scores fresh holdout.\n\
    --acknowledge-holdout requires the recorded frozen experiment; it cannot accompany a check.";
const FROZEN_PATHS: [&str; 11] = [
    "src",
    "evaluation/src",
    "evaluation/corpus",
    "Cargo.toml",
    "evaluation/Cargo.toml",
    "Cargo.lock",
    "evaluation/protocol.json",
    "evaluation/stage3-protocol.json",
    "evaluation/release-protocol.json",
    "evaluation/corrective-protocol.json",
    "rust-toolchain.toml",
];

struct Options {
    output: PathBuf,
    check: bool,
    check_legacy_decisions: bool,
    acknowledge_holdout: bool,
}

impl Options {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut result = Self {
            output: root().join("target/fieldkin-corrective"),
            check: false,
            check_legacy_decisions: false,
            acknowledge_holdout: false,
        };
        let mut args = args.into_iter();
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--output" => {
                    result.output = args.next().ok_or("--output needs a directory")?.into()
                }
                "--check" => result.check = true,
                "--check-legacy-decisions" => result.check_legacy_decisions = true,
                "--acknowledge-holdout" => result.acknowledge_holdout = true,
                _ => return Err(format!("unknown corrective evaluation option\n{USAGE}")),
            }
        }
        if result.check && result.check_legacy_decisions {
            return Err("--check and --check-legacy-decisions are mutually exclusive".into());
        }
        if result.check && result.acknowledge_holdout {
            return Err("--check never scores fresh holdout and cannot acknowledge it".into());
        }
        if result.check_legacy_decisions && result.acknowledge_holdout {
            return Err(
                "--check-legacy-decisions never scores fresh holdout and cannot acknowledge it"
                    .into(),
            );
        }
        Ok(result)
    }

    fn checking(&self) -> bool {
        self.check || self.check_legacy_decisions
    }

    fn partitions(&self) -> &[&'static str] {
        if self.checking() {
            &["original_regression", "fresh_development"]
        } else if self.acknowledge_holdout {
            &["original_regression", "fresh_development", "fresh_holdout"]
        } else {
            &["original_regression", "fresh_development"]
        }
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn number(value: &Value, key: &str) -> Result<f64, String> {
    value[key]
        .as_f64()
        .filter(|v| v.is_finite())
        .ok_or_else(|| format!("invalid protocol number {key}"))
}

fn size(value: &Value, key: &str) -> Result<usize, String> {
    value[key]
        .as_u64()
        .and_then(|v| usize::try_from(v).ok())
        .ok_or_else(|| format!("invalid protocol integer {key}"))
}

fn boolean(value: &Value, key: &str) -> Result<bool, String> {
    value[key]
        .as_bool()
        .ok_or_else(|| format!("invalid protocol boolean {key}"))
}

fn protocol() -> Result<Value, String> {
    let value: Value = serde_json::from_str(PROTOCOL).map_err(|e| e.to_string())?;
    if value["models"] != json!(MODELS)
        || value["assignment_modes"] != json!(MODES)
        || value["threshold"] != 0.7
        || value["candidate_policy"]["corroborated"]
            != json!({"min_name_score":0.8,"min_sample_score":0.5})
        || value["candidate_policy"]["sample_gate"]
            != json!({"min_name_score":0.0,"min_sample_score":0.5})
        || value["combined"]["sample_reliability"] != "distinct"
    {
        return Err("corrective model settings differ from the prespecified experiment".into());
    }
    Ok(value)
}

fn validate_freeze_metadata(protocol: &Value) -> Result<&str, String> {
    let revision = protocol["feature_freeze_revision"]
        .as_str()
        .ok_or("fresh holdout requires a recorded feature-freeze revision")?;
    if revision.len() != 40 || !revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("feature-freeze revision must be a full commit SHA".into());
    }
    if protocol["default_decision"]
        .as_str()
        .is_none_or(|value| value.trim().is_empty())
    {
        return Err("fresh holdout requires a default decision recorded before scoring".into());
    }
    Ok(revision)
}

fn git_output(arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(root())
        .args(arguments)
        .output()
        .map_err(|_| "Git is unavailable; cannot verify the frozen experiment".to_owned())?;
    if !output.status.success() {
        return Err(
            "Git cannot read the frozen experiment commit; no fresh holdout was scored".into(),
        );
    }
    String::from_utf8(output.stdout)
        .map_err(|_| "frozen experiment contains a non-UTF-8 source or path".into())
}

fn frozen_content_hash(path: &str, text: &str) -> Result<String, String> {
    if path == "evaluation/corrective-protocol.json" {
        let mut value: Value = serde_json::from_str(text)
            .map_err(|_| "cannot parse frozen corrective protocol".to_owned())?;
        // The commit cannot name its own SHA. Every other experimental setting,
        // including the ex-ante default decision, must be identical.
        value
            .as_object_mut()
            .ok_or("corrective protocol must be an object")?
            .remove("feature_freeze_revision");
        Ok(hash(
            &serde_json::to_string(&value).map_err(|e| e.to_string())?,
        ))
    } else {
        Ok(hash(text))
    }
}

fn collect_current_files(path: &Path, files: &mut BTreeMap<String, String>) -> Result<(), String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(_) => return Err("cannot inspect frozen source path".into()),
    };
    if metadata.file_type().is_symlink() {
        return Err("symbolic links are unsupported in frozen experiment source paths".into());
    }
    if metadata.is_dir() {
        for entry in
            fs::read_dir(path).map_err(|_| "cannot list frozen source directory".to_owned())?
        {
            collect_current_files(
                &entry
                    .map_err(|_| "cannot inspect frozen source entry".to_owned())?
                    .path(),
                files,
            )?;
        }
    } else if metadata.is_file() {
        let relative = path
            .strip_prefix(root())
            .map_err(|_| "source path escaped the repository".to_owned())?
            .to_str()
            .ok_or("invalid source path")?
            .replace('\\', "/");
        let text = fs::read_to_string(path)
            .map_err(|_| format!("cannot read frozen source {relative}"))?;
        files.insert(relative.clone(), frozen_content_hash(&relative, &text)?);
    } else {
        return Err("unsupported file type in frozen experiment source paths".into());
    }
    Ok(())
}

fn compare_frozen_files(
    expected: &BTreeMap<String, String>,
    actual: &BTreeMap<String, String>,
) -> Result<(), String> {
    if expected.is_empty() {
        return Err("frozen experiment commit contains no required source files".into());
    }
    for (path, expected_hash) in expected {
        match actual.get(path) {
            None => return Err(format!("frozen experiment source was deleted: {path}")),
            Some(actual_hash) if actual_hash != expected_hash => {
                return Err(format!("frozen experiment source differs: {path}"))
            }
            Some(_) => {}
        }
    }
    if let Some(path) = actual.keys().find(|path| !expected.contains_key(*path)) {
        return Err(format!(
            "source was added after the experiment freeze: {path}"
        ));
    }
    Ok(())
}

fn require_freeze(protocol: &Value) -> Result<(), String> {
    let revision = validate_freeze_metadata(protocol)?;
    git_output(&["cat-file", "-e", &format!("{revision}^{{commit}}")])?;
    let mut arguments = vec!["ls-tree", "-r", "--name-only", revision, "--"];
    arguments.extend(FROZEN_PATHS);
    let mut expected = BTreeMap::new();
    for path in git_output(&arguments)?.lines() {
        let text = git_output(&["show", &format!("{revision}:{path}")])?;
        expected.insert(path.to_owned(), frozen_content_hash(path, &text)?);
    }
    let mut current = BTreeMap::new();
    for path in FROZEN_PATHS {
        collect_current_files(&root().join(path), &mut current)?;
    }
    compare_frozen_files(&expected, &current)
}

fn engine(protocol: &Value, model: &str, one_to_one: bool) -> Result<MatchEngine, String> {
    if !MODELS.contains(&model) {
        return Err("unknown corrective model".into());
    }
    let common = &protocol["common"];
    let global = &common["global_diagnostics"];
    let limits = &protocol["limits"];
    let base = &protocol[if model == "name_only" {
        "name_only"
    } else {
        "combined"
    }];
    let corroboration = if ["sample_gate", "corroborated"].contains(&model) {
        Some(Corroboration {
            min_name_score: number(&protocol["candidate_policy"][model], "min_name_score")?,
            min_sample_score: number(&protocol["candidate_policy"][model], "min_sample_score")?,
        })
    } else {
        None
    };
    let config = Config {
        name_conflicts: Vec::new(),
        min_score: number(protocol, "threshold")?,
        ambiguity_margin: number(common, "ambiguity_margin")?,
        max_candidates: size(common, "max_candidates")?,
        one_to_one,
        abstain_on_ambiguity: boolean(common, "abstain_on_ambiguity")?,
        reject_incompatible_types: boolean(base, "reject_incompatible_types")?,
        corroboration,
        contextual_evidence: None,
        global_diagnostics: GlobalDiagnosticsConfig {
            max_solves: size(global, "max_solves")?,
            max_work: size(global, "max_work")?,
            objective_margin: number(global, "objective_margin")?,
        },
        limits: Limits {
            max_fields: size(limits, "max_fields")?,
            max_pairs: size(limits, "max_pairs")?,
            max_signal_evaluations: size(limits, "max_signal_evaluations")?,
            max_explanation_bytes: size(limits, "max_explanation_bytes")?,
            max_name_bytes: size(limits, "max_name_bytes")?,
            max_samples_per_field: size(limits, "max_samples_per_field")?,
            max_sample_bytes: size(limits, "max_sample_bytes")?,
            max_total_sample_bytes: size(limits, "max_total_sample_bytes")?,
        },
    };
    let aliases = common["aliases"]
        .as_object()
        .ok_or("missing aliases")?
        .iter()
        .map(|(key, value)| {
            value
                .as_str()
                .map(|v| (key.clone(), v.to_owned()))
                .ok_or_else(|| "invalid alias".to_owned())
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let weights = &base["weights"];
    let mut matchers = vec![WeightedMatcher::new(
        number(weights, "name")?,
        NameMatcher { aliases },
    )];
    if model != "name_only" {
        matchers.push(WeightedMatcher::new(number(weights, "type")?, TypeMatcher));
        matchers.push(WeightedMatcher::new(
            number(weights, "samples")?,
            SampleMatcher {
                min_non_null: size(base, "sample_min_non_null")?,
                reliability: SampleReliability::Distinct,
            },
        ));
    }
    MatchEngine::with_matchers(config, matchers).map_err(|error| error.to_string())
}

fn cases(partition: &str) -> Result<Vec<Case>, String> {
    let result = if partition == "original_regression" {
        corpus::cases(&corpus::families()?)?
    } else {
        let split = match partition {
            "fresh_development" => "development",
            "fresh_holdout" => "holdout",
            _ => return Err("unknown corrective partition".into()),
        };
        // Never match a fresh field before filtering by the frozen family split.
        corrective_corpus::cases()?
            .into_iter()
            .filter(|case| case.split == split)
            .collect()
    };
    let expected = if partition == "original_regression" {
        200
    } else {
        36
    };
    if result.len() != expected {
        return Err("frozen corrective partition size changed".into());
    }
    Ok(result)
}

fn group_keys(case: &Case) -> Vec<(String, String)> {
    let mut keys = vec![
        ("all".into(), "all".into()),
        ("domain".into(), case.domain.clone()),
        ("variant".into(), case.variant.clone()),
        ("family".into(), case.family_id.clone()),
    ];
    for tag in case.tags.iter().collect::<BTreeSet<_>>() {
        keys.push(("scenario_tag".into(), tag.clone()));
    }
    keys
}

fn outcome(label: &Label, field: &FieldMatch) -> &'static str {
    match (label.kind, field.selected.as_ref()) {
        (LabelKind::Match, Some(candidate)) if label.targets.contains(&candidate.target.0) => {
            "correct_unique_proposal"
        }
        (LabelKind::Match, Some(_)) => "wrong_unique_proposal",
        (LabelKind::Match, None) => "missed_unique_match",
        (LabelKind::NoMatch, Some(_)) => "false_no_match_proposal",
        (LabelKind::Ambiguous, Some(_)) => "unsafe_ambiguous_proposal",
        (_, None) => "correct_expected_abstention",
    }
}

fn prediction(label: &Label, field: &FieldMatch) -> Value {
    json!({"source":field.source.0,"gold_kind":label.kind,"gold_targets":label.targets,
        "outcome":outcome(label,field),"decision":format!("{:?}",field.decision),
        "selected":field.selected.as_ref().map(|candidate|json!({"target":candidate.target.0,"score":candidate.score})),
        "alternatives":field.alternatives.iter().map(|id|&id.0).collect::<Vec<_>>(),
        "candidates":field.candidates.iter().map(|candidate|json!({"target":candidate.target.0,"score":candidate.score,"eligible":candidate.eligible,
            "issues":candidate.issues.iter().map(|issue|format!("{issue:?}")).collect::<Vec<_>>()})).collect::<Vec<_>>()
    })
}

// Compatibility is an explicit reporting projection after matching and scoring.
// It cannot affect matcher input, eligibility, selection, counts or evidence.
fn legacy_decision_projection(report: &MatchReport) -> MatchReport {
    let mut projected = report.clone();
    for field in &mut projected.fields {
        if field.decision == Decision::InsufficientEvidence {
            field.decision = Decision::BelowThreshold;
        }
        field
            .diagnostics
            .retain(|reason| *reason != FieldDiagnostic::InsufficientEvidence);
    }
    projected
}

fn normalized(value: &str) -> String {
    value.replace("\r\n", "\n")
}
fn hash(value: &str) -> String {
    format!("{:x}", Sha256::digest(normalized(value).as_bytes()))
}
fn source_hashes(directory: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();
    for entry in fs::read_dir(directory).map_err(|e| e.to_string())? {
        let path = entry.map_err(|e| e.to_string())?.path();
        if path.extension().is_some_and(|extension| extension == "rs") {
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or("invalid source filename")?;
            let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
            result.insert(name.to_owned(), hash(&text));
        }
    }
    Ok(result)
}

fn metadata(protocol: &Value, partition: &str, cases: &[Case]) -> Result<Value, String> {
    let lock = fs::read_to_string(root().join("Cargo.lock")).map_err(|e| e.to_string())?;
    let manifests = ["Cargo.toml", "evaluation/Cargo.toml"]
        .into_iter()
        .map(|path| {
            fs::read_to_string(root().join(path))
                .map(|text| (path.to_owned(), normalized(&text)))
                .map_err(|e| e.to_string())
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    Ok(
        json!({"schema_version":"corrective-report-1.0.0","partition":partition,
            "fresh_holdout_scored":partition=="fresh_holdout","protocol":protocol,"protocol_sha256":hash(PROTOCOL),
            "original_protocol_sha256":hash(corpus::PROTOCOL),
            "original_corpus_sha256":corpus::FILES.iter().map(|(name,text)|((*name).to_owned(),hash(text))).collect::<BTreeMap<_,_>>(),
        "fresh_corpus_source_sha256":hash(corrective_corpus::SOURCE_TEXT),
        "fresh_corpus_provenance":corrective_corpus::PROVENANCE,
        "fresh_partition_rationale":corrective_corpus::PARTITION_RATIONALE,
            "generated_partition_sha256":hash(&serde_json::to_string(cases).map_err(|e|e.to_string())?),
            "engine_source_sha256":source_hashes(&root().join("src"))?,"evaluator_source_sha256":source_hashes(&root().join("evaluation/src"))?,
            "manifest_sha256":manifests.iter().map(|(name,text)|(name.clone(),hash(text))).collect::<BTreeMap<_,_>>(),
            "manifests":manifests,"cargo_lock":normalized(&lock),"cargo_lock_sha256":hash(&lock),
        "prediction_policy":"One compact JSONL record per case/model/assignment; stable IDs, gold labels, candidate outcomes and SHA-256 of the complete deterministic MatchReport Debug representation. No sample or semantic-hint values."
        }),
    )
}

fn write_or_check(path: &Path, contents: &str, check: bool) -> Result<(), String> {
    if check || path.exists() {
        let existing = fs::read_to_string(path).map_err(|e| e.to_string())?;
        if normalized(&existing) != contents {
            return Err(format!(
                "{} differs; preserve results and use a new output directory",
                path.display()
            ));
        }
    } else {
        fs::write(path, contents).map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn checked_summary_matches(existing: &str, current: &str) -> Result<bool, String> {
    let strip_current_source_hashes = |text: &str| -> Result<Value, String> {
        let mut value: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
        let metadata = value["metadata"]
            .as_object_mut()
            .ok_or("summary metadata is missing")?;
        // Future code changes should be checked for identical behavior without
        // rewriting historical provenance. Every other recorded input remains
        // strict, including the fresh fixture's separate source hash.
        for key in ["engine_source_sha256", "evaluator_source_sha256"] {
            let hashes = metadata
                .remove(key)
                .ok_or("summary source hashes are missing")?;
            if !hashes.is_object() {
                return Err("summary source hashes must be maps".into());
            }
        }
        Ok(value)
    };
    Ok(strip_current_source_hashes(existing)? == strip_current_source_hashes(current)?)
}

fn write_summary_or_check(path: &Path, contents: &str, check: bool) -> Result<(), String> {
    if !check {
        return write_or_check(path, contents, false);
    }
    let existing = fs::read_to_string(path).map_err(|error| error.to_string())?;
    if !checked_summary_matches(&existing, contents)? {
        return Err(format!(
            "{} differs beyond current engine/evaluator source hashes",
            path.display()
        ));
    }
    Ok(())
}

// Keep one summary row per line rather than expanding every count into dozens
// of lines. The complete JSON document remains easy to load with standard tools.
fn compact_summary(
    metadata: Value,
    inventory: Value,
    rows: &[Value],
    groups: &[Value],
    quality: &[Value],
) -> Result<String, String> {
    fn encoded(value: &impl Serialize) -> Result<String, String> {
        serde_json::to_string(value).map_err(|e| e.to_string())
    }
    let mut text = format!(
        "{{\n  \"metadata\": {},\n  \"inventory\": {},\n",
        encoded(&metadata)?,
        encoded(&inventory)?
    );
    for (index, (name, values)) in [
        ("results", rows),
        ("default_by_group", groups),
        ("quality_targets", quality),
    ]
    .into_iter()
    .enumerate()
    {
        text.push_str(&format!("  \"{name}\": [\n"));
        for (i, value) in values.iter().enumerate() {
            text.push_str(&format!(
                "    {}{}\n",
                encoded(value)?,
                if i + 1 == values.len() { "" } else { "," }
            ));
        }
        text.push_str(if index == 2 { "  ]\n" } else { "  ],\n" });
    }
    text.push_str("}\n");
    Ok(text)
}

fn percent(value: &Value) -> String {
    value
        .as_f64()
        .map_or_else(|| "n/a".into(), |value| format!("{:.2}%", value * 100.0))
}
fn markdown(partition: &str, rows: &[Value], groups: &[Value]) -> String {
    let mut text=format!("# Corrective corroboration experiment: {partition}\n\nAll models use the frozen .70 score threshold and .08 ambiguity margin. Combined is the unchanged weighted default; corroborated adds .80 name/.50 distinct-sample support; sample_gate removes the .80 name floor but still requires positive concrete built-in name evidence. Name-only disables type veto and semantic hints. No thresholds were swept or selected from results.\n\n| Model | Assignment | Correct/proposed | Precision | Recall | Unique coverage | Candidate recall@5 | Wrong unique | No-match proposals | Ambiguous proposals |\n| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for row in rows {
        let c = &row["counts"];
        let m = &row["metrics"];
        text.push_str(&format!(
            "| {} | {} | {}/{} | {} | {} | {} | {} | {} | {}/{} | {}/{} |\n",
            row["model"].as_str().unwrap_or(""),
            row["assignment"].as_str().unwrap_or(""),
            c["correct_proposals"],
            c["proposals"],
            percent(&m["precision"]),
            percent(&m["recall"]),
            percent(&m["unique_coverage"]),
            percent(&m["candidate_recall_at_5"]),
            c["wrong_unique_proposals"],
            c["no_match_proposals"],
            c["no_match_fields"],
            c["ambiguous_proposals"],
            c["ambiguous_fields"]
        ));
    }
    text.push_str("\n## Failure slices by domain and scenario\n\nScenario tags overlap; do not sum these rows. The JSON includes every family and variant. Predictions retain each source outcome, including missed unique matches, to expose the cost of abstention.\n\n| Group | Model | Assignment | Wrong unique | No-match proposals | Ambiguous proposals | Correct/proposed |\n| --- | --- | --- | ---: | ---: | ---: | ---: |\n");
    for row in groups
        .iter()
        .filter(|row| [Some("domain"), Some("scenario_tag")].contains(&row["group_by"].as_str()))
    {
        let c = &row["counts"];
        text.push_str(&format!(
            "| {}: {} | {} | {} | {} | {}/{} | {}/{} | {}/{} |\n",
            row["group_by"].as_str().unwrap_or(""),
            row["group"].as_str().unwrap_or(""),
            row["model"].as_str().unwrap_or(""),
            row["assignment"].as_str().unwrap_or(""),
            c["wrong_unique_proposals"],
            c["no_match_proposals"],
            c["no_match_fields"],
            c["ambiguous_proposals"],
            c["ambiguous_fields"],
            c["correct_proposals"],
            c["proposals"]
        ));
    }
    text.push_str("\nTargets remain >=95% proposal precision, >=60% unique coverage and >=90% candidate recall@5, with nonzero proposals. Empty predictions do not constitute success. The prespecified decision preserves the existing default and keeps corroboration opt-in, regardless of synthetic results. Old families are regression evidence; fresh development and holdout are reported separately. Neither correlated variants nor easy retrieval on small schemas establish production accuracy.\n");
    text
}

fn verify_original_baselines(model: &str, assignment: &str, counts: &Counts) -> Result<(), String> {
    if !["combined", "name_only"].contains(&model) {
        return Ok(());
    }
    let mut baseline = Counts::default();
    for partition in ["development", "holdout"] {
        let path = root().join(format!("evaluation/results/rc-v1/release/{partition}.json"));
        let value: Value =
            serde_json::from_str(&fs::read_to_string(path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        let row = value["threshold_rows"]
            .as_array()
            .ok_or("missing release baseline rows")?
            .iter()
            .find(|row| {
                row["model"] == model && row["assignment"] == assignment && row["threshold"] == 0.7
            })
            .ok_or("missing default baseline row")?;
        let part: Counts =
            serde_json::from_value(row["counts"].clone()).map_err(|e| e.to_string())?;
        baseline.add(&part);
    }
    if baseline != *counts {
        return Err(
            "combined/name-only regression counts differ from the frozen release baseline".into(),
        );
    }
    Ok(())
}

fn evaluate_partition(
    protocol: &Value,
    partition: &str,
    output: &Path,
    check: bool,
    check_legacy_decisions: bool,
) -> Result<(), String> {
    if check_legacy_decisions
        && (!check || !matches!(partition, "original_regression" | "fresh_development"))
    {
        return Err("legacy decision compatibility only checks development/regression artifacts; no fresh holdout is scored".into());
    }
    let cases = cases(partition)?;
    let mut rows = Vec::new();
    let mut grouped_rows = Vec::new();
    let mut predictions = String::new();
    let mut quality = Vec::new();
    let target_precision = number(&protocol["quality_targets"], "proposal_precision")?;
    let target_coverage = number(&protocol["quality_targets"], "unique_coverage")?;
    let target_retrieval = number(&protocol["quality_targets"], "candidate_recall_at_5")?;
    for model in MODELS {
        for assignment in MODES {
            let matcher = engine(protocol, model, assignment == "one_to_one")?;
            let mut totals = Counts::default();
            let mut groups = BTreeMap::<(String, String), Counts>::new();
            for case in &cases {
                let mut source = corpus::schema(&case.source)?;
                let mut target = corpus::schema(&case.target)?;
                if model == "name_only" {
                    for field in source.fields.iter_mut().chain(&mut target.fields) {
                        field.hints = SemanticHints::default();
                    }
                }
                // No labels, concepts, family tags or rationales reach matching.
                let report = matcher
                    .match_schemas(&source, &target)
                    .map_err(|e| e.to_string())?;
                let counts = score_case(&case.labels, &report)?;
                totals.add(&counts);
                for key in group_keys(case) {
                    groups.entry(key).or_default().add(&counts);
                }
                let legacy_report =
                    check_legacy_decisions.then(|| legacy_decision_projection(&report));
                let report = legacy_report.as_ref().unwrap_or(&report);
                let fields = report
                    .fields
                    .iter()
                    .map(|field| {
                        case.labels
                            .iter()
                            .find(|label| label.source == field.source.0)
                            .map(|label| prediction(label, field))
                            .ok_or_else(|| "missing prediction label".to_owned())
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let record = json!({"case":case.id,"family":case.family_id,"domain":case.domain,"variant":case.variant,"model":model,"assignment":assignment,"fields":fields,"report_sha256":hash(&format!("{report:?}"))});
                predictions.push_str(&serde_json::to_string(&record).map_err(|e| e.to_string())?);
                predictions.push('\n');
            }
            if partition == "original_regression" {
                verify_original_baselines(model, assignment, &totals)?;
            }
            let metrics = totals.metrics();
            quality.push(json!({"model":model,"assignment":assignment,
                "nonzero_proposals":totals.proposals>0,
                "precision_met":metrics.precision.is_some_and(|v|v>=target_precision),
                "unique_coverage_met":metrics.unique_coverage.is_some_and(|v|v>=target_coverage),
                "candidate_recall_at_5_met":metrics.candidate_recall_at_5.is_some_and(|v|v>=target_retrieval),
                "all_targets_met":totals.proposals>0&&metrics.precision.is_some_and(|v|v>=target_precision)&&metrics.unique_coverage.is_some_and(|v|v>=target_coverage)&&metrics.candidate_recall_at_5.is_some_and(|v|v>=target_retrieval)
            }));
            rows.push(json!({"model":model,"assignment":assignment,"threshold":0.7,"counts":totals,"metrics":metrics}));
            for ((group_by, group), counts) in groups {
                grouped_rows.push(json!({"model":model,"assignment":assignment,"group_by":group_by,"group":group,"counts":counts,"metrics":counts.metrics()}));
            }
        }
    }
    let inventory = json!({"families":cases.iter().map(|case|&case.family_id).collect::<BTreeSet<_>>().len(),"cases":cases.len(),"labels":cases.iter().map(|case|case.labels.len()).sum::<usize>(),"prediction_records":cases.len()*MODELS.len()*MODES.len(),"predictions_sha256":hash(&predictions)});
    let summary = compact_summary(
        metadata(protocol, partition, &cases)?,
        inventory,
        &rows,
        &grouped_rows,
        &quality,
    )?;
    write_summary_or_check(&output.join(format!("{partition}.json")), &summary, check)?;
    write_or_check(
        &output.join(format!("{partition}.md")),
        &markdown(partition, &rows, &grouped_rows),
        check,
    )?;
    write_or_check(
        &output.join(format!("{partition}-predictions.jsonl")),
        &predictions,
        check,
    )?;
    println!(
        "Corrective {partition} evaluation {}: {} cases, {} model/assignment rows",
        if check_legacy_decisions {
            "verified with explicit legacy decision compatibility"
        } else if check {
            "verified"
        } else {
            "written"
        },
        cases.len(),
        rows.len()
    );
    Ok(())
}

pub fn run(args: Vec<String>) -> Result<(), String> {
    if args.as_slice() == ["--help"] {
        println!("{USAGE}");
        return Ok(());
    }
    let options = Options::parse(args)?;
    let protocol = protocol()?;
    if options.acknowledge_holdout {
        require_freeze(&protocol)?;
    }
    if !options.checking() {
        fs::create_dir_all(&options.output).map_err(|e| e.to_string())?;
    }
    for partition in options.partitions() {
        evaluate_partition(
            &protocol,
            partition,
            &options.output,
            options.checking(),
            options.check_legacy_decisions,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use fieldkin::{DataType, Field, SampleValue, Schema};

    use super::*;

    #[test]
    fn legacy_check_is_explicit_and_rejects_other_checks_or_holdout() {
        let legacy = Options::parse(vec!["--check-legacy-decisions".into()]).unwrap();
        assert!(legacy.check_legacy_decisions);
        assert!(legacy.checking());
        assert!(!legacy.check);
        assert_eq!(
            legacy.partitions(),
            &["original_regression", "fresh_development"]
        );
        for arguments in [
            vec!["--check", "--check-legacy-decisions"],
            vec!["--check-legacy-decisions", "--check"],
            vec!["--check-legacy-decisions", "--acknowledge-holdout"],
            vec!["--acknowledge-holdout", "--check-legacy-decisions"],
        ] {
            assert!(Options::parse(arguments.into_iter().map(str::to_owned).collect()).is_err());
        }
        let strict = Options::parse(vec!["--check".into()]).unwrap();
        assert!(strict.check);
        assert!(!strict.check_legacy_decisions);
        assert!(!Options::parse(vec![]).unwrap().checking());
    }

    #[test]
    fn legacy_check_rejects_holdout_or_writing_before_loading_cases() {
        for (partition, checking) in [
            ("fresh_holdout", true),
            ("fresh_development", false),
            ("unknown_partition", true),
        ] {
            let result = evaluate_partition(
                &Value::Null,
                partition,
                Path::new("unused-legacy-test-output"),
                checking,
                true,
            );
            assert!(result
                .unwrap_err()
                .contains("only checks development/regression artifacts"));
        }
    }

    #[test]
    fn legacy_projection_preserves_matching_evidence_and_all_other_report_fields() {
        let input = schema();
        let actual = MatchEngine::new(Config {
            one_to_one: true,
            corroboration: Some(Corroboration::default()),
            ..Config::default()
        })
        .unwrap()
        .match_schemas(&input, &input)
        .unwrap();
        assert_eq!(actual.fields[0].decision, Decision::Proposed);
        assert_eq!(actual.fields[1].decision, Decision::InsufficientEvidence);
        assert_eq!(
            actual.fields[1].diagnostics,
            vec![
                FieldDiagnostic::NoEligibleTarget,
                FieldDiagnostic::InsufficientEvidence
            ]
        );
        let unchanged = actual.clone();
        let projected = legacy_decision_projection(&actual);
        assert_eq!(actual, unchanged);
        assert_eq!(projected.fields[0], actual.fields[0]);
        assert_eq!(projected.fields[1].decision, Decision::BelowThreshold);
        assert_eq!(
            projected.fields[1].diagnostics,
            vec![FieldDiagnostic::NoEligibleTarget]
        );
        let mut expected = actual.clone();
        expected.fields[1].decision = Decision::BelowThreshold;
        expected.fields[1].diagnostics = vec![FieldDiagnostic::NoEligibleTarget];
        // Full structural equality includes scores, candidate issues/warnings,
        // selection, alternatives, identities and every global report field.
        assert_eq!(projected, expected);
        assert_eq!(legacy_decision_projection(&projected), projected);
        assert_ne!(
            hash(&format!("{actual:?}")),
            hash(&format!("{projected:?}"))
        );
        for decision in [
            Decision::Confirmed,
            Decision::ExcludedByCaller,
            Decision::Proposed,
            Decision::Ambiguous,
            Decision::BelowThreshold,
            Decision::AssignmentConflict,
        ] {
            let mut report = projected.clone();
            report.fields[0].decision = decision;
            report.fields[0].diagnostics = vec![FieldDiagnostic::LocalAmbiguity];
            assert_eq!(legacy_decision_projection(&report), report);
        }
    }

    #[test]
    fn only_acknowledged_frozen_runs_can_score_fresh_holdout() {
        assert_eq!(
            Options::parse(vec![]).unwrap().partitions(),
            &["original_regression", "fresh_development"]
        );
        assert_eq!(
            Options::parse(vec!["--check".into()]).unwrap().partitions(),
            &["original_regression", "fresh_development"]
        );
        assert!(Options::parse(vec!["--check".into(), "--acknowledge-holdout".into()]).is_err());
        assert!(Options::parse(vec!["--split".into(), "fresh_holdout".into()]).is_err());
        let mut value = protocol().unwrap();
        value["feature_freeze_revision"] = Value::Null;
        assert!(validate_freeze_metadata(&value).is_err());
        value["feature_freeze_revision"] = json!("a".repeat(40));
        assert!(validate_freeze_metadata(&value).is_ok());
        value["default_decision"] = Value::Null;
        assert!(validate_freeze_metadata(&value).is_err());
    }

    #[test]
    fn freeze_comparison_detects_changes_additions_and_deletions() {
        let expected = BTreeMap::from([("src/engine.rs".into(), "original".into())]);
        assert!(compare_frozen_files(&expected, &expected).is_ok());
        let changed = BTreeMap::from([("src/engine.rs".into(), "changed".into())]);
        assert!(compare_frozen_files(&expected, &changed)
            .unwrap_err()
            .contains("differs"));
        assert!(compare_frozen_files(&expected, &BTreeMap::new())
            .unwrap_err()
            .contains("deleted"));
        let mut added = expected.clone();
        added.insert("evaluation/corpus/extra.json".into(), "new".into());
        assert!(compare_frozen_files(&expected, &added)
            .unwrap_err()
            .contains("added"));
        assert!(compare_frozen_files(&BTreeMap::new(), &BTreeMap::new()).is_err());
    }

    #[test]
    fn freeze_protocol_exception_allows_only_self_referential_revision_metadata() {
        let mut original = protocol().unwrap();
        original["feature_freeze_revision"] = Value::Null;
        let mut current = original.clone();
        current["feature_freeze_revision"] = json!("a".repeat(40));
        let digest = |value: &Value| {
            frozen_content_hash("evaluation/corrective-protocol.json", &value.to_string()).unwrap()
        };
        assert_eq!(digest(&original), digest(&current));
        current["default_decision"] = json!("changed after freeze");
        assert_ne!(digest(&original), digest(&current));
        current = original.clone();
        current["candidate_policy"]["corroborated"]["min_name_score"] = json!(0.1);
        assert_ne!(digest(&original), digest(&current));
        assert_eq!(
            frozen_content_hash("src/example.rs", "line\r\n").unwrap(),
            frozen_content_hash("src/example.rs", "line\n").unwrap()
        );
    }

    #[test]
    fn default_partition_includes_old_holdout_only_as_regression() {
        assert_eq!(cases("original_regression").unwrap().len(), 200);
        let development = cases("fresh_development").unwrap();
        assert_eq!(development.len(), 36);
        assert_eq!(
            development
                .iter()
                .map(|case| case.labels.len())
                .sum::<usize>(),
            216
        );
        assert_eq!(
            development
                .iter()
                .map(|case| &case.family_id)
                .collect::<BTreeSet<_>>()
                .len(),
            12
        );
        assert!(development.iter().all(|case| case.split == "development"));
    }

    fn schema() -> Schema {
        Schema::new(vec![
            Field::new("a", "amount", DataType::Float).with_samples(vec![
                SampleValue::Number(1.0),
                SampleValue::Number(2.0),
                SampleValue::Number(3.0),
            ]),
            Field::new("b", "date", DataType::Date),
        ])
    }

    #[test]
    fn model_configuration_matches_public_api_and_preserves_weighted_default() {
        let protocol = protocol().unwrap();
        let schema = schema();
        for one_to_one in [false, true] {
            for (model, corroboration) in [
                ("combined", None),
                (
                    "sample_gate",
                    Some(Corroboration {
                        min_name_score: 0.0,
                        min_sample_score: 0.5,
                    }),
                ),
                (
                    "corroborated",
                    Some(Corroboration {
                        min_name_score: 0.8,
                        min_sample_score: 0.5,
                    }),
                ),
            ] {
                let expected = MatchEngine::new(Config {
                    one_to_one,
                    corroboration,
                    ..Config::default()
                })
                .unwrap()
                .match_schemas(&schema, &schema)
                .unwrap();
                assert_eq!(
                    engine(&protocol, model, one_to_one)
                        .unwrap()
                        .match_schemas(&schema, &schema)
                        .unwrap(),
                    expected
                );
            }
        }
        assert_eq!(Corroboration::default().min_name_score, 0.0);
        assert_eq!(Corroboration::default().min_sample_score, 0.5);
    }

    #[test]
    fn summary_keeps_all_denominators_without_pretty_print_explosion() {
        let counts = Counts {
            fields: 6,
            unique_fields: 3,
            no_match_fields: 2,
            ambiguous_fields: 1,
            proposals: 0,
            ..Counts::default()
        };
        let rows = vec![json!({"counts":counts,"metrics":counts.metrics()})];
        let text = compact_summary(
            json!({"schema_version":"test"}),
            json!({"cases":1}),
            &rows,
            &[],
            &[],
        )
        .unwrap();
        let parsed: Value = serde_json::from_str(&text).unwrap();
        assert!(parsed["results"][0]["metrics"]["precision"].is_null());
        assert_eq!(parsed["results"][0]["counts"]["fields"], 6);
        assert!(text.lines().count() < 15);
    }

    #[test]
    fn regression_check_ignores_only_current_engine_and_evaluator_hashes() {
        let baseline = json!({
            "metadata": {
                "engine_source_sha256":{"engine.rs":"original"},
                "evaluator_source_sha256":{"corrective.rs":"original"},
                "fresh_corpus_source_sha256":"fixed-fixture",
                "cargo_lock_sha256":"fixed-lock",
                "protocol_sha256":"fixed-protocol",
                "manifest_sha256":{"Cargo.toml":"fixed-manifest"}
            },
            "inventory":{"predictions_sha256":"fixed-full-report-digests"},
            "results":[{"counts":{"correct_proposals":3}}]
        });
        let mut changed = baseline.clone();
        changed["metadata"]["engine_source_sha256"]["engine.rs"] = json!("new-code");
        changed["metadata"]["evaluator_source_sha256"]["new.rs"] = json!("new-code");
        assert!(checked_summary_matches(&baseline.to_string(), &changed.to_string()).unwrap());
        for key in [
            "fresh_corpus_source_sha256",
            "cargo_lock_sha256",
            "protocol_sha256",
            "manifest_sha256",
        ] {
            let mut changed = baseline.clone();
            changed["metadata"][key] = json!("changed");
            assert!(!checked_summary_matches(&baseline.to_string(), &changed.to_string()).unwrap());
        }
        changed = baseline.clone();
        changed["inventory"]["predictions_sha256"] = json!("different-behavior");
        assert!(!checked_summary_matches(&baseline.to_string(), &changed.to_string()).unwrap());
        changed = baseline.clone();
        changed["results"][0]["counts"]["correct_proposals"] = json!(2);
        assert!(!checked_summary_matches(&baseline.to_string(), &changed.to_string()).unwrap());
        changed = baseline.clone();
        changed["metadata"]
            .as_object_mut()
            .unwrap()
            .remove("engine_source_sha256");
        assert!(checked_summary_matches(&baseline.to_string(), &changed.to_string()).is_err());
    }
}
