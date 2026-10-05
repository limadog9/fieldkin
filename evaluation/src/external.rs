//! Annotation-only external evidence. Reserved classes are never scored here.
//! `--check` retains complete provenance equality; explicit `--check-behavior`
//! exempts only the two implementation-source hash maps in the summary.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use fieldkin::{
    Config, DataType, Decision, Field, FieldMatch, GlobalDiagnosticsConfig, Limits, MatchEngine,
    NameMatcher, SampleMatcher, SampleReliability, Schema, TypeMatcher, WeightedMatcher,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const CORPUS: &str = include_str!("../external/t2d-v1/corpus.json");
const PROTOCOL: &str = include_str!("../external-protocol.json");
const PROVENANCE: &str = include_str!("../external/t2d-v1/provenance.json");
const MODELS: [&str; 2] = ["combined", "name_only"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    format: String,
    classes: Vec<Class>,
    tables: Vec<Table>,
    excluded: Vec<Excluded>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Class {
    uri: String,
    name: String,
    partition: String,
    properties: Vec<Property>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Property {
    uri: String,
    name: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Table {
    id: String,
    class_uri: String,
    fields: Vec<Annotation>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Annotation {
    index: usize,
    name: String,
    positive_targets: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Excluded {
    table_id: String,
    reason: String,
}

fn normalized(text: &str) -> String {
    text.replace("\r\n", "\n")
}

fn hash(text: &str) -> String {
    format!("{:x}", Sha256::digest(normalized(text).as_bytes()))
}

fn partition(uri: &str) -> &'static str {
    if Sha256::digest(uri.as_bytes())[0] % 5 == 0 {
        "holdout"
    } else {
        "development"
    }
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read(path: &Path) -> Result<String, String> {
    fs::read_to_string(path)
        .map(|value| normalized(&value))
        .map_err(|error| format!("{}: {error}", path.display()))
}

fn source_hashes(base: &Path) -> Result<BTreeMap<String, String>, String> {
    fn visit(
        base: &Path,
        directory: &Path,
        result: &mut BTreeMap<String, String>,
    ) -> Result<(), String> {
        for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            if path.is_symlink() {
                return Err("source hashing rejects symlinks".into());
            }
            if path.is_dir() {
                visit(base, &path, result)?;
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let name = path.strip_prefix(base).map_err(|error| error.to_string())?;
                result.insert(
                    name.to_string_lossy().replace('\\', "/"),
                    hash(&read(&path)?),
                );
            }
        }
        Ok(())
    }
    let mut result = BTreeMap::new();
    visit(base, base, &mut result)?;
    Ok(result)
}

fn protocol() -> Result<Value, String> {
    let value: Value = serde_json::from_str(PROTOCOL).map_err(|error| error.to_string())?;
    let settings = &value["settings"];
    if value["protocol_version"] != "t2d-annotations-1.0.0"
        || value["models"] != json!(MODELS)
        || value["assignment_modes"] != json!(["independent", "one_to_one"])
        || settings["min_score"] != 0.7
        || settings["ambiguity_margin"] != 0.08
        || settings["max_candidates"] != 5
        || settings["abstain_on_ambiguity"] != true
        || settings["name_aliases"] != json!({"amt":"amount", "trans":"transaction"})
        || settings["combined_weights"] != json!({"name":0.65,"type":0.2,"samples":0.15})
        || settings["name_only_weight"] != 1.0
        || settings["sample_min_non_null"] != 3
        || settings["sample_reliability"] != "Distinct"
        || settings["global_diagnostics_max_solves"] != 0
        || !settings["corroboration"].is_null()
        || settings["types"] != "Unknown"
        || settings["samples"] != "None"
        || settings["limits"]
            != json!({"max_fields":128,"max_pairs":16384,"max_signal_evaluations":65536,"max_explanation_bytes":16777216,"max_name_bytes":256,"max_samples_per_field":256,"max_sample_bytes":1024,"max_total_sample_bytes":8388608})
    {
        return Err("external protocol settings differ from its fixed experiment".into());
    }
    Ok(value)
}

fn validate(corpus: &Corpus, protocol: &Value) -> Result<(), String> {
    if corpus.format != "fieldkin-t2d-annotations-v1" {
        return Err("unknown external fixture format".into());
    }
    let mut classes = BTreeMap::new();
    for class in &corpus.classes {
        if class.uri.is_empty()
            || class.name.is_empty()
            || class.properties.is_empty()
            || class.partition != partition(&class.uri)
            || classes.insert(class.uri.as_str(), class).is_some()
        {
            return Err("invalid or duplicated class / partition".into());
        }
        let mut properties = BTreeSet::new();
        for property in &class.properties {
            if !properties.insert(&property.uri)
                || property.uri.rsplit(['/', '#']).next() != Some(property.name.as_str())
            {
                return Err("invalid property vocabulary".into());
            }
        }
    }
    let mut ids = BTreeSet::new();
    let mut vocabulary: BTreeMap<&str, BTreeSet<&str>> = BTreeMap::new();
    let mut inventory: BTreeMap<&str, [usize; 4]> = BTreeMap::new();
    for class in &corpus.classes {
        inventory.entry(&class.partition).or_default()[0] += 1;
    }
    for table in &corpus.tables {
        if !ids.insert(&table.id) || table.fields.is_empty() {
            return Err("duplicated table or empty annotated schema".into());
        }
        let class = classes
            .get(table.class_uri.as_str())
            .ok_or("missing table class")?;
        let properties: BTreeSet<_> = class.properties.iter().map(|p| p.uri.as_str()).collect();
        let counts = inventory.entry(&class.partition).or_default();
        counts[1] += 1;
        counts[2] += table.fields.len();
        let mut indices = BTreeSet::new();
        for field in &table.fields {
            let positives: BTreeSet<_> =
                field.positive_targets.iter().map(String::as_str).collect();
            if !indices.insert(field.index)
                || positives.is_empty()
                || positives.len() != field.positive_targets.len()
                || !positives.is_subset(&properties)
            {
                return Err("invalid known-positive annotation set".into());
            }
            counts[3] += positives.len();
            vocabulary.entry(&class.uri).or_default().extend(positives);
        }
    }
    for class in &corpus.classes {
        let expected = class
            .properties
            .iter()
            .map(|property| property.uri.as_str())
            .collect();
        if vocabulary.get(class.uri.as_str()) != Some(&expected) {
            return Err("class vocabulary is not the exact union of its annotations".into());
        }
    }
    for (name, counts) in inventory {
        if protocol["inventory"][name]
            != json!({"classes":counts[0],"tables":counts[1],"source_fields":counts[2],"known_positive_edges":counts[3]})
        {
            return Err("external partition inventory changed".into());
        }
    }
    let mut reasons = BTreeMap::new();
    for excluded in &corpus.excluded {
        if !ids.insert(&excluded.table_id)
            || !["empty_attribute_annotations", "missing_class_annotation"]
                .contains(&excluded.reason.as_str())
        {
            return Err("invalid excluded annotation inventory".into());
        }
        *reasons.entry(excluded.reason.as_str()).or_insert(0) += 1;
    }
    if reasons.get("empty_attribute_annotations") != Some(&971)
        || reasons.get("missing_class_annotation") != Some(&10)
    {
        return Err("excluded annotation counts changed".into());
    }
    Ok(())
}

fn corpus(protocol: &Value) -> Result<Corpus, String> {
    let provenance: Value = serde_json::from_str(PROVENANCE).map_err(|error| error.to_string())?;
    if protocol["corpus_sha256"] != hash(CORPUS)
        || provenance["derived_fixture"]["sha256"] != hash(CORPUS)
    {
        return Err("external fixture integrity failure".into());
    }
    let corpus = serde_json::from_str(CORPUS).map_err(|error| error.to_string())?;
    validate(&corpus, protocol)?;
    Ok(corpus)
}

fn engine(model: &str, one_to_one: bool) -> Result<MatchEngine, String> {
    let config = Config {
        name_conflicts: Vec::new(),
        min_score: 0.7,
        ambiguity_margin: 0.08,
        max_candidates: 5,
        one_to_one,
        abstain_on_ambiguity: true,
        reject_incompatible_types: model != "name_only",
        corroboration: None,
        contextual_evidence: None,
        global_diagnostics: GlobalDiagnosticsConfig {
            max_solves: 0,
            max_work: 8_388_608,
            objective_margin: 0.08,
        },
        limits: Limits {
            max_fields: 128,
            max_pairs: 16_384,
            max_signal_evaluations: 65_536,
            max_explanation_bytes: 16_777_216,
            max_name_bytes: 256,
            max_samples_per_field: 256,
            max_sample_bytes: 1024,
            max_total_sample_bytes: 8_388_608,
        },
    };
    let names = NameMatcher {
        aliases: BTreeMap::from([
            ("amt".into(), "amount".into()),
            ("trans".into(), "transaction".into()),
        ]),
    };
    let matchers = match model {
        "combined" => vec![
            WeightedMatcher::new(0.65, names),
            WeightedMatcher::new(0.2, TypeMatcher),
            WeightedMatcher::new(
                0.15,
                SampleMatcher {
                    min_non_null: 3,
                    reliability: SampleReliability::Distinct,
                },
            ),
        ],
        "name_only" => vec![WeightedMatcher::new(1.0, names)],
        _ => return Err("unknown external model".into()),
    };
    MatchEngine::with_matchers(config, matchers).map_err(|error| error.to_string())
}

#[derive(Default, Serialize)]
struct Counts {
    tables: usize,
    source_fields: usize,
    known_positive_edges: usize,
    proposed: usize,
    known_positive_proposals: usize,
    unknown_proposals: usize,
    abstentions: usize,
    candidate_positive_edges_at_5: usize,
    rejected_tables: usize,
    rejected_source_fields: usize,
}

fn outcome(positive_targets: &[String], selected: Option<&str>) -> &'static str {
    match selected {
        Some(target) if positive_targets.iter().any(|value| value == target) => "known_positive",
        Some(_) => "unknown",
        None => "abstention",
    }
}

fn add_field(
    counts: &mut Counts,
    annotation: &Annotation,
    field: &FieldMatch,
) -> Result<(), String> {
    if field.selected.is_some() && field.decision != Decision::Proposed {
        return Err("external evaluation must not count caller confirmations as proposals".into());
    }
    let selected = field
        .selected
        .as_ref()
        .map(|candidate| candidate.target.0.as_str());
    match outcome(&annotation.positive_targets, selected) {
        "known_positive" => {
            counts.proposed += 1;
            counts.known_positive_proposals += 1;
        }
        "unknown" => {
            counts.proposed += 1;
            counts.unknown_proposals += 1;
        }
        _ => counts.abstentions += 1,
    }
    let ranked: BTreeSet<_> = field
        .candidates
        .iter()
        .map(|candidate| candidate.target.0.as_str())
        .collect();
    counts.candidate_positive_edges_at_5 += annotation
        .positive_targets
        .iter()
        .filter(|target| ranked.contains(target.as_str()))
        .count();
    Ok(())
}

fn ratio(numerator: usize, denominator: usize) -> Option<f64> {
    (denominator > 0).then(|| numerator as f64 / denominator as f64)
}

fn row(class: &str, model: &str, assignment: &str, counts: &Counts) -> Value {
    json!({"class":class,"model":model,"assignment":assignment,"counts":counts,
        "known_positive_field_recall":ratio(counts.known_positive_proposals, counts.source_fields),
        "known_positive_edge_recall_at_5":ratio(counts.candidate_positive_edges_at_5, counts.known_positive_edges)})
}

fn evaluate(corpus: &Corpus) -> Result<(Vec<Value>, String), String> {
    let mut rows = Vec::new();
    let mut predictions = String::new();
    // Filter before creating an engine or converting a schema. There is no holdout CLI.
    let classes: BTreeMap<_, _> = corpus
        .classes
        .iter()
        .filter(|class| class.partition == "development")
        .map(|class| (class.uri.as_str(), class))
        .collect();
    for model in MODELS {
        for one_to_one in [false, true] {
            let assignment = if one_to_one {
                "one_to_one"
            } else {
                "independent"
            };
            let engine = engine(model, one_to_one)?;
            let mut groups: BTreeMap<String, Counts> = BTreeMap::new();
            for table in &corpus.tables {
                let Some(class) = classes.get(table.class_uri.as_str()) else {
                    continue;
                };
                let source = Schema::new(
                    table
                        .fields
                        .iter()
                        .map(|field| {
                            Field::new(
                                format!("column:{}", field.index),
                                &field.name,
                                DataType::Unknown,
                            )
                        })
                        .collect(),
                );
                let target = Schema::new(
                    class
                        .properties
                        .iter()
                        .map(|property| {
                            Field::new(property.uri.clone(), &property.name, DataType::Unknown)
                        })
                        .collect(),
                );
                let keys = ["all", class.uri.as_str()];
                for key in keys {
                    let counts = groups.entry(key.into()).or_default();
                    counts.tables += 1;
                    counts.source_fields += table.fields.len();
                    counts.known_positive_edges += table
                        .fields
                        .iter()
                        .map(|field| field.positive_targets.len())
                        .sum::<usize>();
                }
                let report = match engine.match_schemas(&source, &target) {
                    Ok(report) => report,
                    Err(error) => {
                        for key in keys {
                            let counts = groups.entry(key.into()).or_default();
                            counts.rejected_tables += 1;
                            counts.rejected_source_fields += table.fields.len();
                        }
                        predictions.push_str(&json!({"table":table.id,"class":class.uri,"model":model,"assignment":assignment,"input_rejected":format!("{error:?}"),"source_fields":table.fields.len()}).to_string());
                        predictions.push('\n');
                        continue;
                    }
                };
                let annotations: BTreeMap<_, _> = table
                    .fields
                    .iter()
                    .map(|field| (format!("column:{}", field.index), field))
                    .collect();
                if report.fields.len() != annotations.len() {
                    return Err("report omitted annotated fields".into());
                }
                let mut fields = Vec::new();
                for field in &report.fields {
                    let annotation = annotations
                        .get(&field.source.0)
                        .ok_or("unknown report source")?;
                    for key in keys {
                        add_field(groups.entry(key.into()).or_default(), annotation, field)?;
                    }
                    fields.push(json!({"source":field.source.0,"known_positive_targets":annotation.positive_targets,
                        "outcome":outcome(&annotation.positive_targets,field.selected.as_ref().map(|candidate|candidate.target.0.as_str())),
                        "decision":format!("{:?}",field.decision),
                        "selected":field.selected.as_ref().map(|candidate|json!({"target":candidate.target.0,"score":candidate.score})),
                        "candidates":field.candidates.iter().map(|candidate|json!({"target":candidate.target.0,"score":candidate.score,"eligible":candidate.eligible})).collect::<Vec<_>>() }));
                }
                predictions.push_str(&json!({"table":table.id,"class":class.uri,"model":model,"assignment":assignment,"fields":fields,"report_sha256":hash(&format!("{report:?}"))}).to_string());
                predictions.push('\n');
            }
            rows.extend(
                groups
                    .iter()
                    .map(|(class, counts)| row(class, model, assignment, counts)),
            );
        }
    }
    Ok((rows, predictions))
}

struct Options {
    output: PathBuf,
    check: bool,
    check_behavior: bool,
}

impl Options {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut result = Self {
            output: root().join("target/fieldkin-external"),
            check: false,
            check_behavior: false,
        };
        let mut arguments = args.into_iter();
        while let Some(argument) = arguments.next() {
            match argument.as_str() {
                "--output" => result.output = arguments.next().ok_or("--output needs a directory")?.into(),
                "--check" => result.check = true,
                "--check-behavior" => result.check_behavior = true,
                _ => return Err("external evaluation accepts --output DIR, --check or --check-behavior only; holdout classes remain reserved".into()),
            }
        }
        if result.check && result.check_behavior {
            return Err("--check and --check-behavior are mutually exclusive".into());
        }
        result.check |= result.check_behavior;
        Ok(result)
    }
}

fn write_or_check(
    path: &Path,
    contents: &str,
    check: bool,
    check_behavior: bool,
) -> Result<(), String> {
    if check || path.exists() {
        let existing = read(path)?;
        let same = if check_behavior {
            crate::snapshot::same_behavior_json(&existing, contents)?
        } else {
            existing == contents
        };
        if !same {
            return Err(format!(
                "{} differs; refusing to replace existing evaluation evidence",
                path.display()
            ));
        }
        return Ok(());
    }
    fs::write(path, contents).map_err(|error| error.to_string())
}

pub fn run(args: Vec<String>) -> Result<(), String> {
    let options = Options::parse(args)?;
    let protocol = protocol()?;
    let corpus = corpus(&protocol)?;
    let (rows, predictions) = evaluate(&corpus)?;
    let metadata = json!({"protocol":protocol,"protocol_sha256":hash(PROTOCOL),"corpus_sha256":hash(CORPUS),"provenance_sha256":hash(PROVENANCE),
        "engine_source_sha256":source_hashes(&root().join("src"))?,
        "evaluator_source_sha256":source_hashes(&root().join("evaluation/src"))?,
        "library_manifest_sha256":hash(&read(&root().join("Cargo.toml"))?),
        "evaluator_manifest_sha256":hash(&read(&root().join("evaluation/Cargo.toml"))?),
        "dependency_lock_sha256":hash(&read(&root().join("Cargo.lock"))?),
        "importer_sha256":hash(&read(&root().join("tooling/src/imports.rs"))?),
        "partition":"development","holdout":"reserved; no matcher executed for holdout classes"});
    let summary = serde_json::to_string_pretty(&json!({"metadata":metadata,"rows":rows}))
        .map_err(|error| error.to_string())?
        + "\n";
    let mut markdown = String::from("# T2D annotation-only development evidence\n\nThese are induced schema lookups using original T2D annotation headers and class-derived property vocabularies. No raw values or types are supplied. All absent labels are unknown; precision and unmatched-field safety cannot be measured. Holdout classes remain reserved.\n\n| Model | Assignment | Known-positive / proposed | Unknown proposals | Known-positive field recall | Positive edge recall@5 | Rejected tables / fields |\n| --- | --- | ---: | ---: | ---: | ---: | ---: |\n");
    for row in &rows {
        if row["class"] == "all" {
            let counts = &row["counts"];
            markdown.push_str(&format!(
                "| {} | {} | {} / {} | {} | {:.2}% | {:.2}% | {} / {} |\n",
                row["model"].as_str().unwrap_or(""),
                row["assignment"].as_str().unwrap_or(""),
                counts["known_positive_proposals"],
                counts["proposed"],
                counts["unknown_proposals"],
                row["known_positive_field_recall"].as_f64().unwrap_or(0.0) * 100.0,
                row["known_positive_edge_recall_at_5"]
                    .as_f64()
                    .unwrap_or(0.0)
                    * 100.0,
                counts["rejected_tables"],
                counts["rejected_source_fields"]
            ));
        }
    }
    markdown.push_str("\nRecall denominators include all 1,451 annotated development fields/edges, including input rejections. Top-5 candidate recall includes ineligible candidates. Known-positive/proposed is a count, not a precision estimate: unannotated choices remain unknown. Per-class counts and every rejected input/proposal are retained in JSON/JSONL. The 218 tables from 19 reserved classes were not scored.\n");
    if !options.check {
        fs::create_dir_all(&options.output).map_err(|error| error.to_string())?;
    }
    write_or_check(
        &options.output.join("development.json"),
        &summary,
        options.check,
        options.check_behavior,
    )?;
    write_or_check(
        &options.output.join("development-predictions.jsonl"),
        &predictions,
        options.check,
        false,
    )?;
    write_or_check(
        &options.output.join("development.md"),
        &markdown,
        options.check,
        false,
    )?;
    println!("external annotation development {}: 549 tables, 4 fixed model/assignment combinations; holdout reserved", if options.check { "verified" } else { "written" });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_integrity_and_class_partition_are_valid_without_matching() {
        let corpus = corpus(&protocol().unwrap()).unwrap();
        assert_eq!(corpus.tables.len(), 767);
        assert_eq!(corpus.classes.len(), 90);
        assert_eq!(corpus.excluded.len(), 981);
    }

    #[test]
    fn unknown_alternatives_are_never_false_positive_labels() {
        let positive = vec!["first".into(), "second".into()];
        assert_eq!(outcome(&positive, Some("first")), "known_positive");
        assert_eq!(outcome(&positive, Some("second")), "known_positive");
        assert_eq!(outcome(&positive, Some("other")), "unknown");
        assert_eq!(outcome(&positive, None), "abstention");
    }

    #[test]
    fn cli_cannot_score_reserved_classes() {
        for flag in ["--acknowledge-holdout", "--holdout", "--split"] {
            assert!(Options::parse(vec![flag.into()]).is_err());
        }
        let exact = Options::parse(vec!["--check".into()]).unwrap();
        assert!(exact.check && !exact.check_behavior);
        let behavior = Options::parse(vec!["--check-behavior".into()]).unwrap();
        assert!(behavior.check && behavior.check_behavior);
        assert!(Options::parse(vec!["--check".into(), "--check-behavior".into()]).is_err());
        for mode in ["--check", "--check-behavior"] {
            for flag in ["--acknowledge-holdout", "--holdout", "--split"] {
                assert!(Options::parse(vec![mode.into(), flag.into()]).is_err());
            }
        }
    }

    #[test]
    fn behavior_snapshot_exempts_only_implementation_hashes_and_never_rewrites() {
        let path = std::env::temp_dir().join(format!(
            "fieldkin-external-snapshot-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let original = json!({"metadata":{
            "engine_source_sha256":{"engine.rs":"old"},
            "evaluator_source_sha256":{"external.rs":"old"},
            "corpus_sha256":"corpus", "protocol_sha256":"protocol",
            "provenance_sha256":"license", "dependency_lock_sha256":"lock",
            "library_manifest_sha256":"manifest", "evaluator_manifest_sha256":"eval-manifest",
            "importer_sha256":"importer", "protocol":{"min_score":0.7}
        },"rows":[{"counts":{"proposed":3},"known_positive_field_recall":0.5}]})
        .to_string();
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        std::io::Write::write_all(&mut file, original.as_bytes()).unwrap();
        drop(file);
        let base: Value = serde_json::from_str(&original).unwrap();
        let mut current = base.clone();
        current["metadata"]["engine_source_sha256"] = json!({"engine.rs":"new", "new.rs":"new"});
        current["metadata"]["evaluator_source_sha256"] = json!({"external.rs":"new"});
        assert!(write_or_check(&path, &current.to_string(), true, false).is_err());
        assert!(write_or_check(&path, &current.to_string(), true, true).is_ok());
        for pointer in [
            "/metadata/corpus_sha256",
            "/metadata/protocol_sha256",
            "/metadata/provenance_sha256",
            "/metadata/dependency_lock_sha256",
            "/metadata/library_manifest_sha256",
            "/metadata/evaluator_manifest_sha256",
            "/metadata/importer_sha256",
            "/metadata/protocol/min_score",
            "/rows/0/counts/proposed",
            "/rows/0/known_positive_field_recall",
        ] {
            let mut current = base.clone();
            *current.pointer_mut(pointer).unwrap() = json!("changed");
            assert!(write_or_check(&path, &current.to_string(), true, true).is_err());
        }
        assert_eq!(fs::read_to_string(&path).unwrap(), original);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn metrics_keep_unknowns_separate_and_count_positive_edges_once() {
        let candidate = |target: &str| fieldkin::Candidate {
            target: target.into(),
            score: 0.9,
            eligible: true,
            signals: Vec::new(),
            warnings: Vec::new(),
            issues: Vec::new(),
        };
        let annotation = Annotation {
            index: 0,
            name: "field".into(),
            positive_targets: vec!["first".into(), "second".into()],
        };
        let mut field = FieldMatch {
            source: "column:0".into(),
            candidates: vec![candidate("first"), candidate("first"), candidate("second")],
            alternatives: Vec::new(),
            selected: Some(candidate("other")),
            decision: Decision::Proposed,
            diagnostics: Vec::new(),
        };
        let mut counts = Counts {
            // A second field was rejected with its schema. It still contributes
            // to recall denominators, but not to successful abstentions.
            source_fields: 2,
            known_positive_edges: 3,
            rejected_source_fields: 1,
            ..Counts::default()
        };
        add_field(&mut counts, &annotation, &field).unwrap();
        assert_eq!(counts.proposed, 1);
        assert_eq!(counts.unknown_proposals, 1);
        assert_eq!(counts.known_positive_proposals, 0);
        assert_eq!(counts.abstentions, 0);
        assert_eq!(counts.candidate_positive_edges_at_5, 2);
        let output = row("class", "name_only", "independent", &counts);
        assert_eq!(output["known_positive_field_recall"], 0.0);
        assert_eq!(output["known_positive_edge_recall_at_5"], 2.0 / 3.0);
        assert!(output.get("precision").is_none());
        field.decision = Decision::Confirmed;
        assert!(add_field(&mut counts, &annotation, &field).is_err());
    }

    #[test]
    fn invalid_class_split_or_unknown_positive_is_rejected() {
        let protocol = protocol().unwrap();
        let mut fixture = corpus(&protocol).unwrap();
        fixture.classes[0].partition = "not-a-partition".into();
        assert!(validate(&fixture, &protocol).is_err());
        let mut fixture = corpus(&protocol).unwrap();
        fixture.tables[0].fields[0]
            .positive_targets
            .push("unknown-property".into());
        assert!(validate(&fixture, &protocol).is_err());
    }
}
