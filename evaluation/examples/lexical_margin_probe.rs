//! Diagnostic-only lexical-margin probe.
//! Simulates extra proposals without changing Fieldkin's matcher behavior.

#[allow(dead_code)]
#[path = "../src/model.rs"]
mod model;
#[allow(dead_code)]
#[path = "../src/metrics.rs"]
mod metrics;
#[allow(dead_code)]
#[path = "../src/corpus.rs"]
mod corpus;
#[allow(dead_code)]
#[path = "../src/corrective_corpus.rs"]
mod corrective_corpus;
#[allow(dead_code)]
#[path = "../src/snapshot.rs"]
mod snapshot;
#[allow(dead_code)]
#[path = "../src/stage3.rs"]
mod stage3;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use fieldkin::{
    normalize_name, CandidateIssue, Config, ContextualEvidence, DataType, MatchEngine,
    NameConflictKind, NameConflictRule, Schema,
};
use serde_json::Value;

const CONFLICT_PROTOCOL: &str = include_str!("../precision-protocol.json");
const OLD_QUALIFICATION: &str = include_str!("../corpus/qualification-20261004.json");

type Result<T> = std::result::Result<T, String>;

#[derive(Clone)]
struct ProbeCase {
    id: String,
    corpus_name: String,
    source: Schema,
    target: Schema,
    labels: Vec<model::Label>,
}

#[derive(Clone, Copy, Default)]
struct ProbeCounts {
    unique: usize,
    unique_proposals: usize,
    proposals: usize,
    correct: usize,
    unsafe_or_wrong: usize,
}

impl ProbeCounts {
    fn precision(self) -> Option<f64> {
        (self.proposals > 0).then(|| self.correct as f64 / self.proposals as f64)
    }
    fn coverage(self) -> Option<f64> {
        (self.unique > 0).then(|| self.unique_proposals as f64 / self.unique as f64)
    }
}

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

fn engine() -> Result<MatchEngine> {
    MatchEngine::new(Config {
        min_score: 0.70,
        ambiguity_margin: 0.08,
        max_candidates: 16,
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

fn model_case(case: model::Case, corpus_name: &str) -> Result<ProbeCase> {
    Ok(ProbeCase {
        id: case.id,
        corpus_name: corpus_name.to_owned(),
        source: corpus::schema(&case.source)?,
        target: corpus::schema(&case.target)?,
        labels: case.labels,
    })
}

fn cases(fresh_path: &PathBuf) -> Result<Vec<ProbeCase>> {
    let mut out = Vec::new();

    for case in corpus::cases(&corpus::families()?)? {
        if case.split == "development" {
            out.push(model_case(case, "original_development")?);
        }
    }

    for case in corrective_corpus::cases()? {
        if case.split == "development" {
            out.push(model_case(case, "corrective_development")?);
        }
    }

    for mut case in stage3::extension_cases()? {
        for field in case.source.fields.iter_mut().chain(&mut case.target.fields) {
            field.hints = Default::default();
        }
        out.push(ProbeCase {
            id: case.id,
            corpus_name: "stage3_extension".into(),
            source: case.source,
            target: case.target,
            labels: case.labels,
        });
    }

    let old_families: Vec<model::Family> =
        serde_json::from_str(OLD_QUALIFICATION).map_err(|e| e.to_string())?;
    for case in corpus::cases(&old_families)? {
        out.push(model_case(case, "examined_qualification")?);
    }

    let fresh_bytes = fs::read(fresh_path).map_err(|e| e.to_string())?;
    let fresh_families: Vec<model::Family> =
        serde_json::from_slice(&fresh_bytes).map_err(|e| e.to_string())?;
    for family in &fresh_families {
        corpus::validate_family(family)?;
    }
    for case in corpus::cases(&fresh_families)? {
        out.push(model_case(case, "fresh_development")?);
    }

    Ok(out)
}

fn base_score(candidate: &fieldkin::Candidate) -> f64 {
    candidate
        .signals
        .iter()
        .filter_map(|signal| signal.evidence.score.map(|score| score * signal.weight))
        .sum()
}

fn hard_blocked(candidate: &fieldkin::Candidate) -> bool {
    candidate.issues.iter().any(|issue| {
        matches!(
            issue,
            CandidateIssue::IncompatibleTypes
                | CandidateIssue::NameConflict(_)
                | CandidateIssue::SemanticConflict(_)
        )
    })
}

fn raw_name_equal(a: &str, b: &str) -> bool {
    normalize_name(a) == normalize_name(b)
}

#[derive(Clone, Copy, Debug)]
struct Rule {
    min_base: f64,
    row_margin: f64,
    col_margin: f64,
}

#[derive(Default)]
struct Evaluation {
    by_corpus: BTreeMap<String, ProbeCounts>,
    additions: Vec<String>,
}

fn evaluate(all_cases: &[ProbeCase], rule: Rule) -> Result<Evaluation> {
    let engine = engine()?;
    let mut result = Evaluation::default();

    for case in all_cases {
        let report = engine
            .match_schemas(&case.source, &case.target)
            .map_err(|e| e.to_string())?;

        let labels: BTreeMap<_, _> = case
            .labels
            .iter()
            .map(|label| (label.source.as_str(), label))
            .collect();
        let source_by_id: BTreeMap<_, _> = case
            .source
            .fields
            .iter()
            .map(|field| (field.id.0.as_str(), field))
            .collect();
        let target_by_id: BTreeMap<_, _> = case
            .target
            .fields
            .iter()
            .map(|field| (field.id.0.as_str(), field))
            .collect();

        // Column best/second among hard-compatible, exact-declared-type pairs,
        // excluding raw normalized-name equality.
        let mut column_scores: BTreeMap<String, Vec<(String, f64)>> = BTreeMap::new();
        for field in &report.fields {
            let source = source_by_id
                .get(field.source.0.as_str())
                .ok_or("missing source")?;
            for candidate in &field.candidates {
                let target = target_by_id
                    .get(candidate.target.0.as_str())
                    .ok_or("missing target")?;
                if hard_blocked(candidate)
                    || source.data_type == DataType::Unknown
                    || source.data_type != target.data_type
                    || raw_name_equal(&source.name, &target.name)
                {
                    continue;
                }
                column_scores
                    .entry(candidate.target.0.clone())
                    .or_default()
                    .push((field.source.0.clone(), base_score(candidate)));
            }
        }
        for scores in column_scores.values_mut() {
            scores.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
        }

        for field in &report.fields {
            let label = labels
                .get(field.source.0.as_str())
                .ok_or("missing label")?;
            let counts = result
                .by_corpus
                .entry(case.corpus_name.clone())
                .or_default();

            if label.kind == model::LabelKind::Match {
                counts.unique += 1;
            }

            if let Some(selected) = &field.selected {
                counts.proposals += 1;
                if label.kind == model::LabelKind::Match {
                    counts.unique_proposals += 1;
                }
                match label.kind {
                    model::LabelKind::Match if selected.target.0 == label.targets[0] => {
                        counts.correct += 1;
                    }
                    _ => counts.unsafe_or_wrong += 1,
                }
                continue;
            }

            let source = source_by_id
                .get(field.source.0.as_str())
                .ok_or("missing source")?;

            let mut candidates: Vec<_> = field
                .candidates
                .iter()
                .filter_map(|candidate| {
                    let target = target_by_id.get(candidate.target.0.as_str())?;
                    if hard_blocked(candidate)
                        || source.data_type == DataType::Unknown
                        || source.data_type != target.data_type
                        || raw_name_equal(&source.name, &target.name)
                    {
                        return None;
                    }
                    Some((candidate, *target, base_score(candidate)))
                })
                .collect();

            candidates.sort_by(|a, b| {
                b.2.total_cmp(&a.2)
                    .then(a.0.target.0.cmp(&b.0.target.0))
            });

            let Some((best, target, best_score)) = candidates.first().copied() else {
                continue;
            };
            let second = candidates.get(1).map(|entry| entry.2).unwrap_or(0.0);
            if best_score < rule.min_base || best_score < second + rule.row_margin {
                continue;
            }

            let column = column_scores
                .get(best.target.0.as_str())
                .cloned()
                .unwrap_or_default();
            let best_col = column.first().cloned();
            let second_col = column.get(1).map(|entry| entry.1).unwrap_or(0.0);
            let Some((best_source, col_score)) = best_col else {
                continue;
            };
            if best_source != field.source.0
                || col_score < second_col + rule.col_margin
            {
                continue;
            }

            counts.proposals += 1;
            if label.kind == model::LabelKind::Match {
                counts.unique_proposals += 1;
            }
            let correct =
                label.kind == model::LabelKind::Match && best.target.0 == label.targets[0];
            if correct {
                counts.correct += 1;
            } else {
                counts.unsafe_or_wrong += 1;
            }

            if case.corpus_name == "fresh_development" {
                result.additions.push(format!(
                    "{} :: {} -> {} :: truth={:?} {:?} :: base={:.3} row_margin={:.3} col_margin={:.3}",
                    case.id,
                    source.name,
                    target.name,
                    label.kind,
                    label.targets,
                    best_score,
                    best_score - second,
                    col_score - second_col,
                ));
            }
        }
    }

    Ok(result)
}

fn pct(v: Option<f64>) -> String {
    v.map_or_else(|| "n/a".into(), |x| format!("{:.2}%", x * 100.0))
}

fn main() {
    if let Err(error) = run() {
        eprintln!("probe failed: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    let fresh = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .ok_or("usage: lexical_margin_probe PATH_TO_FRESH_CORPUS")?;
    let all_cases = cases(&fresh)?;

    let mut viable = Vec::new();
    for min_base in [0.30, 0.35, 0.40, 0.45, 0.50, 0.55] {
        for row_margin in [0.05, 0.08, 0.10, 0.12, 0.15, 0.20] {
            for col_margin in [0.00, 0.05, 0.08, 0.10, 0.12, 0.15] {
                let rule = Rule {
                    min_base,
                    row_margin,
                    col_margin,
                };
                let eval = evaluate(&all_cases, rule)?;

                let safe = ["original_development", "stage3_extension", "corrective_development", "examined_qualification"]
                    .iter()
                    .all(|name| {
                        eval.by_corpus
                            .get(*name)
                            .and_then(|c| c.precision())
                            .is_some_and(|p| p >= 0.95)
                    });

                let fresh = eval
                    .by_corpus
                    .get("fresh_development")
                    .copied()
                    .unwrap_or_default();

                if safe {
                    viable.push((rule, fresh, eval));
                }
            }
        }
    }

    viable.sort_by(|a, b| {
        b.1.coverage()
            .unwrap_or(0.0)
            .total_cmp(&a.1.coverage().unwrap_or(0.0))
            .then(
                b.1.precision()
                    .unwrap_or(0.0)
                    .total_cmp(&a.1.precision().unwrap_or(0.0)),
            )
    });

    println!("# Lexical margin probe");
    println!();
    println!("No matcher behavior was changed. Raw normalized-name equality is excluded from the simulated rule.");
    println!();

    for (index, (rule, fresh, eval)) in viable.iter().take(10).enumerate() {
        println!(
            "{}. min_base={:.2}, row_margin={:.2}, col_margin={:.2} => fresh precision {}, coverage {}, {}/{}",
            index + 1,
            rule.min_base,
            rule.row_margin,
            rule.col_margin,
            pct(fresh.precision()),
            pct(fresh.coverage()),
            fresh.correct,
            fresh.proposals,
        );
        for name in [
            "original_development",
            "stage3_extension",
            "corrective_development",
            "examined_qualification",
        ] {
            let c = eval.by_corpus.get(name).copied().unwrap_or_default();
            println!(
                "   {name}: precision {}, correct/proposed {}/{}",
                pct(c.precision()),
                c.correct,
                c.proposals
            );
        }
    }

    if let Some((rule, _, best)) = viable.first() {
        println!();
        println!(
            "## Fresh additions for best safe rule ({:.2}/{:.2}/{:.2})",
            rule.min_base, rule.row_margin, rule.col_margin
        );
        let mut additions = best.additions.clone();
        additions.sort();
        additions.dedup();
        for line in additions {
            println!("{line}");
        }
    } else {
        println!("No threshold combination preserved >=95% precision on every old evidence corpus.");
    }

    Ok(())
}
