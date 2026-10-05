//! Fixed precision policies on the unchanged original development dataset.
//! No CLI option can score holdouts, alter thresholds or supply answer-derived hints.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use fieldkin::{
    Config, Corroboration, MatchEngine, MatchReport, NameConflictKind, NameConflictRule,
    NameMatcher, SampleMatcher, SampleReliability, TypeMatcher, WeightedMatcher,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::corpus;
use crate::metrics::{score_case, Counts, Metrics};
use crate::model::{Case, LabelKind};

const PROTOCOL: &str = include_str!("../precision-protocol.json");
const BASELINE_REPORT: &str = include_str!("../results/continuation-v3/stage3/report.json");
const BASELINE_PREDICTIONS: &str =
    include_str!("../results/continuation-v3/stage3/predictions.jsonl");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule {
    kind: String,
    alternatives: Vec<Vec<String>>,
}

#[derive(Default, Serialize)]
struct Transitions {
    retained_correct: usize,
    lost_correct: usize,
    new_correct: usize,
    removed_false: usize,
    added_false: usize,
}

#[derive(Serialize)]
struct Row {
    model: String,
    assignment: String,
    counts: Counts,
    metrics: Metrics,
    transitions: Transitions,
    decisions: BTreeMap<String, usize>,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn hash(text: &str) -> String {
    format!(
        "{:x}",
        Sha256::digest(text.replace("\r\n", "\n").as_bytes())
    )
}

fn source_hashes(directory: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.extension().is_some_and(|extension| extension == "rs") {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or("invalid source path")?;
            result.insert(
                name.to_owned(),
                hash(&fs::read_to_string(&path).map_err(|error| error.to_string())?),
            );
        }
    }
    Ok(result)
}

fn options(args: Vec<String>) -> Result<(PathBuf, bool), String> {
    let mut output = root().join("target/fieldkin-precision");
    let mut check = false;
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => output = args.next().ok_or("--output needs a directory")?.into(),
            "--check" => check = true,
            _ => return Err(format!("unsupported precision argument {arg}; only --output DIR and --check are allowed; holdouts remain reserved")),
        }
    }
    Ok((output, check))
}

fn development_cases() -> Result<Vec<Case>, String> {
    // Generate in original family order to preserve the exact original seeds.
    // Structural validation/generation do not score the reserved cases.
    Ok(corpus::cases(&corpus::families()?)?
        .into_iter()
        .filter(|case| case.split == "development")
        .collect())
}

fn engine(protocol: &Value, model: &str, one_to_one: bool) -> Result<MatchEngine, String> {
    if !["default", "conflicts", "support", "conflicts_and_support"].contains(&model) {
        return Err("unknown precision policy".into());
    }
    let rules: Vec<Rule> =
        serde_json::from_value(protocol["rules"].clone()).map_err(|error| error.to_string())?;
    let rules = rules
        .into_iter()
        .map(|rule| {
            Ok(NameConflictRule {
                kind: match rule.kind.as_str() {
                    "qualifier" => NameConflictKind::Qualifier,
                    "unit" => NameConflictKind::Unit,
                    _ => return Err("unknown name conflict kind".to_owned()),
                },
                alternatives: rule.alternatives,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    let number = |value: &Value, key: &str| {
        value[key]
            .as_f64()
            .ok_or_else(|| format!("missing numeric setting {key}"))
    };
    let config = Config {
        min_score: number(protocol, "min_score")?,
        ambiguity_margin: number(protocol, "ambiguity_margin")?,
        max_candidates: protocol["max_candidates"].as_u64().ok_or("missing top-k")? as usize,
        one_to_one,
        name_conflicts: if ["conflicts", "conflicts_and_support"].contains(&model) {
            rules
        } else {
            Vec::new()
        },
        corroboration: if ["support", "conflicts_and_support"].contains(&model) {
            Some(Corroboration {
                min_name_score: number(&protocol["corroboration"], "min_name_score")?,
                min_sample_score: number(&protocol["corroboration"], "min_sample_score")?,
            })
        } else {
            None
        },
        ..Config::default()
    };
    let aliases = protocol["aliases"]
        .as_object()
        .ok_or("missing aliases")?
        .iter()
        .map(|(key, value)| {
            Ok((
                key.clone(),
                value.as_str().ok_or("invalid alias")?.to_owned(),
            ))
        })
        .collect::<Result<_, String>>()?;
    let weights = &protocol["weights"];
    MatchEngine::with_matchers(
        config,
        vec![
            WeightedMatcher::new(number(weights, "name")?, NameMatcher { aliases }),
            WeightedMatcher::new(number(weights, "type")?, TypeMatcher),
            WeightedMatcher::new(
                number(weights, "samples")?,
                SampleMatcher {
                    min_non_null: protocol["sample_min_non_null"]
                        .as_u64()
                        .ok_or("missing sample minimum")?
                        as usize,
                    reliability: SampleReliability::Distinct,
                },
            ),
        ],
    )
    .map_err(|error| error.to_string())
}

pub(crate) fn baseline_digests() -> Result<BTreeMap<(String, String), String>, String> {
    let mut digests = BTreeMap::new();
    for line in BASELINE_PREDICTIONS.lines() {
        let record: Value = serde_json::from_str(line).map_err(|error| error.to_string())?;
        if record["corpus"] == "original_development" && record["model"] == "combined" {
            let string = |key: &str| {
                record[key]
                    .as_str()
                    .map(str::to_owned)
                    .ok_or_else(|| format!("missing baseline {key}"))
            };
            digests.insert(
                (string("assignment")?, string("case")?),
                string("report_sha256")?,
            );
        }
    }
    if digests.len() != 320 {
        return Err("baseline must have exactly 320 development reports".into());
    }
    Ok(digests)
}

pub(crate) fn verify_baseline(assignment: &str, counts: &Counts) -> Result<(), String> {
    let baseline: Value =
        serde_json::from_str(BASELINE_REPORT).map_err(|error| error.to_string())?;
    let row = baseline["threshold_rows"]
        .as_array()
        .ok_or("missing baseline rows")?
        .iter()
        .find(|row| {
            row["corpus"] == "original_development"
                && row["model"] == "combined"
                && row["assignment"] == assignment
                && row["threshold"] == 0.7
        })
        .ok_or("missing default baseline row")?;
    if serde_json::to_value(counts).map_err(|error| error.to_string())? != row["counts"] {
        return Err(format!(
            "current default differs from frozen {assignment} baseline"
        ));
    }
    Ok(())
}

fn transitions(
    case: &Case,
    baseline: &MatchReport,
    current: &MatchReport,
    totals: &mut Transitions,
) -> Result<(), String> {
    for label in &case.labels {
        let find = |report: &MatchReport| {
            report
                .fields
                .iter()
                .find(|field| field.source.0 == label.source)
                .map(|field| {
                    field
                        .selected
                        .as_ref()
                        .map(|selected| selected.target.0.clone())
                })
        };
        let before = find(baseline).ok_or("missing baseline field")?;
        let after = find(current).ok_or("missing candidate field")?;
        let correct = |selected: &Option<String>| {
            label.kind == LabelKind::Match
                && selected
                    .as_ref()
                    .is_some_and(|target| label.targets.contains(target))
        };
        let before_correct = correct(&before);
        let after_correct = correct(&after);
        let before_false = before.is_some() && !before_correct;
        let after_false = after.is_some() && !after_correct;
        totals.retained_correct += usize::from(before_correct && after_correct);
        totals.lost_correct += usize::from(before_correct && !after_correct);
        totals.new_correct += usize::from(!before_correct && after_correct);
        totals.removed_false += usize::from(before_false && !after_false);
        totals.added_false += usize::from(!before_false && after_false);
    }
    Ok(())
}

fn percent(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".into(), |value| format!("{:.2}%", value * 100.0))
}

fn markdown(rows: &[Row]) -> String {
    let mut text = String::from("# Fixed precision policies on original development\n\n160 unchanged cases / 960 labeled decisions: 730 unique, 190 unmatched, 40 ambiguous. No holdout scored. Default counts and all 320 complete report digests verified against the frozen current-default baseline. No score, threshold, label, split or alias tuning.\n\n| Policy | Assignment | Correct/proposed | Precision | Unique coverage | Overall coverage | Wrong unique | Unmatched proposals | Ambiguous proposals | Lost correct | Removed false |\n| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for row in rows {
        text.push_str(&format!(
            "| {} | {} | {}/{} | {} | {} | {} | {} | {}/190 | {}/40 | {} | {} |\n",
            row.model,
            row.assignment,
            row.counts.correct_proposals,
            row.counts.proposals,
            percent(row.metrics.precision),
            percent(row.metrics.unique_coverage),
            percent(row.metrics.proposal_coverage),
            row.counts.wrong_unique_proposals,
            row.counts.no_match_proposals,
            row.counts.ambiguous_proposals,
            row.transitions.lost_correct,
            row.transitions.removed_false
        ));
    }
    text.push_str("\nUnique coverage counts selections on uniquely labeled fields, including wrong targets. Overall coverage counts all proposals / 960. Sample-support policies trade useful matches for abstention and cannot detect hidden scopes with identical observations. Empty proposals would have undefined precision. Defaults remain unchanged. These are descriptive synthetic development results, not independent validation.\n");
    text
}

fn write_or_check(path: &Path, text: &str, check: bool) -> Result<(), String> {
    if path.exists() {
        let existing = fs::read_to_string(path).map_err(|error| error.to_string())?;
        if existing.replace("\r\n", "\n") != text.replace("\r\n", "\n") {
            return Err(format!(
                "{} differs; preserve historical artifacts and use a new output directory",
                path.display()
            ));
        }
    } else if check {
        return Err(format!("missing {}", path.display()));
    } else {
        fs::write(path, text).map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn run(args: Vec<String>) -> Result<(), String> {
    let (output, check) = options(args)?;
    let protocol: Value = serde_json::from_str(PROTOCOL).map_err(|error| error.to_string())?;
    let cases = development_cases()?;
    let digests = baseline_digests()?;
    let models = protocol["models"].as_array().ok_or("missing models")?;
    let mut rows = Vec::new();
    let mut predictions = String::new();
    for assignment in ["independent", "one_to_one"] {
        let one_to_one = assignment == "one_to_one";
        let default = engine(&protocol, "default", one_to_one)?;
        let baselines = cases
            .iter()
            .map(|case| {
                default
                    .match_schemas(
                        &corpus::schema(&case.source)?,
                        &corpus::schema(&case.target)?,
                    )
                    .map_err(|error| error.to_string())
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (case, report) in cases.iter().zip(&baselines) {
            if digests.get(&(assignment.into(), case.id.clone()))
                != Some(&hash(&format!("{report:?}")))
            {
                return Err(format!(
                    "default full report differs for development case {} / {assignment}",
                    case.id
                ));
            }
        }
        for model in models {
            let model = model.as_str().ok_or("invalid policy name")?;
            let matcher = engine(&protocol, model, one_to_one)?;
            let mut counts = Counts::default();
            let mut changes = Transitions::default();
            let mut decisions = BTreeMap::<String, usize>::new();
            for (case, baseline) in cases.iter().zip(&baselines) {
                // Only names, IDs, declared types and samples cross this boundary.
                let report = matcher
                    .match_schemas(
                        &corpus::schema(&case.source)?,
                        &corpus::schema(&case.target)?,
                    )
                    .map_err(|error| error.to_string())?;
                counts.add(&score_case(&case.labels, &report)?);
                transitions(case, baseline, &report, &mut changes)?;
                for field in &report.fields {
                    *decisions
                        .entry(format!("{:?}", field.decision))
                        .or_default() += 1;
                }
                let fields: Vec<_> = report.fields.iter().map(|field| json!({"source":field.source.0,"selected":field.selected.as_ref().map(|candidate|&candidate.target.0),"decision":format!("{:?}",field.decision)})).collect();
                let record = json!({"model":model,"assignment":assignment,"case":case.id,"fields":fields,"report_sha256":hash(&format!("{report:?}"))});
                predictions
                    .push_str(&serde_json::to_string(&record).map_err(|error| error.to_string())?);
                predictions.push('\n');
            }
            if model == "default" {
                verify_baseline(assignment, &counts)?;
            }
            rows.push(Row {
                model: model.into(),
                assignment: assignment.into(),
                metrics: counts.metrics(),
                counts,
                transitions: changes,
                decisions,
            });
        }
    }
    let lock = fs::read_to_string(root().join("Cargo.lock")).map_err(|error| error.to_string())?;
    let manifests = ["Cargo.toml", "evaluation/Cargo.toml"]
        .into_iter()
        .map(|name| {
            Ok((
                name,
                hash(&fs::read_to_string(root().join(name)).map_err(|error| error.to_string())?),
            ))
        })
        .collect::<Result<BTreeMap<_, _>, String>>()?;
    let summary = json!({
        "metadata": {"protocol":protocol,"protocol_sha256":hash(PROTOCOL),"original_protocol_sha256":hash(corpus::PROTOCOL),"original_corpus_sha256":corpus::FILES.iter().map(|(name,text)|((*name).to_owned(),hash(text))).collect::<BTreeMap<_,_>>(),"development_cases_sha256":hash(&serde_json::to_string(&cases).map_err(|error|error.to_string())?),"baseline_report_sha256":hash(BASELINE_REPORT),"baseline_predictions_sha256":hash(BASELINE_PREDICTIONS),"default_full_reports_verified":320,"holdout_scored":false,"engine_source_sha256":source_hashes(&root().join("src"))?,"evaluator_source_sha256":source_hashes(&root().join("evaluation/src"))?,"manifest_sha256":manifests,"cargo_lock_sha256":hash(&lock),"cargo_lock":lock.replace("\r\n","\n")},
        "inventory":{"families":cases.iter().map(|case|&case.family_id).collect::<BTreeSet<_>>().len(),"cases":cases.len(),"decisions":cases.iter().map(|case|case.labels.len()).sum::<usize>()},"rows":rows
    });
    if !check {
        fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    }
    write_or_check(
        &output.join("report.json"),
        &(serde_json::to_string_pretty(&summary).map_err(|error| error.to_string())? + "\n"),
        check,
    )?;
    write_or_check(&output.join("report.md"), &markdown(&rows), check)?;
    write_or_check(&output.join("predictions.jsonl"), &predictions, check)?;
    println!(
        "{} fixed precision development evidence in {}",
        if check { "Verified" } else { "Recorded" },
        output.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn precision_runner_cannot_opt_into_holdouts_or_tuning() {
        for args in [
            vec!["--split", "holdout"],
            vec!["--acknowledge-holdout"],
            vec!["--min-score", "0.9"],
            vec!["--stage3"],
        ] {
            assert!(options(args.into_iter().map(str::to_owned).collect()).is_err());
        }
        let cases = development_cases().unwrap();
        assert_eq!(cases.len(), 160);
        assert!(cases.iter().all(|case| case.split == "development"));
        assert_eq!(
            cases
                .iter()
                .map(|case| &case.family_id)
                .collect::<BTreeSet<_>>()
                .len(),
            32
        );
    }

    #[test]
    fn answer_metadata_never_enters_matcher_schemas() {
        let case = development_cases().unwrap().remove(0);
        let before = corpus::schema(&case.source).unwrap();
        let mut fields = case.source;
        for field in &mut fields {
            field.concept = "deliberately incorrect answer metadata".into();
        }
        assert_eq!(before, corpus::schema(&fields).unwrap());
        assert!(before
            .fields
            .iter()
            .all(|field| field.hints == Default::default()));
    }

    #[test]
    fn frozen_policies_preserve_every_default_report() {
        let protocol: Value = serde_json::from_str(PROTOCOL).unwrap();
        let digests = baseline_digests().unwrap();
        let cases = development_cases().unwrap();
        for assignment in ["independent", "one_to_one"] {
            let matcher = engine(&protocol, "default", assignment == "one_to_one").unwrap();
            let mut counts = Counts::default();
            for case in &cases {
                let report = matcher
                    .match_schemas(
                        &corpus::schema(&case.source).unwrap(),
                        &corpus::schema(&case.target).unwrap(),
                    )
                    .unwrap();
                assert_eq!(
                    digests[&(assignment.into(), case.id.clone())],
                    hash(&format!("{report:?}"))
                );
                counts.add(&score_case(&case.labels, &report).unwrap());
            }
            verify_baseline(assignment, &counts).unwrap();
        }
        for model in ["conflicts", "support", "conflicts_and_support"] {
            engine(&protocol, model, false).unwrap();
        }
    }
}
