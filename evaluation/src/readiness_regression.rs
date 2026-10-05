//! Same-input comparison; the published qualification corpus is regression only.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use fieldkin::{
    CandidateIssue, Config, ContextualEvidence, Decision, FieldDiagnostic, MatchEngine,
    NameMatcher, WeightedMatcher,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::metrics::Counts;
use crate::model::LabelKind;
use crate::{context_qualification, metrics, readiness};

const MODELS: [&str; 4] = ["default", "name_only_070", "name_only_exact", "contextual"];
const ASSIGNMENTS: [&str; 2] = ["independent", "one_to_one"];

fn configuration(model: &str, assignment: &str) -> Result<Config, String> {
    if model == "contextual_quality" {
        let mut config = Config::contextual_quality();
        config.one_to_one = assignment == "one_to_one";
        return Ok(config);
    }
    let contextual = model.starts_with("contextual");
    let mut evidence = ContextualEvidence::default();
    if model == "contextual_relaxed_identifier" {
        evidence.strict_identifier_samples = false;
    }
    evidence.distinguish_relationships =
        model == "contextual_relationship_repair" || model == "contextual_identifier_word_forms";
    evidence.identifier_word_forms = model == "contextual_identifier_word_forms";
    evidence.independent_sample_populations = model == "contextual_population_repair";
    evidence.preserve_score_ranking = model == "contextual_ranking_repair";
    if model == "contextual_unscoped_support" || model == "contextual_relaxed_unscoped" {
        evidence.scoped_support = false;
    }
    if model == "contextual_relaxed_unscoped" {
        evidence.strict_identifier_samples = false;
    }
    Ok(Config {
        ambiguity_margin: 0.08,
        max_candidates: 5,
        one_to_one: assignment == "one_to_one",
        min_score: if model == "name_only_exact" {
            1.0
        } else {
            0.70
        },
        contextual_evidence: contextual.then_some(evidence),
        name_conflicts: if contextual {
            readiness::rules()?
        } else {
            Vec::new()
        },
        ..Config::default()
    })
}

fn model_configuration(model: &str) -> Result<Value, String> {
    let mut value = crate::quality::configuration_json(&configuration(model, "independent")?);
    if model.starts_with("name_only") {
        value["weights"] = json!({"name":1.0});
        value["built_in_signals"] = json!({"name_aliases":NameMatcher::default().aliases});
    }
    Ok(value)
}

/// Runtime issue codes may overlap. The scorer observes them after matching;
/// labels never participate in runtime diagnostics or selection.
fn decision_reasons(field: &fieldkin::FieldMatch) -> BTreeSet<String> {
    let mut reasons = BTreeSet::new();
    for candidate in &field.candidates {
        for issue in &candidate.issues {
            match issue {
                CandidateIssue::IncompatibleTypes
                | CandidateIssue::SemanticConflict(_)
                | CandidateIssue::NameConflict(_) => {
                    reasons.insert("semantic_or_representation_conflict".into());
                }
                CandidateIssue::InsufficientScore => {
                    reasons.insert("score_threshold".into());
                }
                CandidateIssue::InsufficientSampleSupport => {
                    reasons.insert("insufficient_sample_support".into());
                }
                CandidateIssue::InsufficientContextSupport => {
                    reasons.insert("contextual_support_unspecified".into());
                }
                CandidateIssue::ContextualReason(reason) => {
                    reasons.insert(format!("{reason:?}"));
                }
                _ => {}
            }
        }
    }
    if field.decision == Decision::Ambiguous
        || field.diagnostics.contains(&FieldDiagnostic::LocalAmbiguity)
    {
        reasons.insert("local_ambiguity".into());
    }
    if field.decision == Decision::AssignmentConflict
        || field.diagnostics.iter().any(|reason| {
            matches!(
                reason,
                FieldDiagnostic::Displaced { .. } | FieldDiagnostic::UnassignedByGlobalConstraint
            )
        })
    {
        reasons.insert("global_assignment_displacement".into());
    }
    reasons
}

fn unique_failures(
    case: &readiness::Case,
    report: &fieldkin::MatchReport,
) -> Result<Vec<Value>, String> {
    let mut failures = Vec::new();
    for label in &case.labels {
        if label.kind != LabelKind::Match {
            continue;
        }
        let field = report
            .fields
            .iter()
            .find(|field| field.source.0 == label.source)
            .ok_or("missing diagnostic source")?;
        let gold = &label.targets[0];
        if field
            .selected
            .as_ref()
            .is_some_and(|selected| selected.target.0 == *gold)
        {
            continue;
        }
        let candidate = field
            .candidates
            .iter()
            .find(|candidate| candidate.target.0 == *gold);
        let mut gold_view = field.clone();
        gold_view.candidates = candidate.cloned().into_iter().collect();
        let mut reasons = decision_reasons(&gold_view);
        if field.selected.is_some() {
            reasons.insert("wrong_target_selected".into());
        }
        if candidate.is_some_and(|candidate| candidate.eligible) && field.selected.is_some() {
            reasons.insert(
                if report.one_to_one {
                    "eligible_gold_not_selected_in_global_assignment"
                } else {
                    "another_candidate_ranked_first"
                }
                .into(),
            );
        }
        if candidate.is_none() {
            reasons.insert("outside_displayed_candidates".into());
        }
        let source = case
            .source
            .fields
            .iter()
            .find(|source| source.id.0 == label.source)
            .ok_or("missing diagnostic source input")?;
        let target = case
            .target
            .fields
            .iter()
            .find(|target| target.id.0 == *gold)
            .ok_or("missing diagnostic target input")?;
        let profile = |field: &fieldkin::Field| {
            let samples = field.samples.as_deref().unwrap_or(&[]);
            let non_null = samples
                .iter()
                .filter(|sample| !matches!(sample, fieldkin::SampleValue::Null))
                .count();
            let distinct = samples
                .iter()
                .filter(|sample| !matches!(sample, fieldkin::SampleValue::Null))
                .map(|sample| {
                    let mut single = field.clone();
                    single.samples = Some(vec![sample.clone()]);
                    serde_json::to_string(&crate::readiness::observable_schema(
                        &fieldkin::Schema::new(vec![single]),
                    ))
                    .expect("observable sample serialization")
                })
                .collect::<BTreeSet<_>>()
                .len();
            json!({"observations":samples.len(),"non_null":non_null,"distinct":distinct,"declared_type":format!("{:?}",field.data_type)})
        };
        let base_score = candidate.map(|candidate| {
            candidate
                .signals
                .iter()
                .map(|signal| signal.weight * signal.evidence.score.unwrap_or(0.0))
                .sum::<f64>()
        });
        let (min, max) = field
            .candidates
            .iter()
            .filter(|alternative| {
                alternative.eligible && field.alternatives.contains(&alternative.target)
            })
            .map(|alternative| {
                alternative
                    .signals
                    .iter()
                    .map(|signal| signal.weight * signal.evidence.score.unwrap_or(0.0))
                    .sum::<f64>()
            })
            .fold((f64::INFINITY, f64::NEG_INFINITY), |(min, max), value| {
                (min.min(value), max.max(value))
            });
        let floor_compression = field.decision == Decision::Ambiguous
            && candidate.is_some_and(|candidate| candidate.score > base_score.unwrap_or(0.0))
            && max - min >= 0.08;
        failures.push(json!({"source":label.source,"source_name":source.name,"gold_target":gold,"gold_target_name":target.name,"actual_selected":field.selected.as_ref().map(|selected|&selected.target.0),"actual_decision":format!("{:?}",field.decision),
            "gold_candidate_eligible":candidate.map(|candidate|candidate.eligible),"gold_candidate_score":candidate.map(|candidate|candidate.score),"weighted_base_score":base_score,"overlapping_reasons":reasons,
            "gold_candidate_issues":candidate.map(|candidate|candidate.issues.iter().map(|issue|format!("{issue:?}")).collect::<Vec<_>>()),"source_profile":profile(source),"target_profile":profile(target),"score_floor_compression":floor_compression}));
    }
    Ok(failures)
}

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

pub(crate) fn scorer_fingerprint(cases: &[readiness::Case]) -> Result<String, String> {
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
    let mut experiment = None;
    let mut include_examined = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => output = Some(PathBuf::from(args.next().ok_or("missing output")?)),
            "--before" => before = Some(PathBuf::from(args.next().ok_or("missing before")?)),
            "--check" => check = true,
            "--include-examined-regression" => include_examined = true,
            "--policies" => experiment = Some(args.next().ok_or("--policies needs a comma-separated list")?),
            _ => return Err("regression accepts --output DIR, --before DIR, --policies LIST and --check; reserved holdouts are unavailable".into()),
        }
    }
    let output = output.ok_or("--output is required")?;
    let experimental = experiment.is_some();
    let models: Vec<&str> = experiment
        .as_ref()
        .map_or_else(|| MODELS.to_vec(), |value| value.split(',').collect());
    let mut distinct = BTreeSet::new();
    if models.is_empty()
        || models.iter().any(|model| {
            ![
                "default",
                "name_only_070",
                "name_only_exact",
                "contextual",
                "contextual_relaxed_identifier",
                "contextual_relationship_repair",
                "contextual_population_repair",
                "contextual_ranking_repair",
                "contextual_quality",
                "contextual_unscoped_support",
                "contextual_relaxed_unscoped",
                "contextual_identifier_word_forms",
            ]
            .contains(model)
                || !distinct.insert(*model)
        })
    {
        return Err("unknown or duplicate development policy".into());
    }
    if output.exists() && !check {
        return Err("regression requires a fresh output directory".into());
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let mut corpora = readiness::cases()?;
    if !experimental || include_examined {
        corpora.push((
            "examined_qualification".into(),
            context_qualification::examined_cases()?,
        ));
    }
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
    let model_configuration: BTreeMap<_, _> = models.iter().map(|&model| (model, if experimental { model_configuration(model).expect("validated runtime configuration") } else { json!({
        "min_score":if model=="name_only_exact" {1.0} else {0.70}, "ambiguity_margin":0.08,"top_k":5,
        "weights":if model.starts_with("name_only") {json!({"name":1.0})} else {json!({"name":0.65,"type":0.20,"sample":0.15})},
        "contextual_evidence":if model.starts_with("contextual") {json!({"strict_identifier_samples":model!="contextual_relaxed_identifier","scoped_support":true})} else {Value::Null},
        "name_conflicts":if model.starts_with("contextual") {"precision-protocol.json"} else {"disabled"}
    })})).collect();
    let contract = json!({"models":models,"model_configuration":model_configuration,"assignments":ASSIGNMENTS,"threshold":0.70,"ambiguity_margin":0.08,"top_k":5,
        "quality_targets":{"precision":0.95,"unique_coverage":0.60,"candidate_recall_at_5":0.90,"nonzero_proposals":true},
        "protocol_sha256":protocols,"observable_inputs_sha256":inputs,"scorer_inputs_sha256":scorer_inputs});
    let mut previous = BTreeMap::new();
    let mut before_digest = None;
    if let Some(before) = before {
        let text =
            fs::read_to_string(before.join("report.json")).map_err(|error| error.to_string())?;
        let report: Value = serde_json::from_str(&text).map_err(|error| error.to_string())?;
        let baseline_contract = &report["contract"];
        let same_controls = [
            "assignments",
            "threshold",
            "ambiguity_margin",
            "top_k",
            "quality_targets",
            "protocol_sha256",
        ]
        .iter()
        .all(|name| baseline_contract[name] == contract[name])
            && inputs
                .iter()
                .all(|(name, hash)| baseline_contract["observable_inputs_sha256"][name] == *hash)
            && scorer_inputs
                .iter()
                .all(|(name, hash)| baseline_contract["scorer_inputs_sha256"][name] == *hash);
        if (!experimental && baseline_contract != &contract) || (experimental && !same_controls) {
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
        for &model in &models {
            for assignment in ASSIGNMENTS {
                let config = configuration(model, assignment)?;
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
                let mut reasons = BTreeMap::<String, usize>::new();
                let mut by_sample_availability = BTreeMap::<String, Counts>::new();
                let mut missed_unique_reasons = BTreeMap::<String, usize>::new();
                let mut score_floor_compression_cases = 0;
                for case in &cases {
                    let report = engine
                        .match_schemas(&case.source, &case.target)
                        .map_err(|error| error.to_string())?;
                    let outcome = metrics::score_case(&case.labels, &report)?;
                    let failures = if experimental {
                        unique_failures(case, &report)?
                    } else {
                        Vec::new()
                    };
                    for failure in &failures {
                        for reason in failure["overlapping_reasons"]
                            .as_array()
                            .ok_or("missing structured failure reasons")?
                        {
                            *missed_unique_reasons
                                .entry(reason.as_str().ok_or("invalid reason")?.to_owned())
                                .or_default() += 1;
                        }
                        score_floor_compression_cases +=
                            usize::from(failure["score_floor_compression"] == true);
                    }
                    for field in &report.fields {
                        for reason in decision_reasons(field) {
                            *reasons.entry(reason).or_default() += 1;
                        }
                        let label = case
                            .labels
                            .iter()
                            .find(|label| label.source == field.source.0)
                            .ok_or("missing slice label")?;
                        let source = case
                            .source
                            .fields
                            .iter()
                            .find(|source| source.id.0 == field.source.0)
                            .ok_or("missing slice source")?;
                        let available = source.samples.as_ref().is_some_and(|samples| {
                            samples
                                .iter()
                                .any(|sample| !matches!(sample, fieldkin::SampleValue::Null))
                        });
                        let slice = if available {
                            "observed_source_samples"
                        } else {
                            "unavailable_source_samples"
                        };
                        let mut single = report.clone();
                        single.fields = vec![field.clone()];
                        by_sample_availability
                            .entry(slice.into())
                            .or_default()
                            .add(&metrics::score_case(std::slice::from_ref(label), &single)?);
                    }
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
                    let mut record = json!({"corpus":name,"case":case.id,"model":model,"assignment":assignment,"fields":fields,"report_sha256":digest(format!("{report:?}").as_bytes())});
                    if experimental {
                        record["diagnostics"] = json!(report.fields.iter().map(|field| json!({"source":field.source.0,"decision":format!("{:?}",field.decision),"reasons":decision_reasons(field),"candidate_reasons":field.candidates.iter().map(|candidate|json!({"target":candidate.target.0,"eligible":candidate.eligible,"score":candidate.score,"issues":candidate.issues.iter().map(|issue|format!("{issue:?}")).collect::<Vec<_>>()})).collect::<Vec<_>>()})).collect::<Vec<_>>());
                        record["missed_unique_diagnostics"] = json!(failures);
                    }
                    if compare {
                        let mut baseline_key = record.clone();
                        if experimental && model.starts_with("contextual") {
                            baseline_key["model"] = json!("contextual");
                        }
                        let old = if experimental {
                            previous.get(&key(&baseline_key)?).cloned()
                        } else {
                            previous.remove(&key(&record)?)
                        }
                        .ok_or("missing baseline case")?;
                        if !experimental
                            && model != "contextual"
                            && old["report_sha256"] != record["report_sha256"]
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
                let mut row = json!({"corpus":name,"model":model,"assignment":assignment,"counts":counts,"metrics":metrics,"targets_met_on_examined_data":met,"transitions_from_before":if compare {Some(&totals)} else {None},"by_family":by_family,"by_domain":by_domain,"by_variant":by_variant});
                if experimental {
                    row["overlapping_decision_reasons"] = json!(reasons);
                    row["by_sample_availability"] = json!(by_sample_availability);
                    row["missed_unique_overlapping_reasons"] = json!(missed_unique_reasons);
                    row["score_floor_compression_cases"] = json!(score_floor_compression_cases);
                }
                rows.push(row);
            }
        }
    }
    if !experimental && !previous.is_empty() {
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
    let report = json!({"contract":contract,"inventory":inventory,"rows":rows,"metadata":{"engine_source_sha256":source_hashes(&root.join("src"))?,"evaluator_source_sha256":source_hashes(&root.join("evaluation/src"))?,"manifest_sha256":normalized_hash(&fs::read_to_string(root.join("Cargo.toml")).map_err(|error|error.to_string())?),"cargo_lock_sha256":normalized_hash(&fs::read_to_string(root.join("Cargo.lock")).map_err(|error|error.to_string())?),"before":before_digest,"examined_qualification_scored":!experimental || include_examined,"new_holdout_scored":false,"reserved_t2d_scored":false,"reserved_corrective_scored":false},"qualified":false});
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
    fn controlled_policy_metadata_matches_actual_signals_and_public_preset() {
        for model in ["name_only_070", "name_only_exact"] {
            let metadata = model_configuration(model).unwrap();
            assert_eq!(metadata["weights"], json!({"name":1.0}));
            assert_eq!(metadata["built_in_signals"].as_object().unwrap().len(), 1);
        }
        assert_eq!(
            model_configuration("contextual_quality").unwrap(),
            crate::quality::candidate_configuration()
        );
    }

    #[test]
    fn missed_gold_reasons_overlap_without_importing_wrong_alternative_conflicts() {
        let case = readiness::cases().unwrap().remove(0).1.remove(0);
        let label = case
            .labels
            .iter()
            .find(|label| label.kind == LabelKind::Match)
            .unwrap();
        let mut report =
            MatchEngine::new(configuration("contextual_quality", "independent").unwrap())
                .unwrap()
                .match_schemas(&case.source, &case.target)
                .unwrap();
        let field = report
            .fields
            .iter_mut()
            .find(|field| field.source.0 == label.source)
            .unwrap();
        field.selected = None;
        field.decision = Decision::InsufficientEvidence;
        field.diagnostics.clear();
        for candidate in &mut field.candidates {
            candidate.issues = if candidate.target.0 == label.targets[0] {
                vec![
                    CandidateIssue::ContextualReason(
                        fieldkin::ContextualReason::MissingIdentifierSamples,
                    ),
                    CandidateIssue::ContextualReason(
                        fieldkin::ContextualReason::CompetingCandidate,
                    ),
                ]
            } else {
                vec![CandidateIssue::NameConflict(
                    fieldkin::NameConflictKind::Unit,
                )]
            };
        }
        let failures = unique_failures(&case, &report).unwrap();
        let failure = failures
            .iter()
            .find(|failure| failure["source"] == label.source)
            .unwrap();
        let reasons = failure["overlapping_reasons"].as_array().unwrap();
        assert!(reasons.contains(&json!("MissingIdentifierSamples")));
        assert!(reasons.contains(&json!("CompetingCandidate")));
        assert!(!reasons.contains(&json!("semantic_or_representation_conflict")));
    }

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
