use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use fieldkin::{
    Config, Evidence, Field, Limits, MatchEngine, Matcher, NameMatcher, SampleMatcher,
    SampleReliability, TypeMatcher, WeightedMatcher,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::corpus;
use crate::metrics::{score_case, Counts, Metrics};
use crate::model::Case;

struct Options {
    output: PathBuf,
    split: String,
    check: bool,
    check_behavior: bool,
    acknowledge_holdout: bool,
}

impl Options {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut result = Self {
            output: root().join("target/fieldkin-eval"),
            split: "development".into(),
            check: false,
            check_behavior: false,
            acknowledge_holdout: false,
        };
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--output" => result.output = args.next().ok_or("--output needs a directory")?.into(),
                "--split" => result.split = args.next().ok_or("--split needs development, holdout, or all")?,
                "--check" => result.check = true,
                "--check-behavior" => result.check_behavior = true,
                "--acknowledge-holdout" => result.acknowledge_holdout = true,
                _ => return Err(format!("unknown argument {arg}; use --output DIR, --split development|holdout|all, --check, --check-behavior, --acknowledge-holdout")),
            }
        }
        if !["development", "holdout", "all"].contains(&result.split.as_str()) {
            return Err("invalid split".into());
        }
        if result.check && result.check_behavior {
            return Err("--check and --check-behavior are mutually exclusive".into());
        }
        if result.check_behavior && result.split != "development" {
            return Err("--check-behavior is restricted to development data".into());
        }
        if result.split != "development" && !result.acknowledge_holdout {
            return Err("holdout evaluation requires --acknowledge-holdout; do not use holdout results for tuning".into());
        }
        Ok(result)
    }

    fn checking(&self) -> bool {
        self.check || self.check_behavior
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn number(protocol: &Value, key: &str) -> Result<f64, String> {
    protocol[key]
        .as_f64()
        .ok_or_else(|| format!("missing numeric protocol setting {key}"))
}

fn size(protocol: &Value, key: &str) -> Result<usize, String> {
    protocol[key]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| format!("invalid integer protocol setting {key}"))
}

struct MissingSignal(&'static str);
impl Matcher for MissingSignal {
    fn name(&self) -> &str {
        self.0
    }
    fn evaluate(&self, _: &Field, _: &Field) -> Result<Evidence, String> {
        Ok(Evidence {
            score: None,
            explanation: "Signal disabled for fixed-weight ablation".into(),
        })
    }
}

fn engine(protocol: &Value, model: &str, one_to_one: bool) -> Result<MatchEngine, String> {
    let limits = &protocol["limits"];
    let config = Config {
        corroboration: None,
        global_diagnostics: Default::default(),
        min_score: number(protocol, "min_score")?,
        ambiguity_margin: number(protocol, "ambiguity_margin")?,
        max_candidates: size(protocol, "max_candidates")?,
        one_to_one,
        abstain_on_ambiguity: protocol["abstain_on_ambiguity"]
            .as_bool()
            .ok_or("missing abstention setting")?,
        reject_incompatible_types: !["name_only", "no_type", "no_type_veto"].contains(&model),
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
    let aliases = protocol["aliases"]
        .as_object()
        .ok_or("missing aliases")?
        .iter()
        .map(|(k, v)| {
            v.as_str()
                .map(|v| (k.clone(), v.to_owned()))
                .ok_or_else(|| "invalid alias".to_owned())
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    let names = NameMatcher { aliases };
    let samples = SampleMatcher {
        min_non_null: size(protocol, "sample_min_non_null")?,
        reliability: SampleReliability::Legacy,
    };
    let weights = &protocol["weights"];
    let mut matchers = Vec::new();
    match model {
        "name_only" => matchers.push(WeightedMatcher::new(1.0, names)),
        "combined" | "no_name" | "no_samples" | "no_type" | "no_type_veto" => {
            matchers.push(if model == "no_name" {
                WeightedMatcher::new(number(weights, "name")?, MissingSignal("name"))
            } else {
                WeightedMatcher::new(number(weights, "name")?, names)
            });
            matchers.push(if model == "no_type" {
                WeightedMatcher::new(number(weights, "type")?, MissingSignal("type"))
            } else {
                WeightedMatcher::new(number(weights, "type")?, TypeMatcher)
            });
            matchers.push(if model == "no_samples" {
                WeightedMatcher::new(number(weights, "samples")?, MissingSignal("samples"))
            } else {
                WeightedMatcher::new(number(weights, "samples")?, samples)
            });
        }
        _ => return Err("unknown evaluation model".into()),
    }
    MatchEngine::with_matchers(config, matchers).map_err(|e| e.to_string())
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn normalized(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map(|s| normalized(&s))
        .map_err(|e| format!("{}: {e}", path.display()))
}

fn source_hashes(directory: &Path) -> Result<BTreeMap<String, String>, String> {
    fn visit(base: &Path, dir: &Path, files: &mut BTreeMap<String, String>) -> Result<(), String> {
        for entry in fs::read_dir(dir).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_dir() {
                visit(base, &path, files)?;
            } else if path.extension().is_some_and(|e| e == "rs") {
                let relative = path
                    .strip_prefix(base)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");
                files.insert(relative, hash(read(&path)?.as_bytes()));
            }
        }
        Ok(())
    }
    let mut files = BTreeMap::new();
    visit(directory, directory, &mut files)?;
    Ok(files)
}

fn metadata(protocol: &Value, corpus_bytes: &[u8]) -> Result<Value, String> {
    let lock = read(&root().join("Cargo.lock"))?;
    Ok(json!({
        "protocol": protocol,
        "protocol_sha256": hash(normalized(corpus::PROTOCOL).as_bytes()),
        "generated_corpus_sha256": hash(corpus_bytes),
        "family_file_sha256": corpus::FILES.iter().map(|(name,text)| (name.to_string(),hash(normalized(text).as_bytes()))).collect::<BTreeMap<_,_>>(),
        "engine_source_sha256": source_hashes(&root().join("src"))?,
        "evaluator_source_sha256": source_hashes(&root().join("evaluation/src"))?,
        "library_manifest_sha256": hash(read(&root().join("Cargo.toml"))?.as_bytes()),
        "evaluator_manifest_sha256": hash(read(&root().join("evaluation/Cargo.toml"))?.as_bytes()),
        "dependency_lock_sha256": hash(lock.as_bytes()),
        "dependency_lock": lock,
        "interpretation": "Synthetic micro averages; variants within a family are correlated. These are descriptive results, not population accuracy estimates."
    }))
}

#[derive(Default)]
struct Bucket {
    cases: usize,
    families: BTreeSet<String>,
    counts: Counts,
}

#[derive(Serialize)]
struct Row {
    model: String,
    assignment_mode: String,
    group_by: String,
    group: String,
    cases: usize,
    families: usize,
    counts: Counts,
    metrics: Metrics,
}

#[derive(Serialize)]
struct Report {
    metadata: Value,
    split: String,
    inventory: Value,
    results: Vec<Row>,
}

fn evaluate(
    cases: &[Case],
    protocol: &Value,
    metadata: &Value,
    split: &str,
) -> Result<(Report, String), String> {
    let cases: Vec<_> = cases.iter().filter(|c| c.split == split).collect();
    let mut buckets: BTreeMap<(String, String, String, String), Bucket> = BTreeMap::new();
    let mut details = String::new();
    let models = protocol["models"].as_array().ok_or("missing models")?;
    let modes = protocol["assignment_modes"]
        .as_array()
        .ok_or("missing assignment modes")?;
    for model in models {
        let model = model.as_str().ok_or("invalid model")?;
        for mode in modes {
            let mode = mode.as_str().ok_or("invalid assignment mode")?;
            let one_to_one = match mode {
                "independent" => false,
                "one_to_one" => true,
                _ => return Err("invalid assignment mode".into()),
            };
            let engine = engine(protocol, model, one_to_one)?;
            for case in &cases {
                let report = engine
                    .match_schemas(
                        &corpus::schema(&case.source)?,
                        &corpus::schema(&case.target)?,
                    )
                    .map_err(|e| format!("{} / {model} / {mode}: {e}", case.id))?;
                let counts = score_case(&case.labels, &report)?;
                let mut groups = vec![
                    ("all", "all"),
                    ("domain", case.domain.as_str()),
                    ("variant", case.variant.as_str()),
                ];
                for tag in &case.tags {
                    groups.push(("scenario", tag));
                }
                for (dimension, group) in groups {
                    let entry = buckets
                        .entry((model.into(), mode.into(), dimension.into(), group.into()))
                        .or_default();
                    entry.cases += 1;
                    entry.families.insert(case.family_id.clone());
                    entry.counts.add(&counts);
                }
                // Holdout reports are aggregate only. Individual failures are not opened
                // as part of this baseline measurement or routine CI.
                if split == "development" {
                    let fields: Vec<_> = report.fields.iter().map(|f| json!({
                        "source": f.source.0,
                        "selected": f.selected.as_ref().map(|c| c.target.0.as_str()),
                        "selected_score": f.selected.as_ref().map(|c| c.score),
                        "decision": format!("{:?}",f.decision),
                        "alternatives": f.alternatives.iter().map(|id| &id.0).collect::<Vec<_>>(),
                        "candidates": f.candidates.iter().map(|c| json!({"target":c.target.0,"score":c.score,"eligible":c.eligible})).collect::<Vec<_>>()
                    })).collect();
                    let line = json!({"protocol_version":protocol["protocol_version"],"baseline_commit":protocol["baseline_commit"],
                        "case":case.id,"family":case.family_id,"seed":case.seed,"model":model,"assignment_mode":mode,
                        "counts":counts,"predictions":fields});
                    details.push_str(&serde_json::to_string(&line).map_err(|e| e.to_string())?);
                    details.push('\n');
                }
            }
        }
    }
    let results = buckets
        .into_iter()
        .map(|((model, assignment_mode, group_by, group), bucket)| Row {
            model,
            assignment_mode,
            group_by,
            group,
            cases: bucket.cases,
            families: bucket.families.len(),
            metrics: bucket.counts.metrics(),
            counts: bucket.counts,
        })
        .collect();
    let family_ids: BTreeSet<_> = cases.iter().map(|c| c.family_id.as_str()).collect();
    let inventory = json!({"schema_pairs":cases.len(),"families":family_ids.len(),"family_ids":family_ids,
        "source_decisions":cases.iter().map(|c| c.labels.len()).sum::<usize>()});
    Ok((
        Report {
            metadata: metadata.clone(),
            split: split.into(),
            inventory,
            results,
        },
        details,
    ))
}

fn percentage(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".into(), |v| format!("{:.2}%", v * 100.0))
}

fn markdown(report: &Report) -> String {
    let mut text = format!("# Fieldkin baseline v1: {}\n\n{} schema pairs; {} fixture families; {} labeled source decisions.\n\nBaseline commit: `{}`. Protocol and exact source/dependency hashes are in the adjacent JSON.\n\n",report.split,report.inventory["schema_pairs"],report.inventory["families"],report.inventory["source_decisions"],report.metadata["protocol"]["baseline_commit"].as_str().unwrap_or("unknown"));
    text.push_str("These synthetic observations are not population accuracy estimates. Variants are correlated within families. Every metric is computed from the raw counts in JSON; n/a means a zero denominator.\n\n");
    text.push_str("Precision counts only correct unique-target proposals; any proposal on an ambiguous/no-match label is an error. Unique coverage counts proposals, including wrong targets, on uniquely labeled fields. Recall counts correct unique proposals. Candidate recall@5 includes ineligible ranks and all labeled alternatives. Expected-abstention accuracy covers both ambiguous and no-match labels.\n\n");
    text.push_str("Ablations preserve original weights with absent-evidence placeholders. In particular, no_name cannot reach the fixed 0.70 threshold; its abstention is not evidence that types or samples contain no information. No thresholds were tuned.\n\n");
    for (dimension, title) in [
        ("all", "Overall"),
        ("domain", "By domain"),
        ("variant", "By variant"),
        ("scenario", "By scenario (overlapping slices)"),
    ] {
        text.push_str(&format!("## {title}\n\n| Slice | Model | Assignment | Fields | Correct/proposed | Precision | Recall | Unique coverage | Candidate recall@5 | Abstention accuracy | No-match false proposals | Ambiguous unsafe proposals |\n| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n"));
        for row in report.results.iter().filter(|r| r.group_by == dimension) {
            text.push_str(&format!(
                "| {} | {} | {} | {} | {}/{} | {} | {} | {} | {} | {} | {}/{} | {}/{} |\n",
                row.group,
                row.model,
                row.assignment_mode,
                row.counts.fields,
                row.counts.correct_proposals,
                row.counts.proposals,
                percentage(row.metrics.precision),
                percentage(row.metrics.recall),
                percentage(row.metrics.unique_coverage),
                percentage(row.metrics.candidate_recall_at_5),
                percentage(row.metrics.expected_abstention_accuracy),
                row.counts.no_match_proposals,
                row.counts.no_match_fields,
                row.counts.ambiguous_proposals,
                row.counts.ambiguous_fields
            ));
        }
        text.push('\n');
    }
    text.push_str("Independent and one-to-one results use the same evidence and thresholds but different assignment constraints. JSON also reports feasible_recall: correct unique proposals divided by distinct unique gold targets per case in one-to-one mode. This exposes the ceiling when duplicate source facts share a target, while preserving ordinary field-level recall.\n");
    text
}

// Behavioral regression intentionally permits reviewed implementation changes.
// Everything else, including protocol, reference configuration, corpus hashes,
// inventory, counts and metrics, remains part of the comparison.
const IMPLEMENTATION_PROVENANCE: &[&str] = &["engine_source_sha256", "evaluator_source_sha256"];

fn behavioral_document(text: &str) -> Result<Value, String> {
    let mut document: Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
    let metadata = document["metadata"]
        .as_object_mut()
        .ok_or("behavioral snapshot has no metadata object")?;
    for key in IMPLEMENTATION_PROVENANCE {
        metadata
            .remove(*key)
            .ok_or_else(|| format!("behavioral snapshot is missing provenance field {key}"))?;
    }
    Ok(document)
}

fn snapshot_matches(
    name: &str,
    expected: &str,
    actual: &str,
    behavior: bool,
) -> Result<bool, String> {
    if behavior && name == "development.json" {
        Ok(behavioral_document(expected)? == behavioral_document(actual)?)
    } else {
        Ok(normalized(expected) == normalized(actual))
    }
}

fn artifact(options: &Options, name: &str, text: &str) -> Result<(), String> {
    let path = options.output.join(name);
    if options.checking() {
        if !snapshot_matches(name, &read(&path)?, text, options.check_behavior)? {
            return Err(format!(
                "snapshot differs: {}; investigate before updating a frozen baseline",
                path.display()
            ));
        }
    } else {
        if path.exists() && read(&path)? != normalized(text) {
            return Err(format!(
                "refusing to overwrite different frozen artifact {}; choose a new output directory",
                path.display()
            ));
        }
        fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))?;
    }
    Ok(())
}

pub fn run(args: Vec<String>) -> Result<(), String> {
    let options = Options::parse(args)?;
    let protocol = corpus::protocol()?;
    let families = corpus::families()?;
    let cases = corpus::cases(&families)?;
    let corpus_document = json!({"corpus_version":protocol["corpus_version"],"baseline_commit":protocol["baseline_commit"],
        "provenance":protocol["provenance"],"license":protocol["license"],"attribution":protocol["attribution"],"cases":cases});
    let mut corpus_text = serde_json::to_string(&corpus_document).map_err(|e| e.to_string())?;
    corpus_text.push('\n');
    let metadata = metadata(&protocol, corpus_text.as_bytes())?;
    if !options.checking() {
        fs::create_dir_all(&options.output).map_err(|e| e.to_string())?;
    }
    artifact(&options, "corpus.json", &corpus_text)?;
    let splits = if options.split == "all" {
        vec!["development", "holdout"]
    } else {
        vec![options.split.as_str()]
    };
    for split in splits {
        let (report, details) = evaluate(&cases, &protocol, &metadata, split)?;
        let mut json = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
        json.push('\n');
        artifact(&options, &format!("{split}.json"), &json)?;
        artifact(&options, &format!("{split}.md"), &markdown(&report))?;
        if split == "development" {
            artifact(&options, "development-predictions.jsonl", &details)?;
        }
        println!(
            "{split}: {} pairs / {} families / {} source decisions; 6 models x 2 assignment modes",
            report.inventory["schema_pairs"],
            report.inventory["families"],
            report.inventory["source_decisions"]
        );
    }
    if !options.checking() {
        let output = Command::new("rustc")
            .arg("-Vv")
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err("could not record Rust compiler environment".into());
        }
        // Environment is deliberately separate from reproducible result files.
        let environment = json!({"rustc":String::from_utf8_lossy(&output.stdout),"os":std::env::consts::OS,"arch":std::env::consts::ARCH,
            "note":"Compiler/environment metadata; not compared across platforms by --check. No timings in this accuracy evaluation."});
        artifact(
            &options,
            "environment.json",
            &format!(
                "{}\n",
                serde_json::to_string_pretty(&environment).map_err(|e| e.to_string())?
            ),
        )?;
    }
    println!(
        "{} {}",
        if options.check_behavior {
            "Verified development behavior (implementation provenance excluded) in"
        } else if options.check {
            "Verified frozen artifacts in"
        } else {
            "Wrote deterministic artifacts to"
        },
        options.output.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn holdout_requires_deliberate_opt_in() {
        assert!(Options::parse(vec!["--split".into(), "all".into()]).is_err());
        assert!(Options::parse(vec![
            "--split".into(),
            "holdout".into(),
            "--acknowledge-holdout".into()
        ])
        .is_ok());
        assert!(Options::parse(vec!["--output".into()]).is_err());
        assert!(Options::parse(vec!["--split".into(), "other".into()]).is_err());
        assert_eq!(Options::parse(Vec::new()).unwrap().split, "development");
    }

    #[test]
    fn behavioral_checks_are_explicit_and_development_only() {
        assert!(Options::parse(vec!["--check".into(), "--check-behavior".into()]).is_err());
        for split in ["holdout", "all"] {
            assert!(Options::parse(vec![
                "--check-behavior".into(),
                "--split".into(),
                split.into(),
                "--acknowledge-holdout".into(),
            ])
            .is_err());
        }
        let options = Options::parse(vec!["--check-behavior".into()]).unwrap();
        assert!(options.checking());
        assert_eq!(options.split, "development");
    }

    #[test]
    fn behavioral_checks_ignore_only_the_named_implementation_provenance() {
        let mut expected = json!({
            "metadata": {
                "protocol": {"min_score": 0.70, "baseline_commit": "frozen"},
                "protocol_sha256": "protocol",
                "generated_corpus_sha256": "corpus",
                "family_file_sha256": {"family.json": "fixture"},
                "library_manifest_sha256": "library manifest",
                "evaluator_manifest_sha256": "evaluator manifest",
                "dependency_lock_sha256": "locked dependencies",
                "dependency_lock": "lockfile",
            },
            "split": "development",
            "inventory": {"families": 32},
            "results": [{"counts": {"proposals": 438}, "metrics": {"precision": 0.76}}],
        });
        for key in IMPLEMENTATION_PROVENANCE {
            expected["metadata"][*key] = json!("old implementation");
        }
        let mut implementation_change = expected.clone();
        for key in IMPLEMENTATION_PROVENANCE {
            implementation_change["metadata"][*key] = json!("new implementation");
        }
        let expected_text = expected.to_string();
        assert!(snapshot_matches(
            "development.json",
            &expected_text,
            &implementation_change.to_string(),
            true,
        )
        .unwrap());
        assert!(!snapshot_matches(
            "development.json",
            &expected_text,
            &implementation_change.to_string(),
            false,
        )
        .unwrap());

        for pointer in [
            "/metadata/protocol/min_score",
            "/metadata/protocol/baseline_commit",
            "/metadata/protocol_sha256",
            "/metadata/generated_corpus_sha256",
            "/metadata/family_file_sha256/family.json",
            "/metadata/library_manifest_sha256",
            "/metadata/evaluator_manifest_sha256",
            "/metadata/dependency_lock_sha256",
            "/metadata/dependency_lock",
            "/inventory/families",
            "/results/0/counts/proposals",
            "/results/0/metrics/precision",
        ] {
            let mut changed = implementation_change.clone();
            *changed.pointer_mut(pointer).unwrap() = json!("changed");
            assert!(
                !snapshot_matches(
                    "development.json",
                    &expected_text,
                    &changed.to_string(),
                    true
                )
                .unwrap(),
                "behavioral comparison failed to detect {pointer}",
            );
        }
        let mut missing = expected.clone();
        missing["metadata"]
            .as_object_mut()
            .unwrap()
            .remove(IMPLEMENTATION_PROVENANCE[0]);
        assert!(behavioral_document(&missing.to_string()).is_err());
        let mut additional = expected.clone();
        additional["metadata"]["future_provenance"] = json!("must not be silently ignored");
        assert!(!snapshot_matches(
            "development.json",
            &expected_text,
            &additional.to_string(),
            true
        )
        .unwrap());
        for artifact in [
            "corpus.json",
            "development.md",
            "development-predictions.jsonl",
        ] {
            assert!(!snapshot_matches(artifact, "before", "after", true).unwrap());
            assert!(snapshot_matches(artifact, "same\r\n", "same\n", true).unwrap());
        }
    }

    #[test]
    fn historical_protocol_preserves_no_sample_defaults_and_ablation_scale() {
        let protocol = corpus::protocol().unwrap();
        let schema =
            fieldkin::Schema::new(vec![Field::new("x", "amount", fieldkin::DataType::Decimal)]);
        for one_to_one in [false, true] {
            let expected = MatchEngine::new(Config {
                one_to_one,
                ..Config::default()
            })
            .unwrap()
            .match_schemas(&schema, &schema)
            .unwrap();
            assert_eq!(
                engine(&protocol, "combined", one_to_one)
                    .unwrap()
                    .match_schemas(&schema, &schema)
                    .unwrap(),
                expected
            );
        }
        let report = engine(&protocol, "no_name", false)
            .unwrap()
            .match_schemas(&schema, &schema)
            .unwrap();
        assert!(report.fields[0].selected.is_none());
        assert!((report.fields[0].candidates[0].score - 0.2).abs() < 1e-12);
        assert_eq!(report.fields[0].candidates[0].signals[0].weight, 0.65);
    }
}
