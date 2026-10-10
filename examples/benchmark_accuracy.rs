//! Compare Valentine accuracy on frozen corpora; see validation/accuracy_protocol.md.
use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    path::Path,
    process::ExitCode,
};

use fieldkin::algorithms::{
    Coma, ComaConfig, Cupid, CupidConfig, DistributionBased, Formula, JaccardConfig,
    JaccardDistanceMatcher, Policy, SimilarityFlooding, StringDistanceFunction, StringMatcher,
};
use fieldkin::{ColumnPair, Matcher, Table, valentine_match};
use serde::{Deserialize, Serialize};

#[path = "support/independent.rs"]
mod independent;
#[path = "support/labeled.rs"]
mod labeled;
use labeled::{Answer, EvalCase, fingerprint, parse_case};

const CUTOFF: f64 = 0.5;
const CORPORA: &[&str] = &["eval", "eval_realworld"];
const FREEZE: &str = "validation/quality_baseline.json";

type NamedMatcher = (&'static str, Box<dyn Matcher>);

// Read only corpus identities, not the suggestion API's accuracy measurements.
#[derive(Deserialize)]
struct FrozenCorpus {
    datasets: BTreeMap<String, FrozenDataset>,
}

#[derive(Deserialize)]
struct FrozenDataset {
    sha256: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
struct Counts {
    sources: usize,
    matches: usize,
    no_matches: usize,
    ambiguous: usize,
    automatic: usize,
    true_positives: usize,
    top1_correct: usize,
    recall_at5_correct: usize,
    no_match_automatic: usize,
    ambiguous_automatic: usize,
    ambiguous_any_at5: usize,
    ambiguous_all_at5: usize,
}

impl Counts {
    fn observe(answer: &Answer, ranked: &[(&str, f64)]) -> Self {
        let automatic = ranked.first().filter(|(_, score)| *score >= CUTOFF);
        let mut counts = Self {
            sources: 1,
            automatic: usize::from(automatic.is_some()),
            ..Self::default()
        };
        match answer {
            Answer::Match { target, .. } => {
                counts.matches = 1;
                counts.true_positives =
                    usize::from(automatic.is_some_and(|(name, _)| *name == target));
                counts.top1_correct =
                    usize::from(ranked.first().is_some_and(|(name, _)| *name == target));
                counts.recall_at5_correct =
                    usize::from(ranked.iter().take(5).any(|(name, _)| *name == target));
            }
            Answer::NoMatch { .. } => {
                counts.no_matches = 1;
                counts.no_match_automatic = counts.automatic;
            }
            Answer::Ambiguous { targets, .. } => {
                counts.ambiguous = 1;
                counts.ambiguous_automatic = counts.automatic;
                counts.ambiguous_any_at5 = usize::from(
                    targets
                        .iter()
                        .any(|target| ranked.iter().take(5).any(|(name, _)| *name == target)),
                );
                counts.ambiguous_all_at5 = usize::from(
                    targets
                        .iter()
                        .all(|target| ranked.iter().take(5).any(|(name, _)| *name == target)),
                );
            }
        }
        counts
    }

    fn add(&mut self, other: &Self) {
        self.sources += other.sources;
        self.matches += other.matches;
        self.no_matches += other.no_matches;
        self.ambiguous += other.ambiguous;
        self.automatic += other.automatic;
        self.true_positives += other.true_positives;
        self.top1_correct += other.top1_correct;
        self.recall_at5_correct += other.recall_at5_correct;
        self.no_match_automatic += other.no_match_automatic;
        self.ambiguous_automatic += other.ambiguous_automatic;
        self.ambiguous_any_at5 += other.ambiguous_any_at5;
        self.ambiguous_all_at5 += other.ambiguous_all_at5;
    }

    fn scores(&self) -> Scores {
        Scores {
            precision: ratio(self.true_positives, self.automatic),
            recall: ratio(self.true_positives, self.matches),
            f1: ratio(2 * self.true_positives, self.automatic + self.matches),
            top1_accuracy: ratio(self.top1_correct, self.matches),
            recall_at5: ratio(self.recall_at5_correct, self.matches),
            coverage: ratio(self.automatic, self.sources),
        }
    }
}

#[derive(Debug, Serialize)]
struct Scores {
    precision: Option<f64>,
    recall: Option<f64>,
    f1: Option<f64>,
    top1_accuracy: Option<f64>,
    recall_at5: Option<f64>,
    coverage: Option<f64>,
}

fn ratio(numerator: usize, denominator: usize) -> Option<f64> {
    (denominator != 0).then(|| numerator as f64 / denominator as f64)
}

#[derive(Default, Serialize)]
struct DatasetResult {
    counts: Counts,
    match_conditions: BTreeMap<String, Counts>,
    diagnostics: Vec<String>,
}

#[derive(Default)]
struct AlgorithmResult {
    overall: Counts,
    datasets: BTreeMap<String, DatasetResult>,
    match_conditions: BTreeMap<String, Counts>,
}

fn load_cases(root: &Path, independent: bool) -> Result<BTreeMap<String, EvalCase>, String> {
    let (corpora, freeze) = if independent {
        (
            &["eval_independent"][..],
            "validation/independent_corpus.json",
        )
    } else {
        (CORPORA, FREEZE)
    };
    let raw =
        fs::read_to_string(root.join(freeze)).map_err(|error| format!("{freeze}: {error}"))?;
    let frozen: FrozenCorpus =
        serde_json::from_str(&raw).map_err(|error| format!("{freeze}: {error}"))?;
    let mut cases = BTreeMap::new();
    let mut names = BTreeSet::new();
    for directory in corpora {
        let mut paths = fs::read_dir(root.join(directory))
            .and_then(|entries| {
                entries
                    .map(|entry| entry.map(|entry| entry.path()))
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|error| format!("{directory}: {error}"))?;
        paths.sort();
        for path in paths {
            if path.extension().is_none_or(|extension| extension != "json") {
                continue;
            }
            let id = format!(
                "{directory}/{}",
                path.file_name().unwrap().to_string_lossy()
            );
            let raw = fs::read_to_string(&path).map_err(|error| format!("{id}: {error}"))?;
            let case = parse_case(&raw).map_err(|error| format!("{id}: {error}"))?;
            if !names.insert(case.name.clone()) {
                return Err(format!("{id}: duplicate dataset name {:?}", case.name));
            }
            if cases.insert(id.clone(), case).is_some() {
                return Err(format!("duplicate dataset identifier: {id}"));
            }
        }
    }
    verify_freeze(&cases, &frozen)?;
    if independent {
        independent::verify(root, &raw, &cases).map_err(|error| format!("{freeze}: {error}"))?;
    }
    Ok(cases)
}

fn verify_freeze(cases: &BTreeMap<String, EvalCase>, frozen: &FrozenCorpus) -> Result<(), String> {
    if cases.is_empty() || frozen.datasets.is_empty() {
        return Err("empty evaluation corpus or frozen dataset list".into());
    }
    for id in frozen.datasets.keys() {
        if !cases.contains_key(id) {
            return Err(format!("{id}: missing frozen evaluation dataset"));
        }
    }
    for (id, case) in cases {
        let expected = frozen
            .datasets
            .get(id)
            .ok_or_else(|| format!("{id}: dataset absent from frozen corpus"))?;
        if fingerprint(case)? != expected.sha256 {
            return Err(format!("{id}: evaluation inputs or labels changed"));
        }
    }
    Ok(())
}

fn assess<'a>(
    case: &EvalCase,
    entries: impl IntoIterator<Item = (&'a ColumnPair, &'a f64)>,
) -> Result<DatasetResult, String> {
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
    let mut seen = BTreeSet::new();
    let mut rankings: BTreeMap<&str, Vec<(&str, f64)>> = BTreeMap::new();
    for (pair, score) in entries {
        if pair.source_table != "source"
            || pair.target_table != "target"
            || !sources.contains(pair.source_column.as_str())
            || !targets.contains(pair.target_column.as_str())
        {
            return Err(format!("unexpected prediction identifier: {pair:?}"));
        }
        if !seen.insert(pair) {
            return Err(format!("duplicate prediction: {pair:?}"));
        }
        if !score.is_finite() || !(0.0..=1.0).contains(score) {
            return Err(format!("invalid prediction score for {pair:?}: {score}"));
        }
        if *score > 0.0 {
            rankings
                .entry(&pair.source_column)
                .or_default()
                .push((&pair.target_column, *score));
        }
    }
    for ranked in rankings.values_mut() {
        ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    }
    let mut result = DatasetResult::default();
    for answer in &case.answers {
        let ranked = rankings
            .get(answer.source())
            .map(Vec::as_slice)
            .unwrap_or_default();
        let counts = Counts::observe(answer, ranked);
        result.counts.add(&counts);
        if let Answer::Match { source, target } = answer {
            let source = case
                .source
                .fields
                .iter()
                .find(|field| &field.name == source)
                .unwrap();
            let target = case
                .target
                .fields
                .iter()
                .find(|field| &field.name == target)
                .unwrap();
            let name_group = if source.name == target.name {
                "unchanged_name"
            } else {
                "renamed"
            };
            let type_group = if source.data_type == target.data_type {
                "same_declared_type"
            } else {
                "different_declared_type"
            };
            for condition in [name_group, type_group] {
                result
                    .match_conditions
                    .entry(condition.into())
                    .or_default()
                    .add(&counts);
            }
        }
        if counts.true_positives < counts.matches
            || counts.automatic > counts.true_positives
            || counts.ambiguous != 0
        {
            let candidates = ranked
                .iter()
                .take(5)
                .map(|(name, score)| format!("{name} ({score:.6})"))
                .collect::<Vec<_>>();
            let automatic = ranked
                .first()
                .filter(|(_, score)| *score >= CUTOFF)
                .map(|(name, score)| format!("{name} ({score:.6})"))
                .unwrap_or_else(|| "abstain".into());
            result.diagnostics.push(format!(
                "source {:?}; expected {}; automatic {}; top 5 [{}]",
                answer.source(),
                answer.display(),
                automatic,
                candidates.join(", ")
            ));
        }
    }
    Ok(result)
}

// Explicit values pin the protocol even if library defaults change later.
fn fixed_matchers() -> Result<Vec<NamedMatcher>, fieldkin::Error> {
    Ok(vec![
        (
            "COMA",
            Box::new(Coma::new(ComaConfig {
                max_n: 0,
                use_instances: false,
                use_schema: true,
                delta: 0.15,
                threshold: 0.0,
                instance_weight: 1.0,
            })?),
        ),
        (
            "Cupid",
            Box::new(Cupid::new(CupidConfig {
                leaf_w_struct: 0.2,
                w_struct: 0.2,
                th_accept: 0.7,
                th_high: 0.6,
                th_low: 0.35,
                c_inc: 1.2,
                c_dec: 0.9,
                th_ns: 0.7,
                process_num: 1,
            })?),
        ),
        (
            "DistributionBased",
            Box::new(DistributionBased {
                threshold1: 0.15,
                threshold2: 0.15,
                quantiles: 256,
                process_num: 1,
                use_bloom_filters: false,
            }),
        ),
        (
            "JaccardDistanceMatcher",
            Box::new(JaccardDistanceMatcher::new(JaccardConfig {
                threshold_dist: 0.8,
                distance_fun: StringDistanceFunction::Levenshtein,
                process_num: 1,
                tversky_alpha: 1.0,
                tversky_beta: 1.0,
            })?),
        ),
        (
            "SimilarityFlooding",
            Box::new(SimilarityFlooding {
                coeff_policy: Policy::InverseAverage,
                formula: Formula::FormulaC,
                string_matcher: StringMatcher::PrefixSuffix,
                tfidf_corpus: vec![],
                max_iterations: 100,
                residual_threshold: 1e-4,
            }),
        ),
    ])
}

fn evaluate(
    cases: &BTreeMap<String, EvalCase>,
) -> Result<BTreeMap<String, AlgorithmResult>, String> {
    let mut results = BTreeMap::new();
    for (name, matcher) in fixed_matchers().map_err(|error| error.to_string())? {
        let mut algorithm = AlgorithmResult::default();
        for (id, case) in cases {
            let tables = [
                Table::from_schema("source", &case.source),
                Table::from_schema("target", &case.target),
            ];
            let matches = valentine_match(&tables, matcher.as_ref())
                .map_err(|error| format!("{name} / {id}: {error}"))?;
            let result =
                assess(case, matches.iter()).map_err(|error| format!("{name} / {id}: {error}"))?;
            algorithm.overall.add(&result.counts);
            for (condition, counts) in &result.match_conditions {
                algorithm
                    .match_conditions
                    .entry(condition.clone())
                    .or_default()
                    .add(counts);
            }
            algorithm.datasets.insert(id.clone(), result);
        }
        results.insert(name.into(), algorithm);
    }
    Ok(results)
}

fn percent(value: Option<f64>) -> String {
    value
        .map(|value| format!("{:.2}%", 100.0 * value))
        .unwrap_or_else(|| "n/a".into())
}

fn print_summary(results: &BTreeMap<String, AlgorithmResult>, details: bool, independent: bool) {
    if independent {
        println!(
            "Independent-corpus accuracy; publisher evidence and inputs frozen before predictions."
        );
    } else {
        println!("Development-corpus accuracy; see --independent for held-out measurements.");
    }
    println!(
        "Fixed defaults; positive candidates ranked per source; automatic cutoff >= {CUTOFF:.2}."
    );
    println!(
        "Ambiguous automatic selections count as FP. Top-1 and recall@5 use only match labels, before cutoff.\n"
    );
    println!("| Algorithm | Precision | Recall | F1 | Top-1 | Recall@5 | Coverage |");
    println!("| --- | ---: | ---: | ---: | ---: | ---: | ---: |");
    for (name, algorithm) in results {
        let scores = algorithm.overall.scores();
        println!(
            "| {name} | {} | {} | {} | {} | {} | {} |",
            percent(scores.precision),
            percent(scores.recall),
            percent(scores.f1),
            percent(scores.top1_accuracy),
            percent(scores.recall_at5),
            percent(scores.coverage)
        );
    }
    println!(
        "\n| Algorithm | TP / automatic | No-match automatic / labels | Ambiguous automatic / labels | Ambiguous any / all alternatives @5 |"
    );
    println!("| --- | ---: | ---: | ---: | ---: |");
    for (name, algorithm) in results {
        let c = &algorithm.overall;
        println!(
            "| {name} | {} / {} | {} / {} | {} / {} | {} / {} (of {}) |",
            c.true_positives,
            c.automatic,
            c.no_match_automatic,
            c.no_matches,
            c.ambiguous_automatic,
            c.ambiguous,
            c.ambiguous_any_at5,
            c.ambiguous_all_at5,
            c.ambiguous
        );
    }
    println!(
        "\nPer-dataset top-1 correct / match labels (before cutoff; full metrics in --json):\n"
    );
    print!("| Dataset |");
    for name in results.keys() {
        print!(" {name} |");
    }
    println!("\n| --- |{}", " ---: |".repeat(results.len()));
    if let Some(first) = results.values().next() {
        for id in first.datasets.keys() {
            print!("| {id} |");
            for algorithm in results.values() {
                let c = &algorithm.datasets[id].counts;
                print!(" {} / {} |", c.top1_correct, c.matches);
            }
            println!();
        }
    }
    println!("\nMatch-only conditions; groups overlap across name/type dimensions:\n");
    println!("| Algorithm | Condition | Labels | Top-1 | Recall@5 | Automatic recall |");
    println!("| --- | --- | ---: | ---: | ---: | ---: |");
    for (name, algorithm) in results {
        for (condition, c) in &algorithm.match_conditions {
            let scores = c.scores();
            println!(
                "| {name} | {condition} | {} | {} | {} | {} |",
                c.matches,
                percent(scores.top1_accuracy),
                percent(scores.recall_at5),
                percent(scores.recall)
            );
        }
    }
    if details {
        println!(
            "\nField diagnostics (all ambiguity rows and incorrect/missed automatic selections):\n"
        );
        for (name, algorithm) in results {
            for (id, dataset) in &algorithm.datasets {
                for diagnostic in &dataset.diagnostics {
                    println!("{name} / {id}: {diagnostic}");
                }
            }
        }
    }
}

fn options(args: &[String]) -> Result<(bool, &'static str), String> {
    let mut independent = false;
    let mut mode = "summary";
    for flag in args {
        match flag.as_str() {
            "--independent" if !independent => independent = true,
            "--details" if mode == "summary" => mode = "details",
            "--json" if mode == "summary" => mode = "json",
            _ => {
                return Err(
                    "usage: benchmark_accuracy [--independent] [--details | --json]".into(),
                );
            }
        }
    }
    Ok((independent, mode))
}

fn run() -> Result<(), String> {
    let args: Vec<_> = env::args().skip(1).collect();
    let (independent, mode) = options(&args)?;
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let cases = load_cases(root, independent)?; // Validate the complete frozen corpus before any predictions.
    let results = evaluate(&cases)?;
    if mode == "json" {
        let mut datasets = BTreeMap::new();
        for (id, case) in &cases {
            datasets.insert(
                id,
                serde_json::json!({"name": case.name, "sha256": fingerprint(case)?}),
            );
        }
        let algorithms: BTreeMap<_, _> = results.iter().map(|(name, result)| {
            let datasets: BTreeMap<_, _> = result.datasets.iter().map(|(id, dataset)| {
                (id, serde_json::json!({"counts": dataset.counts, "scores": dataset.counts.scores(),
                    "match_conditions": dataset.match_conditions, "diagnostics": dataset.diagnostics}))
            }).collect();
            (name, serde_json::json!({"counts": result.overall, "scores": result.overall.scores(),
                "match_conditions": result.match_conditions, "datasets": datasets}))
        }).collect();
        let report = serde_json::json!({"protocol": if independent { "validation/independent_accuracy.md" } else { "validation/accuracy_protocol.md" },
            "corpus_status": if independent { "independent; publisher evidence frozen before predictions" } else { "development" },
            "automatic_cutoff": CUTOFF, "datasets": datasets, "algorithms": algorithms});
        println!(
            "{}",
            serde_json::to_string_pretty(&report).map_err(|error| error.to_string())?
        );
    } else {
        print_summary(&results, mode == "details", independent);
    }
    Ok(())
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("Accuracy benchmark failed: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fieldkin::MatcherResults;
    use fieldkin::metrics::{F1Score, Metric, OneToOneMethod, Precision, Recall};
    use serde_json::{Value, json};

    #[test]
    fn independent_options_are_opt_in_and_keep_output_modes() {
        assert_eq!(options(&[]).unwrap(), (false, "summary"));
        for args in [
            vec!["--independent", "--json"],
            vec!["--json", "--independent"],
        ] {
            assert_eq!(
                options(&args.into_iter().map(String::from).collect::<Vec<_>>()).unwrap(),
                (true, "json")
            );
        }
        for args in [
            vec!["--independent", "--independent"],
            vec!["--details", "--json"],
            vec!["--unknown"],
        ] {
            assert!(options(&args.into_iter().map(String::from).collect::<Vec<_>>()).is_err());
        }
    }

    #[test]
    fn independent_corpus_validates_without_running_predictions() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let cases = load_cases(root, true).unwrap();
        let mut counts = Counts::default();
        for case in cases.values() {
            for answer in &case.answers {
                counts.add(&Counts::observe(answer, &[]));
            }
        }
        assert_eq!(cases.len(), 15);
        assert_eq!(
            (
                counts.sources,
                counts.matches,
                counts.no_matches,
                counts.ambiguous
            ),
            (91, 40, 51, 0)
        );
        assert_eq!(counts.automatic, 0);
        assert_eq!(counts.scores().recall, Some(0.0));
        assert!(counts.scores().precision.is_none());
    }

    #[test]
    fn independent_annotation_evidence_must_be_complete_and_consistent() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let cases = load_cases(root, true).unwrap();
        let raw = fs::read_to_string(root.join("validation/independent_corpus.json")).unwrap();
        let manifest: Value = serde_json::from_str(&raw).unwrap();
        let id = "eval_independent/01_penguins.json";
        let source = "Species";
        for (key, value) in [
            ("target", json!("island")),
            ("decision", json!("no_match")),
            ("explanation", json!(" ")),
            ("references", json!([])),
            ("references", json!(["unknown source document"])),
        ] {
            let mut changed = manifest.clone();
            changed["datasets"][id]["evidence"][source][key] = value;
            let error = independent::verify(root, &changed.to_string(), &cases).unwrap_err();
            assert!(error.contains(id) && error.contains(source), "{error}");
        }
        let mut changed = manifest.clone();
        changed["datasets"][id]["evidence"]
            .as_object_mut()
            .unwrap()
            .remove(source);
        assert!(
            independent::verify(root, &changed.to_string(), &cases)
                .unwrap_err()
                .contains("incomplete label evidence")
        );
        let mut changed = manifest;
        changed["datasets"][id]["source_view"] = json!("unknown view");
        assert!(
            independent::verify(root, &changed.to_string(), &cases)
                .unwrap_err()
                .contains("unknown publisher view")
        );
    }

    fn raw_case() -> Value {
        json!({
            "name": "metric fixture",
            "source": {"fields": [
                {"name":"correct", "data_type":"text"},
                {"name":"wrong", "data_type":"integer"},
                {"name":"missing", "data_type":"text"},
                {"name":"no_match", "data_type":"text"},
                {"name":"ambiguous", "data_type":"text"}
            ]},
            "target": {"fields": [
                {"name":"x", "data_type":"text"},
                {"name":"y", "data_type":"text"},
                {"name":"z", "data_type":"text"},
                {"name":"w", "data_type":"text"},
                {"name":"u", "data_type":"text"},
                {"name":"v", "data_type":"text"}
            ]},
            "answers": [
                {"kind":"match", "source":"correct", "target":"x"},
                {"kind":"match", "source":"wrong", "target":"y"},
                {"kind":"match", "source":"missing", "target":"z"},
                {"kind":"no_match", "source":"no_match"},
                {"kind":"ambiguous", "source":"ambiguous", "targets":["x","y"]}
            ]
        })
    }

    fn case() -> EvalCase {
        parse_case(&raw_case().to_string()).unwrap()
    }

    fn predictions(values: &[(&str, &str, f64)]) -> Vec<(ColumnPair, f64)> {
        values
            .iter()
            .map(|(source, target, score)| {
                (
                    ColumnPair::new("source", *source, "target", *target),
                    *score,
                )
            })
            .collect()
    }

    fn assessed(values: &[(ColumnPair, f64)]) -> DatasetResult {
        assess(&case(), values.iter().map(|(pair, score)| (pair, score))).unwrap()
    }

    #[test]
    fn metrics_distinguish_ranking_abstention_false_matches_and_ambiguity() {
        let entries = predictions(&[
            ("correct", "x", 0.9),
            ("correct", "y", 0.6),
            ("wrong", "x", 0.8),
            ("wrong", "y", 0.7),
            ("missing", "z", 0.4),
            ("no_match", "x", 0.7),
            ("ambiguous", "y", 0.9),
            ("ambiguous", "x", 0.8),
        ]);
        let result = assessed(&entries);
        assert_eq!(
            result.counts,
            Counts {
                sources: 5,
                matches: 3,
                no_matches: 1,
                ambiguous: 1,
                automatic: 4,
                true_positives: 1,
                top1_correct: 2,
                recall_at5_correct: 3,
                no_match_automatic: 1,
                ambiguous_automatic: 1,
                ambiguous_any_at5: 1,
                ambiguous_all_at5: 1,
            }
        );
        let scores = result.counts.scores();
        assert_eq!(scores.precision, Some(0.25));
        assert_eq!(scores.recall, Some(1.0 / 3.0));
        assert_eq!(scores.f1, Some(2.0 / 7.0));
        assert_eq!(scores.top1_accuracy, Some(2.0 / 3.0));
        assert_eq!(scores.recall_at5, Some(1.0));
        assert_eq!(scores.coverage, Some(0.8));
        assert_eq!(result.match_conditions["renamed"].matches, 3);
        assert_eq!(
            result.match_conditions["different_declared_type"].matches,
            1
        );
        assert_eq!(result.match_conditions["same_declared_type"].matches, 2);
        // Check the pair metrics against the existing library without assignment selection.
        let selected = MatcherResults::new(entries)
            .unwrap()
            .take_top_n_per_source(1)
            .filter(CUTOFF)
            .unwrap();
        let truth = fieldkin::metrics::GroundTruth::Names(vec![
            ("correct".into(), "x".into()),
            ("wrong".into(), "y".into()),
            ("missing".into(), "z".into()),
        ]);
        for (metric, expected) in [
            (
                &Precision { one_to_one: false } as &dyn Metric,
                scores.precision,
            ),
            (&Recall { one_to_one: false }, scores.recall),
            (&F1Score { one_to_one: false }, scores.f1),
        ] {
            assert_eq!(
                metric
                    .apply(&selected, &truth, OneToOneMethod::Hungarian)
                    .unwrap(),
                expected.unwrap()
            );
        }
        let diagnostics = result.diagnostics.join("\n");
        assert!(
            diagnostics.contains("source \"wrong\"; expected match -> y; automatic x (0.800000)")
        );
        assert!(diagnostics.contains("source \"missing\"; expected match -> z; automatic abstain"));
        assert!(diagnostics.contains("expected ambiguous -> [x, y]"));
    }

    #[test]
    fn empty_predictions_and_no_match_abstentions_do_not_inflate_precision() {
        let counts = assessed(&[]).counts;
        let scores = counts.scores();
        assert_eq!(scores.precision, None);
        assert_eq!(scores.recall, Some(0.0));
        assert_eq!(scores.f1, Some(0.0));
        assert_eq!(scores.top1_accuracy, Some(0.0));
        assert_eq!(scores.recall_at5, Some(0.0));
        assert_eq!(scores.coverage, Some(0.0));
        let mut counts =
            assessed(&predictions(&[("correct", "x", 1.0), ("wrong", "x", 1.0)])).counts;
        let precision = counts.scores().precision;
        for _ in 0..100 {
            counts.add(&Counts::observe(
                &Answer::NoMatch {
                    source: "unmatched".into(),
                },
                &[],
            ));
        }
        assert_eq!(counts.scores().precision, precision);
        let empty = Counts::default().scores();
        assert!(empty.precision.is_none() && empty.recall.is_none() && empty.f1.is_none());
        assert!(
            empty.top1_accuracy.is_none() && empty.recall_at5.is_none() && empty.coverage.is_none()
        );
    }

    #[test]
    fn ties_cutoff_zero_scores_and_rank_five_have_fixed_semantics() {
        let mut entries = predictions(&[("correct", "y", CUTOFF), ("correct", "x", CUTOFF)]);
        let forward = assessed(&entries);
        entries.reverse();
        let reverse = assessed(&entries);
        assert_eq!(forward.counts, reverse.counts);
        assert_eq!(forward.diagnostics, reverse.diagnostics);
        assert_eq!(forward.counts.true_positives, 1);
        assert_eq!(
            assessed(&predictions(&[("correct", "x", 0.0)]))
                .counts
                .top1_correct,
            0
        );
        for (gold_score, expected) in [(0.55, 1), (0.4, 0)] {
            let entries = predictions(&[
                ("correct", "y", 0.9),
                ("correct", "z", 0.8),
                ("correct", "w", 0.7),
                ("correct", "u", 0.6),
                ("correct", "v", 0.5),
                ("correct", "x", gold_score),
            ]);
            assert_eq!(assessed(&entries).counts.recall_at5_correct, expected);
        }
        let row = Counts::observe(
            &Answer::Ambiguous {
                source: "a".into(),
                targets: vec!["x".into(), "y".into()],
            },
            &[("x", 0.9)],
        );
        assert_eq!(
            (
                row.true_positives,
                row.ambiguous_automatic,
                row.ambiguous_any_at5,
                row.ambiguous_all_at5
            ),
            (0, 1, 1, 0)
        );
    }

    #[test]
    fn rejects_duplicate_unknown_and_invalid_predictions() {
        let one = predictions(&[("correct", "x", 1.0)]);
        let duplicate = vec![one[0].clone(), one[0].clone()];
        assert!(
            assess(&case(), duplicate.iter().map(|(p, s)| (p, s)))
                .err()
                .unwrap()
                .contains("duplicate prediction")
        );
        for (source, target) in [("unknown", "x"), ("correct", "unknown")] {
            let entries = predictions(&[(source, target, 1.0)]);
            assert!(
                assess(&case(), entries.iter().map(|(p, s)| (p, s)))
                    .err()
                    .unwrap()
                    .contains("unexpected prediction identifier")
            );
        }
        for score in [f64::NAN, f64::INFINITY, -0.1, 1.1] {
            let entries = predictions(&[("correct", "x", score)]);
            assert!(
                assess(&case(), entries.iter().map(|(p, s)| (p, s)))
                    .err()
                    .unwrap()
                    .contains("invalid prediction score")
            );
        }
        let pair = ColumnPair::new("other_table", "correct", "target", "x");
        assert!(assess(&case(), [(&pair, &1.0)]).is_err());
    }

    #[test]
    fn rejects_invalid_duplicate_and_missing_labels() {
        for raw in ["", "{", "{}"] {
            assert!(parse_case(raw).is_err());
        }
        for (pointer, value, expected) in [
            ("/answers/0/kind", json!("maybe"), "unknown variant"),
            (
                "/answers/0/target",
                json!("absent"),
                "invalid answer targets",
            ),
            ("/answers/4/targets", json!(["x"]), "invalid answer targets"),
            (
                "/answers/4/targets",
                json!(["x", "x"]),
                "invalid answer targets",
            ),
            (
                "/answers/4/targets",
                json!(["x", "absent"]),
                "invalid answer targets",
            ),
            (
                "/answers/1/source",
                json!("correct"),
                "duplicate answer source",
            ),
            (
                "/source/fields/1/name",
                json!("correct"),
                "duplicate field name",
            ),
            ("/target/fields/1/name", json!("x"), "duplicate field name"),
            ("/answers", json!([]), "answers"),
        ] {
            let mut raw = raw_case();
            *raw.pointer_mut(pointer).unwrap() = value;
            assert!(parse_case(&raw.to_string()).unwrap_err().contains(expected));
        }
        let mut raw = raw_case();
        raw["answers"].as_array_mut().unwrap().pop();
        assert!(
            parse_case(&raw.to_string())
                .unwrap_err()
                .contains("missing answers for source fields")
        );
        // No-match-only evaluation with no targets is meaningful.
        let mut raw = raw_case();
        raw["source"]["fields"] = json!([raw["source"]["fields"][3]]);
        raw["answers"] = json!([raw["answers"][3]]);
        raw["target"]["fields"] = json!([]);
        let result = assess(&parse_case(&raw.to_string()).unwrap(), []).unwrap();
        assert_eq!((result.counts.sources, result.counts.no_matches), (1, 1));
        assert!(result.counts.scores().top1_accuracy.is_none());
    }

    #[test]
    fn frozen_corpus_rejects_changed_missing_extra_and_empty_data() {
        let mut cases = BTreeMap::from([("case.json".into(), case())]);
        let frozen = FrozenCorpus {
            datasets: BTreeMap::from([(
                "case.json".into(),
                FrozenDataset {
                    sha256: fingerprint(&cases["case.json"]).unwrap(),
                },
            )]),
        };
        verify_freeze(&cases, &frozen).unwrap();
        assert!(
            verify_freeze(&BTreeMap::new(), &frozen)
                .unwrap_err()
                .contains("empty evaluation")
        );
        cases.insert("extra.json".into(), case());
        assert!(
            verify_freeze(&cases, &frozen)
                .unwrap_err()
                .contains("absent from frozen corpus")
        );
        cases.remove("extra.json");
        cases.get_mut("case.json").unwrap().source.fields[0]
            .samples
            .push("changed".into());
        assert!(
            verify_freeze(&cases, &frozen)
                .unwrap_err()
                .contains("inputs or labels changed")
        );
        cases.insert("case.json".into(), case());
        if let Answer::Match { target, .. } = &mut cases.get_mut("case.json").unwrap().answers[0] {
            *target = "y".into(); // A valid changed label must also break the freeze.
        }
        assert!(verify_freeze(&cases, &frozen).is_err());
        cases.remove("case.json");
        cases.insert("extra.json".into(), case());
        assert!(
            verify_freeze(&cases, &frozen)
                .unwrap_err()
                .contains("missing frozen evaluation dataset")
        );
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        assert_eq!(load_cases(root, false).unwrap().len(), 15);
    }

    #[test]
    fn loader_rejects_duplicate_dataset_names_and_malformed_files() {
        let root = env::temp_dir().join(format!(
            "fieldkin-accuracy-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        for directory in ["validation", "eval", "eval_realworld"] {
            fs::create_dir_all(root.join(directory)).unwrap();
        }
        fs::write(root.join(FREEZE), r#"{"datasets":{}}"#).unwrap();
        assert!(
            load_cases(&root, false)
                .unwrap_err()
                .contains("empty evaluation corpus")
        );
        let path = root.join("eval/a.json");
        fs::write(&path, "{").unwrap();
        assert!(
            load_cases(&root, false)
                .unwrap_err()
                .starts_with("eval/a.json:")
        );
        let raw = raw_case().to_string();
        fs::write(&path, &raw).unwrap();
        fs::write(root.join("eval/b.json"), &raw).unwrap();
        assert!(
            load_cases(&root, false)
                .unwrap_err()
                .contains("duplicate dataset name")
        );
        fs::write(root.join(FREEZE), "{").unwrap();
        assert!(load_cases(&root, false).unwrap_err().starts_with(FREEZE));
        fs::remove_dir_all(root).unwrap();
    }
}
