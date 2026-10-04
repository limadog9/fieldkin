//! Fixed Northix class-equivalence diagnostic; never loads the T2D holdout.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use fieldkin::{
    Config, DataType, Decision, Evidence, Field, FieldMatch, GlobalDiagnosticsConfig, Limits,
    MatchEngine, Matcher, NameMatcher, SampleMatcher, SampleReliability, SampleValue, Schema,
    TypeMatcher, WeightedMatcher,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const CORPUS: &str = include_str!("../fixtures/northix-v1.json");
const PROTOCOL: &str = include_str!("../northix-protocol.json");
const PROVENANCE: &str = include_str!("../external/northix-v1/provenance.json");
const MAX_SCORE_FILE_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Corpus {
    format: String,
    tables: Vec<Table>,
    pairs: Vec<Pair>,
    labels: Vec<Label>,
    discrepancies: Vec<Value>,
    encoding_audit: Value,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Table {
    id: String,
    database: String,
    name: String,
    fields: Vec<InputField>,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct InputField {
    id: String,
    name: String,
    samples: Vec<Option<String>>,
    sample_row_indices: Vec<usize>,
    row_count: usize,
    input_path: String,
    input_sha256: String,
    input_bytes: usize,
    blank_rows: usize,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pair {
    id: String,
    source_table: String,
    target_table: String,
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Label {
    field: String,
    class: String,
    class_path: String,
    class_sha256: String,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ScoreFile {
    schema_version: usize,
    corpus_sha256: String,
    protocol_sha256: String,
    producer: Producer,
    tables: Vec<TableScores>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Producer {
    name: String,
    version: String,
    mode: String,
    provenance: Value,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct TableScores {
    id: String,
    scores: Vec<PairScore>,
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct PairScore {
    source: String,
    target: String,
    score: f64,
}

type ScoreMap = BTreeMap<(String, String), f64>;

struct Imported {
    producer: Producer,
    file_sha256: String,
    tables: BTreeMap<String, Arc<ScoreMap>>,
}

struct ImportedMatcher(Arc<ScoreMap>);

impl Matcher for ImportedMatcher {
    fn name(&self) -> &str {
        "valentine_coma"
    }

    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        let score = self
            .0
            .get(&(source.id.0.clone(), target.id.0.clone()))
            .copied();
        Ok(Evidence {
            score,
            explanation: if score.is_some() {
                "Pinned external COMA score; heuristic, not a probability."
            } else {
                "External matcher returned no score for this pair."
            }
            .into(),
        })
    }
}

fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn text_hash(text: &str) -> String {
    hash(text.replace("\r\n", "\n").as_bytes())
}

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn read(path: &Path) -> Result<Vec<u8>, String> {
    if path.is_symlink() {
        return Err(format!("refusing symlink input: {}", path.display()));
    }
    fs::read(path).map_err(|error| format!("{}: {error}", path.display()))
}

fn expected_indices(rows: usize) -> Vec<usize> {
    let count = rows.min(64);
    if count < 2 {
        return (0..count).collect();
    }
    (0..count).map(|i| i * (rows - 1) / (count - 1)).collect()
}

fn config(one_to_one: bool, reject_types: bool) -> Config {
    Config {
        min_score: 0.7,
        ambiguity_margin: 0.08,
        max_candidates: 5,
        one_to_one,
        abstain_on_ambiguity: true,
        reject_incompatible_types: reject_types,
        corroboration: None,
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
    }
}

fn protocol() -> Result<Value, String> {
    let value: Value = serde_json::from_str(PROTOCOL).map_err(|e| e.to_string())?;
    let settings = &value["settings"];
    if value["protocol_version"] != "northix-class-equivalence-1.0.0"
        || value["models"] != json!(["combined", "name_only"])
        || value["sample_modes"] != json!(["absent", "present"])
        || value["assignment_modes"] != json!(["independent", "one_to_one"])
        || value["inputs"]["types"] != "Unknown"
        || value["inputs"]["hints"] != "None"
        || value["inputs"]["sample_cap"] != 64
        || settings["min_score"] != 0.7
        || settings["ambiguity_margin"] != 0.08
        || settings["max_candidates"] != 5
        || settings["abstain_on_ambiguity"] != true
        || settings["combined_weights"] != json!({"name":0.65,"type":0.2,"samples":0.15})
        || settings["name_only_weight"] != 1.0
        || settings["name_aliases"] != json!({"amt":"amount","trans":"transaction"})
        || settings["sample_min_non_null"] != 3
        || settings["sample_reliability"] != "Distinct"
        || !settings["corroboration"].is_null()
        || settings["global_diagnostics_max_solves"] != 0
        || settings["limits"]
            != json!({"max_fields":128,"max_pairs":16384,"max_signal_evaluations":65536,"max_explanation_bytes":16777216,"max_name_bytes":256,"max_samples_per_field":256,"max_sample_bytes":1024,"max_total_sample_bytes":8388608})
        || value["external_scores"]["schema_version"] != 1
        || value["external_scores"]["producer"] != "valentine"
        || value["external_scores"]["version"] != "1.0.0"
        || value["external_scores"]["matcher"] != "Coma"
        || value["external_scores"]["modes"] != json!(["schema_only", "schema_and_samples"])
        || value["external_scores"]["parameters"]
            != json!({"max_n":0,"delta":1.0,"threshold":0.0,"use_schema":true,"instance_weight":1.0})
        || value["external_scores"]["table_names"] != json!(["aaa", "bbb"])
    {
        return Err("Northix fixed protocol settings changed".into());
    }
    Ok(value)
}

fn positive_targets(source: &str, target: &Table, labels: &BTreeMap<&str, &str>) -> Vec<String> {
    let class = labels[source];
    target
        .fields
        .iter()
        .filter(|field| class != "UNCLASSED" && labels[field.id.as_str()] == class)
        .map(|field| field.id.clone())
        .collect()
}

fn validate(corpus: &Corpus, protocol: &Value) -> Result<(), String> {
    if corpus.format != "fieldkin-northix-v1"
        || corpus.encoding_audit
            != json!({"encoding":"ISO-8859-1","ascii_files":104,"non_ascii_files":11,"cp1252_equivalent":true})
    {
        return Err("unknown Northix format or changed encoding audit".into());
    }
    let mut ids = BTreeSet::new();
    let mut tables = BTreeMap::new();
    let mut database_fields = BTreeMap::new();
    let mut database_tables = BTreeMap::new();
    for table in &corpus.tables {
        if !["1", "2"].contains(&table.database.as_str())
            || table.name.is_empty()
            || table.id != format!("db{}:{}", table.database, table.name)
            || table.fields.is_empty()
            || tables.insert(table.id.as_str(), table).is_some()
        {
            return Err("invalid or duplicate Northix table".into());
        }
        *database_tables
            .entry(table.database.as_str())
            .or_insert(0usize) += 1;
        *database_fields
            .entry(table.database.as_str())
            .or_insert(0usize) += table.fields.len();
        let mut names = BTreeSet::new();
        for field in &table.fields {
            let extension = if table.database == "1" { "dat" } else { "txt" };
            if field.id
                != format!(
                    "{}@{}@{}.{}",
                    field.name, table.name, table.database, extension
                )
                || !ids.insert(field.id.as_str())
                || !names.insert(field.name.as_str())
                || field.input_path != format!("Northix/InputData/{}", field.id)
                || field.row_count > 20_000
                || field.input_bytes > 512 * 1024
                || field.blank_rows > field.row_count
                || field.input_sha256.len() != 64
                || field.sample_row_indices != expected_indices(field.row_count)
                || field.samples.len() != field.sample_row_indices.len()
                || field.samples.iter().flatten().any(|s| s.trim().is_empty())
            {
                return Err("invalid Northix column or sampling metadata".into());
            }
        }
    }
    let mut labels = BTreeMap::new();
    for label in &corpus.labels {
        if !ids.contains(label.field.as_str())
            || label.class.is_empty()
            || labels
                .insert(label.field.as_str(), label.class.as_str())
                .is_some()
            || label.class_path != format!("Northix/Classes/{}/{}", label.class, label.field)
            || label.class_sha256.len() != 64
        {
            return Err("invalid or duplicate Northix class label".into());
        }
    }
    if labels.len() != ids.len() {
        return Err("unlabeled Northix field".into());
    }
    let mut pair_ids = BTreeSet::new();
    let mut fields = 0;
    let mut positives = 0;
    let mut positive_fields = 0;
    let mut no_match = 0;
    let mut unclassed = 0;
    let mut positive_pairs = 0;
    for pair in &corpus.pairs {
        let source = tables
            .get(pair.source_table.as_str())
            .ok_or("unknown source table")?;
        let target = tables
            .get(pair.target_table.as_str())
            .ok_or("unknown target table")?;
        if source.database != "1"
            || target.database != "2"
            || pair.id != format!("{}->{}", source.id, target.id)
            || !pair_ids.insert(&pair.id)
        {
            return Err("invalid or duplicated Northix table pair".into());
        }
        let before = positives;
        for field in &source.fields {
            let acceptable = positive_targets(&field.id, target, &labels);
            fields += 1;
            positives += acceptable.len();
            positive_fields += usize::from(!acceptable.is_empty());
            no_match += usize::from(acceptable.is_empty());
            unclassed += usize::from(labels[field.id.as_str()] == "UNCLASSED");
        }
        positive_pairs += usize::from(positives > before);
    }
    let mut discrepancies = BTreeSet::new();
    for item in &corpus.discrepancies {
        let field = item["field"].as_str().ok_or("missing discrepancy field")?;
        if !ids.contains(field)
            || !discrepancies.insert(field)
            || item["input_sha256"] == item["class_sha256"]
        {
            return Err("invalid input/class discrepancy inventory".into());
        }
    }
    let inventory = json!({"database_1_fields":database_fields.get("1").copied().unwrap_or(0),
        "database_2_fields":database_fields.get("2").copied().unwrap_or(0),
        "database_1_tables":database_tables.get("1").copied().unwrap_or(0),
        "database_2_tables":database_tables.get("2").copied().unwrap_or(0),
        "table_pairs":pair_ids.len(),"unique_fields":ids.len(),"label_classes":labels.values().collect::<BTreeSet<_>>().len(),
        "source_field_occurrences":fields,"positive_edges":positives,"positive_field_occurrences":positive_fields,
        "no_match_field_occurrences":no_match,"explicit_unclassed_field_occurrences":unclassed,
        "pairs_with_positive_edges":positive_pairs,"pairs_without_positive_edges":pair_ids.len()-positive_pairs,
        "value_copy_discrepancies":discrepancies.len()});
    if protocol["inventory"] != inventory || pair_ids.len() != 84 || tables.len() != 19 {
        return Err("Northix inventory differs from the frozen 84-pair task".into());
    }
    Ok(())
}

fn corpus(protocol: &Value) -> Result<Corpus, String> {
    let provenance: Value = serde_json::from_str(PROVENANCE).map_err(|e| e.to_string())?;
    let sha = hash(CORPUS.as_bytes());
    if protocol["corpus_sha256"] != sha || provenance["derived_fixture"]["sha256"] != sha {
        return Err("Northix fixture integrity failure".into());
    }
    let corpus = serde_json::from_str(CORPUS).map_err(|e| e.to_string())?;
    validate(&corpus, protocol)?;
    Ok(corpus)
}

fn validate_scores(
    file: ScoreFile,
    corpus: &Corpus,
    file_sha256: String,
) -> Result<Imported, String> {
    if file.schema_version != 1
        || file.corpus_sha256 != hash(CORPUS.as_bytes())
        || file.protocol_sha256 != hash(PROTOCOL.as_bytes())
        || file.producer.name != "valentine"
        || file.producer.version != "1.0.0"
        || !["schema_only", "schema_and_samples"].contains(&file.producer.mode.as_str())
        || !file.producer.provenance.is_object()
    {
        return Err("external scores have wrong corpus, protocol, producer, or format".into());
    }
    let tables: BTreeMap<_, _> = corpus
        .tables
        .iter()
        .map(|table| (table.id.as_str(), table))
        .collect();
    let pairs: BTreeMap<_, _> = corpus
        .pairs
        .iter()
        .map(|pair| (pair.id.as_str(), pair))
        .collect();
    let mut result = BTreeMap::new();
    for table in file.tables {
        let pair = pairs
            .get(table.id.as_str())
            .ok_or("unknown external score table pair")?;
        let source: BTreeSet<_> = tables[pair.source_table.as_str()]
            .fields
            .iter()
            .map(|f| f.id.as_str())
            .collect();
        let target: BTreeSet<_> = tables[pair.target_table.as_str()]
            .fields
            .iter()
            .map(|f| f.id.as_str())
            .collect();
        if table.scores.len() > source.len() * target.len() {
            return Err("too many external score pairs".into());
        }
        let mut scores = BTreeMap::new();
        for score in table.scores {
            if !source.contains(score.source.as_str())
                || !target.contains(score.target.as_str())
                || !score.score.is_finite()
                || !(0.0..=1.0).contains(&score.score)
                || scores
                    .insert((score.source, score.target), score.score)
                    .is_some()
            {
                return Err("external scores contain unknown, duplicate, or invalid pairs".into());
            }
        }
        if result.insert(table.id, Arc::new(scores)).is_some() {
            return Err("duplicated external score table pair".into());
        }
    }
    if result.len() != pairs.len() {
        return Err("external scores omit table pairs".into());
    }
    Ok(Imported {
        producer: file.producer,
        file_sha256,
        tables: result,
    })
}

fn load_scores(path: &Path, corpus: &Corpus) -> Result<Imported, String> {
    if path.is_symlink() {
        return Err("external score input cannot be a symlink".into());
    }
    if fs::metadata(path).map_err(|e| e.to_string())?.len() > MAX_SCORE_FILE_BYTES {
        return Err("external score file exceeds byte limit".into());
    }
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(|e| e.to_string())?
        .take(MAX_SCORE_FILE_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > MAX_SCORE_FILE_BYTES {
        return Err("external score file exceeds byte limit".into());
    }
    let file =
        serde_json::from_slice(&bytes).map_err(|e| format!("invalid external score JSON: {e}"))?;
    validate_scores(file, corpus, hash(&bytes))
}

fn engine(
    model: &str,
    one_to_one: bool,
    imported: Option<Arc<ScoreMap>>,
) -> Result<MatchEngine, String> {
    let names = NameMatcher {
        aliases: BTreeMap::from([
            ("amt".into(), "amount".into()),
            ("trans".into(), "transaction".into()),
        ]),
    };
    let matchers = if let Some(scores) = imported {
        vec![WeightedMatcher::new(1.0, ImportedMatcher(scores))]
    } else if model == "combined" {
        vec![
            WeightedMatcher::new(0.65, names),
            WeightedMatcher::new(0.2, TypeMatcher),
            WeightedMatcher::new(
                0.15,
                SampleMatcher {
                    min_non_null: 3,
                    reliability: SampleReliability::Distinct,
                },
            ),
        ]
    } else if model == "name_only" {
        vec![WeightedMatcher::new(1.0, names)]
    } else {
        return Err("unknown Northix matcher".into());
    };
    MatchEngine::with_matchers(config(one_to_one, model == "combined"), matchers)
        .map_err(|e| e.to_string())
}

fn schema(table: &Table, samples: bool) -> Schema {
    Schema::new(
        table
            .fields
            .iter()
            .map(|input| {
                let field = Field::new(input.id.clone(), input.name.clone(), DataType::Unknown);
                if samples {
                    field.with_samples(
                        input
                            .samples
                            .iter()
                            .map(|value| {
                                value.as_ref().map_or(SampleValue::Null, |value| {
                                    SampleValue::Text(value.clone())
                                })
                            })
                            .collect(),
                    )
                } else {
                    field
                }
            })
            .collect(),
    )
}

#[derive(Default, Serialize)]
struct Counts {
    table_pairs: usize,
    source_fields: usize,
    positive_source_fields: usize,
    positive_edges: usize,
    no_match_fields: usize,
    explicit_unclassed_fields: usize,
    proposed: usize,
    correct_proposals: usize,
    false_proposals: usize,
    false_proposals_on_matchable_fields: usize,
    no_match_false_proposals: usize,
    explicit_unclassed_false_proposals: usize,
    abstentions: usize,
    no_match_abstentions: usize,
    explicit_unclassed_abstentions: usize,
    candidate_positive_edges_at_5: usize,
    rejected_table_pairs: usize,
    rejected_source_fields: usize,
    rejected_no_match_fields: usize,
    rejected_explicit_unclassed_fields: usize,
}

fn ratio(n: usize, d: usize) -> Option<f64> {
    (d > 0).then(|| n as f64 / d as f64)
}

fn outcome(positives: &[String], selected: Option<&str>) -> &'static str {
    match selected {
        Some(target) if positives.iter().any(|p| p == target) => "correct",
        Some(_) => "false_proposal",
        None => "abstention",
    }
}

fn add_field(
    counts: &mut Counts,
    positive: &[String],
    unclassed: bool,
    field: &FieldMatch,
    scores: Option<&ScoreMap>,
) -> Result<(), String> {
    if field.selected.is_some() && field.decision != Decision::Proposed {
        return Err("caller confirmations cannot enter this diagnostic".into());
    }
    let selected = field.selected.as_ref().map(|c| c.target.0.as_str());
    if let (Some(target), Some(scores)) = (selected, scores) {
        if !scores.contains_key(&(field.source.0.clone(), target.into())) {
            return Err("missing external evidence was proposed".into());
        }
    }
    match outcome(positive, selected) {
        "correct" => {
            counts.proposed += 1;
            counts.correct_proposals += 1;
        }
        "false_proposal" => {
            counts.proposed += 1;
            counts.false_proposals += 1;
            counts.false_proposals_on_matchable_fields += usize::from(!positive.is_empty());
            counts.no_match_false_proposals += usize::from(positive.is_empty());
            counts.explicit_unclassed_false_proposals += usize::from(unclassed);
        }
        _ => {
            counts.abstentions += 1;
            counts.no_match_abstentions += usize::from(positive.is_empty());
            counts.explicit_unclassed_abstentions += usize::from(unclassed);
        }
    }
    let ranked: BTreeSet<_> = field
        .candidates
        .iter()
        .take(5)
        .filter(|candidate| {
            scores.is_none_or(|scores| {
                scores.contains_key(&(field.source.0.clone(), candidate.target.0.clone()))
            })
        })
        .map(|c| c.target.0.as_str())
        .collect();
    counts.candidate_positive_edges_at_5 += positive
        .iter()
        .filter(|target| ranked.contains(target.as_str()))
        .count();
    Ok(())
}

fn validate_counts(counts: &Counts) -> Result<(), String> {
    if counts.correct_proposals + counts.false_proposals != counts.proposed
        || counts.proposed + counts.abstentions + counts.rejected_source_fields
            != counts.source_fields
        || counts.positive_source_fields + counts.no_match_fields != counts.source_fields
        || counts.correct_proposals > counts.positive_source_fields
        || counts.false_proposals_on_matchable_fields + counts.no_match_false_proposals
            != counts.false_proposals
        || counts.no_match_false_proposals
            + counts.no_match_abstentions
            + counts.rejected_no_match_fields
            != counts.no_match_fields
        || counts.explicit_unclassed_false_proposals
            + counts.explicit_unclassed_abstentions
            + counts.rejected_explicit_unclassed_fields
            != counts.explicit_unclassed_fields
        || counts.explicit_unclassed_fields > counts.no_match_fields
        || counts.candidate_positive_edges_at_5 > counts.positive_edges
    {
        return Err("Northix outcome accounting is inconsistent".into());
    }
    Ok(())
}

fn result_row(group: &str, model: &str, samples: &str, assignment: &str, counts: &Counts) -> Value {
    json!({"group":group,"model":model,"samples":samples,"assignment":assignment,"counts":counts,
        "precision":ratio(counts.correct_proposals,counts.proposed),
        "positive_field_recall":ratio(counts.correct_proposals,counts.positive_source_fields),
        "positive_edge_recall":ratio(counts.correct_proposals,counts.positive_edges),
        "positive_edge_recall_at_5":ratio(counts.candidate_positive_edges_at_5,counts.positive_edges),
        "proposal_coverage":ratio(counts.proposed,counts.source_fields),
        "no_match_false_proposal_rate":ratio(counts.no_match_false_proposals,counts.no_match_fields),
        "explicit_unclassed_false_proposal_rate":ratio(counts.explicit_unclassed_false_proposals,counts.explicit_unclassed_fields)})
}

fn evaluate(corpus: &Corpus, imported: &[Imported]) -> Result<(Vec<Value>, String), String> {
    let tables: BTreeMap<_, _> = corpus.tables.iter().map(|t| (t.id.as_str(), t)).collect();
    let labels: BTreeMap<_, _> = corpus
        .labels
        .iter()
        .map(|l| (l.field.as_str(), l.class.as_str()))
        .collect();
    let mut specifications = Vec::new();
    for model in ["combined", "name_only"] {
        for samples in [false, true] {
            specifications.push((model.to_owned(), samples, None));
        }
    }
    for item in imported {
        specifications.push((
            format!("valentine_coma_{}", item.producer.mode),
            item.producer.mode == "schema_and_samples",
            Some(item),
        ));
    }
    let mut rows = Vec::new();
    let mut predictions = String::new();
    for (model, samples, external) in specifications {
        for one_to_one in [false, true] {
            let assignment = if one_to_one {
                "one_to_one"
            } else {
                "independent"
            };
            let sample_mode = if samples { "present" } else { "absent" };
            let mut groups: BTreeMap<String, Counts> = BTreeMap::new();
            for pair in &corpus.pairs {
                let source = tables[pair.source_table.as_str()];
                let target = tables[pair.target_table.as_str()];
                let keys = ["all", source.id.as_str()];
                let annotations: BTreeMap<_, _> = source
                    .fields
                    .iter()
                    .map(|field| {
                        (
                            field.id.as_str(),
                            (
                                positive_targets(&field.id, target, &labels),
                                labels[field.id.as_str()] == "UNCLASSED",
                            ),
                        )
                    })
                    .collect();
                for key in keys {
                    let counts = groups.entry(key.into()).or_default();
                    counts.table_pairs += 1;
                    counts.source_fields += source.fields.len();
                    for (positive, unclassed) in annotations.values() {
                        counts.positive_edges += positive.len();
                        counts.positive_source_fields += usize::from(!positive.is_empty());
                        counts.no_match_fields += usize::from(positive.is_empty());
                        counts.explicit_unclassed_fields += usize::from(*unclassed);
                    }
                }
                let score_map = external.map(|item| Arc::clone(&item.tables[&pair.id]));
                let matcher = engine(&model, one_to_one, score_map.clone())?;
                let report = match matcher
                    .match_schemas(&schema(source, samples), &schema(target, samples))
                {
                    Ok(report) => report,
                    Err(error) => {
                        for key in keys {
                            let counts = groups.entry(key.into()).or_default();
                            counts.rejected_table_pairs += 1;
                            counts.rejected_source_fields += source.fields.len();
                            counts.rejected_no_match_fields +=
                                annotations.values().filter(|(p, _)| p.is_empty()).count();
                            counts.rejected_explicit_unclassed_fields +=
                                annotations.values().filter(|(_, u)| *u).count();
                        }
                        predictions.push_str(&json!({"table":pair.id,"model":model,"samples":sample_mode,"assignment":assignment,"input_rejected":format!("{error:?}"),"source_fields":source.fields.len()}).to_string());
                        predictions.push('\n');
                        continue;
                    }
                };
                if report.fields.len() != source.fields.len() {
                    return Err("report omitted source fields".into());
                }
                let mut fields = Vec::new();
                for field in &report.fields {
                    let (positive, unclassed) = annotations
                        .get(field.source.0.as_str())
                        .ok_or("unknown report source")?;
                    for key in keys {
                        add_field(
                            groups.entry(key.into()).or_default(),
                            positive,
                            *unclassed,
                            field,
                            score_map.as_deref(),
                        )?;
                    }
                    fields.push(json!({"source":field.source.0,"positive_targets":positive,"explicit_unclassed":unclassed,"no_match":positive.is_empty(),
                    "outcome":outcome(positive,field.selected.as_ref().map(|c|c.target.0.as_str())),"decision":format!("{:?}",field.decision),
                    "selected":field.selected.as_ref().map(|c|json!({"target":c.target.0,"score":c.score})),
                    "candidates":field.candidates.iter().map(|c|json!({"target":c.target.0,"score":c.score,"eligible":c.eligible,"evidence_available":score_map.as_ref().is_none_or(|scores|scores.contains_key(&(field.source.0.clone(),c.target.0.clone())))})).collect::<Vec<_>>() }));
                }
                predictions.push_str(&json!({"table":pair.id,"model":model,"samples":sample_mode,"assignment":assignment,"fields":fields,"report_sha256":text_hash(&format!("{report:?}"))}).to_string());
                predictions.push('\n');
            }
            for counts in groups.values() {
                validate_counts(counts)?;
            }
            rows.extend(
                groups.iter().map(|(group, counts)| {
                    result_row(group, &model, sample_mode, assignment, counts)
                }),
            );
        }
    }
    Ok((rows, predictions))
}

fn input_hashes() -> Result<BTreeMap<String, String>, String> {
    fn visit(
        base: &Path,
        path: &Path,
        recursive: bool,
        result: &mut BTreeMap<String, String>,
    ) -> Result<(), String> {
        for entry in fs::read_dir(path).map_err(|e| e.to_string())? {
            let path = entry.map_err(|e| e.to_string())?.path();
            if path.is_symlink() {
                return Err("input inventory rejects symlinks".into());
            }
            if path.is_dir() {
                if recursive {
                    visit(base, &path, true, result)?;
                }
            } else if path
                .extension()
                .is_some_and(|e| e == "rs" || (!recursive && e == "py"))
            {
                let name = path
                    .strip_prefix(base)
                    .map_err(|e| e.to_string())?
                    .to_string_lossy()
                    .replace('\\', "/");
                let text = String::from_utf8(read(&path)?).map_err(|e| e.to_string())?;
                result.insert(name, text_hash(&text));
            }
        }
        Ok(())
    }
    let base = root();
    let mut result = BTreeMap::new();
    visit(&base, &base.join("src"), true, &mut result)?;
    visit(&base, &base.join("evaluation/src"), true, &mut result)?;
    visit(&base, &base.join("evaluation"), false, &mut result)?;
    for name in [
        "Cargo.toml",
        "Cargo.lock",
        "evaluation/Cargo.toml",
        "evaluation/fixtures/northix-v1.json",
        "evaluation/northix-protocol.json",
        "evaluation/external/northix-v1/northix.zip",
        "evaluation/external/northix-v1/provenance.json",
        "evaluation/external/northix-v1/license-evidence.json",
        "evaluation/external/northix-v1/LICENSE-CC-BY-4.0",
        "evaluation/external/northix-v1/NOTICE",
    ] {
        let bytes = read(&base.join(name))?;
        let digest = if matches!(name, "Cargo.toml" | "Cargo.lock" | "evaluation/Cargo.toml") {
            text_hash(&String::from_utf8(bytes).map_err(|e| e.to_string())?)
        } else {
            hash(&bytes)
        };
        result.insert(name.into(), digest);
    }
    if result.get("evaluation/fixtures/northix-v1.json") != Some(&hash(CORPUS.as_bytes()))
        || result.get("evaluation/northix-protocol.json") != Some(&hash(PROTOCOL.as_bytes()))
        || result.get("evaluation/external/northix-v1/provenance.json")
            != Some(&hash(PROVENANCE.as_bytes()))
    {
        return Err("embedded Northix inputs differ from checkout; rebuild before scoring".into());
    }
    let provenance: Value = serde_json::from_str(PROVENANCE).map_err(|e| e.to_string())?;
    if result
        .get("evaluation/external/northix-v1/northix.zip")
        .map(String::as_str)
        != provenance["archive"]["sha256"].as_str()
    {
        return Err("pinned Northix archive hash changed".into());
    }
    Ok(result)
}

struct Options {
    output: PathBuf,
    check: bool,
    scores: Vec<PathBuf>,
}
impl Options {
    fn parse(args: Vec<String>) -> Result<Self, String> {
        let mut output = None;
        let mut check = false;
        let mut scores = Vec::new();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            match arg.as_str() {
            "--output" if output.is_none()=>output=Some(PathBuf::from(args.next().ok_or("--output needs a directory")?)),
            "--check" if !check=>check=true,
            "--scores" if scores.len()<2=>scores.push(args.next().ok_or("--scores needs a file")?.into()),
            _=>return Err("Northix accepts --output DIR, --check, and at most two --scores FILE arguments".into()),
        }
        }
        if scores.len() == 1 {
            return Err(
                "Northix requires either no external score files or both approved modes".into(),
            );
        }
        Ok(Self {
            output: output.ok_or("Northix requires --output DIR")?,
            check,
            scores,
        })
    }
}

fn write_or_check(path: &Path, text: &str, check: bool) -> Result<(), String> {
    if check {
        if read(path)? != text.as_bytes() {
            return Err(format!(
                "{} differs from strict Northix snapshot",
                path.display()
            ));
        }
    } else {
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path)
            .map_err(|e| e.to_string())?
            .write_all(text.as_bytes())
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

pub fn run(args: Vec<String>) -> Result<(), String> {
    let options = Options::parse(args)?;
    let protocol = protocol()?;
    let corpus = corpus(&protocol)?;
    if !options.check && options.output.exists() {
        return Err(
            "Northix output must be a new directory; use --check to verify existing evidence"
                .into(),
        );
    }
    let before = input_hashes()?;
    let mut external = Vec::new();
    let mut modes = BTreeSet::new();
    for path in &options.scores {
        let item = load_scores(path, &corpus)?;
        if !modes.insert(item.producer.mode.clone()) {
            return Err("duplicated external producer mode".into());
        }
        external.push(item);
    }
    external.sort_by(|a, b| a.producer.mode.cmp(&b.producer.mode));
    if !options.check {
        if let Some(parent) = options.output.parent() {
            fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        fs::create_dir(&options.output).map_err(|e| e.to_string())?;
    }
    let (rows, predictions) = evaluate(&corpus, &external)?;
    if before != input_hashes()? {
        return Err("Northix source inputs changed during evaluation".into());
    }
    for path in &options.scores {
        let after = load_scores(path, &corpus)?;
        if !external.iter().any(|before| {
            before.producer.mode == after.producer.mode && before.file_sha256 == after.file_sha256
        }) {
            return Err("external scores changed during evaluation".into());
        }
    }
    let metadata = json!({"protocol":protocol,"input_sha256":before,"corpus_sha256":hash(CORPUS.as_bytes()),"protocol_sha256":hash(PROTOCOL.as_bytes()),
        "external_scores":external.iter().map(|item|json!({"producer":item.producer,"file_sha256":item.file_sha256,"table_pairs":item.tables.len(),"returned_pair_scores":item.tables.values().map(|s|s.len()).sum::<usize>()})).collect::<Vec<_>>(),
        "t2d_holdout":"reserved and not loaded or scored","source_hash_semantics":"normalized LF for Rust/Python source and Cargo files; exact bytes for corpus/protocol/data/licenses/scores. These are checkout hashes, not executable attestation; use the verified runner for recorded execution."});
    let summary = serde_json::to_string_pretty(&json!({"metadata":metadata,"rows":rows}))
        .map_err(|e| e.to_string())?
        + "\n";
    let mut markdown=String::from("# Northix class-equivalence diagnostic\n\nFixed 84 native table pairs, with 469 source-field occurrences, 28 labeled positive edges, 441 complement no-match occurrences and 70 explicitly UNCLASSED occurrences. Reused tables are correlated; this historical demonstration-data task does not certify production accuracy. No tuning or T2D holdout scoring.\n\n| Model | Samples | Assignment | Correct / proposed | Precision | Positive field recall | Candidate edge recall@5 | No-match false proposals | Explicit UNCLASSED false proposals | Rejected pairs |\n| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n");
    let percent = |value: &Value| {
        value
            .as_f64()
            .map_or("n/a".into(), |x| format!("{:.2}%", x * 100.0))
    };
    for row in &rows {
        if row["group"] == "all" {
            let c = &row["counts"];
            markdown.push_str(&format!(
                "| {} | {} | {} | {} / {} | {} | {} | {} | {} / {} | {} / {} | {} |\n",
                row["model"].as_str().unwrap_or(""),
                row["samples"].as_str().unwrap_or(""),
                row["assignment"].as_str().unwrap_or(""),
                c["correct_proposals"],
                c["proposed"],
                percent(&row["precision"]),
                percent(&row["positive_field_recall"]),
                percent(&row["positive_edge_recall_at_5"]),
                c["no_match_false_proposals"],
                c["no_match_fields"],
                c["explicit_unclassed_false_proposals"],
                c["explicit_unclassed_fields"],
                c["rejected_table_pairs"]
            ));
        }
    }
    markdown.push_str("\nPrecision uses published class equivalence as a closed task definition. Complement no-match cases include fields whose class has no representative in this particular target table; explicit UNCLASSED is a separately labeled subset. Rejected inputs stay in recall/no-match denominators and are not successful abstentions. Top-five retrieval includes ineligible candidates, but omitted external-score pairs are never counted as retrieved evidence. Candidate scores are heuristic, not calibrated probabilities. Detailed outcomes, rejected inputs and per-source-table groups remain in JSON/JSONL.\n");
    write_or_check(
        &options.output.join("results.json"),
        &summary,
        options.check,
    )?;
    write_or_check(
        &options.output.join("predictions.jsonl"),
        &predictions,
        options.check,
    )?;
    write_or_check(&options.output.join("results.md"), &markdown, options.check)?;
    println!(
        "Northix {}: 84 table pairs, {} fixed configurations; T2D holdout untouched",
        if options.check { "verified" } else { "written" },
        8 + external.len() * 2
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixture_and_sampling_validate_without_matching() {
        let c = corpus(&protocol().unwrap()).unwrap();
        assert_eq!(c.tables.len(), 19);
        assert_eq!(c.pairs.len(), 84);
        assert_eq!(expected_indices(0), Vec::<usize>::new());
        assert_eq!(expected_indices(1), vec![0]);
        assert_eq!(expected_indices(100).len(), 64);
        assert_eq!(expected_indices(100).last(), Some(&99));
    }

    #[test]
    fn class_labels_do_not_enter_input_schemas() {
        let c = corpus(&protocol().unwrap()).unwrap();
        let table = &c.tables[0];
        let absent = schema(table, false);
        let present = schema(table, true);
        for ((input, a), p) in table.fields.iter().zip(absent.fields).zip(present.fields) {
            assert_eq!(a.id.0, input.id);
            assert_eq!(a.name, input.name);
            assert_eq!(a.data_type, DataType::Unknown);
            assert!(a.samples.is_none());
            assert_eq!(p.samples.unwrap().len(), input.samples.len());
            assert_eq!(a.hints, fieldkin::SemanticHints::default());
        }
    }

    fn empty_scores(corpus: &Corpus) -> ScoreFile {
        ScoreFile {
            schema_version: 1,
            corpus_sha256: hash(CORPUS.as_bytes()),
            protocol_sha256: hash(PROTOCOL.as_bytes()),
            producer: Producer {
                name: "valentine".into(),
                version: "1.0.0".into(),
                mode: "schema_only".into(),
                provenance: json!({"test":true}),
            },
            tables: corpus
                .pairs
                .iter()
                .map(|p| TableScores {
                    id: p.id.clone(),
                    scores: Vec::new(),
                })
                .collect(),
        }
    }

    #[test]
    fn external_scores_require_complete_task_inventory_and_exact_hashes() {
        let c = corpus(&protocol().unwrap()).unwrap();
        let file = empty_scores(&c);
        assert!(validate_scores(file.clone(), &c, "test".into()).is_ok());
        let mut missing = file.clone();
        missing.tables.pop();
        assert!(validate_scores(missing, &c, "test".into()).is_err());
        let mut duplicate = file.clone();
        duplicate.tables[1] = duplicate.tables[0].clone();
        assert!(validate_scores(duplicate, &c, "test".into()).is_err());
        let mut changed = file;
        changed.protocol_sha256 = "different".into();
        assert!(validate_scores(changed, &c, "test".into()).is_err());
    }

    #[test]
    fn external_unknown_duplicate_and_nonfinite_scores_are_rejected() {
        let c = corpus(&protocol().unwrap()).unwrap();
        let pair = &c.pairs[0];
        let source = c
            .tables
            .iter()
            .find(|t| t.id == pair.source_table)
            .unwrap()
            .fields[0]
            .id
            .clone();
        let target = c
            .tables
            .iter()
            .find(|t| t.id == pair.target_table)
            .unwrap()
            .fields[0]
            .id
            .clone();
        for score in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
            let mut file = empty_scores(&c);
            file.tables[0].scores.push(PairScore {
                source: source.clone(),
                target: target.clone(),
                score,
            });
            assert!(validate_scores(file, &c, "test".into()).is_err());
        }
        let mut file = empty_scores(&c);
        let score = PairScore {
            source: source.clone(),
            target: target.clone(),
            score: 0.0,
        };
        file.tables[0].scores = vec![score.clone(), score];
        assert!(validate_scores(file, &c, "test".into()).is_err());
        let mut file = empty_scores(&c);
        file.tables[0].scores.push(PairScore {
            source: "unknown".into(),
            target,
            score: 0.5,
        });
        assert!(validate_scores(file, &c, "test".into()).is_err());
    }

    #[test]
    fn missing_external_score_stays_missing_while_explicit_zero_is_evidence() {
        let matcher = ImportedMatcher(Arc::new(BTreeMap::from([(("s".into(), "t".into()), 0.0)])));
        let source = Field::new("s", "same", DataType::Unknown);
        let target = Field::new("t", "same", DataType::Unknown);
        assert_eq!(matcher.evaluate(&source, &target).unwrap().score, Some(0.0));
        assert_eq!(
            matcher
                .evaluate(&source, &Field::new("other", "same", DataType::Unknown))
                .unwrap()
                .score,
            None
        );
    }

    #[test]
    fn no_match_and_explicit_unclassed_metrics_are_distinct() {
        let candidate = fieldkin::Candidate {
            target: "wrong".into(),
            score: 0.9,
            eligible: true,
            signals: vec![],
            warnings: vec![],
            issues: vec![],
        };
        let field = FieldMatch {
            source: "s".into(),
            candidates: vec![candidate.clone()],
            alternatives: vec![],
            selected: Some(candidate),
            decision: Decision::Proposed,
            diagnostics: vec![],
        };
        let mut counts = Counts {
            source_fields: 4,
            no_match_fields: 3,
            explicit_unclassed_fields: 1,
            positive_source_fields: 1,
            positive_edges: 1,
            rejected_source_fields: 2,
            rejected_no_match_fields: 1,
            ..Counts::default()
        };
        add_field(&mut counts, &[], false, &field, None).unwrap();
        add_field(&mut counts, &[], true, &field, None).unwrap();
        assert_eq!(counts.false_proposals, 2);
        assert_eq!(counts.no_match_false_proposals, 2);
        assert_eq!(counts.explicit_unclassed_false_proposals, 1);
        assert_eq!(counts.abstentions, 0);
        validate_counts(&counts).unwrap();
        let row = result_row("all", "test", "absent", "independent", &counts);
        assert_eq!(row["precision"], 0.0);
        assert_eq!(row["positive_field_recall"], 0.0);
        assert_eq!(row["no_match_false_proposal_rate"], 2.0 / 3.0);
        assert_eq!(row["explicit_unclassed_false_proposal_rate"], 1.0);
        assert_eq!(outcome(&["a".into(), "b".into()], Some("b")), "correct");
        assert_eq!(outcome(&[], None), "abstention");
    }

    #[test]
    fn omitted_external_pairs_cannot_inflate_candidate_recall() {
        let candidate = fieldkin::Candidate {
            target: "right".into(),
            score: 0.0,
            eligible: false,
            signals: vec![],
            warnings: vec![],
            issues: vec![],
        };
        let field = FieldMatch {
            source: "s".into(),
            candidates: vec![candidate],
            alternatives: vec![],
            selected: None,
            decision: Decision::BelowThreshold,
            diagnostics: vec![],
        };
        let mut counts = Counts::default();
        let scores = BTreeMap::new();
        add_field(&mut counts, &["right".into()], false, &field, Some(&scores)).unwrap();
        assert_eq!(counts.candidate_positive_edges_at_5, 0);
        let scores = BTreeMap::from([(("s".into(), "right".into()), 0.0)]);
        add_field(&mut counts, &["right".into()], false, &field, Some(&scores)).unwrap();
        assert_eq!(counts.candidate_positive_edges_at_5, 1);
    }

    #[test]
    fn cli_requires_explicit_destination_and_rejects_holdout_flags() {
        assert!(Options::parse(vec![]).is_err());
        assert!(Options::parse(vec!["--holdout".into()]).is_err());
        assert!(Options::parse(vec![
            "--output".into(),
            "fresh".into(),
            "--scores".into(),
            "only-one-mode".into()
        ])
        .is_err());
        let options = Options::parse(vec![
            "--output".into(),
            "fresh".into(),
            "--check".into(),
            "--scores".into(),
            "a".into(),
            "--scores".into(),
            "b".into(),
        ])
        .unwrap();
        assert!(options.check);
        assert_eq!(options.scores.len(), 2);
    }
}
