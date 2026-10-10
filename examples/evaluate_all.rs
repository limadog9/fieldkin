use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::Path,
    process::ExitCode,
};

use fieldkin::{Config, Decision, FieldResult, Schema, match_schemas};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const CORPORA: &[&str] = &["eval", "eval_realworld"];
const BASELINE: &str = "validation/quality_baseline.json";

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct EvalCase {
    name: String,
    source: Schema,
    target: Schema,
    answers: Vec<Answer>,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
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
    fn category(&self) -> Category {
        match self {
            Self::Match { .. } => Category::Match,
            Self::NoMatch { .. } => Category::NoMatch,
            Self::Ambiguous { .. } => Category::Ambiguous,
        }
    }

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
                let expected = targets.iter().collect::<BTreeSet<_>>();

                let predicted = predicted.iter().collect::<BTreeSet<_>>();

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

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
enum Category {
    Match,
    NoMatch,
    Ambiguous,
}

impl Category {
    const ALL: [Self; 3] = [Self::Match, Self::NoMatch, Self::Ambiguous];

    fn label(self) -> &'static str {
        match self {
            Self::Match => "match",
            Self::NoMatch => "no_match",
            Self::Ambiguous => "ambiguous",
        }
    }
}

// Store integer counts: with a frozen corpus, comparisons need no rounding or tolerance.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Counts {
    total: usize,
    correct: usize,
    incorrect_automatic_matches: usize,
    incorrect_ambiguities: usize,
}

impl Counts {
    fn observe(&mut self, answer: &Answer, decision: &Decision) {
        self.total += 1;
        if answer.is_correct(decision) {
            self.correct += 1;
        } else {
            self.incorrect_automatic_matches +=
                usize::from(matches!(decision, Decision::Match { .. }));
            self.incorrect_ambiguities +=
                usize::from(matches!(decision, Decision::Ambiguous { .. }));
        }
    }

    fn add(&mut self, other: &Self) {
        self.total += other.total;
        self.correct += other.correct;
        self.incorrect_automatic_matches += other.incorrect_automatic_matches;
        self.incorrect_ambiguities += other.incorrect_ambiguities;
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct Metrics {
    overall: Counts,
    by_expected_decision: BTreeMap<Category, Counts>,
}

impl Default for Metrics {
    fn default() -> Self {
        Self {
            overall: Counts::default(),
            by_expected_decision: Category::ALL
                .into_iter()
                .map(|kind| (kind, Counts::default()))
                .collect(),
        }
    }
}

impl Metrics {
    fn observe(&mut self, answer: &Answer, decision: &Decision) {
        self.overall.observe(answer, decision);
        self.by_expected_decision
            .get_mut(&answer.category())
            .unwrap()
            .observe(answer, decision);
    }

    fn add(&mut self, other: &Self) {
        self.overall.add(&other.overall);
        for kind in Category::ALL {
            self.by_expected_decision
                .get_mut(&kind)
                .unwrap()
                .add(&other.by_expected_decision[&kind]);
        }
    }

    fn automatic_match_precision(&self) -> String {
        let correct = self.by_expected_decision[&Category::Match].correct;
        ratio(
            correct,
            correct.saturating_add(self.overall.incorrect_automatic_matches),
        )
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct DatasetBaseline {
    sha256: String,
    metrics: Metrics,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Baseline {
    overall: Metrics,
    datasets: BTreeMap<String, DatasetBaseline>,
}

#[derive(Clone)]
struct EvaluatedCase {
    name: String,
    sha256: String,
    rows: Vec<(Answer, FieldResult)>,
}

type Evaluation = BTreeMap<String, EvaluatedCase>;

fn parse_case(raw: &str) -> Result<EvalCase, String> {
    let case: EvalCase = serde_json::from_str(raw).map_err(|error| error.to_string())?;
    if case.name.trim().is_empty() || case.source.fields.is_empty() || case.answers.is_empty() {
        return Err("dataset must have a name, source fields, and answers".into());
    }
    for (side, schema) in [("source", &case.source), ("target", &case.target)] {
        let mut names = BTreeSet::new();
        for field in &schema.fields {
            if field.name.trim().is_empty() || !names.insert(&field.name) {
                return Err(format!(
                    "{side}: empty or duplicate field name {:?}",
                    field.name
                ));
            }
        }
    }
    let sources: BTreeSet<_> = case
        .source
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    let targets: BTreeSet<_> = case
        .target
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    let mut labeled = BTreeSet::new();
    for answer in &case.answers {
        let source = answer.source();
        if !sources.contains(source) || !labeled.insert(source) {
            return Err(format!("{source}: unknown or duplicate answer source"));
        }
        let valid_targets = match answer {
            Answer::Match { target, .. } => targets.contains(target.as_str()),
            Answer::NoMatch { .. } => true,
            Answer::Ambiguous {
                targets: expected, ..
            } => {
                expected.len() >= 2
                    && expected.iter().collect::<BTreeSet<_>>().len() == expected.len()
                    && expected
                        .iter()
                        .all(|target| targets.contains(target.as_str()))
            }
        };
        if !valid_targets {
            return Err(format!(
                "{source}: invalid answer targets: {}",
                answer.display()
            ));
        }
    }
    if labeled != sources {
        return Err(format!(
            "missing answers for source fields: {:?}",
            sources.difference(&labeled).collect::<Vec<_>>()
        ));
    }
    Ok(case)
}

fn evaluate(root: &Path, directories: &[&str]) -> Result<Evaluation, String> {
    let mut evaluation = Evaluation::new();
    for directory in directories {
        let path = root.join(directory);
        let mut files = fs::read_dir(&path)
            .and_then(|entries| {
                entries
                    .map(|entry| entry.map(|entry| entry.path()))
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|error| format!("{}: {error}", path.display()))?;
        files.retain(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        });
        files.sort();
        if files.is_empty() {
            return Err(format!("{}: no JSON evaluation datasets", path.display()));
        }
        for path in files {
            let id = format!(
                "{directory}/{}",
                path.file_name().unwrap().to_string_lossy()
            );
            let raw = fs::read_to_string(&path).map_err(|error| format!("{id}: {error}"))?;
            let case = parse_case(&raw).map_err(|error| format!("{id}: {error}"))?;
            // Hash parsed data so checkout line endings/JSON whitespace do not matter.
            let sha256 = format!(
                "{:x}",
                Sha256::digest(serde_json::to_vec(&case).map_err(|error| error.to_string())?)
            );
            let report = match_schemas(&case.source, &case.target, Config::default());
            let field_count = report.fields.len();
            let mut results: BTreeMap<_, _> = report
                .fields
                .into_iter()
                .map(|field| (field.source.clone(), field))
                .collect();
            if results.len() != field_count {
                return Err(format!("{id}: matcher returned duplicate source fields"));
            }
            let mut rows = Vec::with_capacity(case.answers.len());
            for answer in case.answers {
                let result = results.remove(answer.source()).ok_or_else(|| {
                    format!("{id}: matcher omitted source field {}", answer.source())
                })?;
                rows.push((answer, result));
            }
            if !results.is_empty() {
                return Err(format!("{id}: matcher returned unexpected source fields"));
            }
            if evaluation
                .insert(
                    id.clone(),
                    EvaluatedCase {
                        name: case.name,
                        sha256,
                        rows,
                    },
                )
                .is_some()
            {
                return Err(format!("duplicate dataset: {id}"));
            }
        }
    }
    if evaluation.is_empty() {
        return Err("no evaluation datasets".into());
    }
    Ok(evaluation)
}

fn summarize(evaluation: &Evaluation) -> Baseline {
    let mut overall = Metrics::default();
    let datasets = evaluation
        .iter()
        .map(|(id, case)| {
            let mut metrics = Metrics::default();
            for (answer, result) in &case.rows {
                metrics.observe(answer, &result.decision);
            }
            overall.add(&metrics);
            (
                id.clone(),
                DatasetBaseline {
                    sha256: case.sha256.clone(),
                    metrics,
                },
            )
        })
        .collect();
    Baseline { overall, datasets }
}

fn compare_counts(scope: &str, current: &Counts, baseline: &Counts, errors: &mut Vec<String>) {
    if current.total != baseline.total {
        errors.push(format!(
            "{scope}: case count changed: {} -> {}",
            baseline.total, current.total
        ));
    }
    if baseline
        .correct
        .saturating_add(baseline.incorrect_automatic_matches)
        .saturating_add(baseline.incorrect_ambiguities)
        > baseline.total
    {
        errors.push(format!("{scope}: invalid baseline counts"));
    }
    if current.correct < baseline.correct {
        errors.push(format!(
            "{scope}: accuracy regressed: {} -> {}",
            ratio(baseline.correct, baseline.total),
            ratio(current.correct, current.total)
        ));
    }
    for (metric, before, after) in [
        (
            "incorrect automatic matches",
            baseline.incorrect_automatic_matches,
            current.incorrect_automatic_matches,
        ),
        (
            "incorrect ambiguities",
            baseline.incorrect_ambiguities,
            current.incorrect_ambiguities,
        ),
    ] {
        if after > before {
            errors.push(format!("{scope}: {metric} increased: {before} -> {after}"));
        }
    }
}

fn compare_metrics(scope: &str, current: &Metrics, baseline: &Metrics, errors: &mut Vec<String>) {
    let previous_errors = errors.len();
    compare_counts(scope, &current.overall, &baseline.overall, errors);
    for kind in Category::ALL {
        let category_scope = format!("{scope} / expected {}", kind.label());
        match baseline.by_expected_decision.get(&kind) {
            Some(counts) => compare_counts(
                &category_scope,
                &current.by_expected_decision[&kind],
                counts,
                errors,
            ),
            None => errors.push(format!("{category_scope}: missing baseline category")),
        }
    }
    // The correct-match floor and incorrect-match ceiling protect precision;
    // this is diagnostic only, with no comparison of rounded percentages.
    if errors.len() > previous_errors
        && baseline.by_expected_decision.contains_key(&Category::Match)
        && current.automatic_match_precision() != baseline.automatic_match_precision()
    {
        errors.push(format!(
            "{scope}: automatic-match precision: {} -> {}",
            baseline.automatic_match_precision(),
            current.automatic_match_precision()
        ));
    }
}

fn check(evaluation: &Evaluation, baseline: &Baseline) -> Result<(), String> {
    if evaluation.is_empty() || baseline.datasets.is_empty() {
        return Err("quality gate requires nonempty evaluation data and baseline".into());
    }
    let current = summarize(evaluation);
    let mut errors = Vec::new();
    let mut affected = BTreeSet::new();
    compare_metrics("overall", &current.overall, &baseline.overall, &mut errors);
    for (id, before) in &baseline.datasets {
        let Some(after) = current.datasets.get(id) else {
            errors.push(format!("{id}: missing evaluation dataset"));
            continue;
        };
        if before.sha256 != after.sha256 {
            errors.push(format!("{id}: evaluation data changed (SHA-256 differs); the frozen baseline requires unchanged inputs and labels"));
        }
        let previous_errors = errors.len();
        compare_metrics(id, &after.metrics, &before.metrics, &mut errors);
        if errors.len() > previous_errors {
            affected.insert(id);
        }
    }
    for id in current.datasets.keys() {
        if !baseline.datasets.contains_key(id) {
            errors.push(format!("{id}: dataset missing from frozen baseline"));
        }
    }
    if errors.is_empty() {
        return Ok(());
    }
    for id in affected {
        for (answer, result) in &evaluation[id].rows {
            if !answer.is_correct(&result.decision) {
                errors.push(format!(
                    "{id}: source {:?}; expected {}; actual {}",
                    answer.source(),
                    answer.display(),
                    decision_display(&result.decision)
                ));
            }
        }
    }
    Err(format!(
        "Quality regression check failed:\n{}",
        errors.join("\n")
    ))
}

fn ratio(correct: usize, total: usize) -> String {
    if total == 0 {
        "0/0 (n/a)".into()
    } else {
        format!(
            "{correct}/{total} ({:.2}%)",
            100.0 * correct as f64 / total as f64
        )
    }
}

fn print_summary(snapshot: &Baseline) {
    for (id, dataset) in &snapshot.datasets {
        println!(
            "{id}: {}",
            ratio(
                dataset.metrics.overall.correct,
                dataset.metrics.overall.total
            )
        );
    }
    let metrics = &snapshot.overall;
    println!(
        "Overall accuracy: {}",
        ratio(metrics.overall.correct, metrics.overall.total)
    );
    for kind in Category::ALL {
        let counts = &metrics.by_expected_decision[&kind];
        println!(
            "Expected {}: {}",
            kind.label(),
            ratio(counts.correct, counts.total)
        );
    }
    let matches = &metrics.by_expected_decision[&Category::Match];
    println!(
        "Automatic-match precision: {}",
        metrics.automatic_match_precision()
    );
    println!(
        "Incorrect automatic matches: {}",
        metrics.overall.incorrect_automatic_matches
    );
    println!("Missed matches: {}", matches.total - matches.correct);
    println!(
        "Incorrect ambiguities: {}",
        metrics.overall.incorrect_ambiguities
    );
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    let command = args.next().unwrap_or_else(|| "--check".into());
    if args.next().is_some() {
        return Err("usage: evaluate_all [--check | --print-baseline | DIRECTORY]".into());
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    match command.as_str() {
        "--check" | "--print-baseline" => {
            let evaluation = evaluate(root, CORPORA)?;
            let snapshot = summarize(&evaluation);
            if command == "--print-baseline" {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&snapshot).map_err(|error| error.to_string())?
                );
            } else {
                let path = root.join(BASELINE);
                let raw = fs::read_to_string(&path)
                    .map_err(|error| format!("{}: {error}", path.display()))?;
                let baseline: Baseline = serde_json::from_str(&raw)
                    .map_err(|error| format!("{}: {error}", path.display()))?;
                print_summary(&snapshot);
                check(&evaluation, &baseline)?;
                println!(
                    "Quality regression check passed (frozen baseline; existing errors remain visible in metrics)."
                );
            }
        }
        directory if !directory.starts_with('-') => {
            let evaluation = evaluate(Path::new("."), &[directory])?;
            print_summary(&summarize(&evaluation));
            for case in evaluation.values() {
                for (answer, result) in &case.rows {
                    if !answer.is_correct(&result.decision) {
                        print_failure(&case.name, answer, result);
                    }
                }
            }
            println!("Report only; use --check to enforce the frozen quality baseline.");
        }
        _ => return Err("usage: evaluate_all [--check | --print-baseline | DIRECTORY]".into()),
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::{Value, json};

    fn matched(target: &str) -> Decision {
        Decision::Match {
            target: target.into(),
            score: 0.9,
        }
    }

    fn no_match() -> Decision {
        Decision::NoMatch { best_score: None }
    }

    fn ambiguous(targets: &[&str]) -> Decision {
        Decision::Ambiguous {
            targets: targets.iter().map(|target| (*target).into()).collect(),
            best_score: 0.9,
        }
    }

    fn case_json() -> Value {
        json!({
            "name": "small labeled case",
            "source": {"fields": [
                {"name": "mapped", "data_type": "text"},
                {"name": "unmapped", "data_type": "text"},
                {"name": "uncertain", "data_type": "text"}
            ]},
            "target": {"fields": [
                {"name": "target", "data_type": "text"},
                {"name": "other", "data_type": "text"}
            ]},
            "answers": [
                {"kind": "match", "source": "mapped", "target": "target"},
                {"kind": "no_match", "source": "unmapped"},
                {"kind": "ambiguous", "source": "uncertain", "targets": ["target", "other"]}
            ]
        })
    }

    fn evaluation() -> Evaluation {
        let case = parse_case(&case_json().to_string()).unwrap();
        let rows = case
            .answers
            .into_iter()
            .zip([
                matched("target"),
                no_match(),
                ambiguous(&["other", "target"]),
            ])
            .map(|(answer, decision)| {
                let result = FieldResult {
                    source: answer.source().into(),
                    candidates: vec![],
                    decision,
                };
                (answer, result)
            })
            .collect();
        let evaluated = EvaluatedCase {
            name: case.name,
            sha256: "unchanged inputs".into(),
            rows,
        };
        BTreeMap::from([
            ("alpha.json".into(), evaluated.clone()),
            ("beta.json".into(), evaluated),
        ])
    }

    fn set_decision(evaluation: &mut Evaluation, dataset: &str, row: usize, decision: Decision) {
        evaluation.get_mut(dataset).unwrap().rows[row].1.decision = decision;
    }

    #[test]
    fn current_matcher_passes_frozen_quality_baseline() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let baseline =
            serde_json::from_str(&fs::read_to_string(root.join(BASELINE)).unwrap()).unwrap();
        check(&evaluate(root, CORPORA).unwrap(), &baseline).unwrap();
    }

    #[test]
    fn rejects_regression_with_dataset_field_and_metric_diagnostics() {
        let mut evaluation = evaluation();
        let baseline = summarize(&evaluation);
        check(&evaluation, &baseline).unwrap();
        set_decision(&mut evaluation, "alpha.json", 0, no_match());
        let error = check(&evaluation, &baseline).unwrap_err();
        for expected in [
            "alpha.json / expected match: accuracy regressed: 1/1 (100.00%) -> 0/1 (0.00%)",
            "alpha.json: source \"mapped\"; expected match -> target; actual no_match",
            "overall: accuracy regressed: 6/6 (100.00%) -> 5/6 (83.33%)",
        ] {
            assert!(error.contains(expected), "{error}");
        }
        set_decision(&mut evaluation, "alpha.json", 0, matched("target"));
        set_decision(
            &mut evaluation,
            "alpha.json",
            2,
            ambiguous(&["target", "wrong"]),
        );
        let error = check(&evaluation, &baseline).unwrap_err();
        assert!(
            error.contains("alpha.json / expected ambiguous: accuracy regressed"),
            "{error}"
        );
        assert!(error.contains("source \"uncertain\"; expected ambiguous -> [target, other]; actual ambiguous -> [target, wrong]"), "{error}");
    }

    #[test]
    fn permits_improvements_without_requiring_identical_predictions() {
        let mut evaluation = evaluation();
        set_decision(&mut evaluation, "alpha.json", 0, no_match());
        let baseline = summarize(&evaluation);
        set_decision(&mut evaluation, "alpha.json", 0, matched("target"));
        set_decision(
            &mut evaluation,
            "alpha.json",
            2,
            ambiguous(&["target", "other"]),
        );
        check(&evaluation, &baseline).unwrap();
    }

    #[test]
    fn detects_dataset_regressions_hidden_by_unchanged_overall_metrics() {
        let mut evaluation = evaluation();
        set_decision(&mut evaluation, "beta.json", 0, no_match());
        let baseline = summarize(&evaluation);
        set_decision(&mut evaluation, "beta.json", 0, matched("target"));
        set_decision(&mut evaluation, "alpha.json", 0, no_match());
        assert_eq!(summarize(&evaluation).overall, baseline.overall);
        let error = check(&evaluation, &baseline).unwrap_err();
        assert!(error.contains("alpha.json: accuracy regressed"), "{error}");
    }

    #[test]
    fn detects_category_regressions_hidden_by_dataset_and_global_totals() {
        let mut evaluation = evaluation();
        set_decision(&mut evaluation, "alpha.json", 1, matched("target"));
        set_decision(&mut evaluation, "beta.json", 0, matched("wrong"));
        let baseline = summarize(&evaluation);
        set_decision(&mut evaluation, "alpha.json", 0, matched("wrong"));
        set_decision(&mut evaluation, "alpha.json", 1, no_match());
        set_decision(&mut evaluation, "beta.json", 0, matched("target"));
        set_decision(&mut evaluation, "beta.json", 1, matched("target"));
        let current = summarize(&evaluation);
        assert_eq!(current.overall, baseline.overall);
        for (id, before) in &baseline.datasets {
            assert_eq!(current.datasets[id].metrics.overall, before.metrics.overall);
        }
        let error = check(&evaluation, &baseline).unwrap_err();
        assert!(
            error.contains("alpha.json / expected match: accuracy regressed"),
            "{error}"
        );
        assert!(
            error.contains("beta.json / expected no_match: accuracy regressed"),
            "{error}"
        );
    }

    #[test]
    fn rejects_more_unsafe_decisions_even_when_accuracy_is_unchanged() {
        let mut evaluation = evaluation();
        set_decision(&mut evaluation, "alpha.json", 0, no_match());
        let baseline = summarize(&evaluation);
        for (decision, metric) in [
            (matched("wrong"), "incorrect automatic matches"),
            (ambiguous(&["target", "other"]), "incorrect ambiguities"),
        ] {
            set_decision(&mut evaluation, "alpha.json", 0, decision);
            assert_eq!(
                summarize(&evaluation).overall.overall.correct,
                baseline.overall.overall.correct
            );
            let error = check(&evaluation, &baseline).unwrap_err();
            assert!(
                error.contains(&format!("{metric} increased: 0 -> 1")),
                "{error}"
            );
        }
    }

    #[test]
    fn metrics_count_exact_decisions_without_inflating_automatic_match_precision() {
        let case = parse_case(&case_json().to_string()).unwrap();
        let mut metrics = Metrics::default();
        assert_eq!(metrics.automatic_match_precision(), "0/0 (n/a)");
        for (answer, correct) in case.answers.iter().zip([
            matched("target"),
            no_match(),
            ambiguous(&["other", "target"]),
        ]) {
            metrics.observe(answer, &correct);
            metrics.observe(answer, &matched("wrong"));
            metrics.observe(answer, &ambiguous(&["target", "wrong"]));
        }
        metrics.observe(&case.answers[0], &no_match());
        metrics.observe(&case.answers[2], &no_match());
        assert_eq!(
            metrics.overall,
            Counts {
                total: 11,
                correct: 3,
                incorrect_automatic_matches: 3,
                incorrect_ambiguities: 3
            }
        );
        assert_eq!(metrics.by_expected_decision[&Category::Match].correct, 1);
        assert_eq!(metrics.by_expected_decision[&Category::Match].total, 4);
        assert_eq!(metrics.automatic_match_precision(), "1/4 (25.00%)");
        // True negatives do not improve the precision of automatic matches.
        for _ in 0..100 {
            metrics.observe(&case.answers[1], &no_match());
        }
        assert_eq!(metrics.automatic_match_precision(), "1/4 (25.00%)");
        // Choosing one of the expected ambiguous targets is still incorrect.
        assert!(!case.answers[2].is_correct(&matched("target")));
    }

    #[test]
    fn rejects_empty_malformed_and_incomplete_labels() {
        for raw in ["", "{", "{}", r#"{"name":"only a name"}"#] {
            assert!(parse_case(raw).is_err());
        }
        for (path, replacement, message) in [
            ("/name", json!(""), "must have a name"),
            ("/source/fields", json!([]), "source fields"),
            ("/answers", json!([]), "answers"),
            ("/source/fields/0/name", json!(""), "empty or duplicate"),
            (
                "/source/fields/1/name",
                json!("mapped"),
                "empty or duplicate",
            ),
            (
                "/target/fields/1/name",
                json!("target"),
                "empty or duplicate",
            ),
            (
                "/answers/0/source",
                json!("missing"),
                "unknown or duplicate answer",
            ),
            (
                "/answers/1/source",
                json!("mapped"),
                "unknown or duplicate answer",
            ),
            (
                "/answers/0/target",
                json!("missing"),
                "invalid answer targets",
            ),
            ("/answers/2/targets", json!([]), "invalid answer targets"),
            (
                "/answers/2/targets",
                json!(["target"]),
                "invalid answer targets",
            ),
            (
                "/answers/2/targets",
                json!(["target", "target"]),
                "invalid answer targets",
            ),
            (
                "/answers/2/targets",
                json!(["target", "missing"]),
                "invalid answer targets",
            ),
            ("/answers/0/kind", json!("maybe"), "unknown variant"),
            (
                "/source/fields/0/data_type",
                json!("typo"),
                "unknown variant",
            ),
        ] {
            let mut raw = case_json();
            *raw.pointer_mut(path).unwrap() = replacement;
            let error = parse_case(&raw.to_string()).unwrap_err();
            assert!(error.contains(message), "{path}: {error}");
        }
        let mut raw = case_json();
        raw["answers"].as_array_mut().unwrap().pop();
        assert!(
            parse_case(&raw.to_string())
                .unwrap_err()
                .contains("missing answers for source fields: [\"uncertain\"]")
        );
    }

    #[test]
    fn rejects_missing_or_changed_corpora_and_incomplete_baselines() {
        let original = evaluation();
        let baseline = summarize(&original);
        let mut current = original.clone();
        current.remove("alpha.json");
        assert!(
            check(&current, &baseline)
                .unwrap_err()
                .contains("alpha.json: missing evaluation dataset")
        );
        current = original.clone();
        current.insert("extra.json".into(), current["alpha.json"].clone());
        assert!(
            check(&current, &baseline)
                .unwrap_err()
                .contains("extra.json: dataset missing from frozen baseline")
        );
        current = original.clone();
        current.get_mut("alpha.json").unwrap().sha256 = "changed input".into();
        assert!(
            check(&current, &baseline)
                .unwrap_err()
                .contains("alpha.json: evaluation data changed")
        );
        assert!(check(&Evaluation::new(), &baseline).is_err());
        let mut incomplete = baseline.clone();
        incomplete
            .overall
            .by_expected_decision
            .remove(&Category::Match);
        assert!(
            check(&original, &incomplete)
                .unwrap_err()
                .contains("missing baseline category")
        );
        assert!(serde_json::from_str::<Baseline>("{}").is_err());
        let mut invalid = baseline;
        invalid.overall.overall.correct = usize::MAX;
        assert!(
            check(&original, &invalid)
                .unwrap_err()
                .contains("invalid baseline counts")
        );
    }

    #[test]
    fn loader_checks_directories_files_and_format_independent_fingerprints() {
        let root = env::temp_dir().join(format!(
            "fieldkin-quality-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        assert!(
            evaluate(&root, &["missing"])
                .err()
                .unwrap()
                .contains("missing")
        );
        assert!(
            evaluate(&root, &["."])
                .err()
                .unwrap()
                .contains("no JSON evaluation datasets")
        );
        let path = root.join("case.json");
        fs::write(&path, "{").unwrap();
        assert!(
            evaluate(&root, &["."])
                .err()
                .unwrap()
                .contains("./case.json:")
        );
        let raw = case_json();
        fs::write(&path, raw.to_string()).unwrap();
        let original = evaluate(&root, &["."]).unwrap();
        fs::write(
            &path,
            serde_json::to_string_pretty(&raw)
                .unwrap()
                .replace('\n', "\r\n"),
        )
        .unwrap();
        let reformatted = evaluate(&root, &["."]).unwrap();
        assert_eq!(
            original["./case.json"].sha256,
            reformatted["./case.json"].sha256
        );
        assert_eq!(
            summarize(&original).overall,
            summarize(&reformatted).overall
        );
        let mut changed = raw;
        changed["source"]["fields"][0]["samples"] = json!(["changed"]);
        fs::write(&path, changed.to_string()).unwrap();
        let error = check(&evaluate(&root, &["."]).unwrap(), &summarize(&original)).unwrap_err();
        assert!(error.contains("evaluation data changed"), "{error}");
        // An empty target is meaningful when every label is no_match.
        let mut empty_target = case_json();
        empty_target["source"]["fields"] = json!([empty_target["source"]["fields"][1]]);
        empty_target["answers"] = json!([empty_target["answers"][1]]);
        empty_target["target"]["fields"] = json!([]);
        fs::write(&path, empty_target.to_string()).unwrap();
        let counts = summarize(&evaluate(&root, &["."]).unwrap()).overall.overall;
        assert_eq!((counts.correct, counts.total), (1, 1));
        fs::remove_dir_all(root).unwrap();
    }
}
