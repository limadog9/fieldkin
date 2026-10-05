//! Prospective evaluation of one fixed runtime candidate, after a native freeze.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use fieldkin::{
    Config, ContextualEvidence, Decision, MatchEngine, NameMatcher, Schema, WeightedMatcher,
};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

use crate::metrics::{score_case, Counts, Metrics};
use crate::model::{Case, Family};
use crate::{corpus, readiness};

const PROTOCOL: &str = include_str!("../context-qualification-protocol.json");
const CORPUS: &str = include_str!("../corpus/qualification-20261004.json");
const PROVENANCE: &str = include_str!("../corpus/qualification-20261004.provenance.json");
const MODELS: [&str; 4] = ["default", "name_only_070", "name_only_exact", "context_v3"];

#[derive(Serialize)]
struct Row {
    model: String,
    assignment: String,
    counts: Counts,
    metrics: Metrics,
    quality_targets_met: bool,
    by_family: BTreeMap<String, Counts>,
    by_domain: BTreeMap<String, Counts>,
    by_variant: BTreeMap<String, Counts>,
}

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn normalized_hash(text: &str) -> String {
    digest(text.replace("\r\n", "\n").as_bytes())
}

fn protocol() -> Result<Value, String> {
    let value: Value = serde_json::from_str(PROTOCOL).map_err(|error| error.to_string())?;
    if value["models"] != json!(MODELS)
        || value["nominated_candidate"] != "context_v3"
        || value["assignments"] != json!(["independent", "one_to_one"])
        || value["threshold"] != 0.70
        || value["ambiguity_margin"] != 0.08
        || value["top_k"] != 5
        || value["partition"] != "all_holdout"
        || value["corpus"] != "qualification-20261004.json"
        || value["families"] != 12
        || value["cases"] != 60
        || value["decisions"] != 360
        || value["conflict_rules"] != "precision-protocol.json"
        || value["contextual_evidence"]
            != json!({"strict_identifier_samples":true,"scoped_support":true})
        || value["quality_targets"]
            != json!({"precision":0.95,"unique_coverage":0.60,"candidate_recall_at_5":0.90,"nonzero_proposals":true})
    {
        return Err("qualification protocol differs from the implemented fixed experiment".into());
    }
    Ok(value)
}

fn cases(protocol: &Value) -> Result<Vec<Case>, String> {
    if normalized_hash(CORPUS) != protocol["corpus_lf_sha256"] {
        return Err("reserved corpus differs from its pre-scoring author digest".into());
    }
    let mut families: Vec<Family> =
        serde_json::from_str(CORPUS).map_err(|error| error.to_string())?;
    if families.len() != 12 {
        return Err("qualification needs twelve independent families".into());
    }
    let mut ids = BTreeSet::new();
    let mut domains = BTreeMap::<String, usize>::new();
    for family in &families {
        corpus::validate_family(family)?;
        if !ids.insert(&family.id) {
            return Err("duplicate qualification family".into());
        }
        *domains.entry(family.domain.clone()).or_default() += 1;
    }
    if domains
        != [
            "product_import",
            "financial_records",
            "crm_contacts",
            "operational_telemetry",
        ]
        .into_iter()
        .map(|domain| (domain.to_owned(), 3))
        .collect()
    {
        return Err("qualification requires three families in each of the four domains".into());
    }
    families.sort_by(|left, right| left.id.cmp(&right.id));
    let mut cases = corpus::cases(&families)?;
    // The original protocol's old partition does not apply to this new corpus.
    // Every variant of every freshly authored family is reserved.
    for case in &mut cases {
        case.split = "holdout".into();
    }
    if cases.len() != 60 || cases.iter().map(|case| case.labels.len()).sum::<usize>() != 360 {
        return Err("qualification inventory differs from its fixed shape".into());
    }
    Ok(cases)
}

fn engine(model: &str, one_to_one: bool) -> Result<MatchEngine, String> {
    let config = Config {
        one_to_one,
        contextual_evidence: (model == "context_v3").then_some(ContextualEvidence::default()),
        name_conflicts: if model == "context_v3" {
            readiness::rules()?
        } else {
            Vec::new()
        },
        min_score: if model == "name_only_exact" {
            1.0
        } else {
            0.70
        },
        ..Config::default()
    };
    if model.starts_with("name_only") {
        MatchEngine::with_matchers(
            config,
            vec![WeightedMatcher::new(1.0, NameMatcher::default())],
        )
    } else {
        MatchEngine::new(config)
    }
    .map_err(|error| error.to_string())
}

fn quality(counts: &Counts) -> bool {
    let metrics = counts.metrics();
    counts.proposals > 0
        && metrics.precision.is_some_and(|value| value >= 0.95)
        && metrics.unique_coverage.is_some_and(|value| value >= 0.60)
        && metrics
            .candidate_recall_at_5
            .is_some_and(|value| value >= 0.90)
}

fn schemas(case: &Case) -> Result<(Schema, Schema), String> {
    Ok((corpus::schema(&case.source)?, corpus::schema(&case.target)?))
}

fn pct(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".into(), |value| format!("{:.2}%", value * 100.0))
}

fn markdown(rows: &[Row], qualified: bool) -> String {
    let mut text = String::from("# Prospective contextual qualification\n\nOne fixed opt-in runtime candidate, frozen before one newly authored synthetic holdout run. No caller hints or confirmations. All 12 families and five related variants are included; the variants are not independent observations. Labels reach only the scorer after schema matching.\n\n| Model | Assignment | Correct/proposed | Precision | Correct recall | Unique coverage | Candidate recall@5 | Wrong unique | No-match proposals | Ambiguous proposals | Targets met |\n| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | --- |\n");
    for row in rows {
        text.push_str(&format!(
            "| {} | {} | {}/{} | {} | {} | {} | {} | {} | {} | {} | {} |\n",
            row.model,
            row.assignment,
            row.counts.correct_proposals,
            row.counts.proposals,
            pct(row.metrics.precision),
            pct(row.metrics.recall),
            pct(row.metrics.unique_coverage),
            pct(row.metrics.candidate_recall_at_5),
            row.counts.wrong_unique_proposals,
            row.counts.no_match_proposals,
            row.counts.ambiguous_proposals,
            row.quality_targets_met
        ));
    }
    text.push_str(&format!("\nFixed synthetic quality targets met in both assignment modes: **{qualified}**. This is one agent-authored synthetic holdout, not a production accuracy guarantee. Defaults remain unchanged. No holdout-driven tuning is permitted after this report.\n"));
    text
}

fn write_new(path: &Path, text: &str) -> Result<(), String> {
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.write_all(text.as_bytes())
        .map_err(|error| error.to_string())
}

struct Options {
    output: PathBuf,
    binary_sha256: String,
    build_record_sha256: String,
}

fn options(args: Vec<String>) -> Result<Options, String> {
    let mut args = args.into_iter();
    let mut output = None;
    let mut binary = None;
    let mut build = None;
    let mut acknowledge = false;
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--output" => {
                output = Some(PathBuf::from(
                    args.next().ok_or("--output needs a directory")?,
                ))
            }
            "--frozen-binary-sha256" => binary = Some(args.next().ok_or("missing binary digest")?),
            "--frozen-build-record-sha256" => {
                build = Some(args.next().ok_or("missing build record digest")?)
            }
            "--acknowledge-new-holdout" => acknowledge = true,
            _ => return Err("qualification accepts only its fixed frozen-run options".into()),
        }
    }
    let binary_sha256 = binary.ok_or("run through native record-context with a frozen build")?;
    let build_record_sha256 = build.ok_or("frozen build record is required")?;
    if !acknowledge
        || [binary_sha256.as_str(), build_record_sha256.as_str()]
            .iter()
            .any(|value| value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()))
    {
        return Err(
            "qualification requires explicit new-holdout acknowledgement and frozen digests".into(),
        );
    }
    Ok(Options {
        output: output.ok_or("--output is required")?,
        binary_sha256,
        build_record_sha256,
    })
}

pub fn run(args: Vec<String>) -> Result<(), String> {
    let options = options(args)?;
    if options.output.exists() {
        return Err("prospective qualification requires a fresh output directory".into());
    }
    let binary = std::env::current_exe().map_err(|error| error.to_string())?;
    if digest(&fs::read(&binary).map_err(|error| error.to_string())?) != options.binary_sha256 {
        return Err("running evaluator differs from its frozen executable".into());
    }
    let protocol = protocol()?;
    let cases = cases(&protocol)?;
    let mut rows = Vec::new();
    let mut predictions = String::new();
    let mut observable_pair_hashes = Vec::new();
    for case in &cases {
        let (source, target) = schemas(case)?;
        observable_pair_hashes.push(digest(
            &serde_json::to_vec(&(
                readiness::observable_schema(&source),
                readiness::observable_schema(&target),
            ))
            .map_err(|error| error.to_string())?,
        ));
    }
    for model in MODELS {
        for assignment in ["independent", "one_to_one"] {
            let engine = engine(model, assignment == "one_to_one")?;
            let mut counts = Counts::default();
            let mut families = BTreeMap::<String, Counts>::new();
            let mut domains = BTreeMap::<String, Counts>::new();
            let mut variants = BTreeMap::<String, Counts>::new();
            for case in &cases {
                let (source, target) = schemas(case)?;
                let report = engine
                    .match_schemas(&source, &target)
                    .map_err(|error| error.to_string())?;
                if report
                    .fields
                    .iter()
                    .any(|field| field.decision == Decision::Confirmed)
                {
                    return Err(
                        "caller confirmations cannot count as autonomous qualification".into(),
                    );
                }
                let outcome = score_case(&case.labels, &report)?;
                counts.add(&outcome);
                families
                    .entry(case.family_id.clone())
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
                let fields: Vec<_> = report.fields.iter().map(|field| json!({"source":field.source.0,
                "decision":format!("{:?}",field.decision),"selected":field.selected.as_ref().map(|candidate|&candidate.target.0)})).collect();
                predictions.push_str(&serde_json::to_string(&json!({"case":case.id,"family":case.family_id,"domain":case.domain,
                "variant":case.variant,"model":model,"assignment":assignment,"counts":outcome,"fields":fields,
                "report_sha256":digest(format!("{report:?}").as_bytes())})).map_err(|error| error.to_string())?);
                predictions.push('\n');
            }
            rows.push(Row {
                model: model.into(),
                assignment: assignment.into(),
                metrics: counts.metrics(),
                quality_targets_met: quality(&counts),
                counts,
                by_family: families,
                by_domain: domains,
                by_variant: variants,
            });
        }
    }
    let qualified = rows
        .iter()
        .filter(|row| row.model == "context_v3")
        .all(|row| row.quality_targets_met);
    let report = json!({"metadata":{"protocol":protocol,"protocol_sha256":normalized_hash(PROTOCOL),
        "corpus_sha256":normalized_hash(CORPUS),"provenance_sha256":normalized_hash(PROVENANCE),
        "author_provenance":serde_json::from_str::<Value>(PROVENANCE).map_err(|error|error.to_string())?,
        "conflict_protocol_sha256":normalized_hash(include_str!("../precision-protocol.json")),
        "observable_inputs_sha256":digest(&serde_json::to_vec(&observable_pair_hashes).map_err(|error|error.to_string())?),
        "frozen_binary_sha256":options.binary_sha256,"frozen_build_record_sha256":options.build_record_sha256,
        "caller_hints_used":false,"caller_confirmations_used":false,"new_reserved_holdout_scored":true,
        "other_reserved_holdouts_scored":false,"defaults_changed":false},
        "inventory":{"families":12,"cases":60,"decisions":360,"partition":"all_holdout"},
        "rows":rows,"fixed_synthetic_quality_targets_met":qualified,"production_accuracy_certified":false});
    fs::create_dir_all(&options.output).map_err(|error| error.to_string())?;
    write_new(
        &options.output.join("qualification.json"),
        &(serde_json::to_string_pretty(&report).map_err(|error| error.to_string())? + "\n"),
    )?;
    write_new(
        &options.output.join("qualification.md"),
        &markdown(&rows, qualified),
    )?;
    write_new(&options.output.join("predictions.jsonl"), &predictions)?;
    println!("Frozen prospective qualification recorded; fixed synthetic targets met: {qualified}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_protocol_has_no_tuning_route() {
        protocol().unwrap();
        for flag in ["--min-score", "--check", "--split", "--answers", "--hints"] {
            assert!(options(vec![flag.into()]).is_err());
        }
        assert!(options(Vec::new()).is_err());
    }
    #[test]
    fn zero_proposals_and_low_coverage_do_not_qualify() {
        assert!(!quality(&Counts::default()));
        let mut counts = Counts {
            proposals: 5,
            correct_proposals: 5,
            unique_fields: 10,
            unique_proposals: 5,
            candidate_relevant: 10,
            candidate_hits: 10,
            ..Counts::default()
        };
        assert!(!quality(&counts));
        counts.unique_proposals = 6;
        assert!(quality(&counts));
        counts.correct_proposals = 4;
        assert!(!quality(&counts));
    }
}
