//! Prespecified development-only evidence evaluation. No holdout scoring path.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use fieldkin::{
    Config, Evidence, ExactDecimal, Field, Limits, MatchEngine, Matcher, NameMatcher,
    SampleMatcher, SampleProfileMatcher, SampleReliability, SampleValue, Schema, SemanticHints,
    TypeMatcher, WeightedMatcher,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::corpus;
use crate::metrics::{score_case, Counts, Metrics};
use crate::model::{InputField, Label, LabelKind};

const PROTOCOL: &str = include_str!("../stage3-protocol.json");
const FIXTURES: &str = include_str!("../corpus/stage3-evidence.json");

#[derive(Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct InputHints {
    unit: Option<String>,
    currency: Option<String>,
    identifier_scope: Option<String>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtensionField {
    id: String,
    name: String,
    data_type: String,
    concept: String,
    samples: Option<Vec<Value>>,
    #[serde(default)]
    hints: InputHints,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ExtensionFamily {
    id: String,
    domain: String,
    title: String,
    tags: Vec<String>,
    rationale: String,
    source: Vec<ExtensionField>,
    target: Vec<ExtensionField>,
    labels: Vec<Label>,
}

pub(crate) struct EvaluationCase {
    pub(crate) id: String,
    pub(crate) family: String,
    pub(crate) source: Schema,
    pub(crate) target: Schema,
    pub(crate) labels: Vec<Label>,
}

#[derive(Clone, Serialize)]
struct Row {
    corpus: String,
    model: String,
    assignment: String,
    threshold: f64,
    counts: Counts,
    metrics: Metrics,
}

#[derive(Serialize)]
struct FamilyRow {
    corpus: String,
    family: String,
    model: String,
    assignment: String,
    counts: Counts,
    metrics: Metrics,
}

#[derive(Serialize)]
struct OperatingPoint {
    corpus: String,
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

fn numeric_sample(value: &Value) -> Result<SampleValue, String> {
    match value {
        Value::Null => Ok(SampleValue::Null),
        Value::Bool(value) => Ok(SampleValue::Boolean(*value)),
        Value::Number(value) => value
            .as_f64()
            .filter(|value| value.is_finite())
            .map(SampleValue::Number)
            .ok_or_else(|| "invalid finite numeric sample".into()),
        Value::String(value) => Ok(SampleValue::Text(value.clone())),
        Value::Object(map) if map.len() == 1 && map.contains_key("integer") => map["integer"]
            .as_str()
            .and_then(|value| value.parse().ok())
            .map(SampleValue::Integer)
            .ok_or_else(|| "invalid exact integer sample".into()),
        Value::Object(map) if map.len() == 1 && map.contains_key("decimal") => {
            let decimal = map["decimal"].as_object().ok_or("invalid decimal object")?;
            if decimal.len() != 2 {
                return Err("decimal requires only coefficient and scale".into());
            }
            let coefficient = decimal
                .get("coefficient")
                .and_then(Value::as_str)
                .and_then(|value| value.parse::<i128>().ok())
                .ok_or("invalid decimal coefficient")?;
            let scale = decimal
                .get("scale")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok())
                .ok_or("invalid decimal scale")?;
            ExactDecimal::new(coefficient, scale)
                .map(SampleValue::Decimal)
                .map_err(|error| error.to_string())
        }
        _ => Err("unsupported sample representation".into()),
    }
}

fn extension_schema(fields: &[ExtensionField]) -> Result<Schema, String> {
    let mut converted = Vec::new();
    let mut ids = BTreeSet::new();
    for input in fields {
        if input.id.is_empty() || input.concept.trim().is_empty() || !ids.insert(&input.id) {
            return Err("invalid extension field identity or concept".into());
        }
        // Reuse declared-type parsing while keeping descriptive concepts out of matching.
        let schema = corpus::schema(&[InputField {
            id: input.id.clone(),
            name: input.name.clone(),
            data_type: input.data_type.clone(),
            concept: input.concept.clone(),
            samples: None,
        }])?;
        let mut field = schema.fields.into_iter().next().ok_or("missing field")?;
        field.samples = input
            .samples
            .as_ref()
            .map(|values| values.iter().map(numeric_sample).collect())
            .transpose()?;
        field.hints = SemanticHints {
            unit: input.hints.unit.clone(),
            currency: input.hints.currency.clone(),
            identifier_scope: input.hints.identifier_scope.clone(),
        };
        converted.push(field);
    }
    Ok(Schema::new(converted))
}

pub(crate) fn extension_cases() -> Result<Vec<EvaluationCase>, String> {
    let families: Vec<ExtensionFamily> =
        serde_json::from_str(FIXTURES).map_err(|error| error.to_string())?;
    if families.len() != 12 {
        return Err("exactly twelve development extension families are required".into());
    }
    let mut ids = BTreeSet::new();
    let mut cases = Vec::new();
    for family in families {
        if !ids.insert(family.id.clone())
            || family.domain != "stage3_development"
            || family.title.trim().is_empty()
            || family.tags.is_empty()
            || family.rationale.trim().is_empty()
        {
            return Err("invalid extension family metadata".into());
        }
        let source = extension_schema(&family.source)?;
        let target = extension_schema(&family.target)?;
        let source_ids: BTreeSet<_> = source.fields.iter().map(|field| &field.id.0).collect();
        let target_ids: BTreeSet<_> = target.fields.iter().map(|field| &field.id.0).collect();
        let mut label_ids = BTreeSet::new();
        for label in &family.labels {
            let gold: BTreeSet<_> = label.targets.iter().collect();
            let valid = match label.kind {
                LabelKind::Match => gold.len() == 1,
                LabelKind::NoMatch => gold.is_empty(),
                LabelKind::Ambiguous => gold.len() >= 2,
            };
            if !label_ids.insert(&label.source)
                || !source_ids.contains(&label.source)
                || !gold.is_subset(&target_ids)
                || gold.len() != label.targets.len()
                || !valid
                || label.rationale.trim().is_empty()
            {
                return Err("invalid extension gold labels".into());
            }
        }
        if source_ids != label_ids {
            return Err("extension must label every source exactly once".into());
        }
        cases.push(EvaluationCase {
            id: family.id.clone(),
            family: family.id,
            source,
            target,
            labels: family.labels,
        });
    }
    cases.sort_by(|a, b| a.family.cmp(&b.family));
    Ok(cases)
}

fn original_development() -> Result<Vec<EvaluationCase>, String> {
    corpus::cases(&corpus::families()?)?
        .into_iter()
        .filter(|case| case.split == "development")
        .map(|case| {
            Ok(EvaluationCase {
                id: case.id,
                family: case.family_id,
                source: corpus::schema(&case.source)?,
                target: corpus::schema(&case.target)?,
                labels: case.labels,
            })
        })
        .collect()
}

struct MissingProfile;
impl Matcher for MissingProfile {
    fn name(&self) -> &str {
        "sample_profile"
    }

    fn evaluate(&self, _: &Field, _: &Field) -> Result<Evidence, String> {
        Ok(Evidence {
            score: None,
            explanation: "Profile disabled for fixed-weight ablation".into(),
        })
    }
}

fn number(value: &Value, key: &str) -> Result<f64, String> {
    value[key]
        .as_f64()
        .filter(|number| number.is_finite())
        .ok_or_else(|| format!("invalid numeric protocol setting {key}"))
}

fn size(value: &Value, key: &str) -> Result<usize, String> {
    value[key]
        .as_u64()
        .and_then(|number| usize::try_from(number).ok())
        .ok_or_else(|| format!("invalid size protocol setting {key}"))
}

fn engine(
    protocol: &Value,
    model: &str,
    one_to_one: bool,
    threshold: f64,
) -> Result<MatchEngine, String> {
    let limits = &protocol["limits"];
    let config = Config {
        name_conflicts: Vec::new(),
        corroboration: None,
        contextual_evidence: None,
        global_diagnostics: Default::default(),
        min_score: threshold,
        ambiguity_margin: number(protocol, "ambiguity_margin")?,
        max_candidates: size(protocol, "max_candidates")?,
        one_to_one,
        abstain_on_ambiguity: protocol["abstain_on_ambiguity"]
            .as_bool()
            .ok_or("missing ambiguity policy")?,
        reject_incompatible_types: model != "name_only"
            && protocol["reject_incompatible_types"]
                .as_bool()
                .ok_or("missing type policy")?,
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
    let mut names = NameMatcher {
        aliases: protocol["aliases"]
            .as_object()
            .ok_or("missing alias object")?
            .iter()
            .map(|(key, value)| {
                value
                    .as_str()
                    .map(|value| (key.clone(), value.to_owned()))
                    .ok_or_else(|| "invalid alias".to_owned())
            })
            .collect::<Result<_, _>>()?,
    };
    if model == "no_aliases" {
        names.aliases.clear();
    }
    if model == "name_only" {
        return MatchEngine::with_matchers(config, vec![WeightedMatcher::new(1.0, names)])
            .map_err(|error| error.to_string());
    }
    if ![
        "combined",
        "no_cardinality",
        "profile",
        "no_profile",
        "no_aliases",
        "no_hints",
    ]
    .contains(&model)
    {
        return Err("unknown Stage 3 model".into());
    }
    let profile = ["profile", "no_profile"].contains(&model);
    let weights = &protocol[if profile {
        "profile_weights"
    } else {
        "weights"
    }];
    let mut matchers = vec![
        WeightedMatcher::new(number(weights, "name")?, names),
        WeightedMatcher::new(number(weights, "type")?, TypeMatcher),
        WeightedMatcher::new(
            number(weights, "samples")?,
            SampleMatcher {
                min_non_null: size(protocol, "sample_min_non_null")?,
                reliability: if model == "no_cardinality" {
                    SampleReliability::Legacy
                } else {
                    SampleReliability::Distinct
                },
            },
        ),
    ];
    if profile {
        matchers.push(if model == "no_profile" {
            WeightedMatcher::new(number(weights, "profile")?, MissingProfile)
        } else {
            WeightedMatcher::new(
                number(weights, "profile")?,
                SampleProfileMatcher {
                    min_non_null: size(protocol, "profile_min_non_null")?,
                },
            )
        });
    }
    MatchEngine::with_matchers(config, matchers).map_err(|error| error.to_string())
}

fn best_at_precision<'a>(rows: impl Iterator<Item = &'a Row>, minimum: f64) -> Option<&'a Row> {
    rows.filter(|row| {
        row.metrics
            .precision
            .is_some_and(|precision| precision >= minimum)
    })
    .max_by(|a, b| {
        a.counts
            .correct_proposals
            .cmp(&b.counts.correct_proposals)
            .then_with(|| {
                a.metrics
                    .precision
                    .unwrap_or(0.0)
                    .total_cmp(&b.metrics.precision.unwrap_or(0.0))
            })
            .then_with(|| a.threshold.total_cmp(&b.threshold))
    })
}

fn normalized(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn hash(text: &str) -> String {
    format!("{:x}", Sha256::digest(normalized(text).as_bytes()))
}

fn source_hashes(directory: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut result = BTreeMap::new();
    for path in fs::read_dir(directory).map_err(|error| error.to_string())? {
        let path = path.map_err(|error| error.to_string())?.path();
        if path.extension().is_some_and(|extension| extension == "rs") {
            let text = fs::read_to_string(&path).map_err(|error| error.to_string())?;
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or("invalid source filename")?;
            result.insert(name.into(), hash(&text));
        }
    }
    Ok(result)
}

fn percentage(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".into(), |value| format!("{:.2}%", value * 100.0))
}

fn markdown(rows: &[Row], operating: &[OperatingPoint], default: f64) -> String {
    let mut text = String::from("# Stage 3 development evidence evaluation\n\nNo held-out schema family was scored. These are descriptive development results on public synthetic data, not production accuracy estimates. Thresholds and labels were fixed before scoring. Scores remain uncalibrated heuristics.\n\n## Default threshold\n\n| Corpus | Model | Assignment | Correct/proposed | Precision | Recall | Unique coverage | Candidate recall@5 | No-match proposals |\n| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    for row in rows.iter().filter(|row| row.threshold == default) {
        text.push_str(&format!(
            "| {} | {} | {} | {}/{} | {} | {} | {} | {} | {}/{} |\n",
            row.corpus,
            row.model,
            row.assignment,
            row.counts.correct_proposals,
            row.counts.proposals,
            percentage(row.metrics.precision),
            percentage(row.metrics.recall),
            percentage(row.metrics.unique_coverage),
            percentage(row.metrics.candidate_recall_at_5),
            row.counts.no_match_proposals,
            row.counts.no_match_fields,
        ));
    }
    text.push_str("\n## Prespecified matched-precision operating points\n\nChoose maximum correct recall among qualifying grid points; ties use greater precision, then higher threshold. A baseline anchor uses that baseline's .70 default precision. A candidate-default anchor compares baselines at least as precise as that candidate. Undefined precision or no feasible threshold is never counted as a success. Threshold selections below are development analyses and do not alter library defaults.\n\n| Corpus | Assignment | Precision anchor | Floor | Model | Status | Threshold | Precision | Recall | Unique coverage |\n| --- | --- | --- | ---: | --- | --- | ---: | ---: | ---: | ---: |\n");
    for point in operating {
        let selected = &point.selected;
        text.push_str(&format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            point.corpus,
            point.assignment,
            point.precision_anchor,
            percentage(point.minimum_precision),
            point.model,
            point.status,
            selected
                .as_ref()
                .map_or_else(|| "n/a".into(), |row| format!("{:.3}", row.threshold)),
            percentage(selected.as_ref().and_then(|row| row.metrics.precision)),
            percentage(selected.as_ref().and_then(|row| row.metrics.recall)),
            percentage(
                selected
                    .as_ref()
                    .and_then(|row| row.metrics.unique_coverage)
            ),
        ));
    }
    text.push_str("\nFull threshold curves and per-family default counts are in report.json. Original development and new extension results are deliberately not pooled. The extension is small and designed to exercise implementation assumptions; labels are not independent statistical samples, and candidate recall is easy when schemas contain few targets. A semantic-hint ablation measures extra caller knowledge, not better inference from the original inputs. The profile is optional and shape agreement cannot establish meaning.\n");
    text
}

fn write_or_check(
    path: &Path,
    contents: &str,
    check: bool,
    check_behavior: bool,
) -> Result<(), String> {
    if check || path.exists() {
        let existing = fs::read_to_string(path).map_err(|error| error.to_string())?;
        let same = if check_behavior {
            crate::snapshot::same_behavior_json(&existing, contents)?
        } else {
            normalized(&existing) == contents
        };
        if !same {
            return Err(format!(
                "{} differs; preserve prior results and use a new output directory",
                path.display()
            ));
        }
    } else {
        fs::write(path, contents).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn options(args: Vec<String>) -> Result<(PathBuf, bool, bool), String> {
    let mut output = root().join("target/fieldkin-stage3");
    let mut check = false;
    let mut check_behavior = false;
    let mut args = args.into_iter();
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--output" => output = args.next().ok_or("--output needs a directory")?.into(),
            "--check" => check = true,
            "--check-behavior" => check_behavior = true,
            _ => {
                return Err(
                    "Stage 3 accepts only --output DIR, --check or --check-behavior; holdout scoring is unavailable"
                        .into(),
                )
            }
        }
    }
    if check && check_behavior {
        return Err("--check and --check-behavior are mutually exclusive".into());
    }
    Ok((output, check || check_behavior, check_behavior))
}

/// Evaluate only original development cases and the Stage 3 development extension.
pub fn run(args: Vec<String>) -> Result<(), String> {
    let (output, check, check_behavior) = options(args)?;
    let protocol: Value = serde_json::from_str(PROTOCOL).map_err(|error| error.to_string())?;
    let strings = |key: &str| -> Result<Vec<&str>, String> {
        protocol[key]
            .as_array()
            .ok_or_else(|| format!("missing {key}"))?
            .iter()
            .map(|value| value.as_str().ok_or_else(|| format!("invalid {key}")))
            .collect()
    };
    let models = strings("models")?;
    let assignments = strings("assignment_modes")?;
    let thresholds = protocol["threshold_grid"]
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
    let default = number(&protocol, "default_threshold")?;
    if thresholds.windows(2).any(|pair| pair[0] >= pair[1]) || !thresholds.contains(&default) {
        return Err("threshold grid must be increasing and include the default".into());
    }
    let corpora = [
        ("original_development", original_development()?),
        ("stage3_extension", extension_cases()?),
    ];
    let mut rows = Vec::new();
    let mut by_family = Vec::new();
    let mut predictions = String::new();
    for (corpus_name, cases) in &corpora {
        for model in &models {
            for assignment in &assignments {
                let one_to_one = match *assignment {
                    "independent" => false,
                    "one_to_one" => true,
                    _ => return Err("invalid assignment mode".into()),
                };
                for threshold in &thresholds {
                    let matcher = engine(&protocol, model, one_to_one, *threshold)?;
                    let mut totals = Counts::default();
                    let mut families = BTreeMap::<String, Counts>::new();
                    for case in cases {
                        let mut source = case.source.clone();
                        let mut target = case.target.clone();
                        if ["no_hints", "name_only"].contains(model) {
                            for field in source.fields.iter_mut().chain(&mut target.fields) {
                                field.hints = SemanticHints::default();
                            }
                        }
                        let report = matcher
                            .match_schemas(&source, &target)
                            .map_err(|error| error.to_string())?;
                        let counts = score_case(&case.labels, &report)?;
                        totals.add(&counts);
                        if *threshold == default {
                            let record = json!({
                                "corpus":corpus_name,"case":case.id,"family":case.family,
                                "model":model,"assignment":assignment,
                                "report_sha256":hash(&format!("{report:?}")),
                            });
                            predictions.push_str(
                                &serde_json::to_string(&record)
                                    .map_err(|error| error.to_string())?,
                            );
                            predictions.push('\n');
                            families
                                .entry(case.family.clone())
                                .or_default()
                                .add(&counts);
                        }
                    }
                    for (family, counts) in families {
                        by_family.push(FamilyRow {
                            corpus: (*corpus_name).into(),
                            family,
                            model: (*model).into(),
                            assignment: (*assignment).into(),
                            metrics: counts.metrics(),
                            counts,
                        });
                    }
                    rows.push(Row {
                        corpus: (*corpus_name).into(),
                        model: (*model).into(),
                        assignment: (*assignment).into(),
                        threshold: *threshold,
                        metrics: totals.metrics(),
                        counts: totals,
                    });
                }
            }
        }
    }
    // Verify the original baselines against their frozen development counts.
    // The historical holdout file is deliberately not opened.
    let baseline: Value = serde_json::from_str(
        &fs::read_to_string(root().join("evaluation/results/baseline-v1/development.json"))
            .map_err(|error| error.to_string())?,
    )
    .map_err(|error| error.to_string())?;
    for row in rows.iter().filter(|row| {
        row.corpus == "original_development"
            && row.threshold == default
            && ["no_cardinality", "name_only"].contains(&row.model.as_str())
    }) {
        let reference_model = if row.model == "no_cardinality" {
            "combined"
        } else {
            "name_only"
        };
        let reference = baseline["results"]
            .as_array()
            .ok_or("missing baseline results")?
            .iter()
            .find(|value| {
                value["group_by"] == "all"
                    && value["model"] == reference_model
                    && value["assignment_mode"] == row.assignment
            })
            .ok_or("missing baseline reference")?;
        let counts: Counts = serde_json::from_value(reference["counts"].clone())
            .map_err(|error| error.to_string())?;
        if counts != row.counts {
            return Err(
                "Stage 3 legacy/name-only model does not reproduce frozen development counts"
                    .into(),
            );
        }
    }
    let mut operating = Vec::new();
    for (corpus_name, _) in &corpora {
        for assignment in &assignments {
            let eligible: Vec<_> = rows
                .iter()
                .filter(|row| row.corpus == *corpus_name && row.assignment == *assignment)
                .collect();
            let mut anchors = protocol["precision_targets"]
                .as_array()
                .ok_or("missing precision targets")?
                .iter()
                .map(|value| {
                    value
                        .as_f64()
                        .filter(|value| (0.0..=1.0).contains(value))
                        .map(|value| (format!("fixed_{value:.2}"), Some(value)))
                        .ok_or_else(|| "invalid precision target".to_owned())
                })
                .collect::<Result<Vec<_>, _>>()?;
            for model in ["no_cardinality", "name_only", "combined", "profile"] {
                let row = eligible
                    .iter()
                    .find(|row| row.model == model && row.threshold == default)
                    .ok_or("missing default operating point")?;
                anchors.push((format!("{model}_default"), row.metrics.precision));
            }
            for (anchor, floor) in anchors {
                for model in &models {
                    let selected = floor.and_then(|floor| {
                        best_at_precision(
                            eligible.iter().copied().filter(|row| row.model == *model),
                            floor,
                        )
                    });
                    operating.push(OperatingPoint {
                        corpus: (*corpus_name).into(),
                        assignment: (*assignment).into(),
                        precision_anchor: anchor.clone(),
                        minimum_precision: floor,
                        model: (*model).into(),
                        status: if floor.is_none() {
                            "undefined_anchor_precision"
                        } else if selected.is_none() {
                            "no_feasible_threshold"
                        } else {
                            "feasible"
                        }
                        .into(),
                        selected: selected.cloned(),
                    });
                }
            }
        }
    }
    let lock = fs::read_to_string(root().join("Cargo.lock")).map_err(|error| error.to_string())?;
    let manifests = ["Cargo.toml", "evaluation/Cargo.toml"]
        .iter()
        .map(|path| {
            fs::read_to_string(root().join(path))
                .map(|text| ((*path).to_owned(), hash(&text)))
                .map_err(|error| error.to_string())
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let limits = Config::default().limits;
    let inventory: Vec<_> = corpora.iter().map(|(name, cases)| json!({
        "corpus":name, "cases":cases.len(), "families":cases.iter().map(|case| &case.family).collect::<BTreeSet<_>>().len(),
        "labels":cases.iter().map(|case| case.labels.len()).sum::<usize>(),
    })).collect();
    let report = json!({
        "metadata": {
            "protocol":protocol, "protocol_sha256":hash(PROTOCOL), "extension_sha256":hash(FIXTURES),
            "original_protocol_sha256":hash(corpus::PROTOCOL),
            "original_corpus_sha256":corpus::FILES.iter().map(|(name,text)| ((*name).to_owned(),hash(text))).collect::<BTreeMap<_,_>>(),
            "engine_source_sha256":source_hashes(&root().join("src"))?,
            "evaluator_source_sha256":source_hashes(&root().join("evaluation/src"))?,
            "cargo_lock_sha256":hash(&lock),
            "cargo_lock":normalized(&lock),"manifest_sha256":manifests,
            "prediction_digest":"SHA-256 of the complete deterministic MatchReport Debug representation; includes ranking, scores, selection, ambiguity, warnings and explanations, without sample values.",
            "limits": {"max_fields":limits.max_fields,"max_pairs":limits.max_pairs,
                "max_signal_evaluations":limits.max_signal_evaluations,"max_explanation_bytes":limits.max_explanation_bytes,
                "max_name_bytes":limits.max_name_bytes,"max_samples_per_field":limits.max_samples_per_field,
                "max_sample_bytes":limits.max_sample_bytes,"max_total_sample_bytes":limits.max_total_sample_bytes},
            "holdout_scored":false,
            "interpretation":"Public synthetic development data; threshold sweeps are descriptive, not calibration or independent validation. Baseline legacy/default counts verified against frozen development artifact."
        },
        "inventory":inventory,"threshold_rows":rows,"default_by_family":by_family,"operating_points":operating,
    });
    let json = serde_json::to_string_pretty(&report).map_err(|error| error.to_string())? + "\n";
    if !check {
        fs::create_dir_all(&output).map_err(|error| error.to_string())?;
    }
    write_or_check(&output.join("report.json"), &json, check, check_behavior)?;
    write_or_check(
        &output.join("predictions.jsonl"),
        &predictions,
        check,
        false,
    )?;
    write_or_check(
        &output.join("report.md"),
        &markdown(&rows, &operating, default),
        check,
        false,
    )?;
    println!("Stage 3 development evaluation {}: 172 schema pairs, 32 + 12 families, 992 labels; {} aggregate threshold rows; no holdout scoring",
        if check { "verified" } else { "written" }, rows.len());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn behavior_check_is_explicit_and_cannot_enable_holdout() {
        let (_, check, behavior) = options(vec!["--check".into()]).unwrap();
        assert!(check && !behavior);
        let (_, check, behavior) = options(vec!["--check-behavior".into()]).unwrap();
        assert!(check && behavior);
        assert!(options(vec!["--check".into(), "--check-behavior".into()]).is_err());
        for flag in ["--check", "--check-behavior"] {
            assert!(options(vec![flag.into(), "--acknowledge-holdout".into()]).is_err());
        }
    }

    #[test]
    fn exact_check_still_rejects_implementation_hash_changes() {
        let path = std::env::temp_dir().join(format!(
            "fieldkin-stage3-snapshot-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let original = json!({"metadata":{"engine_source_sha256":{"engine.rs":"old"},"evaluator_source_sha256":{"stage3.rs":"old"}},"counts":{"proposals":3}}).to_string();
        let mut current: Value = serde_json::from_str(&original).unwrap();
        current["metadata"]["engine_source_sha256"]["engine.rs"] = json!("new");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        std::io::Write::write_all(&mut file, original.as_bytes()).unwrap();
        drop(file);
        let exact = write_or_check(&path, &current.to_string(), true, false);
        let behavior = write_or_check(&path, &current.to_string(), true, true);
        let retained = fs::read_to_string(&path).unwrap();
        fs::remove_file(path).unwrap();
        assert!(exact.is_err());
        assert!(behavior.is_ok());
        assert_eq!(retained, original);
    }

    #[test]
    fn extension_has_independent_complete_labels_and_exact_samples() {
        let cases = extension_cases().unwrap();
        assert_eq!(cases.len(), 12);
        assert_eq!(
            cases.iter().map(|case| case.labels.len()).sum::<usize>(),
            32
        );
        assert!(cases[0].source.fields[0]
            .samples
            .as_ref()
            .unwrap()
            .iter()
            .all(|value| matches!(value, SampleValue::Integer(_))));
        assert!(cases[1].source.fields[0]
            .samples
            .as_ref()
            .unwrap()
            .iter()
            .all(|value| matches!(value, SampleValue::Decimal(_))));
    }

    #[test]
    fn exact_sample_parser_rejects_invalid_bounds_and_extra_keys() {
        for value in [
            json!({"integer":"170141183460469231731687303715884105728"}),
            json!({"decimal":{"coefficient":"1","scale":39}}),
            json!({"decimal":{"coefficient":"1","scale":2,"extra":true}}),
            json!({"integer":"1","extra":true}),
        ] {
            assert!(numeric_sample(&value).is_err());
        }
        assert_eq!(
            numeric_sample(&json!({"integer":"9007199254740993"})).unwrap(),
            SampleValue::Integer(9007199254740993)
        );
    }

    fn row(correct: usize, proposals: usize, threshold: f64) -> Row {
        let counts = Counts {
            correct_proposals: correct,
            proposals,
            unique_fields: 20,
            ..Counts::default()
        };
        Row {
            corpus: "test".into(),
            model: "test".into(),
            assignment: "independent".into(),
            threshold,
            metrics: counts.metrics(),
            counts,
        }
    }

    #[test]
    fn precision_frontier_requires_proposals_and_uses_declared_ties() {
        let rows = [
            row(0, 0, 1.0),
            row(8, 10, 0.6),
            row(8, 9, 0.7),
            row(8, 9, 0.8),
            row(7, 7, 0.9),
        ];
        assert_eq!(best_at_precision(rows.iter(), 0.8).unwrap().threshold, 0.8);
        assert_eq!(best_at_precision(rows.iter(), 0.95).unwrap().threshold, 0.9);
        assert!(best_at_precision(rows[..4].iter(), 0.95).is_none());
        assert!(best_at_precision(rows[..1].iter(), 0.0).is_none());
    }

    #[test]
    fn original_development_excludes_every_holdout_family() {
        let cases = original_development().unwrap();
        let protocol = corpus::protocol().unwrap();
        let holdout = protocol["holdout_families"].as_array().unwrap();
        assert_eq!(cases.len(), 160);
        assert!(cases.iter().all(|case| !holdout
            .iter()
            .any(|id| id.as_str() == Some(case.family.as_str()))));
    }

    #[test]
    fn combined_protocol_reproduces_public_defaults_for_both_assignment_modes() {
        use fieldkin::DataType;
        let protocol: Value = serde_json::from_str(PROTOCOL).unwrap();
        let mut source = Schema::new(vec![
            Field::new("source_amount", "Amt", DataType::Decimal)
                .with_samples(vec![SampleValue::Number(10.0); 8]),
            Field::new("source_key", "record_key", DataType::Integer).with_samples(vec![
                SampleValue::Integer(1),
                SampleValue::Integer(2),
                SampleValue::Integer(3),
            ]),
        ]);
        let mut target = Schema::new(vec![
            Field::new("target_amount", "amount", DataType::Decimal)
                .with_samples(vec![SampleValue::Number(10.0); 8]),
            Field::new("target_key", "record_key", DataType::Integer).with_samples(vec![
                SampleValue::Integer(1),
                SampleValue::Integer(2),
                SampleValue::Integer(3),
            ]),
        ]);
        for (id, kind, samples) in [
            (
                "flags",
                DataType::Boolean,
                vec![
                    SampleValue::Boolean(true),
                    SampleValue::Boolean(false),
                    SampleValue::Null,
                    SampleValue::Boolean(true),
                ],
            ),
            (
                "notes",
                DataType::Text,
                vec![
                    SampleValue::Text("one".into()),
                    SampleValue::Text("two".into()),
                    SampleValue::Text("three".into()),
                ],
            ),
            (
                "exact_amount",
                DataType::Decimal,
                vec![
                    SampleValue::Decimal(ExactDecimal::new(101, 2).unwrap()),
                    SampleValue::Decimal(ExactDecimal::new(202, 2).unwrap()),
                    SampleValue::Decimal(ExactDecimal::new(303, 2).unwrap()),
                ],
            ),
        ] {
            source
                .fields
                .push(Field::new(format!("source_{id}"), id, kind).with_samples(samples.clone()));
            target
                .fields
                .push(Field::new(format!("target_{id}"), id, kind).with_samples(samples));
        }
        for one_to_one in [false, true] {
            let configured = engine(&protocol, "combined", one_to_one, 0.70).unwrap();
            let public = MatchEngine::new(Config {
                one_to_one,
                ..Config::default()
            })
            .unwrap();
            assert_eq!(
                configured.match_schemas(&source, &target).unwrap(),
                public.match_schemas(&source, &target).unwrap()
            );
        }
    }

    #[test]
    fn stage3_has_no_holdout_scoring_option() {
        assert!(run(vec!["--split".into(), "holdout".into()])
            .unwrap_err()
            .contains("holdout scoring is unavailable"));
        assert!(run(vec!["--acknowledge-holdout".into()]).is_err());
    }
}
