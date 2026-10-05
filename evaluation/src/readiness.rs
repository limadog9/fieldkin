//! Development-only release-candidate exploration. Labels are scorer inputs only.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use fieldkin::{
    Config, ContextualEvidence, DataType, Decision, MatchEngine, MatchReport, NameConflictKind,
    NameConflictRule, SampleValue, Schema,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::context_policy::{self, Policy};
use crate::metrics::{score_case, Counts, Metrics};
use crate::model::{Label, LabelKind};
use crate::{corpus, corrective_corpus, precision, stage3};

const PROTOCOL: &str = include_str!("../readiness-protocol.json");
const CONFLICT_PROTOCOL: &str = include_str!("../precision-protocol.json");

pub(crate) struct Case {
    pub(crate) id: String,
    pub(crate) family: String,
    pub(crate) domain: String,
    pub(crate) variant: String,
    pub(crate) source: Schema,
    pub(crate) target: Schema,
    pub(crate) labels: Vec<Label>,
}

#[derive(Serialize)]
struct Row {
    corpus: String,
    model: String,
    assignment: String,
    counts: Counts,
    metrics: Metrics,
    quality_targets_met: bool,
    transitions_from_default: Transitions,
    by_family: BTreeMap<String, Counts>,
    by_domain: BTreeMap<String, Counts>,
    by_variant: BTreeMap<String, Counts>,
}

#[derive(Default, Serialize, Debug, PartialEq, Eq)]
struct Transitions {
    preserved_correct: usize,
    lost_correct: usize,
    new_correct: usize,
    removed_false: usize,
    new_false: usize,
}

// These private views are serialized only into the input fingerprint, never into
// result artifacts. Debug is unsuitable here because it deliberately redacts
// SampleValue and SemanticHints. Tag every sample kind and retain exact values.
#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
enum ObservableSample<'a> {
    Null,
    Boolean(bool),
    NumberBits(u64),
    Integer(i128),
    Decimal { coefficient: i128, scale: u8 },
    Text(&'a str),
}

impl<'a> From<&'a SampleValue> for ObservableSample<'a> {
    fn from(sample: &'a SampleValue) -> Self {
        match sample {
            SampleValue::Null => Self::Null,
            SampleValue::Boolean(value) => Self::Boolean(*value),
            SampleValue::Number(value) => Self::NumberBits(value.to_bits()),
            SampleValue::Integer(value) => Self::Integer(*value),
            SampleValue::Decimal(value) => Self::Decimal {
                coefficient: value.coefficient(),
                scale: value.scale(),
            },
            SampleValue::Text(value) => Self::Text(value),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct ObservableField<'a> {
    id: &'a str,
    name: &'a str,
    data_type: &'static str,
    samples: Option<Vec<ObservableSample<'a>>>,
    unit: Option<&'a str>,
    currency: Option<&'a str>,
    identifier_scope: Option<&'a str>,
}

pub(crate) fn observable_schema(schema: &Schema) -> Vec<ObservableField<'_>> {
    schema
        .fields
        .iter()
        .map(|field| ObservableField {
            id: &field.id.0,
            name: &field.name,
            data_type: match field.data_type {
                DataType::Unknown => "unknown",
                DataType::Boolean => "boolean",
                DataType::Integer => "integer",
                DataType::Float => "float",
                DataType::Decimal => "decimal",
                DataType::Text => "text",
                DataType::Date => "date",
                DataType::Timestamp => "timestamp",
                DataType::Binary => "binary",
            },
            samples: field
                .samples
                .as_ref()
                .map(|samples| samples.iter().map(ObservableSample::from).collect()),
            unit: field.hints.unit.as_deref(),
            currency: field.hints.currency.as_deref(),
            identifier_scope: field.hints.identifier_scope.as_deref(),
        })
        .collect()
}

pub(crate) fn observable_inputs_sha256(cases: &[Case]) -> Result<String, String> {
    let schemas: Vec<_> = cases
        .iter()
        .map(|case| {
            (
                observable_schema(&case.source),
                observable_schema(&case.target),
            )
        })
        .collect();
    let bytes = serde_json::to_vec(&schemas).map_err(|error| error.to_string())?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

fn require_automatic_report(report: &MatchReport) -> Result<(), String> {
    if report
        .fields
        .iter()
        .any(|field| field.decision == Decision::Confirmed)
    {
        return Err("readiness reports cannot contain caller-confirmed selections".into());
    }
    Ok(())
}

fn transitions(
    labels: &[Label],
    baseline: &MatchReport,
    candidate: &MatchReport,
    totals: &mut Transitions,
) -> Result<(), String> {
    fn selected<'a>(report: &'a MatchReport, source: &str) -> Option<Option<&'a String>> {
        report
            .fields
            .iter()
            .find(|field| field.source.0 == source)
            .map(|field| field.selected.as_ref().map(|selected| &selected.target.0))
    }
    for label in labels {
        let before = selected(baseline, &label.source).ok_or("missing default transition field")?;
        let after =
            selected(candidate, &label.source).ok_or("missing candidate transition field")?;
        let correct = |selected: Option<&String>| {
            label.kind == LabelKind::Match
                && selected.is_some_and(|target| label.targets.contains(target))
        };
        let before_correct = correct(before);
        let after_correct = correct(after);
        let before_false = before.is_some() && !before_correct;
        let after_false = after.is_some() && !after_correct;
        totals.preserved_correct += usize::from(before_correct && after_correct);
        totals.lost_correct += usize::from(before_correct && !after_correct);
        totals.new_correct += usize::from(!before_correct && after_correct);
        totals.removed_false += usize::from(before_false && !after_false);
        totals.new_false += usize::from(!before_false && after_false);
    }
    Ok(())
}

fn failed_quality_targets(counts: &Counts) -> Vec<&'static str> {
    let metrics = counts.metrics();
    let mut failures = Vec::new();
    if counts.proposals == 0 {
        failures.push("nonzero proposals");
    }
    if !metrics.precision.is_some_and(|value| value >= 0.95) {
        failures.push("proposal precision >=95%");
    }
    if !metrics.unique_coverage.is_some_and(|value| value >= 0.60) {
        failures.push("unique-match coverage >=60%");
    }
    if !metrics
        .candidate_recall_at_5
        .is_some_and(|value| value >= 0.90)
    {
        failures.push("candidate recall@5 >=90%");
    }
    failures
}

fn release_blockers(rows: &[Row]) -> Vec<String> {
    let mut blockers = vec![
        "Development exploration does not perform frozen prospective qualification; see the separate qualification report".into(),
        "Synthetic development data alone does not certify production accuracy".into(),
    ];
    for row in rows
        .iter()
        .filter(|row| row.model == "runtime_context_strict_identifiers" && !row.quality_targets_met)
    {
        let scope = if row.corpus == "original_development" {
            "Development"
        } else {
            "Development stress"
        };
        blockers.push(format!(
            "{scope} targets failed for {} / {} / {}: {}",
            row.corpus,
            row.model,
            row.assignment,
            failed_quality_targets(&row.counts).join(", ")
        ));
    }
    blockers
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

fn source_hashes(dir: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(dir).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.extension().is_some_and(|extension| extension == "rs") {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or("invalid source path")?;
            files.insert(
                name.to_owned(),
                hash(&fs::read_to_string(&path).map_err(|error| error.to_string())?),
            );
        }
    }
    Ok(files)
}

fn options(args: Vec<String>) -> Result<(PathBuf, bool), String> {
    let mut args = args.into_iter();
    let mut output = root().join("target/fieldkin-readiness");
    let mut check = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output"=>output=args.next().ok_or("--output needs a directory")?.into(),
            "--check"=>check=true,
            _=>return Err("readiness exploration accepts only --output DIR and --check; no holdout or tuning options".into()),
        }
    }
    Ok((output, check))
}

fn protocol() -> Result<Value, String> {
    let value: Value = serde_json::from_str(PROTOCOL).map_err(|error| error.to_string())?;
    if value["models"]
        != json!([
            "default",
            "conflicts",
            "distinctive_context",
            "distinctive_context_strict_identifiers",
            "scoped_context",
            "scoped_context_strict_identifiers",
            "qualified_context",
            "qualified_context_strict_identifiers",
            "runtime_context_strict_identifiers"
        ])
        || value["nominated_candidate"] != "runtime_context_strict_identifiers"
        || value["assignments"] != json!(["independent", "one_to_one"])
        || value["threshold"] != 0.70
        || value["ambiguity_margin"] != 0.08
        || value["sample_min_distinct"] != 3
        || value["sample_min_coverage"] != 0.25
        || value["sample_min_jaccard"] != 0.90
        || value["sample_mutual_margin"] != 0.10
        || value["informative_lexical_score"] != 0.90
        || value["distinctive_sample_score"] != 0.95
        || value["scoped_context_rules"]
            != json!({"observed_nonboolean_lexical_requires_distinctive_samples":true,"bare_single_core_nonidentifier_numeric_requires_representation":true,"temporal_recovery_requires_shared_informative_role":true,"identical_source_observations_do_not_compete":true})
        || value["qualified_context_refinements"]
            != json!({"exact_nonidentifier_lexical_uses_adequacy_without_mutual_contrast":true,"empty_or_all_null_samples_are_unavailable_for_nonidentifier_lexical_support":true,"integral_declared_integer_does_not_trigger_measurement_heuristic":true})
        || value["quality_targets"]
            != json!({"precision":0.95,"unique_coverage":0.60,"candidate_recall_at_5":0.90})
    {
        return Err("readiness protocol differs from the fixed implemented experiment".into());
    }
    Ok(value)
}

pub(crate) fn cases() -> Result<Vec<(String, Vec<Case>)>, String> {
    let convert = |case: crate::model::Case| -> Result<Case, String> {
        Ok(Case {
            id: case.id,
            family: case.family_id,
            domain: case.domain,
            variant: case.variant,
            source: corpus::schema(&case.source)?,
            target: corpus::schema(&case.target)?,
            labels: case.labels,
        })
    };
    let original = corpus::cases(&corpus::families()?)?
        .into_iter()
        .filter(|case| case.split == "development")
        .map(convert)
        .collect::<Result<Vec<_>, _>>()?;
    let corrective = corrective_corpus::cases()?
        .into_iter()
        .filter(|case| case.split == "development")
        .map(convert)
        .collect::<Result<Vec<_>, _>>()?;
    let extension = stage3::extension_cases()?
        .into_iter()
        .map(|mut case| {
            // Hints are caller information; the roadmap's quality bar excludes them.
            for field in case.source.fields.iter_mut().chain(&mut case.target.fields) {
                field.hints = Default::default();
            }
            Case {
                id: case.id,
                family: case.family,
                domain: "extension".into(),
                variant: "base".into(),
                source: case.source,
                target: case.target,
                labels: case.labels,
            }
        })
        .collect();
    Ok(vec![
        ("original_development".into(), original),
        ("stage3_extension_without_hints".into(), extension),
        ("corrective_development".into(), corrective),
    ])
}

pub(crate) fn rules() -> Result<Vec<NameConflictRule>, String> {
    let protocol: Value =
        serde_json::from_str(CONFLICT_PROTOCOL).map_err(|error| error.to_string())?;
    protocol["rules"]
        .as_array()
        .ok_or("missing conflict rules")?
        .iter()
        .map(|rule| {
            Ok(NameConflictRule {
                kind: match rule["kind"].as_str() {
                    Some("qualifier") => NameConflictKind::Qualifier,
                    Some("unit") => NameConflictKind::Unit,
                    _ => return Err("invalid conflict kind".into()),
                },
                alternatives: serde_json::from_value(rule["alternatives"].clone())
                    .map_err(|error| error.to_string())?,
            })
        })
        .collect()
}

fn match_case(case: &Case, model: &str, one_to_one: bool) -> Result<MatchReport, String> {
    match model {
        "default" | "conflicts" => MatchEngine::new(Config {
            one_to_one,
            name_conflicts: if model == "conflicts" {
                rules()?
            } else {
                Vec::new()
            },
            ..Config::default()
        })
        .map_err(|error| error.to_string())?
        .match_schemas(&case.source, &case.target)
        .map_err(|error| error.to_string()),
        "distinctive_context"
        | "distinctive_context_strict_identifiers"
        | "scoped_context"
        | "scoped_context_strict_identifiers"
        | "qualified_context"
        | "qualified_context_strict_identifiers" => context_policy::match_schemas(
            &case.source,
            &case.target,
            Policy {
                require_identifier_samples: model.ends_with("strict_identifiers"),
                strict_observed_support: model.starts_with("scoped_context")
                    || model.starts_with("qualified_context"),
                refined_lexical_support: model.starts_with("qualified_context"),
            },
            rules()?,
            one_to_one,
        ),
        "runtime_context_strict_identifiers" => MatchEngine::new(Config {
            one_to_one,
            contextual_evidence: Some(ContextualEvidence::default()),
            name_conflicts: rules()?,
            ..Config::default()
        })
        .map_err(|error| error.to_string())?
        .match_schemas(&case.source, &case.target)
        .map_err(|error| error.to_string()),
        _ => Err("unknown readiness model".into()),
    }
}

fn quality(counts: &Counts) -> bool {
    failed_quality_targets(counts).is_empty()
}

fn pct(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".into(), |value| format!("{:.2}%", value * 100.0))
}

fn markdown(rows: &[Row]) -> String {
    let mut text=String::from("# Release-readiness development exploration\n\nV1 distinctive-context policies preceded their first run. V2 scoped-context policies were developed from V1 development failures. V3 qualified-context refinements and their opt-in runtime port were developed from V2 development failures and useful-match losses, then declared before V3 scoring. Earlier policies remain as comparators. Existing datasets and labels are unchanged. Caller hints and confirmations are excluded. No holdout was scored in this exploration. Evidence derives from observable inputs, never from labels. The frozen V3 prototype is verified against its archived complete development reports; the current runtime has separate exact snapshots and a same-input before/after regression report. Development success does not establish release readiness; the historical failed prospective qualification is preserved separately.\n\n| Corpus | Policy | Assignment | Correct/proposed | Precision | Recall | Unique coverage | Overall coverage | Candidate recall@5 | Wrong unique | No-match proposals | Ambiguous proposals | Development targets met |\n| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |\n");
    for row in rows {
        text.push_str(&format!(
            "| {} | {} | {} | {}/{} | {} | {} | {} | {} | {} | {} | {}/{} | {}/{} | {} |\n",
            row.corpus,
            row.model,
            row.assignment,
            row.counts.correct_proposals,
            row.counts.proposals,
            pct(row.metrics.precision),
            pct(row.metrics.recall),
            pct(row.metrics.unique_coverage),
            pct(row.metrics.proposal_coverage),
            pct(row.metrics.candidate_recall_at_5),
            row.counts.wrong_unique_proposals,
            row.counts.no_match_proposals,
            row.counts.no_match_fields,
            row.counts.ambiguous_proposals,
            row.counts.ambiguous_fields,
            row.quality_targets_met
        ));
    }
    text.push_str("\nCompared with the default on the same corpus and assignment policy:\n\n| Corpus | Policy | Assignment | Preserved correct | Lost correct | New correct | Removed false | New false |\n| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |\n");
    for row in rows {
        let transitions = &row.transitions_from_default;
        text.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |\n",
            row.corpus,
            row.model,
            row.assignment,
            transitions.preserved_correct,
            transitions.lost_correct,
            transitions.new_correct,
            transitions.removed_false,
            transitions.new_false,
        ));
    }
    text.push_str("\nRelease decision: **not qualified**. Historical published holdouts cannot be retuned and called fresh.\n\n");
    for blocker in release_blockers(rows) {
        text.push_str(&format!("- {blocker}\n"));
    }
    text
}

fn write_or_check(path: &Path, text: &str, check: bool) -> Result<(), String> {
    if path.exists() {
        if fs::read_to_string(path)
            .map_err(|error| error.to_string())?
            .replace("\r\n", "\n")
            != text.replace("\r\n", "\n")
        {
            return Err(format!(
                "{} differs; preserve historical evidence and choose a new output directory",
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
    let protocol = protocol()?;
    let corpora = cases()?;
    let models = protocol["models"].as_array().ok_or("missing models")?;
    let baseline_digests = precision::baseline_digests()?;
    let mut rows = Vec::new();
    let mut predictions = String::new();
    let mut inventory = Vec::new();
    for (name, cases) in &corpora {
        inventory.push(json!({"corpus":name,"families":cases.iter().map(|case|&case.family).collect::<BTreeSet<_>>().len(),"cases":cases.len(),"decisions":cases.iter().map(|case|case.labels.len()).sum::<usize>()}));
        let mut default_reports = BTreeMap::new();
        for model in models {
            let model = model.as_str().ok_or("invalid model")?;
            for assignment in ["independent", "one_to_one"] {
                let mut counts = Counts::default();
                let mut families = BTreeMap::<String, Counts>::new();
                let mut domains = BTreeMap::<String, Counts>::new();
                let mut variants = BTreeMap::<String, Counts>::new();
                let mut changed = Transitions::default();
                for case in cases {
                    let report = match_case(case, model, assignment == "one_to_one")?;
                    require_automatic_report(&report)?;
                    if name == "original_development"
                        && model == "default"
                        && baseline_digests.get(&(assignment.into(), case.id.clone()))
                            != Some(&hash(&format!("{report:?}")))
                    {
                        return Err(format!(
                            "default baseline full report changed for {} / {assignment}",
                            case.id
                        ));
                    }
                    let outcome = score_case(&case.labels, &report)?;
                    if model == "default" {
                        default_reports.insert((assignment, &case.id), report.clone());
                    }
                    let baseline = default_reports
                        .get(&(assignment, &case.id))
                        .ok_or("default comparison report is missing")?;
                    transitions(&case.labels, baseline, &report, &mut changed)?;
                    counts.add(&outcome);
                    families
                        .entry(case.family.clone())
                        .or_default()
                        .add(&outcome);
                    domains
                        .entry(case.domain.clone())
                        .or_default()
                        .add(&outcome);
                    variants
                        .entry(case.variant.clone())
                        .or_default()
                        .add(&outcome);
                    let fields:Vec<_>=report.fields.iter().map(|field|json!({"source":field.source.0,"decision":format!("{:?}",field.decision),"selected":field.selected.as_ref().map(|candidate|&candidate.target.0)})).collect();
                    let record = json!({"corpus":name,"case":case.id,"model":model,"assignment":assignment,"fields":fields,"report_sha256":hash(&format!("{report:?}"))});
                    predictions.push_str(
                        &serde_json::to_string(&record).map_err(|error| error.to_string())?,
                    );
                    predictions.push('\n');
                }
                if name == "original_development" && model == "default" {
                    precision::verify_baseline(assignment, &counts)?;
                }
                rows.push(Row {
                    corpus: name.clone(),
                    model: model.into(),
                    assignment: assignment.into(),
                    metrics: counts.metrics(),
                    quality_targets_met: quality(&counts),
                    transitions_from_default: changed,
                    counts,
                    by_family: families,
                    by_domain: domains,
                    by_variant: variants,
                });
            }
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
    let observable_inputs: BTreeMap<_, _> = corpora
        .iter()
        .map(|(name, cases)| Ok((name.clone(), observable_inputs_sha256(cases)?)))
        .collect::<Result<_, String>>()?;
    let blockers = release_blockers(&rows);
    let report = json!({"metadata":{"protocol":protocol,"protocol_sha256":hash(PROTOCOL),"conflict_protocol_sha256":hash(CONFLICT_PROTOCOL),"original_protocol_sha256":hash(corpus::PROTOCOL),"original_corpus_sha256":corpus::FILES.iter().map(|(name,text)|((*name).to_owned(),hash(text))).collect::<BTreeMap<_,_>>(),"corrective_source_sha256":hash(corrective_corpus::SOURCE_TEXT),"stage3_extension_sha256":hash(include_str!("../corpus/stage3-evidence.json")),"engine_source_sha256":source_hashes(&root().join("src"))?,"evaluator_source_sha256":source_hashes(&root().join("evaluation/src"))?,"manifest_sha256":manifests,"cargo_lock_sha256":hash(&lock),"cargo_lock":lock.replace("\r\n","\n"),"observable_inputs_sha256":observable_inputs,"default_full_reports_verified":320,"holdout_scored":false,"caller_hints_used":false,"caller_confirmations_used":false},"inventory":inventory,"rows":rows,"release_ready":false,"release_blockers":blockers});
    if !check {
        fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    }
    write_or_check(
        &output.join("report.json"),
        &(serde_json::to_string_pretty(&report).map_err(|error| error.to_string())? + "\n"),
        check,
    )?;
    write_or_check(&output.join("report.md"), &markdown(&rows), check)?;
    write_or_check(&output.join("predictions.jsonl"), &predictions, check)?;
    println!(
        "{} development-only readiness evidence in {}; release qualification remains pending",
        if check { "Verified" } else { "Recorded" },
        output.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use fieldkin::{Candidate, ExactDecimal, Field, FieldMatch};

    #[test]
    fn fixed_prototype_preserves_every_archived_development_report() {
        // The current runtime intentionally evolves beyond this prototype. Keep
        // the historical comparator fixed and verify its complete reports; the
        // current runtime has its own exact CI snapshots and adversarial tests.
        let archived: BTreeMap<_, _> = include_str!("../results/readiness-v3/predictions.jsonl")
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).unwrap())
            .filter(|record| record["model"] == "qualified_context_strict_identifiers")
            .map(|record| {
                (
                    (
                        record["corpus"].as_str().unwrap().to_owned(),
                        record["case"].as_str().unwrap().to_owned(),
                        record["assignment"].as_str().unwrap().to_owned(),
                    ),
                    record["report_sha256"].as_str().unwrap().to_owned(),
                )
            })
            .collect();
        let mut checked = 0;
        for (corpus, cases) in cases().unwrap() {
            for case in cases {
                for one_to_one in [false, true] {
                    let prototype =
                        match_case(&case, "qualified_context_strict_identifiers", one_to_one)
                            .unwrap();
                    let assignment = if one_to_one {
                        "one_to_one"
                    } else {
                        "independent"
                    };
                    assert_eq!(
                        archived.get(&(corpus.clone(), case.id.clone(), assignment.into())),
                        Some(&hash(&format!("{prototype:?}"))),
                        "{} / {assignment}",
                        case.id
                    );
                    checked += 1;
                }
            }
        }
        assert_eq!(checked, archived.len());
    }

    fn synthetic_case(samples: Option<Vec<SampleValue>>) -> Case {
        let mut source = Field::new("s", "amount", DataType::Decimal);
        source.samples = samples;
        Case {
            id: "synthetic-case".into(),
            family: "synthetic-family".into(),
            domain: "synthetic-domain".into(),
            variant: "base".into(),
            source: Schema::new(vec![source]),
            target: Schema::new(vec![Field::new("t", "amount", DataType::Decimal)]),
            labels: vec![Label {
                source: "s".into(),
                kind: LabelKind::Match,
                targets: vec!["t".into()],
                rationale: "Synthetic test annotation".into(),
            }],
        }
    }

    fn selected_report(selections: &[Option<&str>]) -> MatchReport {
        MatchReport {
            fields: selections
                .iter()
                .enumerate()
                .map(|(index, target)| FieldMatch {
                    source: format!("s{index}").into(),
                    candidates: Vec::new(),
                    alternatives: Vec::new(),
                    selected: target.map(|target| Candidate {
                        target: target.into(),
                        score: 1.0,
                        eligible: true,
                        signals: Vec::new(),
                        warnings: Vec::new(),
                        issues: Vec::new(),
                    }),
                    decision: if target.is_some() {
                        Decision::Proposed
                    } else {
                        Decision::BelowThreshold
                    },
                    diagnostics: Vec::new(),
                })
                .collect(),
            unmatched_sources: Vec::new(),
            unmatched_targets: Vec::new(),
            one_to_one: false,
            target_competition: Vec::new(),
            assignment_diagnostics: Default::default(),
        }
    }

    #[test]
    fn observable_fingerprint_changes_when_only_sample_values_change() {
        for (before, after) in [
            (SampleValue::Boolean(false), SampleValue::Boolean(true)),
            (
                SampleValue::Number(1.0),
                SampleValue::Number(1.0 + f64::EPSILON),
            ),
            (SampleValue::Number(0.0), SampleValue::Number(-0.0)),
            (
                SampleValue::Integer(9_007_199_254_740_992),
                SampleValue::Integer(9_007_199_254_740_993),
            ),
            (
                SampleValue::Integer(i128::MAX - 1),
                SampleValue::Integer(i128::MAX),
            ),
            (
                SampleValue::Decimal(ExactDecimal::new(1, 38).unwrap()),
                SampleValue::Decimal(ExactDecimal::new(2, 38).unwrap()),
            ),
            (
                SampleValue::Text("synthetic-private-value-a".into()),
                SampleValue::Text("synthetic-private-value-b".into()),
            ),
            (SampleValue::Null, SampleValue::Text("null".into())),
            (SampleValue::Integer(1), SampleValue::Number(1.0)),
            (
                SampleValue::Integer(1),
                SampleValue::Decimal(ExactDecimal::new(1, 0).unwrap()),
            ),
        ] {
            let before = synthetic_case(Some(vec![before]));
            let after = synthetic_case(Some(vec![after]));
            let fingerprint = observable_inputs_sha256(&[before]).unwrap();
            assert_ne!(fingerprint, observable_inputs_sha256(&[after]).unwrap());
            assert_eq!(fingerprint.len(), 64);
            assert!(fingerprint.bytes().all(|byte| byte.is_ascii_hexdigit()));
            assert!(!fingerprint.contains("synthetic-private-value"));
        }
        let unavailable = observable_inputs_sha256(&[synthetic_case(None)]).unwrap();
        let empty = observable_inputs_sha256(&[synthetic_case(Some(Vec::new()))]).unwrap();
        let null =
            observable_inputs_sha256(&[synthetic_case(Some(vec![SampleValue::Null]))]).unwrap();
        assert_ne!(unavailable, empty);
        assert_ne!(empty, null);
    }

    #[test]
    fn label_and_fixture_metadata_cannot_change_observable_fingerprint() {
        let mut case = synthetic_case(Some(vec![SampleValue::Integer(123)]));
        let before = observable_inputs_sha256(std::slice::from_ref(&case)).unwrap();
        case.labels[0].source = "different annotation source".into();
        case.labels[0].targets.clear();
        case.labels[0].kind = LabelKind::NoMatch;
        case.labels[0].rationale = "deliberately incorrect answer metadata".into();
        case.id = "different case metadata".into();
        case.family = "different family metadata".into();
        case.domain = "different domain metadata".into();
        case.variant = "different variant metadata".into();
        assert_eq!(
            before,
            observable_inputs_sha256(std::slice::from_ref(&case)).unwrap()
        );
        // Target samples are inputs too; neither side is omitted from the hash.
        case.target.fields[0].samples = Some(vec![SampleValue::Integer(123)]);
        assert_ne!(
            before,
            observable_inputs_sha256(std::slice::from_ref(&case)).unwrap()
        );
    }

    #[test]
    fn transitions_count_preservation_loss_recovery_and_false_proposals() {
        let labels: Vec<_> = (0..7)
            .map(|index| Label {
                source: format!("s{index}"),
                kind: match index {
                    3 => LabelKind::NoMatch,
                    4 => LabelKind::Ambiguous,
                    _ => LabelKind::Match,
                },
                targets: match index {
                    3 => Vec::new(),
                    4 => vec!["a".into(), "b".into()],
                    _ => vec!["t".into()],
                },
                rationale: "Synthetic transition annotation".into(),
            })
            .collect();
        let baseline = selected_report(&[
            Some("t"),
            Some("t"),
            None,
            Some("x"),
            None,
            Some("wrong"),
            Some("t"),
        ]);
        let candidate = selected_report(&[
            Some("t"),
            None,
            Some("t"),
            None,
            Some("a"),
            Some("t"),
            Some("wrong"),
        ]);
        let mut changed = Transitions::default();
        transitions(&labels, &baseline, &candidate, &mut changed).unwrap();
        assert_eq!(
            changed,
            Transitions {
                preserved_correct: 1,
                lost_correct: 2,
                new_correct: 2,
                removed_false: 2,
                new_false: 2,
            }
        );
        // Unsupported selection on an ambiguous label remains a false proposal,
        // including when the selected target is listed as plausible.
        assert_eq!(
            score_case(&labels, &candidate).unwrap().ambiguous_proposals,
            1
        );
        let missing = selected_report(&[]);
        assert!(transitions(&labels, &baseline, &missing, &mut changed).is_err());
    }

    #[test]
    fn confirmed_selections_are_rejected_before_readiness_scoring() {
        let mut report = selected_report(&[Some("t")]);
        assert!(require_automatic_report(&report).is_ok());
        report.fields[0].decision = Decision::Confirmed;
        assert!(require_automatic_report(&report).is_err());
    }

    #[test]
    fn release_blockers_report_failed_development_stress_targets() {
        let counts = Counts {
            fields: 100,
            unique_fields: 100,
            proposals: 50,
            correct_proposals: 40,
            unique_proposals: 50,
            candidate_relevant: 100,
            candidate_hits: 100,
            ..Default::default()
        };
        let row = Row {
            corpus: "corrective_development".into(),
            model: "runtime_context_strict_identifiers".into(),
            assignment: "independent".into(),
            metrics: counts.metrics(),
            quality_targets_met: quality(&counts),
            counts,
            transitions_from_default: Transitions::default(),
            by_family: BTreeMap::new(),
            by_domain: BTreeMap::new(),
            by_variant: BTreeMap::new(),
        };
        let blockers = release_blockers(std::slice::from_ref(&row));
        let stress = blockers
            .iter()
            .find(|text| text.contains("Development stress"))
            .unwrap();
        assert!(stress
            .contains("corrective_development / runtime_context_strict_identifiers / independent"));
        assert!(stress.contains("proposal precision >=95%"));
        assert!(stress.contains("unique-match coverage >=60%"));
        assert!(!stress.contains("candidate recall@5"));
        assert!(markdown(&[row]).contains(stress));
    }

    #[test]
    fn empty_or_incomplete_quality_cannot_pass() {
        assert!(!quality(&Counts::default()));
        let counts = Counts {
            fields: 100,
            unique_fields: 100,
            proposals: 60,
            correct_proposals: 57,
            unique_proposals: 60,
            candidate_relevant: 100,
            candidate_hits: 90,
            ..Default::default()
        };
        assert!(quality(&counts));
        for changed in [
            Counts {
                correct_proposals: 56,
                ..counts.clone()
            },
            Counts {
                unique_proposals: 59,
                ..counts.clone()
            },
            Counts {
                candidate_hits: 89,
                ..counts.clone()
            },
        ] {
            assert!(!quality(&changed));
        }
    }

    #[test]
    fn exploration_excludes_holdouts_hints_and_tuning_options() {
        for flag in [
            "--holdout",
            "--acknowledge-holdout",
            "--min-score",
            "--hints",
        ] {
            assert!(options(vec![flag.into()]).is_err());
        }
        let corpora = cases().unwrap();
        assert_eq!(corpora[0].1.len(), 160);
        assert_eq!(corpora[2].1.len(), 36);
        for (_, cases) in corpora {
            for case in cases {
                assert!(case
                    .source
                    .fields
                    .iter()
                    .chain(&case.target.fields)
                    .all(|field| field.hints == Default::default()));
            }
        }
    }
}
