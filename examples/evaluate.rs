use std::{collections::BTreeSet, env, fs};

use fieldkin::{match_schemas, Config, Decision, Schema};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct EvalCase {
    name: String,
    source: Schema,
    target: Schema,
    answers: Vec<Answer>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum Answer {
    Match {
        source: String,
        target: String,
    },
    NoMatch {
        source: String,
    },
    Ambiguous {
        source: String,
        targets: Vec<String>,
    },
}

impl Answer {
    fn source(&self) -> &str {
        match self {
            Self::Match { source, .. }
            | Self::NoMatch { source }
            | Self::Ambiguous { source, .. } => source,
        }
    }

    fn expected_text(&self) -> String {
        match self {
            Self::Match { target, .. } => format!("match -> {target}"),
            Self::NoMatch { .. } => "no_match".into(),
            Self::Ambiguous { targets, .. } => {
                format!("ambiguous -> [{}]", targets.join(", "))
            }
        }
    }

    fn is_correct(&self, decision: &Decision) -> bool {
        match (self, decision) {
            (
                Self::Match { target, .. },
                Decision::Match {
                    target: predicted, ..
                },
            ) => target == predicted,

            (Self::NoMatch { .. }, Decision::NoMatch { .. }) => true,

            (
                Self::Ambiguous { targets, .. },
                Decision::Ambiguous {
                    targets: predicted, ..
                },
            ) => {
                let expected = targets.iter().cloned().collect::<BTreeSet<_>>();
                let predicted = predicted.iter().cloned().collect::<BTreeSet<_>>();
                expected == predicted
            }

            _ => false,
        }
    }
}

fn decision_text(decision: &Decision) -> String {
    match decision {
        Decision::Match { target, score } => {
            format!("match -> {target} ({score:.3})")
        }
        Decision::Ambiguous {
            targets,
            best_score,
        } => {
            format!("ambiguous -> [{}] ({best_score:.3})", targets.join(", "))
        }
        Decision::NoMatch { best_score } => match best_score {
            Some(score) => format!("no_match ({score:.3})"),
            None => "no_match".into(),
        },
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "eval/customers.json".into());

    let raw = fs::read_to_string(&path)?;
    let case: EvalCase = serde_json::from_str(&raw)?;

    let report = match_schemas(&case.source, &case.target, Config::default());

    println!();
    println!("CASE: {}", case.name);
    println!("{}", "=".repeat(100));
    println!("{:<22} {:<32} {:<32} RESULT", "SOURCE", "EXPECTED", "PREDICTED");
    println!("{}", "-".repeat(100));

    let mut correct = 0usize;

    for answer in &case.answers {
        let result = report
            .fields
            .iter()
            .find(|result| result.source == answer.source())
            .expect("answer key references a missing source field");

        let ok = answer.is_correct(&result.decision);
        if ok {
            correct += 1;
        }

        println!(
            "{:<22} {:<32} {:<32} {}",
            answer.source(),
            answer.expected_text(),
            decision_text(&result.decision),
            if ok { "PASS" } else { "FAIL" }
        );

        if !ok {
            println!("  candidates:");
            for candidate in &result.candidates {
                let sample = candidate
                    .sample_score
                    .map(|score| format!("{score:.3}"))
                    .unwrap_or_else(|| "n/a".into());

                println!(
                    "    {:<22} total={:.3}  name={:.3}  type={:.3}  samples={}",
                    candidate.target,
                    candidate.score,
                    candidate.name_score,
                    candidate.type_score,
                    sample
                );
            }
        }
    }

    println!("{}", "-".repeat(100));
    println!(
        "SCORE: {correct}/{} ({:.1}%)",
        case.answers.len(),
        100.0 * correct as f64 / case.answers.len() as f64
    );
    println!();

    Ok(())
}
