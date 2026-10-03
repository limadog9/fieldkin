//! Frozen release qualification with an explicit, one-time holdout scoring gate.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use fieldkin::{
    Config, FieldMatch, GlobalDiagnosticsConfig, Limits, MatchEngine, NameMatcher, SampleMatcher,
    SampleReliability, SemanticHints, TypeMatcher, WeightedMatcher,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::corpus;
use crate::metrics::{score_case, Counts, Metrics};
use crate::model::{Case, Label, LabelKind};

const PROTOCOL: &str = include_str!("../release-protocol.json");
const MODELS: [&str; 2] = ["combined", "name_only"];
const ASSIGNMENTS: [&str; 2] = ["independent", "one_to_one"];

struct Options {
    output: PathBuf,
    check: bool,
    acknowledge_holdout: bool,
}

impl Options {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut options = Self {
            output: root().join("target/fieldkin-release"),
            check: false,
            acknowledge_holdout: false,
        };
        let mut args = args.into_iter();
        while let Some(argument) = args.next() {
            match argument.as_str() {
                "--output" => options.output = args.next().ok_or("--output needs a directory")?.into(),
                "--check" => options.check = true,
                "--acknowledge-holdout" => options.acknowledge_holdout = true,
                _ => return Err("release evaluation accepts --output DIR, --check, and --acknowledge-holdout only".into()),
            }
        }
        if options.check && options.acknowledge_holdout {
            return Err(
                "--check is development-only and cannot acknowledge holdout scoring".into(),
            );
        }
        Ok(options)
    }

    fn partitions(&self) -> &[&'static str] {
        if self.acknowledge_holdout {
            &["development", "holdout"]
        } else {
            &["development"]
        }
    }
}

#[derive(Clone, Serialize)]
struct Row {
    model: String,
    assignment: String,
    threshold: f64,
    counts: Counts,
    metrics: Metrics,
}

#[derive(Serialize)]
struct GroupRow {
    model: String,
    assignment: String,
    group_by: String,
    group: String,
    counts: Counts,
    metrics: Metrics,
}

#[derive(Serialize)]
struct OperatingPoint {
    assignment: String,
    precision_anchor: String,
    minimum_precision: Option<f64>,
    model: String,
    status: String,
    selected: Option<Row>,
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn number(value: &Value, key: &str) -> Result<f64, String> {
    value[key]
        .as_f64()
        .filter(|value| value.is_finite())
        .ok_or_else(|| format!("invalid numeric protocol setting {key}"))
}

fn size(value: &Value, key: &str) -> Result<usize, String> {
    value[key]
        .as_u64()
        .and_then(|value| usize::try_from(value).ok())
        .ok_or_else(|| format!("invalid integer protocol setting {key}"))
}

fn boolean(value: &Value, key: &str) -> Result<bool, String> {
    value[key]
        .as_bool()
        .ok_or_else(|| format!("invalid boolean protocol setting {key}"))
}

fn protocol() -> Result<Value, String> {
    let protocol: Value = serde_json::from_str(PROTOCOL).map_err(|error| error.to_string())?;
    if protocol["models"] != json!(MODELS)
        || protocol["assignment_modes"] != json!(ASSIGNMENTS)
        || protocol["combined"]["sample_reliability"] != "distinct"
        || protocol["precision_anchors"] != json!(["combined_default", "fixed_0.95"])
    {
        return Err("release protocol model, assignment or anchor definition changed".into());
    }
    thresholds(&protocol)?;
    Ok(protocol)
}

fn thresholds(protocol: &Value) -> Result<Vec<f64>, String> {
    let grid = protocol["threshold_grid"]
        .as_array()
        .ok_or("missing threshold grid")?
        .iter()
        .map(|value| {
            value
                .as_f64()
                .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
                .ok_or_else(|| "invalid threshold".to_owned())
        })
        .collect::<Result<Vec<_>, _>>()?;
    if grid.len() != 21
        || grid
            .iter()
            .enumerate()
            .any(|(index, value)| (*value - (0.5 + index as f64 * 0.025)).abs() > f64::EPSILON)
        || !grid.contains(&number(protocol, "default_threshold")?)
    {
        return Err("release threshold grid must remain .50 through 1.00 in .025 steps".into());
    }
    Ok(grid)
}

fn engine(
    protocol: &Value,
    model: &str,
    one_to_one: bool,
    threshold: f64,
) -> Result<MatchEngine, String> {
    if !MODELS.contains(&model) {
        return Err("unknown release model".into());
    }
    let common = &protocol["common"];
    let limits = &protocol["limits"];
    let global = &common["global_diagnostics"];
    let config = Config {
        min_score: threshold,
        ambiguity_margin: number(common, "ambiguity_margin")?,
        max_candidates: size(common, "max_candidates")?,
        one_to_one,
        abstain_on_ambiguity: boolean(common, "abstain_on_ambiguity")?,
        reject_incompatible_types: boolean(&protocol[model], "reject_incompatible_types")?,
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
        .map(|(from, to)| {
            to.as_str()
                .map(|to| (from.clone(), to.to_owned()))
                .ok_or_else(|| "invalid alias".to_owned())
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let weights = &protocol[model]["weights"];
    let mut matchers = vec![WeightedMatcher::new(
        number(weights, "name")?,
        NameMatcher { aliases },
    )];
    if model == "combined" {
        matchers.push(WeightedMatcher::new(number(weights, "type")?, TypeMatcher));
        matchers.push(WeightedMatcher::new(
            number(weights, "samples")?,
            SampleMatcher {
                min_non_null: size(&protocol[model], "sample_min_non_null")?,
                reliability: SampleReliability::Distinct,
            },
        ));
    }
    MatchEngine::with_matchers(config, matchers).map_err(|error| error.to_string())
}

fn cases_in_partition(partition: &str) -> Result<Vec<Case>, String> {
    if !["development", "holdout"].contains(&partition) {
        return Err("unknown release partition".into());
    }
    // Structural validation is shared; matching happens only after partitioning.
    let cases: Vec<_> = corpus::cases(&corpus::families()?)?
        .into_iter()
        .filter(|case| case.split == partition)
        .collect();
    let expected = if partition == "development" { 160 } else { 40 };
    if cases.len() != expected {
        return Err("frozen release partition size changed".into());
    }
    Ok(cases)
}

fn groups(case: &Case) -> Vec<(String, String)> {
    let mut groups = vec![
        ("all".into(), "all".into()),
        ("domain".into(), case.domain.clone()),
        ("variant".into(), case.variant.clone()),
        ("family".into(), case.family_id.clone()),
    ];
    for tag in case.tags.iter().collect::<BTreeSet<_>>() {
        groups.push(("scenario_tag".into(), tag.clone()));
    }
    groups
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

fn prediction(
    case: &Case,
    model: &str,
    assignment: &str,
    field: &FieldMatch,
    label: &Label,
) -> Value {
    json!({
        "case":case.id,"family":case.family_id,"domain":case.domain,"variant":case.variant,
        "model":model,"assignment":assignment,"source":field.source.0,
        "gold_kind":label.kind,"gold_targets":label.targets,"outcome":outcome(label,field),
        "decision":format!("{:?}",field.decision),
        "diagnostics":field.diagnostics.iter().map(|value|format!("{value:?}")).collect::<Vec<_>>(),
        "selected":field.selected.as_ref().map(|candidate|json!({"target":candidate.target.0,"score":candidate.score})),
        "alternatives":field.alternatives.iter().map(|id|&id.0).collect::<Vec<_>>(),
        "candidates":field.candidates.iter().map(|candidate|json!({
            "target":candidate.target.0,"score":candidate.score,"eligible":candidate.eligible,
            "signals":candidate.signals.iter().map(|signal|json!({"name":signal.name,"weight":signal.weight,"score":signal.evidence.score})).collect::<Vec<_>>()
        })).collect::<Vec<_>>()
    })
}

fn best_at_precision<'a>(rows: impl Iterator<Item = &'a Row>, minimum: f64) -> Option<&'a Row> {
    rows.filter(|row| {
        row.counts.proposals > 0
            && row
                .metrics
                .precision
                .is_some_and(|precision| precision >= minimum)
    })
    .max_by(|left, right| {
        left.counts
            .correct_proposals
            .cmp(&right.counts.correct_proposals)
            .then_with(|| {
                left.metrics
                    .precision
                    .unwrap_or(0.0)
                    .total_cmp(&right.metrics.precision.unwrap_or(0.0))
            })
            .then_with(|| left.threshold.total_cmp(&right.threshold))
    })
}

fn operating_points(rows: &[Row], default: f64) -> Result<Vec<OperatingPoint>, String> {
    let mut operating = Vec::new();
    for assignment in ASSIGNMENTS {
        let default_precision = rows
            .iter()
            .find(|row| {
                row.assignment == assignment && row.model == "combined" && row.threshold == default
            })
            .ok_or("missing candidate default row")?
            .metrics
            .precision;
        for (anchor, minimum) in [
            ("combined_default", default_precision),
            ("fixed_0.95", Some(0.95)),
        ] {
            for model in MODELS {
                let selected = minimum
                    .and_then(|floor| {
                        best_at_precision(
                            rows.iter()
                                .filter(|row| row.assignment == assignment && row.model == model),
                            floor,
                        )
                    })
                    .cloned();
                operating.push(OperatingPoint {
                    assignment: assignment.into(),
                    precision_anchor: anchor.into(),
                    minimum_precision: minimum,
                    model: model.into(),
                    status: if minimum.is_none() {
                        "undefined_anchor_precision"
                    } else if selected.is_none() {
                        "no_feasible_threshold"
                    } else {
                        "feasible"
                    }
                    .into(),
                    selected,
                });
            }
        }
    }
    Ok(operating)
}

fn normalized(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn hash(text: &str) -> String {
    format!("{:x}", Sha256::digest(normalized(text).as_bytes()))
}

fn source_hashes(directory: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut hashes = BTreeMap::new();
    for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let path = entry.map_err(|error| error.to_string())?.path();
        if path.extension().is_some_and(|extension| extension == "rs") {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or("invalid source filename")?;
            let text = fs::read_to_string(&path).map_err(|error| error.to_string())?;
            hashes.insert(name.to_owned(), hash(&text));
        }
    }
    Ok(hashes)
}

fn metadata(protocol: &Value, partition: &str) -> Result<Value, String> {
    let lock = fs::read_to_string(root().join("Cargo.lock")).map_err(|error| error.to_string())?;
    let manifests = ["Cargo.toml", "evaluation/Cargo.toml"]
        .into_iter()
        .map(|path| {
            fs::read_to_string(root().join(path))
                .map(|text| (path.to_owned(), normalized(&text)))
                .map_err(|error| error.to_string())
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    Ok(json!({
        "protocol":protocol,"protocol_sha256":hash(PROTOCOL),
        "original_protocol":corpus::protocol()?,"original_protocol_sha256":hash(corpus::PROTOCOL),
        "original_corpus_sha256":corpus::FILES.iter().map(|(name,text)|((*name).to_owned(),hash(text))).collect::<BTreeMap<_,_>>(),
        "engine_source_sha256":source_hashes(&root().join("src"))?,
        "evaluator_source_sha256":source_hashes(&root().join("evaluation/src"))?,
        "manifest_sha256":manifests.iter().map(|(path,text)|(path.clone(),hash(text))).collect::<BTreeMap<_,_>>(),
        "manifests":manifests,"cargo_lock":normalized(&lock),"cargo_lock_sha256":hash(&lock),
        "partition":partition,"holdout_scored":partition=="holdout",
        "report_schema_version":"release-report-1.0.0",
        "interpretation":"Public synthetic data, fixed family partitions and prespecified thresholds. Threshold frontiers are descriptive comparisons, not tuning or calibrated probabilities. Holdout data must not guide subsequent changes.",
    }))
}

fn percent(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".into(), |value| format!("{:.2}%", value * 100.0))
}

fn markdown(
    partition: &str,
    rows: &[Row],
    groups: &[GroupRow],
    operating: &[OperatingPoint],
    default: f64,
) -> String {
    let mut text = format!("# Release qualification: {partition}\n\nFeatures, thresholds and labels were frozen before this evaluation. These are synthetic fixture results, not production accuracy estimates. Holdout threshold curves are descriptive and must not be used to tune this release. No selected frontier threshold changes the library default.\n\n## Default threshold {default:.2}\n\n| Model | Assignment | Correct/proposed | Precision | Recall | Unique coverage | Candidate recall@5 | No-match proposals | Ambiguous proposals |\n| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for row in rows.iter().filter(|row| row.threshold == default) {
        text.push_str(&format!(
            "| {} | {} | {}/{} | {} | {} | {} | {} | {}/{} | {}/{} |\n",
            row.model,
            row.assignment,
            row.counts.correct_proposals,
            row.counts.proposals,
            percent(row.metrics.precision),
            percent(row.metrics.recall),
            percent(row.metrics.unique_coverage),
            percent(row.metrics.candidate_recall_at_5),
            row.counts.no_match_proposals,
            row.counts.no_match_fields,
            row.counts.ambiguous_proposals,
            row.counts.ambiguous_fields
        ));
    }
    text.push_str("\n## Descriptive matched-precision comparisons\n\nNonempty proposal sets only. Among grid points meeting the floor, select most correct matches, then higher precision, then higher threshold. The combined-default anchor is the combined model's .70 precision in this same partition and assignment mode. These frontier rows can select a different threshold even for combined. They are not default-threshold measurements or release recommendations.\n\n| Assignment | Anchor | Precision floor | Model | Status | Threshold | Precision | Recall | Unique coverage |\n| --- | --- | ---: | --- | --- | ---: | ---: | ---: | ---: |\n");
    for point in operating {
        text.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            point.assignment,
            point.precision_anchor,
            percent(point.minimum_precision),
            point.model,
            point.status,
            point
                .selected
                .as_ref()
                .map_or_else(|| "n/a".into(), |row| format!("{:.3}", row.threshold)),
            percent(
                point
                    .selected
                    .as_ref()
                    .and_then(|row| row.metrics.precision)
            ),
            percent(point.selected.as_ref().and_then(|row| row.metrics.recall)),
            percent(
                point
                    .selected
                    .as_ref()
                    .and_then(|row| row.metrics.unique_coverage)
            )
        ));
    }
    text.push_str("\n## Default results by domain and scenario\n\nScenario tags overlap; do not sum tag rows. Variants of a family are correlated. The JSON includes all family and variant counts and per-field default predictions, including failed proposals and abstentions. Candidate retrieval includes ineligible ranked pairs and is often easy on small schemas.\n\n| Group | Model | Assignment | Correct/proposed | Precision | Recall | Expected-abstention accuracy |\n| --- | --- | --- | ---: | ---: | ---: | ---: |\n");
    for row in groups
        .iter()
        .filter(|row| ["domain", "scenario_tag"].contains(&row.group_by.as_str()))
    {
        text.push_str(&format!(
            "| {}: {} | {} | {} | {}/{} | {} | {} | {} |\n",
            row.group_by,
            row.group,
            row.model,
            row.assignment,
            row.counts.correct_proposals,
            row.counts.proposals,
            percent(row.metrics.precision),
            percent(row.metrics.recall),
            percent(row.metrics.expected_abstention_accuracy)
        ));
    }
    text.push_str("\nQualification targets remain precision >=95%, unique coverage >=60%, and candidate recall@5 >=90%. See `quality_targets` in JSON for each combined default result. An unmet target is a release limitation; it is not silently waived. Hidden semantic differences, arbitrary identifiers and missing evidence remain known failure cases.\n");
    text
}

fn write_or_check(path: &Path, contents: &str, check: bool) -> Result<(), String> {
    if check || path.exists() {
        let existing = fs::read_to_string(path).map_err(|error| error.to_string())?;
        if normalized(&existing) != contents {
            return Err(format!(
                "{} differs; preserve frozen results and use a new output directory",
                path.display()
            ));
        }
    } else {
        fs::write(path, contents).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn evaluate_partition(
    protocol: &Value,
    partition: &str,
    output: &Path,
    check: bool,
) -> Result<(), String> {
    let cases = cases_in_partition(partition)?;
    let grid = thresholds(protocol)?;
    let default = number(protocol, "default_threshold")?;
    let mut rows = Vec::new();
    let mut by_group = Vec::new();
    let mut predictions = Vec::new();
    for model in MODELS {
        for assignment in ASSIGNMENTS {
            for threshold in &grid {
                let matcher = engine(protocol, model, assignment == "one_to_one", *threshold)?;
                let mut totals = Counts::default();
                let mut grouped = BTreeMap::<(String, String), Counts>::new();
                for case in &cases {
                    let mut source = corpus::schema(&case.source)?;
                    let mut target = corpus::schema(&case.target)?;
                    if model == "name_only" {
                        for field in source.fields.iter_mut().chain(&mut target.fields) {
                            field.hints = SemanticHints::default();
                        }
                    }
                    // Labels, concepts and metadata never enter the matching API.
                    let report = matcher
                        .match_schemas(&source, &target)
                        .map_err(|error| error.to_string())?;
                    let counts = score_case(&case.labels, &report)?;
                    totals.add(&counts);
                    if *threshold == default {
                        for group in groups(case) {
                            grouped.entry(group).or_default().add(&counts);
                        }
                        for field in &report.fields {
                            let label = case
                                .labels
                                .iter()
                                .find(|label| label.source == field.source.0)
                                .ok_or("missing prediction label")?;
                            predictions.push(prediction(case, model, assignment, field, label));
                        }
                    }
                }
                for ((group_by, group), counts) in grouped {
                    by_group.push(GroupRow {
                        model: model.into(),
                        assignment: assignment.into(),
                        group_by,
                        group,
                        metrics: counts.metrics(),
                        counts,
                    });
                }
                rows.push(Row {
                    model: model.into(),
                    assignment: assignment.into(),
                    threshold: *threshold,
                    metrics: totals.metrics(),
                    counts: totals,
                });
            }
        }
    }
    let operating = operating_points(&rows, default)?;
    let targets = &protocol["quality_targets"];
    let target_precision = number(targets, "overall_precision")?;
    let target_coverage = number(targets, "unique_coverage")?;
    let target_retrieval = number(targets, "candidate_recall_at_5")?;
    let mut quality = Vec::new();
    for row in rows
        .iter()
        .filter(|row| row.model == "combined" && row.threshold == default)
    {
        quality.push(json!({"model":row.model,"assignment":row.assignment,"threshold":default,
            "overall_precision":{"target":target_precision,"actual":row.metrics.precision,"met":row.metrics.precision.is_some_and(|value|value>=target_precision)},
            "unique_coverage":{"target":target_coverage,"actual":row.metrics.unique_coverage,"met":row.metrics.unique_coverage.is_some_and(|value|value>=target_coverage)},
            "candidate_recall_at_5":{"target":target_retrieval,"actual":row.metrics.candidate_recall_at_5,"met":row.metrics.candidate_recall_at_5.is_some_and(|value|value>=target_retrieval)},
        }));
    }
    let report = json!({"metadata":metadata(protocol,partition)?,
        "inventory":{"cases":cases.len(),"families":cases.iter().map(|case|&case.family_id).collect::<BTreeSet<_>>().len(),"labels":cases.iter().map(|case|case.labels.len()).sum::<usize>()},
        "threshold_rows":rows,"default_by_group":by_group,"operating_points":operating,
        "quality_targets":quality,"default_predictions":predictions});
    let data = serde_json::to_string_pretty(&report).map_err(|error| error.to_string())? + "\n";
    write_or_check(&output.join(format!("{partition}.json")), &data, check)?;
    write_or_check(
        &output.join(format!("{partition}.md")),
        &markdown(partition, &rows, &by_group, &operating, default),
        check,
    )?;
    println!("Release {partition} evaluation {}: {} cases, {} threshold rows, {} per-field default predictions",if check {"verified"} else {"written"},cases.len(),rows.len(),predictions.len());
    Ok(())
}

/// Score development by default; final holdout scoring requires explicit acknowledgement.
pub fn run(args: Vec<String>) -> Result<(), String> {
    let options = Options::parse(args)?;
    let protocol = protocol()?;
    let freeze = protocol["feature_freeze_revision"]
        .as_str()
        .ok_or("record the feature-freeze commit in release-protocol.json before scoring")?;
    if freeze.len() != 40 || !freeze.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("feature-freeze revision must be a full 40-character commit SHA".into());
    }
    if !options.check {
        fs::create_dir_all(&options.output).map_err(|error| error.to_string())?;
    }
    for partition in options.partitions() {
        evaluate_partition(&protocol, partition, &options.output, options.check)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use fieldkin::{DataType, Field, SampleValue, Schema};

    use super::*;

    #[test]
    fn holdout_requires_explicit_acknowledgement_and_check_never_scores_it() {
        assert_eq!(
            Options::parse(vec![]).unwrap().partitions(),
            &["development"]
        );
        assert_eq!(
            Options::parse(vec!["--check".into()]).unwrap().partitions(),
            &["development"]
        );
        assert_eq!(
            Options::parse(vec!["--acknowledge-holdout".into()])
                .unwrap()
                .partitions(),
            &["development", "holdout"]
        );
        assert!(Options::parse(vec!["--check".into(), "--acknowledge-holdout".into()]).is_err());
        assert!(Options::parse(vec!["--split".into(), "holdout".into()]).is_err());
    }

    #[test]
    fn partitioning_keeps_every_variant_in_its_frozen_family() {
        let cases = cases_in_partition("development").unwrap();
        assert_eq!(cases.len(), 160);
        assert_eq!(
            cases.iter().map(|case| case.labels.len()).sum::<usize>(),
            960
        );
        let families = cases
            .iter()
            .map(|case| &case.family_id)
            .collect::<BTreeSet<_>>();
        assert_eq!(families.len(), 32);
        let original = corpus::protocol().unwrap();
        for family in families {
            assert!(!original["holdout_families"]
                .as_array()
                .unwrap()
                .contains(&json!(family)));
            assert_eq!(
                cases
                    .iter()
                    .filter(|case| &case.family_id == family)
                    .count(),
                5
            );
        }
        assert!(cases
            .iter()
            .any(|case| case.tags.iter().any(|tag| tag == "opaque_identifiers")));
    }

    #[test]
    fn frozen_combined_matches_current_public_defaults() {
        let protocol = protocol().unwrap();
        let schema = Schema::new(vec![
            Field::new("a", "GrossAmt", DataType::Decimal).with_samples(vec![
                SampleValue::Integer(1),
                SampleValue::Integer(2),
                SampleValue::Integer(3),
            ]),
            Field::new("b", "amount", DataType::Decimal)
                .with_samples(vec![SampleValue::Number(1.0); 8]),
            Field::new("c", "date", DataType::Date),
        ]);
        for one_to_one in [false, true] {
            let configured = engine(&protocol, "combined", one_to_one, 0.7)
                .unwrap()
                .match_schemas(&schema, &schema)
                .unwrap();
            let current = MatchEngine::new(Config {
                one_to_one,
                ..Config::default()
            })
            .unwrap()
            .match_schemas(&schema, &schema)
            .unwrap();
            assert_eq!(configured, current);
        }
    }

    #[test]
    fn name_only_ignores_type_and_sample_scores() {
        let protocol = protocol().unwrap();
        let source = Schema::new(vec![Field::new("s", "same", DataType::Text)
            .with_samples(vec![SampleValue::Text("private-sample".into()); 3])]);
        let target = Schema::new(vec![Field::new("t", "same", DataType::Binary)]);
        let report = engine(&protocol, "name_only", false, 0.7)
            .unwrap()
            .match_schemas(&source, &target)
            .unwrap();
        let selected = report.fields[0].selected.as_ref().unwrap();
        assert_eq!(selected.score, 1.0);
        assert_eq!(selected.signals.len(), 1);
        assert_eq!(selected.signals[0].name, "name");
    }

    fn row(threshold: f64, correct: usize, proposed: usize) -> Row {
        let counts = Counts {
            correct_proposals: correct,
            proposals: proposed,
            unique_fields: 100,
            ..Counts::default()
        };
        Row {
            model: "combined".into(),
            assignment: "independent".into(),
            threshold,
            metrics: counts.metrics(),
            counts,
        }
    }

    #[test]
    fn frontier_excludes_empty_proposals_and_preserves_deterministic_ties() {
        let rows = [
            row(0.5, 9, 10),
            row(0.6, 8, 8),
            row(0.7, 8, 8),
            row(1.0, 0, 0),
        ];
        assert_eq!(best_at_precision(rows.iter(), 0.9).unwrap().threshold, 0.5);
        assert_eq!(best_at_precision(rows.iter(), 0.95).unwrap().threshold, 0.7);
        assert!(best_at_precision([row(1.0, 0, 0)].iter(), 0.95).is_none());
        assert_eq!(thresholds(&protocol().unwrap()).unwrap().len(), 21);
    }

    #[test]
    fn prediction_outcomes_agree_with_public_metrics_and_do_not_export_samples() {
        // A development fixture suffices to verify output and denominator handling.
        let case = cases_in_partition("development").unwrap().remove(0);
        let source = corpus::schema(&case.source).unwrap();
        let target = corpus::schema(&case.target).unwrap();
        let report = engine(&protocol().unwrap(), "combined", false, 0.7)
            .unwrap()
            .match_schemas(&source, &target)
            .unwrap();
        let counts = score_case(&case.labels, &report).unwrap();
        let predictions = report
            .fields
            .iter()
            .map(|field| {
                let label = case
                    .labels
                    .iter()
                    .find(|label| label.source == field.source.0)
                    .unwrap();
                prediction(&case, "combined", "independent", field, label)
            })
            .collect::<Vec<_>>();
        assert_eq!(predictions.len(), counts.fields);
        assert_eq!(
            predictions
                .iter()
                .filter(|value| value["outcome"] == "correct_unique_proposal")
                .count(),
            counts.correct_proposals
        );
        assert_eq!(
            predictions
                .iter()
                .filter(|value| !value["selected"].is_null())
                .count(),
            counts.proposals
        );
        assert!(predictions
            .iter()
            .all(|value| value.get("samples").is_none()));
        let grouped = groups(&case);
        assert!(grouped.contains(&("family".into(), case.family_id.clone())));
        assert!(grouped.contains(&("all".into(), "all".into())));
    }
}
