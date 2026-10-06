//! Post-qualification diagnostic helper.
//! Reads the already-examined fresh corpus and prints candidate score/eligibility details.
//! It does not modify matcher code, corpus data, or historical qualification outputs.

#[allow(dead_code)]
#[path = "../src/corpus.rs"]
mod corpus;
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
                _ => return Err("unknown conflict-rule kind".into()),
            };
            let alternatives: Vec<Vec<String>> =
                serde_json::from_value(rule["alternatives"].clone()).map_err(|e| e.to_string())?;
            Ok(NameConflictRule { kind, alternatives })
        })
        .collect()
}

fn engine() -> Result<MatchEngine> {
    MatchEngine::new(Config {
        min_score: 0.70,
        ambiguity_margin: 0.08,
        max_candidates: 5,
        one_to_one: false,
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

fn main() {
    if let Err(e) = run() {
        eprintln!("diagnostic failed: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: fresh_diagnose PATH_TO_CORPUS_JSON")?;

    let bytes = fs::read(&path).map_err(|e| e.to_string())?;
    let families: Vec<model::Family> = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;

    let engine = engine()?;

    for family in &families {
        corpus::validate_family(family)?;

        let source_schema = corpus::schema(&family.source)?;
        let target_schema = corpus::schema(&family.target)?;
        let report = engine
            .match_schemas(&source_schema, &target_schema)
            .map_err(|e| e.to_string())?;

        let source_names: BTreeMap<_, _> = family
            .source
            .iter()
            .map(|f| (f.id.as_str(), f.name.as_str()))
            .collect();
        let target_names: BTreeMap<_, _> = family
            .target
            .iter()
            .map(|f| (f.id.as_str(), f.name.as_str()))
            .collect();
        let labels: BTreeMap<_, _> = family
            .labels
            .iter()
            .map(|l| (l.source.as_str(), l))
            .collect();

        for field in &report.fields {
            let label = labels
                .get(field.source.0.as_str())
                .ok_or("missing label for report source")?;

            let correct_selected = match (&label.kind, &field.selected) {
                (model::LabelKind::Match, Some(selected)) => selected.target.0 == label.targets[0],
                _ => false,
            };

            let unsafe_selected = match label.kind {
                model::LabelKind::NoMatch | model::LabelKind::Ambiguous => field.selected.is_some(),
                model::LabelKind::Match => false,
            };

            let missed_match = label.kind == model::LabelKind::Match && !correct_selected;

            if !missed_match && !unsafe_selected {
                continue;
            }

            let source_name = source_names
                .get(field.source.0.as_str())
                .copied()
                .unwrap_or("<unknown>");

            let gold = label
                .targets
                .iter()
                .map(|id| {
                    target_names
                        .get(id.as_str())
                        .copied()
                        .unwrap_or("<unknown>")
                })
                .collect::<Vec<_>>()
                .join(" | ");

            let selected = field
                .selected
                .as_ref()
                .and_then(|c| target_names.get(c.target.0.as_str()).copied())
                .unwrap_or("-");

            println!();
            println!(
                "================================================================================"
            );
            println!("family:   {}  ({})", family.id, family.title);
            println!("source:   {}", source_name);
            println!(
                "truth:    {:?} -> {}",
                label.kind,
                if gold.is_empty() { "-" } else { &gold }
            );
            println!("decision: {:?}", field.decision);
            println!("selected: {}", selected);
            println!("alternatives: {:?}", field.alternatives);
            println!();

            for (rank, candidate) in field.candidates.iter().take(5).enumerate() {
                let target_name = target_names
                    .get(candidate.target.0.as_str())
                    .copied()
                    .unwrap_or("<unknown>");

                println!(
                    "#{} target={}  score={:.6}  eligible={}",
                    rank + 1,
                    target_name,
                    candidate.score,
                    candidate.eligible
                );

                let signals = candidate
                    .signals
                    .iter()
                    .map(|s| {
                        let score = s
                            .evidence
                            .score
                            .map(|v| format!("{v:.6}"))
                            .unwrap_or_else(|| "none".into());
                        format!("{}={}", s.name, score)
                    })
                    .collect::<Vec<_>>()
                    .join(", ");

                println!("   signals: {}", signals);
                println!("   issues:  {:?}", candidate.issues);
            }
        }
    }

    Ok(())
}
