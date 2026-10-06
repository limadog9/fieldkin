//! Repeatable development scoring for the already-examined fresh qualification corpus.
//! This is diagnostic/development evidence only; it does not create a qualification claim.

#[allow(dead_code)]
#[path = "../src/corpus.rs"]
mod corpus;
#[allow(dead_code)]
#[path = "../src/metrics.rs"]
mod metrics;
#[allow(dead_code)]
#[path = "../src/model.rs"]
mod model;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use fieldkin::{Config, ContextualEvidence, MatchEngine, NameConflictKind, NameConflictRule};
use serde_json::Value;

const CONFLICT_PROTOCOL: &str = include_str!("../precision-protocol.json");

type Result<T> = std::result::Result<T, String>;

fn conflict_rules() -> Result<Vec<NameConflictRule>> {
    let value: Value = serde_json::from_str(CONFLICT_PROTOCOL).map_err(|e| e.to_string())?;
    value["rules"]
        .as_array()
        .ok_or("missing conflict rules")?
        .iter()
        .map(|rule| {
            let kind = match rule["kind"].as_str() {
                Some("qualifier") => NameConflictKind::Qualifier,
                Some("unit") => NameConflictKind::Unit,
                _ => return Err("unknown conflict rule".into()),
            };
            let alternatives: Vec<Vec<String>> =
                serde_json::from_value(rule["alternatives"].clone()).map_err(|e| e.to_string())?;
            Ok(NameConflictRule { kind, alternatives })
        })
        .collect()
}

fn engine(one_to_one: bool) -> Result<MatchEngine> {
    MatchEngine::new(Config {
        min_score: 0.70,
        ambiguity_margin: 0.08,
        max_candidates: 5,
        one_to_one,
        abstain_on_ambiguity: true,
        reject_incompatible_types: true,
        corroboration: None,
        contextual_evidence: Some(ContextualEvidence {
            strict_identifier_samples: true,
            scoped_support: true,
        }),
        name_conflicts: conflict_rules()?,
        ..Config::default()
    })
    .map_err(|e| e.to_string())
}

fn pct(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".into(), |v| format!("{:.2}%", v * 100.0))
}

fn passes(c: &metrics::Counts) -> bool {
    let m = c.metrics();
    c.proposals > 0
        && m.precision.is_some_and(|v| v >= 0.95)
        && m.unique_coverage.is_some_and(|v| v >= 0.60)
        && m.candidate_recall_at_5.is_some_and(|v| v >= 0.90)
}

fn line(label: &str, c: &metrics::Counts) -> String {
    let m = c.metrics();
    format!(
        "| {label} | {}/{} | {} | {} | {} | {} | {} | {} |\n",
        c.correct_proposals,
        c.proposals,
        pct(m.precision),
        pct(m.unique_coverage),
        pct(m.candidate_recall_at_5),
        c.wrong_unique_proposals,
        c.no_match_proposals + c.ambiguous_proposals,
        passes(c),
    )
}

fn main() {
    if let Err(e) = run() {
        eprintln!("fresh development score failed: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: fresh_development_score PATH_TO_CORPUS_JSON")?;

    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let mut families: Vec<model::Family> =
        serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;

    if families.len() != 12 {
        return Err("expected exactly 12 families".into());
    }
    for family in &families {
        corpus::validate_family(family)?;
    }
    families.sort_by(|a, b| a.id.cmp(&b.id));

    let cases = corpus::cases(&families)?;
    if cases.len() != 60 {
        return Err(format!("expected 60 expanded cases, got {}", cases.len()));
    }

    let mut md = String::from(
        "# Examined fresh-corpus development score\n\n\
This corpus has already been examined and is now development evidence only.\n\n\
| Scope | Correct/proposed | Precision | Unique coverage | Recall@5 | Wrong unique | Unsafe no-match/ambiguous | Targets met |\n\
| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |\n"
    );

    for (mode, one_to_one) in [("independent", false), ("one_to_one", true)] {
        let engine = engine(one_to_one)?;
        let mut total = metrics::Counts::default();
        let mut by_variant: BTreeMap<String, metrics::Counts> = BTreeMap::new();
        let mut by_family: BTreeMap<String, metrics::Counts> = BTreeMap::new();

        for case in &cases {
            let source = corpus::schema(&case.source)?;
            let target = corpus::schema(&case.target)?;
            let report = engine
                .match_schemas(&source, &target)
                .map_err(|e| e.to_string())?;
            let counts = metrics::score_case(&case.labels, &report)?;

            total.add(&counts);
            by_variant.entry(case.variant.clone()).or_default().add(&counts);
            by_family.entry(case.family_id.clone()).or_default().add(&counts);
        }

        md.push_str(&line(mode, &total));

        md.push_str(&format!("\n## {mode}: variants\n\n"));
        md.push_str("| Scope | Correct/proposed | Precision | Unique coverage | Recall@5 | Wrong unique | Unsafe no-match/ambiguous | Targets met |\n");
        md.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |\n");
        for (variant, counts) in &by_variant {
            md.push_str(&line(variant, counts));
        }

        md.push_str(&format!("\n## {mode}: families\n\n"));
        md.push_str("| Scope | Correct/proposed | Precision | Unique coverage | Recall@5 | Wrong unique | Unsafe no-match/ambiguous | Targets met |\n");
        md.push_str("| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |\n");
        for (family, counts) in &by_family {
            md.push_str(&line(family, counts));
        }
    }

    print!("{md}");
    Ok(())
}
