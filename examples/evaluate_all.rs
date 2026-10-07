use std::{
    collections::BTreeSet,
    env,
    fs,
    path::Path,
};

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

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let eval_dir_arg = env::args()
        .nth(1)
        .unwrap_or_else(|| "eval".to_string());

    let eval_dir = Path::new(&eval_dir_arg);

    let mut files = fs::read_dir(eval_dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect::<Vec<_>>();

    files.sort();

    let mut total_correct = 0usize;
    let mut total_answers = 0usize;

    println!();
    println!("EVALUATION DIRECTORY: {}", eval_dir.display());
    println!();
    println!("{:<35} {:>10}", "DATASET", "SCORE");
    println!("{}", "-".repeat(47));

    for path in files {
        let raw = fs::read_to_string(&path)?;
        let case: EvalCase = serde_json::from_str(&raw)?;

        let report = match_schemas(&case.source, &case.target, Config::default());

        let mut correct = 0usize;

        for answer in &case.answers {
            let result = report
                .fields
                .iter()
                .find(|result| result.source == answer.source())
                .expect("answer key references a missing source field");

            if answer.is_correct(&result.decision) {
                correct += 1;
            }
        }

        total_correct += correct;
        total_answers += case.answers.len();

        println!(
            "{:<35} {:>3}/{:<3}",
            case.name,
            correct,
            case.answers.len()
        );
    }

    println!("{}", "-".repeat(47));

    let percentage = if total_answers == 0 {
        0.0
    } else {
        100.0 * total_correct as f64 / total_answers as f64
    };

    println!(
        "{:<35} {:>3}/{:<3} ({:.1}%)",
        "TOTAL",
        total_correct,
        total_answers,
        percentage
    );

    println!();

    Ok(())
}