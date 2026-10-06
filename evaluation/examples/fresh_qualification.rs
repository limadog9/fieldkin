//! One-shot, external-JSON evaluation of the already frozen local candidate.
//! Reuses the repository's input adapter, five variants, and metric implementation.
//! Install as evaluation/examples/fresh_qualification.rs. No matcher edits.

#[allow(dead_code)]
#[path = "../src/corpus.rs"]
mod corpus;
#[allow(dead_code)]
#[path = "../src/metrics.rs"]
mod metrics;
#[allow(dead_code)]
#[path = "../src/model.rs"]
mod model;

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use fieldkin::{
    Config, ContextualEvidence, Decision, MatchEngine, NameConflictKind, NameConflictRule,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const FROZEN: &str = "b6c76427cfaf35ce8421dd6def35a55f2c9d9bb6";
const DATA_SHA256: &str = "d63de02aa3beb8fcd35c0193f93d1e083930e64ad581f542368779429634c6cf";
const CONFLICT_PROTOCOL: &str = include_str!("../precision-protocol.json");
const VARIANTS: [&str; 5] = [
    "base",
    "reordered",
    "without_samples",
    "null_heavy",
    "separator_noise",
];
const DOMAINS: [&str; 4] = [
    "financial/business records",
    "CRM/contact/customer data",
    "product/catalog/import data",
    "operational/telemetry/logistics data",
];
const PROTECTED: [&str; 9] = [
    "src",
    "tests",
    "Cargo.toml",
    "Cargo.lock",
    "rust-toolchain.toml",
    "evaluation/src",
    "evaluation/Cargo.toml",
    "evaluation/protocol.json",
    "evaluation/precision-protocol.json",
];
type Result<T> = std::result::Result<T, String>;

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}
fn read(path: &Path) -> Result<Vec<u8>> {
    fs::read(path).map_err(|e| format!("{}: {e}", path.display()))
}
fn git(args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .current_dir(root())
        .args(args)
        .output()
        .map_err(|e| format!("cannot run Git: {e}"))?;
    if !output.status.success() {
        return Err(format!(
            "Git check failed: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(output.stdout)
}
fn check_frozen() -> Result<()> {
    if String::from_utf8_lossy(&git(&["rev-parse", "HEAD"])?).trim() != FROZEN {
        return Err(format!(
            "HEAD must remain {FROZEN}; do not reset or discard changes to bypass this check"
        ));
    }
    let mut args = vec!["status", "--porcelain", "--untracked-files=all", "--"];
    args.extend(PROTECTED);
    if !git(&args)?.is_empty() {
        return Err(
            "matcher, tests, dependencies, or shared evaluator inputs changed; stop before scoring"
                .into(),
        );
    }
    // The temporary example is allowed; protected existing files must match HEAD.
    Ok(())
}
fn source_inventory() -> Result<BTreeMap<String, String>> {
    let mut args = vec!["ls-files", "-z", "--"];
    args.extend(PROTECTED);
    let names = git(&args)?;
    let mut result = BTreeMap::new();
    for name in names.split(|b| *b == 0).filter(|name| !name.is_empty()) {
        let name = std::str::from_utf8(name).map_err(|e| e.to_string())?;
        result.insert(name.to_owned(), digest(&read(&root().join(name))?));
    }
    result.insert(
        "evaluation/examples/fresh_qualification.rs".into(),
        digest(&read(
            &root().join("evaluation/examples/fresh_qualification.rs"),
        )?),
    );
    Ok(result)
}
fn conflict_rules() -> Result<Vec<NameConflictRule>> {
    let value: Value = serde_json::from_str(CONFLICT_PROTOCOL).map_err(|e| e.to_string())?;
    value["rules"]
        .as_array()
        .ok_or("missing conflict rules")?
        .iter()
        .map(|rule| {
            let kind = match rule["kind"].as_str() {
                Some("qualifier") => NameConflictKind::Qualifier,
                Some("unit") => NameConflictKind::Unit,
                _ => return Err("unknown conflict-rule kind".into()),
            };
            let alternatives: Vec<Vec<String>> =
                serde_json::from_value(rule["alternatives"].clone()).map_err(|e| e.to_string())?;
            Ok(NameConflictRule { kind, alternatives })
        })
        .collect()
}
fn candidate(one_to_one: bool) -> Result<MatchEngine> {
    MatchEngine::new(Config {
        min_score: 0.70,
        ambiguity_margin: 0.08,
        max_candidates: 5,
        one_to_one,
        abstain_on_ambiguity: true,
        reject_incompatible_types: true,
        corroboration: None,
        contextual_evidence: Some(ContextualEvidence {
            strict_identifier_samples: true,
            scoped_support: true,
        }),
        name_conflicts: conflict_rules()?,
        ..Config::default()
    })
    .map_err(|e| e.to_string())
}
fn load(path: &Path) -> Result<(Vec<model::Case>, Value)> {
    let bytes = read(path)?;
    if digest(&bytes) != DATA_SHA256 {
        return Err("dataset bytes differ from the supplied frozen JSON".into());
    }
    let mut families: Vec<model::Family> =
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
    if families.len() != 12 {
        return Err("expected 12 families".into());
    }
    let mut ids = BTreeSet::new();
    let mut domains = BTreeMap::new();
    let mut counts = [0usize; 3];
    for family in &families {
        corpus::validate_family(family)?;
        if !ids.insert(&family.id) || !(6..=9).contains(&family.target.len()) {
            return Err("duplicate family or wrong target count".into());
        }
        if family
            .source
            .iter()
            .any(|s| family.target.iter().any(|t| s.id == t.id))
        {
            return Err("source/target IDs overlap within a family".into());
        }
        *domains.entry(family.domain.clone()).or_insert(0usize) += 1;
        for label in &family.labels {
            counts[match label.kind {
                model::LabelKind::Match => 0,
                model::LabelKind::NoMatch => 1,
                model::LabelKind::Ambiguous => 2,
            }] += 1;
        }
    }
    let expected_domains: BTreeMap<_, _> = DOMAINS
        .into_iter()
        .map(|s| (s.to_owned(), 3usize))
        .collect();
    if domains != expected_domains || counts != [37, 20, 15] {
        return Err("dataset inventory differs from the validated supplied corpus".into());
    }
    families.sort_by(|a, b| a.id.cmp(&b.id));
    let generator = corpus::protocol()?;
    if generator["variants"] != json!(VARIANTS) {
        return Err("variant protocol changed".into());
    }
    let mut cases = corpus::cases(&families)?;
    for case in &mut cases {
        case.split = "holdout".into();
    }
    if cases.len() != 60 || cases.iter().map(|c| c.labels.len()).sum::<usize>() != 360 {
        return Err("expanded corpus inventory is wrong".into());
    }
    let inventory = json!({"families":12,"domains":domains,"canonical_decisions":72,
        "canonical_labels":{"match":37,"no_match":20,"ambiguous":15},
        "cases":60,"decisions_per_assignment":360,
        "expanded_labels":{"match":185,"no_match":100,"ambiguous":75},
        "dataset_sha256":DATA_SHA256});
    Ok((cases, inventory))
}
fn passes(c: &metrics::Counts) -> bool {
    c.proposals > 0
        && c.unique_fields > 0
        && c.candidate_relevant > 0
        && (c.correct_proposals as u128) * 100 >= (c.proposals as u128) * 95
        && (c.unique_proposals as u128) * 100 >= (c.unique_fields as u128) * 60
        && (c.candidate_hits as u128) * 100 >= (c.candidate_relevant as u128) * 90
}
fn pct(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".into(), |v| format!("{:.2}%", v * 100.0))
}
fn write_new(path: &Path, bytes: &[u8]) -> Result<()> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    file.write_all(bytes).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())
}
fn row_line(mode: &str, c: &metrics::Counts) -> String {
    let m = c.metrics();
    format!(
        "| {mode} | {}/{} | {} | {} | {} | {} | {} | {} |\n",
        c.correct_proposals,
        c.proposals,
        pct(m.precision),
        pct(m.unique_coverage),
        pct(m.candidate_recall_at_5),
        c.wrong_unique_proposals,
        c.no_match_proposals + c.ambiguous_proposals,
        passes(c)
    )
}
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 || !["--validate", "--score-once"].contains(&args[0].as_str()) {
        return Err(
            "usage: fresh_qualification (--validate | --score-once) PATH_TO_CORPUS_JSON".into(),
        );
    }
    check_frozen()?;
    let path = PathBuf::from(&args[1]);
    let (cases, inventory) = load(&path)?;
    // Constructing engines validates their configuration without matching anything.
    let engines = [candidate(false)?, candidate(true)?];
    println!(
        "{}",
        serde_json::to_string_pretty(&inventory).map_err(|e| e.to_string())?
    );
    if args[0] == "--validate" {
        println!("Validation only: no field matching or scoring was performed.");
        return Ok(());
    }
    let output = root().join("target/fresh-qualification/results");
    if output.exists() {
        return Err("results already exist; do not overwrite or rerun a prospective test".into());
    }
    let before = source_inventory()?;
    let binary = std::env::current_exe().map_err(|e| e.to_string())?;
    let metadata = json!({"matcher_commit":FROZEN,"dataset_sha256":DATA_SHA256,
        "binary_sha256":digest(&read(&binary)?),"source_sha256":before,
        "conflict_protocol_sha256":digest(CONFLICT_PROTOCOL.as_bytes()),
        "generator_protocol_sha256":digest(corpus::PROTOCOL.as_bytes()),
        "models":["contextual"],"assignments":["independent","one_to_one"],
        "min_score":0.70,"ambiguity_margin":0.08,"max_candidates":5,
        "strict_identifier_samples":true,"scoped_support":true,
        "quality_targets":{"precision":0.95,"unique_coverage":0.60,"candidate_recall_at_5":0.90,"nonzero_proposals":true},
        "acceptance_scope":"All 60 cases pooled, separately in both assignment modes; domain/family/variant results also reported, without dropping any case",
        "input_adapter":"Existing corpus::schema unchanged: JSON strings remain Text samples, numbers become Number, booleans become Boolean, null remains Null. Declared data types retained. No coercion or synthesized hints.",
        "provenance":"User supplied the corpus as authored in another session; authorship isolation and business annotations were not independently certified",
        "caller_hints":false,"caller_confirmations":false,"production_accuracy_certified":false});
    // This marker is recorded before the FIRST matching call. An interrupted attempt
    // remains an attempt; a different output directory cannot silently restart it.
    let marker = root().join(format!(
        "target/fresh-qualification-{}-{}.started.json",
        &FROZEN[..7],
        &DATA_SHA256[..12]
    ));
    write_new(
        &marker,
        &serde_json::to_vec_pretty(&metadata).map_err(|e| e.to_string())?,
    )?;
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;
    let mut prediction_file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(output.join("predictions.jsonl"))
        .map_err(|e| e.to_string())?;
    let mut rows = Vec::new();
    let mut markdown = String::from("# Fresh synthetic qualification\n\nFrozen local candidate; supplied corpus unchanged. Aggregate gates apply separately to both assignment modes. This is not a production-accuracy guarantee.\n\n| Mode | Correct/proposed | Precision | Unique coverage | Recall@5 | Wrong unique | Unsafe no-match/ambiguous | Targets met |\n| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |\n");
    let mut slices_md = String::new();
    let mut all_pass = true;
    for (mode, engine) in ["independent", "one_to_one"].into_iter().zip(engines) {
        let mut counts = metrics::Counts::default();
        let mut by_family: BTreeMap<String, metrics::Counts> = BTreeMap::new();
        let mut by_domain: BTreeMap<String, metrics::Counts> = BTreeMap::new();
        let mut by_variant: BTreeMap<String, metrics::Counts> = BTreeMap::new();
        for case in &cases {
            // These schemas contain ONLY IDs, names, declared types and samples.
            let source = corpus::schema(&case.source)?;
            let target = corpus::schema(&case.target)?;
            let report = engine
                .match_schemas(&source, &target)
                .map_err(|e| e.to_string())?;
            if report
                .fields
                .iter()
                .any(|f| f.decision == Decision::Confirmed)
            {
                return Err("unexpected caller-confirmed selection".into());
            }
            let outcome = metrics::score_case(&case.labels, &report)?;
            counts.add(&outcome);
            by_family
                .entry(case.family_id.clone())
                .or_default()
                .add(&outcome);
            by_domain
                .entry(case.domain.clone())
                .or_default()
                .add(&outcome);
            by_variant
                .entry(case.variant.clone())
                .or_default()
                .add(&outcome);
            let fields: Vec<_> = report
                .fields
                .iter()
                .map(|f| {
                    json!({"source":f.source.0,
                "decision":format!("{:?}",f.decision),
                "selected":f.selected.as_ref().map(|c| &c.target.0)})
                })
                .collect();
            let prediction = json!({"case":case.id,"family":case.family_id,"domain":case.domain,
                "variant":case.variant,"model":"contextual","assignment":mode,"counts":outcome,
                "fields":fields,"report_sha256":digest(format!("{report:?}").as_bytes())});
            serde_json::to_writer(&mut prediction_file, &prediction).map_err(|e| e.to_string())?;
            prediction_file
                .write_all(b"\n")
                .map_err(|e| e.to_string())?;
        }
        all_pass &= passes(&counts);
        markdown.push_str(&row_line(mode, &counts));
        for (label, slice) in [
            ("Variant", &by_variant),
            ("Domain", &by_domain),
            ("Family", &by_family),
        ] {
            slices_md.push_str(&format!("\n## {mode}: {label} breakdown\n\n| {label} | Correct/proposed | Precision | Unique coverage | Recall@5 | Wrong unique | Unsafe no-match/ambiguous | Same numerical thresholds |\n| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |\n"));
            for (name, c) in slice {
                slices_md.push_str(&row_line(name, c));
            }
        }
        rows.push(
            json!({"model":"contextual","assignment":mode,"counts":counts,
            "metrics":counts.metrics(),"quality_targets_met":passes(&counts),
            "by_family":by_family,"by_domain":by_domain,"by_variant":by_variant}),
        );
    }
    prediction_file.sync_all().map_err(|e| e.to_string())?;
    check_frozen()?;
    if before != source_inventory()? || digest(&read(&path)?) != DATA_SHA256 {
        return Err(
            "source or corpus changed during evaluation; do not treat partial results as valid"
                .into(),
        );
    }
    markdown.push_str(&format!("\nAggregate synthetic targets met in both modes: **{all_pass}**.\n\nSlice tables are diagnostic; no case or variant was excluded. Five variants of each family are correlated, not five independent scenarios.\n"));
    let summary = markdown.clone();
    markdown.push_str(&slices_md);
    let report = json!({"metadata":metadata,"inventory":inventory,"rows":rows,
        "synthetic_targets_met_both_modes":all_pass,"production_accuracy_certified":false,
        "source_and_data_unchanged_after_run":true});
    write_new(
        &output.join("report.json"),
        &serde_json::to_vec_pretty(&report).map_err(|e| e.to_string())?,
    )?;
    write_new(&output.join("report.md"), markdown.as_bytes())?;
    write_new(&output.join("summary.md"), summary.as_bytes())?;
    println!("\n{summary}\nResults written to {}", output.display());
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Fresh qualification stopped: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod wrapper_tests {
    use super::*;
    #[test]
    fn zero_proposals_cannot_pass() {
        assert!(!passes(&metrics::Counts::default()));
    }
    #[test]
    fn gates_use_exact_counts() {
        let mut c = metrics::Counts {
            proposals: 100,
            correct_proposals: 95,
            unique_fields: 100,
            unique_proposals: 60,
            candidate_relevant: 100,
            candidate_hits: 90,
            ..Default::default()
        };
        assert!(passes(&c));
        c.unique_proposals = 59;
        assert!(!passes(&c));
    }
    #[test]
    fn existing_adapter_preserves_quoted_decimals_as_text_samples() {
        let field: model::InputField = serde_json::from_value(json!({
            "id":"s", "name":"fee_amount", "data_type":"Decimal",
            "concept":"only for validation", "samples":["2.40", 2.4, null]
        }))
        .unwrap();
        let schema = corpus::schema(&[field]).unwrap();
        assert_eq!(schema.fields[0].data_type, fieldkin::DataType::Decimal);
        let samples = schema.fields[0].samples.as_ref().unwrap();
        assert!(matches!(&samples[0], fieldkin::SampleValue::Text(s) if s == "2.40"));
        assert!(matches!(&samples[1], fieldkin::SampleValue::Number(v) if *v == 2.4));
        assert!(matches!(&samples[2], fieldkin::SampleValue::Null));
    }
    #[test]
    fn both_frozen_configurations_construct_without_scoring() {
        candidate(false).unwrap();
        candidate(true).unwrap();
    }
}
