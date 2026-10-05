//! Same-input comparison; the published qualification corpus is regression only.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use fieldkin::{Config, ContextualEvidence, MatchEngine, NameMatcher, WeightedMatcher};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::metrics::Counts;
use crate::model::LabelKind;
use crate::{context_qualification, metrics, readiness};

const MODELS: [&str; 4] = ["default", "name_only_070", "name_only_exact", "contextual"];
const ASSIGNMENTS: [&str; 2] = ["independent", "one_to_one"];

#[derive(Clone, Deserialize, Serialize)]
struct Selection {
    source: String,
    selected: Option<String>,
    correct: bool,
}

#[derive(Default, Serialize, PartialEq, Debug)]
struct Transitions {
    retained_correct: usize,
    lost_correct: usize,
    recovered_correct: usize,
    removed_wrong: usize,
    introduced_wrong: usize,
}

fn transition(before: &Selection, after: &Selection, totals: &mut Transitions) {
    totals.retained_correct += usize::from(before.correct && after.correct);
    totals.lost_correct += usize::from(before.correct && !after.correct);
    totals.recovered_correct += usize::from(!before.correct && after.correct);
    let before_wrong = before.selected.is_some() && !before.correct;
    let after_wrong = after.selected.is_some() && !after.correct;
    totals.removed_wrong += usize::from(before_wrong && !after_wrong);
    totals.introduced_wrong += usize::from(!before_wrong && after_wrong);
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn normalized_hash(text: &str) -> String {
    digest(text.replace("\r\n", "\n").as_bytes())
}

fn source_hashes(path: &Path) -> Result<BTreeMap<String, String>, String> {
    fs::read_dir(path)
        .map_err(|error| error.to_string())?
        .map(|entry| {
            let path = entry.map_err(|error| error.to_string())?.path();
            Ok(path)
        })
        .collect::<Result<Vec<_>, String>>()?
        .into_iter()
        .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
        .map(|path| {
            Ok((
                path.file_name().unwrap().to_string_lossy().into_owned(),
                normalized_hash(&fs::read_to_string(path).map_err(|error| error.to_string())?),
            ))
        })
        .collect()
}

fn key(record: &Value) -> Result<String, String> {
    ["corpus", "case", "model", "assignment"]
        .into_iter()
        .map(|name| record[name].as_str().ok_or("invalid prediction key"))
        .collect::<Result<Vec<_>, _>>()
        .map(|parts| parts.join("/"))
        .map_err(str::to_owned)
}

fn pct(value: Option<f64>) -> String {
    value.map_or_else(
        || "undefined".into(),
        |value| format!("{:.2}%", value * 100.0),
    )
}

fn scorer_fingerprint(cases: &[readiness::Case]) -> Result<String, String> {
    // Gold truth and slicing identities are scorer inputs only. Bind them for
    // comparison without passing any of them into the matching engine.
    let labels: Vec<_> = cases
        .iter()
        .map(|case| {
            (
                &case.id,
                &case.family,
                &case.domain,
                &case.variant,
                &case.labels,
            )
        })
        .collect();
    Ok(digest(
        &serde_json::to_vec(&labels).map_err(|error| error.to_string())?,
    ))
}

pub fn run(args: Vec<String>) -> Result<(), String> {
    let mut args = args.into_iter();
    let mut output = None;
    let mut before = None;
    let mut check = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => output = Some(PathBuf::from(args.next().ok_or("missing output")?)),
            "--before" => before = Some(PathBuf::from(args.next().ok_or("missing before")?)),
            "--check" => check = true,
            _ => return Err("regression accepts --output DIR, --before DIR and --check; no new holdout or tuning options".into()),
        }
    }
    let output = output.ok_or("--output is required")?;
    if output.exists() && !check {
        return Err("regression requires a fresh output directory".into());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut corpora = readiness::cases()?;
    corpora.push((
        "examined_qualification".into(),
        context_qualification::examined_cases()?,
    ));
    let inputs = corpora
        .iter()
        .map(|(name, cases)| Ok((name.clone(), readiness::observable_inputs_sha256(cases)?)))
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let scorer_inputs = corpora
        .iter()
        .map(|(name, cases)| Ok((name.clone(), scorer_fingerprint(cases)?)))
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let protocols = [
        "readiness-protocol.json",
        "precision-protocol.json",
        "context-qualification-protocol.json",
    ]
    .into_iter()
    .map(|name| {
        Ok((
            name,
            normalized_hash(
                &fs::read_to_string(root.join("evaluation").join(name))
                    .map_err(|error| error.to_string())?,
            ),
        ))
    })
    .collect::<Result<BTreeMap<_, _>, String>>()?;
    let model_configuration: BTreeMap<_, _> = MODELS.into_iter().map(|model| (model, json!({
        "min_score":if model=="name_only_exact" {1.0} else {0.70}, "ambiguity_margin":0.08,"top_k":5,
        "weights":if model.starts_with("name_only") {json!({"name":1.0})} else {json!({"name":0.65,"type":0.20,"sample":0.15})},
        "contextual_evidence":if model=="contextual" {json!({"strict_identifier_samples":true,"scoped_support":true})} else {Value::Null},
        "name_conflicts":if model=="contextual" {"precision-protocol.json"} else {"disabled"}
    }))).collect();
    let contract = json!({"models":MODELS,"model_configuration":model_configuration,"assignments":ASSIGNMENTS,"threshold":0.70,"ambiguity_margin":0.08,"top_k":5,
        "quality_targets":{"precision":0.95,"unique_coverage":0.60,"candidate_recall_at_5":0.90,"nonzero_proposals":true},
        "protocol_sha256":protocols,"observable_inputs_sha256":inputs,"scorer_inputs_sha256":scorer_inputs});
    let mut previous = BTreeMap::new();
    let mut before_digest = None;
    if let Some(before) = before {
        let text =
            fs::read_to_string(before.join("report.json")).map_err(|error| error.to_string())?;
        let report: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
        if report["contract"] != contract {
            return Err("before/after inputs, protocols or configuration differ".into());
        }
        let predictions = fs::read_to_string(before.join("predictions.jsonl"))
            .map_err(|error| error.to_string())?;
        before_digest = Some(
            json!({"report_sha256":normalized_hash(&text),"predictions_sha256":normalized_hash(&predictions)}),
        );
        for line in predictions.lines() {
            let record: Value = serde_json::from_str(line).map_err(|error| error.to_string())?;
            if previous.insert(key(&record)?, record).is_some() {
                return Err("duplicate baseline prediction".into());
            }
        }
    }
    let compare = before_digest.is_some();
    let mut rows = Vec::new();
    let mut predictions = String::new();
    let mut changes = String::new();
    let mut inventory = Vec::new();
    for (name, cases) in corpora {
        inventory.push(json!({"corpus":name,"families":cases.iter().map(|case|&case.family).collect::<BTreeSet<_>>().len(),"cases":cases.len(),"decisions":cases.iter().map(|case|case.labels.len()).sum::<usize>()}));
        for model in MODELS {
            for assignment in ASSIGNMENTS {
                let config = Config {
                    ambiguity_margin: 0.08,
                    max_candidates: 5,
                    one_to_one: assignment == "one_to_one",
                    min_score: if model == "name_only_exact" {
                        1.0
                    } else {
                        0.70
                    },
                    contextual_evidence: (model == "contextual")
                        .then_some(ContextualEvidence::default()),
                    name_conflicts: if model == "contextual" {
                        readiness::rules()?
                    } else {
                        Vec::new()
                    },
                    ..Config::default()
                };
                let engine = if model.starts_with("name_only") {
                    MatchEngine::with_matchers(
                        config,
                        vec![WeightedMatcher::new(1.0, NameMatcher::default())],
                    )
                } else {
                    MatchEngine::new(config)
                }
                .map_err(|error| error.to_string())?;
                let mut counts = Counts::default();
                let mut by_family = BTreeMap::<String, Counts>::new();
                let mut by_domain = BTreeMap::<String, Counts>::new();
                let mut by_variant = BTreeMap::<String, Counts>::new();
                let mut totals = Transitions::default();
                for case in &cases {
                    let report = engine
                        .match_schemas(&case.source, &case.target)
                        .map_err(|error| error.to_string())?;
                    let outcome = metrics::score_case(&case.labels, &report)?;
                    counts.add(&outcome);
                    by_family
                        .entry(case.family.clone())
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
                        .map(|field| {
                            let selected = field
                                .selected
                                .as_ref()
                                .map(|candidate| candidate.target.0.clone());
                            let label = case
                                .labels
                                .iter()
                                .find(|label| label.source == field.source.0)
                                .unwrap();
                            Selection {
                                source: field.source.0.clone(),
                                correct: label.kind == LabelKind::Match
                                    && selected
                                        .as_ref()
                                        .is_some_and(|target| label.targets.contains(target)),
                                selected,
                            }
                        })
                        .collect();
                    let record = json!({"corpus":name,"case":case.id,"model":model,"assignment":assignment,"fields":fields,"report_sha256":digest(format!("{report:?}").as_bytes())});
                    if compare {
                        let old = previous
                            .remove(&key(&record)?)
                            .ok_or("missing baseline case")?;
                        if model != "contextual" && old["report_sha256"] != record["report_sha256"]
                        {
                            return Err(format!("unchanged comparator drift: {}", key(&record)?));
                        }
                        let old_fields: Vec<Selection> =
                            serde_json::from_value(old["fields"].clone())
                                .map_err(|error| error.to_string())?;
                        if old_fields.len() != fields.len() {
                            return Err("source inventory changed".into());
                        }
                        for field in &fields {
                            let old_field = old_fields
                                .iter()
                                .find(|old| old.source == field.source)
                                .ok_or("baseline source missing")?;
                            transition(old_field, field, &mut totals);
                            if old_field.selected != field.selected {
                                let source_name = &case
                                    .source
                                    .fields
                                    .iter()
                                    .find(|source| source.id.0 == field.source)
                                    .unwrap()
                                    .name;
                                let target_name = |id: &Option<String>| {
                                    id.as_ref().and_then(|id| {
                                        case.target
                                            .fields
                                            .iter()
                                            .find(|target| target.id.0 == *id)
                                            .map(|target| target.name.clone())
                                    })
                                };
                                changes.push_str(&serde_json::to_string(&json!({"corpus":name,"case":case.id,"family":case.family,"variant":case.variant,"model":model,"assignment":assignment,"source_name":source_name,"before":old_field,"after":field,"before_target_name":target_name(&old_field.selected),"after_target_name":target_name(&field.selected)})).map_err(|error|error.to_string())?);
                                changes.push('\n');
                            }
                        }
                    }
                    predictions.push_str(
                        &serde_json::to_string(&record).map_err(|error| error.to_string())?,
                    );
                    predictions.push('\n');
                }
                let metrics = counts.metrics();
                let met = counts.proposals > 0
                    && metrics.precision.is_some_and(|value| value >= 0.95)
                    && metrics.unique_coverage.is_some_and(|value| value >= 0.60)
                    && metrics
                        .candidate_recall_at_5
                        .is_some_and(|value| value >= 0.90);
                rows.push(json!({"corpus":name,"model":model,"assignment":assignment,"counts":counts,"metrics":metrics,"targets_met_on_examined_data":met,"transitions_from_before":if compare {Some(&totals)} else {None},"by_family":by_family,"by_domain":by_domain,"by_variant":by_variant}));
            }
        }
    }
    if !previous.is_empty() {
        return Err("baseline contains extra cases".into());
    }
    let mut markdown=String::from("# Readiness regression\n\nExamined evidence only. Release decision: **not qualified**; no fresh independent qualification was performed. All variants and both assignment modes are retained. Labels reach the scorer after matching. T2D and corrective reserved partitions are untouched.\n\n| Corpus | Model | Assignment | Correct/proposed | Wrong | Abstentions | Precision | Unique coverage | Recall@5 | Retained | Lost | Recovered | New wrong |\n| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for row in &rows {
        let c = &row["counts"];
        let m = &row["metrics"];
        let t = &row["transitions_from_before"];
        markdown.push_str(&format!(
            "| {} | {} | {} | {}/{} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            row["corpus"].as_str().unwrap(),
            row["model"].as_str().unwrap(),
            row["assignment"].as_str().unwrap(),
            c["correct_proposals"],
            c["proposals"],
            c["proposals"].as_u64().unwrap() - c["correct_proposals"].as_u64().unwrap(),
            c["abstentions"],
            pct(m["precision"].as_f64()),
            pct(m["unique_coverage"].as_f64()),
            pct(m["candidate_recall_at_5"].as_f64()),
            t["retained_correct"],
            t["lost_correct"],
            t["recovered_correct"],
            t["introduced_wrong"]
        ));
    }
    let report = json!({"contract":contract,"inventory":inventory,"rows":rows,"metadata":{"engine_source_sha256":source_hashes(&root.join("src"))?,"evaluator_source_sha256":source_hashes(&root.join("evaluation/src"))?,"manifest_sha256":normalized_hash(&fs::read_to_string(root.join("Cargo.toml")).map_err(|error|error.to_string())?),"cargo_lock_sha256":normalized_hash(&fs::read_to_string(root.join("Cargo.lock")).map_err(|error|error.to_string())?),"before":before_digest,"examined_qualification_scored":true,"new_holdout_scored":false,"reserved_t2d_scored":false,"reserved_corrective_scored":false},"qualified":false});
    if !check {
        fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    }
    for (name, text) in [
        (
            "report.json",
            serde_json::to_string_pretty(&report).map_err(|error| error.to_string())? + "\n",
        ),
        ("report.md", markdown),
        ("predictions.jsonl", predictions),
        ("changes.jsonl", changes),
    ] {
        let path = output.join(name);
        if check {
            let expected = fs::read_to_string(&path).map_err(|error| error.to_string())?;
            if expected.replace("\r\n", "\n") != text.replace("\r\n", "\n") {
                return Err(format!(
                    "{} differs from the exact regression snapshot",
                    path.display()
                ));
            }
        } else {
            fs::write(path, text).map_err(|error| error.to_string())?;
        }
    }
    println!(
        "Checked or recorded same-input examined regression in {}; candidate remains not qualified",
        output.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scorer_fingerprint_binds_labels_and_variant_identity_separately_from_observations() {
        let mut cases = readiness::cases().unwrap().remove(0).1;
        let observations = readiness::observable_inputs_sha256(&cases).unwrap();
        let original = scorer_fingerprint(&cases).unwrap();
        cases[0].labels[0].targets = vec!["different-gold-target".into()];
        assert_ne!(original, scorer_fingerprint(&cases).unwrap());
        assert_eq!(
            observations,
            readiness::observable_inputs_sha256(&cases).unwrap()
        );
        let labels_changed = scorer_fingerprint(&cases).unwrap();
        cases[0].variant = "different-slice".into();
        assert_ne!(labels_changed, scorer_fingerprint(&cases).unwrap());
        assert_eq!(
            observations,
            readiness::observable_inputs_sha256(&cases).unwrap()
        );
    }

    #[test]
    fn wrong_to_correct_and_correct_to_abstention_are_counted_separately() {
        let selected = |target: Option<&str>, correct| Selection {
            source: "s".into(),
            selected: target.map(str::to_owned),
            correct,
        };
        let mut counts = Transitions::default();
        transition(
            &selected(Some("wrong"), false),
            &selected(Some("right"), true),
            &mut counts,
        );
        transition(
            &selected(Some("right"), true),
            &selected(None, false),
            &mut counts,
        );
        transition(
            &selected(None, false),
            &selected(Some("wrong"), false),
            &mut counts,
        );
        assert_eq!(
            counts,
            Transitions {
                lost_correct: 1,
                recovered_correct: 1,
                removed_wrong: 1,
                introduced_wrong: 1,
                ..Transitions::default()
            }
        );
    }
}
