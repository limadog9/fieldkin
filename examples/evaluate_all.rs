use std::{collections::BTreeSet, env, fs, path::Path};

use fieldkin::{Config, Decision, FieldResult, Schema, match_schemas};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct EvalCase {
    name: String,
    source: Schema,
    target: Schema,
    answers: Vec<Answer>,
}

#[derive(Debug, Deserialize, Clone)]
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

    fn display(&self) -> String {
        match self {
            Self::Match { target, .. } => {
                format!("match -> {target}")
            }

            Self::NoMatch { .. } => "no_match".to_string(),

            Self::Ambiguous { targets, .. } => {
                format!("ambiguous -> [{}]", targets.join(", "))
            }
        }
    }
}

fn decision_display(decision: &Decision) -> String {
    match decision {
        Decision::Match { target, score } => {
            format!("match -> {target} ({score:.3})")
        }

        Decision::NoMatch { best_score } => match best_score {
            Some(score) => format!("no_match ({score:.3})"),
            None => "no_match".to_string(),
        },

        Decision::Ambiguous {
            targets,
            best_score,
        } => {
            format!("ambiguous -> [{}] ({best_score:.3})", targets.join(", "))
        }
    }
}

fn print_failure(dataset_name: &str, answer: &Answer, result: &FieldResult) {
    println!();
    println!("{}", "=".repeat(100));
    println!("DATASET:  {dataset_name}");
    println!("SOURCE:   {}", answer.source());
    println!("EXPECTED: {}", answer.display());
    println!("GOT:      {}", decision_display(&result.decision));
    println!();

    println!("TOP CANDIDATES");

    if result.candidates.is_empty() {
        println!("  (none)");
        return;
    }

    for candidate in result.candidates.iter().take(3) {
        let sample = candidate
            .sample_score
            .map(|score| format!("{score:.3}"))
            .unwrap_or_else(|| "n/a".to_string());

        println!(
            "  {:<32} total={:.3}  name={:.3}  type={:.3}  samples={}",
            candidate.target, candidate.score, candidate.name_score, candidate.type_score, sample
        );
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let eval_dir_arg = env::args().nth(1).unwrap_or_else(|| "eval".to_string());

    let eval_dir = Path::new(&eval_dir_arg);

    let mut files = fs::read_dir(eval_dir)?
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect::<Vec<_>>();

    files.sort();

    let mut total_correct = 0usize;
    let mut total_answers = 0usize;

    let mut dataset_scores: Vec<(String, usize, usize)> = Vec::new();

    let mut failures: Vec<(String, Answer, FieldResult)> = Vec::new();

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
            } else {
                failures.push((case.name.clone(), answer.clone(), result.clone()));
            }
        }

        total_correct += correct;
        total_answers += case.answers.len();

        dataset_scores.push((case.name, correct, case.answers.len()));
    }

    println!();
    println!("EVALUATION DIRECTORY: {}", eval_dir.display());
    println!();

    println!("{:<70} {:>10}", "DATASET", "SCORE");
    println!("{}", "-".repeat(82));

    for (name, correct, total) in &dataset_scores {
        println!("{:<70} {:>3}/{:<3}", name, correct, total);
    }

    println!("{}", "-".repeat(82));

    let percentage = if total_answers == 0 {
        0.0
    } else {
        100.0 * total_correct as f64 / total_answers as f64
    };

    println!(
        "{:<70} {:>3}/{:<3} ({:.1}%)",
        "TOTAL", total_correct, total_answers, percentage
    );

    println!();
    println!("FAILURES: {}", failures.len());

    for (dataset_name, answer, result) in &failures {
        print_failure(dataset_name, answer, result);
    }

    println!();
    println!("{}", "=".repeat(100));
    println!(
        "FINAL SCORE: {}/{} ({:.1}%)",
        total_correct, total_answers, percentage
    );
    println!("TOTAL FAILURES: {}", failures.len());
    println!();

    Ok(())
}
