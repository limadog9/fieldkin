//! Deterministic generated-case qualification through the public library API.

use std::collections::BTreeSet;
use std::time::Instant;

use fieldkin::{
    normalize_name, AssignmentDiagnosticStatus, Config, Corroboration, DataType, Evidence,
    ExactDecimal, Field, GlobalDiagnosticsConfig, MatchEngine, MatchReport, Matcher, SampleValue,
    Schema, SemanticHints, WeightedMatcher,
};

struct Random(u64);
impl Random {
    fn next(&mut self) -> u64 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        self.0
    }

    fn index(&mut self, bound: usize) -> usize {
        ((self.next() >> 32) as usize) % bound
    }
}

fn ensure(condition: bool, reason: &str) -> Result<(), String> {
    if condition {
        Ok(())
    } else {
        Err(reason.into())
    }
}

fn normalization(random: &mut Random) -> Result<(), String> {
    if random.index(2) == 0 {
        let number = random.next().to_string();
        let input = format!("__SKU.Field{number}--");
        ensure(
            normalize_name(&input) == ["sku", "field", number.as_str()],
            "ASCII separator/case/digit oracle",
        )
    } else {
        let fragments = [
            "SKU_",
            "\u{0301}",
            "K",
            "中",
            "\u{200d}",
            "ＤＡＴＥ",
            "İ",
            "ß",
            "Σ",
            "\0",
            "---",
            "__",
            "1",
            "é",
            "👩",
            "e\u{0301}",
        ];
        let length = random.index(17);
        let mut input = String::new();
        for _ in 0..length {
            input.push_str(fragments[random.index(fragments.len())]);
        }
        let tokens = normalize_name(&input);
        ensure(
            tokens == normalize_name(&input),
            "Unicode normalization determinism",
        )?;
        ensure(
            tokens.iter().all(|token| !token.is_empty()),
            "normalization emits empty token",
        )?;
        ensure(
            tokens.iter().map(String::len).sum::<usize>() <= input.len().saturating_mul(4),
            "bounded normalization expansion",
        )
    }
}

fn basic_schema() -> Schema {
    Schema::new(vec![Field::new("0", "value", DataType::Text)])
}

fn malformed(random: &mut Random) -> Result<(), String> {
    let mut source = basic_schema();
    let field = &mut source.fields[0];
    match random.index(12) {
        0 => source.fields.push(source.fields[0].clone()),
        1 => field.id.0.clear(),
        2 => field.name = "n".repeat(257 + random.index(16)),
        3 => field.id.0 = "i".repeat(257 + random.index(16)),
        4 => field.samples = Some(vec![SampleValue::Null; 257 + random.index(16)]),
        5 => field.samples = Some(vec![SampleValue::Text("x".repeat(1025 + random.index(16)))]),
        6 => field.samples = Some(vec![SampleValue::Number(f64::NAN)]),
        7 => field.samples = Some(vec![SampleValue::Number(f64::INFINITY)]),
        8 => field.samples = Some(vec![SampleValue::Number(f64::NEG_INFINITY)]),
        9 => field.hints.unit = Some(String::new()),
        10 => field.hints.currency = Some(" USD ".into()),
        _ => field.hints.identifier_scope = Some("s".repeat(129 + random.index(16))),
    }
    let engine = MatchEngine::new(Config::default()).map_err(|error| error.to_string())?;
    ensure(
        engine.match_schemas(&source, &basic_schema()).is_err(),
        "malformed schema accepted",
    )
}

fn config_and_limits(random: &mut Random) -> Result<(), String> {
    let choice = random.index(23);
    let mut config = Config::default();
    let mut source = basic_schema();
    let target = basic_schema();
    match choice {
        0 => config.min_score = f64::NAN,
        1 => config.min_score = -0.01,
        2 => config.min_score = 1.01,
        3 => config.ambiguity_margin = f64::INFINITY,
        4 => config.ambiguity_margin = -0.01,
        5 => config.max_candidates = 0,
        6 => config.limits.max_fields = 0,
        7 => config.limits.max_pairs = 0,
        8 => config.limits.max_signal_evaluations = 2,
        9 => config.limits.max_explanation_bytes = 0,
        10 => config.limits.max_name_bytes = 4,
        11 => {
            config.limits.max_samples_per_field = 0;
            source.fields[0].samples = Some(vec![SampleValue::Null]);
        }
        12 => {
            config.limits.max_sample_bytes = 2;
            source.fields[0].samples = Some(vec![SampleValue::Text("abc".into())]);
        }
        13 => {
            config.limits.max_total_sample_bytes = 5;
            source.fields[0].samples = Some(vec![SampleValue::Text("abc".into()); 2]);
        }
        14 => {
            let bytes = 1 + random.index(32);
            config.limits.max_sample_bytes = bytes;
            config.limits.max_total_sample_bytes = bytes;
            config.limits.max_samples_per_field = 1;
            source.fields[0].samples = Some(vec![SampleValue::Text("x".repeat(bytes))]);
        }
        _ => {
            let (name, sample) = [
                (f64::NAN, 0.5),
                (f64::INFINITY, 0.5),
                (-0.01, 0.5),
                (1.01, 0.5),
                (0.0, 0.0),
                (0.0, f64::NAN),
                (0.0, f64::INFINITY),
                (0.0, 1.01),
            ][choice - 15];
            config.corroboration = Some(Corroboration {
                min_name_score: name,
                min_sample_score: sample,
            });
        }
    }
    let engine = MatchEngine::new(config);
    if choice <= 5 || choice >= 15 {
        ensure(engine.is_err(), "invalid configuration accepted")
    } else {
        let engine = engine.map_err(|error| error.to_string())?;
        let report = engine.match_schemas(&source, &target);
        ensure(
            report.is_ok() == (choice == 14),
            "resource boundary accepted/rejected incorrectly",
        )
    }
}

fn random_field(random: &mut Random, index: usize) -> Field {
    let names = [
        "amount",
        "Amt",
        "account_id",
        "GrossAmt",
        "SKU_Code",
        "product_id",
        "date",
        "transaction_date",
        "",
        "東京",
        "ＡＭＴ",
    ];
    let types = [
        DataType::Unknown,
        DataType::Boolean,
        DataType::Integer,
        DataType::Float,
        DataType::Decimal,
        DataType::Text,
        DataType::Date,
        DataType::Timestamp,
        DataType::Binary,
    ];
    let mut field = Field::new(
        index.to_string(),
        names[random.index(names.len())],
        types[random.index(types.len())],
    );
    let value = (random.next() % 100) as i128;
    field.samples = match random.index(8) {
        0 => None,
        1 => Some(Vec::new()),
        2 => Some(vec![SampleValue::Null; 4]),
        3 => Some(vec![
            SampleValue::Integer(value),
            SampleValue::Integer(value + 1),
            SampleValue::Integer(value + 2),
        ]),
        4 => Some(vec![
            SampleValue::Decimal(
                ExactDecimal::new(value, 2).expect("bounded scale")
            );
            4
        ]),
        5 => Some(vec![
            SampleValue::Boolean(true),
            SampleValue::Boolean(false),
            SampleValue::Null,
        ]),
        6 => Some(vec![SampleValue::Number(value as f64); 3]),
        _ => Some(vec![
            SampleValue::Text(format!("v{value}")),
            SampleValue::Text(format!("v{}", value + 1)),
            SampleValue::Text(format!("v{}", value + 2)),
        ]),
    };
    if random.index(5) == 0 {
        field.hints = SemanticHints {
            unit: Some(if random.index(2) == 0 { "ms" } else { "s" }.into()),
            ..SemanticHints::default()
        };
    }
    field
}

fn report_invariants(
    report: &MatchReport,
    source: &Schema,
    target: &Schema,
    max_candidates: usize,
) -> Result<(), String> {
    ensure(
        report.fields.len() == source.fields.len(),
        "report source coverage",
    )?;
    let target_ids: BTreeSet<_> = target.fields.iter().map(|field| &field.id).collect();
    let source_ids: BTreeSet<_> = source.fields.iter().map(|field| &field.id).collect();
    let mut reported = BTreeSet::new();
    let mut selected_targets = BTreeSet::new();
    let mut unmatched_sources = BTreeSet::new();
    for field in &report.fields {
        ensure(
            reported.insert(&field.source) && source_ids.contains(&field.source),
            "duplicate or unknown report source",
        )?;
        ensure(
            field.candidates.len() <= max_candidates,
            "candidate truncation bound",
        )?;
        for candidate in &field.candidates {
            ensure(target_ids.contains(&candidate.target), "unknown target")?;
            ensure(
                candidate.score.is_finite() && (0.0..=1.0).contains(&candidate.score),
                "candidate score range",
            )?;
            for signal in &candidate.signals {
                ensure(
                    signal
                        .evidence
                        .score
                        .is_none_or(|score| score.is_finite() && (0.0..=1.0).contains(&score)),
                    "signal score range",
                )?;
            }
        }
        for pair in field.candidates.windows(2) {
            ensure(
                pair[0].score > pair[1].score
                    || (pair[0].score == pair[1].score && pair[0].target < pair[1].target),
                "deterministic ranking order",
            )?;
        }
        if let Some(selected) = &field.selected {
            ensure(
                selected.eligible && target_ids.contains(&selected.target),
                "ineligible selection",
            )?;
            let first = selected_targets.insert(&selected.target);
            ensure(!report.one_to_one || first, "one-to-one target reused")?;
        } else {
            unmatched_sources.insert(&field.source);
        }
    }
    ensure(
        unmatched_sources == report.unmatched_sources.iter().collect(),
        "unmatched source complement",
    )?;
    let unmatched_targets: BTreeSet<_> =
        target_ids.difference(&selected_targets).copied().collect();
    ensure(
        unmatched_targets == report.unmatched_targets.iter().collect(),
        "unmatched target complement",
    )
}

fn default_reports(random: &mut Random) -> Result<(), String> {
    let mut source = Schema::new(
        (0..random.index(6))
            .map(|index| random_field(random, index))
            .collect(),
    );
    let mut target = Schema::new(
        (0..random.index(6))
            .map(|index| random_field(random, index))
            .collect(),
    );
    let max_candidates = 1 + random.index(5);
    let corroboration = match random.index(3) {
        0 => None,
        1 => Some(Corroboration::default()),
        _ => Some(Corroboration {
            min_name_score: 0.8,
            ..Corroboration::default()
        }),
    };
    let config = Config {
        one_to_one: random.index(2) == 0,
        max_candidates,
        corroboration,
        ..Config::default()
    };
    let engine = MatchEngine::new(config.clone()).map_err(|error| error.to_string())?;
    let report = engine
        .match_schemas(&source, &target)
        .map_err(|error| error.to_string())?;
    report_invariants(&report, &source, &target, max_candidates)?;
    if let Some(policy) = &config.corroboration {
        for field in &report.fields {
            for candidate in field.candidates.iter().chain(&field.selected) {
                if !candidate.eligible {
                    continue;
                }
                // This harness constructed MatchEngine::new explicitly, so these
                // signal reports come from actual active built-ins. Production
                // extension code must not use matcher names as an authenticity check.
                for (name, floor) in [
                    ("name", policy.min_name_score),
                    ("samples", policy.min_sample_score),
                ] {
                    ensure(
                        candidate.signals.iter().any(|signal| {
                            signal.name == name
                                && signal.weight > 0.0
                                && signal
                                    .evidence
                                    .score
                                    .is_some_and(|score| score > 0.0 && score >= floor)
                        }),
                        "eligible corroborated candidate lacks positive built-in support",
                    )?;
                }
            }
        }
        let ungated = MatchEngine::new(Config {
            corroboration: None,
            ..config.clone()
        })
        .map_err(|error| error.to_string())?
        .match_schemas(&source, &target)
        .map_err(|error| error.to_string())?;
        for (gated, original) in report.fields.iter().zip(&ungated.fields) {
            ensure(
                gated.source == original.source
                    && gated.candidates.len() == original.candidates.len(),
                "corroboration changed ranked candidate inventory",
            )?;
            for candidate in &gated.candidates {
                let original = original
                    .candidates
                    .iter()
                    .find(|original| original.target == candidate.target)
                    .ok_or("corroboration changed candidate target")?;
                ensure(
                    candidate.score == original.score && candidate.signals == original.signals,
                    "corroboration changed score or signal evidence",
                )?;
                ensure(
                    !candidate.eligible || original.eligible,
                    "corroboration admitted a previously ineligible pair",
                )?;
            }
        }
        // Selections are deliberately not required to be a subset: removing a
        // weak alternative can resolve local ambiguity and create a proposal.
    }
    source.fields.reverse();
    target.fields.reverse();
    let reordered = engine
        .match_schemas(&source, &target)
        .map_err(|error| error.to_string())?;
    ensure(report == reordered, "field-order changed report")
}

struct Matrix(Vec<Vec<u8>>);
impl Matcher for Matrix {
    fn name(&self) -> &str {
        "qualification_matrix"
    }
    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        let row = source
            .id
            .0
            .parse::<usize>()
            .map_err(|_| "bad harness source")?;
        let column = target
            .id
            .0
            .parse::<usize>()
            .map_err(|_| "bad harness target")?;
        Ok(Evidence {
            score: Some(f64::from(self.0[row][column]) / 100.0),
            explanation: String::new(),
        })
    }
}

fn exhaustive(scores: &[Vec<u8>], threshold: u8, row: usize, used: u8) -> usize {
    if row == scores.len() {
        return 0;
    }
    let mut best = exhaustive(scores, threshold, row + 1, used);
    for (column, score) in scores[row].iter().enumerate() {
        if *score > 0 && *score >= threshold && used & (1 << column) == 0 {
            best = best.max(
                usize::from(*score) + exhaustive(scores, threshold, row + 1, used | (1 << column)),
            );
        }
    }
    best
}

fn best_other_assignment(
    scores: &[Vec<u8>],
    threshold: u8,
    row: usize,
    used: u8,
    selected: &[Option<usize>],
    differs: bool,
) -> Option<usize> {
    if row == scores.len() {
        return differs.then_some(0);
    }
    let mut best = best_other_assignment(
        scores,
        threshold,
        row + 1,
        used,
        selected,
        differs || selected[row].is_some(),
    );
    for (column, score) in scores[row].iter().enumerate() {
        if *score > 0 && *score >= threshold && used & (1 << column) == 0 {
            if let Some(tail) = best_other_assignment(
                scores,
                threshold,
                row + 1,
                used | (1 << column),
                selected,
                differs || selected[row] != Some(column),
            ) {
                best = best.max(Some(usize::from(*score) + tail));
            }
        }
    }
    best
}

fn diagnostic_invariants(
    report: &MatchReport,
    matrix: &[Vec<u8>],
    threshold: u8,
    config: &GlobalDiagnosticsConfig,
) -> Result<(), String> {
    let diagnostic = &report.assignment_diagnostics;
    let rows = matrix.len();
    let columns = matrix[0].len();
    let selected: Vec<Option<usize>> = report
        .fields
        .iter()
        .map(|field| {
            field.selected.as_ref().map(|candidate| {
                candidate
                    .target
                    .0
                    .parse()
                    .expect("numeric harness identity")
            })
        })
        .collect();
    let selected_edges = selected.iter().flatten().count();
    let work_per_solve = rows * rows * (rows + columns);
    let expected_solves = selected_edges
        .min(config.max_solves)
        .min(config.max_work / work_per_solve);
    ensure(
        diagnostic.solves_used == expected_solves,
        "diagnostic solve count",
    )?;
    ensure(
        diagnostic.work_used == expected_solves * work_per_solve
            && diagnostic.work_used <= config.max_work,
        "diagnostic work budget",
    )?;
    let expected_status = if config.max_solves == 0 {
        AssignmentDiagnosticStatus::Disabled
    } else if expected_solves < selected_edges {
        AssignmentDiagnosticStatus::BudgetExhausted
    } else {
        AssignmentDiagnosticStatus::Complete
    };
    ensure(
        diagnostic.status == expected_status,
        "diagnostic completeness status",
    )?;
    if config.max_solves == 0 {
        return ensure(
            diagnostic.base_objective.is_none() && diagnostic.alternatives.is_empty(),
            "disabled diagnostics populated",
        );
    }
    let base: usize = selected
        .iter()
        .enumerate()
        .filter_map(|(row, column)| column.map(|column| usize::from(matrix[row][column])))
        .sum();
    ensure(
        diagnostic
            .base_objective
            .is_some_and(|value| (value - base as f64 / 100.0).abs() < 1e-10),
        "diagnostic base objective",
    )?;
    for alternative in &diagnostic.alternatives {
        let mut mapping = selected.clone();
        let mut changed_sources = BTreeSet::new();
        for change in &alternative.changes {
            let row = change
                .source
                .0
                .parse::<usize>()
                .map_err(|_| "invalid witness source")?;
            ensure(
                row < rows && changed_sources.insert(row),
                "witness duplicate/unknown source",
            )?;
            let original = change
                .selected_target
                .as_ref()
                .map(|id| id.0.parse::<usize>())
                .transpose()
                .map_err(|_| "invalid witness selected target")?;
            ensure(original == selected[row], "witness original selection")?;
            mapping[row] = change
                .alternative_target
                .as_ref()
                .map(|id| id.0.parse::<usize>())
                .transpose()
                .map_err(|_| "invalid witness alternative target")?;
        }
        ensure(
            mapping != selected,
            "witness did not change any real mapping",
        )?;
        let mut used = BTreeSet::new();
        let mut objective = 0;
        for (row, column) in mapping.iter().enumerate() {
            if let Some(column) = column {
                ensure(
                    *column < columns && used.insert(*column),
                    "witness target invalid or reused",
                )?;
                let score = matrix[row][*column];
                ensure(
                    score > 0 && score >= threshold,
                    "witness uses forbidden edge",
                )?;
                objective += usize::from(score);
            }
        }
        ensure(
            objective <= base && (alternative.objective - objective as f64 / 100.0).abs() < 1e-10,
            "witness objective mismatch",
        )?;
        ensure(
            (alternative.gap - (base - objective) as f64 / 100.0).abs() < 1e-10
                && alternative.gap <= config.objective_margin + 1e-10,
            "witness gap or margin mismatch",
        )?;
    }
    if diagnostic.status == AssignmentDiagnosticStatus::Complete {
        let best_other = best_other_assignment(matrix, threshold, 0, 0, &selected, false);
        let should_exist = best_other
            .is_some_and(|value| (base - value) as f64 / 100.0 <= config.objective_margin + 1e-10);
        ensure(
            should_exist != diagnostic.alternatives.is_empty(),
            "complete alternative analysis disagrees with exhaustive oracle",
        )?;
        if should_exist {
            let actual = diagnostic
                .alternatives
                .iter()
                .map(|alternative| alternative.objective)
                .max_by(f64::total_cmp)
                .ok_or("missing best witness")?;
            ensure(
                (actual - best_other.ok_or("missing oracle alternative")? as f64 / 100.0).abs()
                    < 1e-10,
                "best competing objective differs from exhaustive oracle",
            )?;
        }
    }
    Ok(())
}

fn assignment(random: &mut Random) -> Result<(), String> {
    let rows = 2 + random.index(3);
    let columns = 2 + random.index(3);
    let matrix: Vec<Vec<_>> = (0..rows)
        .map(|_| (0..columns).map(|_| random.index(101) as u8).collect())
        .collect();
    let threshold = [0, 25, 50, 75][random.index(4)];
    let expected = exhaustive(&matrix, threshold, 0, 0);
    let schema = |count| {
        Schema::new(
            (0..count)
                .map(|index: usize| Field::new(index.to_string(), "value", DataType::Unknown))
                .collect(),
        )
    };
    let mut source = schema(rows);
    let mut target = schema(columns);
    let diagnostics = GlobalDiagnosticsConfig {
        max_solves: random.index(5),
        max_work: if random.index(3) == 0 { 0 } else { 1024 },
        objective_margin: [0.0, 0.05, 0.2][random.index(3)],
    };
    let max_candidates = 1 + random.index(4);
    let engine = MatchEngine::with_matchers(
        Config {
            min_score: f64::from(threshold) / 100.0,
            one_to_one: true,
            abstain_on_ambiguity: false,
            max_candidates,
            global_diagnostics: diagnostics.clone(),
            ..Config::default()
        },
        vec![WeightedMatcher::new(1.0, Matrix(matrix.clone()))],
    )
    .map_err(|error| error.to_string())?;
    let report = engine
        .match_schemas(&source, &target)
        .map_err(|error| error.to_string())?;
    report_invariants(&report, &source, &target, max_candidates)?;
    let actual: f64 = report
        .fields
        .iter()
        .filter_map(|field| field.selected.as_ref())
        .map(|candidate| candidate.score)
        .sum();
    ensure(
        (actual - expected as f64 / 100.0).abs() < 1e-10,
        "assignment differs from exhaustive integer oracle",
    )?;
    diagnostic_invariants(&report, &matrix, threshold, &diagnostics)?;
    source.fields.reverse();
    target.fields.reverse();
    ensure(
        report
            == engine
                .match_schemas(&source, &target)
                .map_err(|error| error.to_string())?,
        "assignment depends on input order",
    )
}

fn run(cases: u64, seed: u64) -> Result<(), String> {
    if cases == 0 || cases > 10_000_000 {
        return Err("cases must be in 1..=10000000".into());
    }
    let started = Instant::now();
    let mut random = Random(seed);
    let mut counts = [0_u64; 5];
    for index in 0..cases {
        let category = index as usize % counts.len();
        let outcome = match category {
            0 => normalization(&mut random),
            1 => malformed(&mut random),
            2 => config_and_limits(&mut random),
            3 => default_reports(&mut random),
            _ => assignment(&mut random),
        };
        outcome.map_err(|reason| {
            format!("case {index}, category {category}, seed {seed}: {reason}")
        })?;
        counts[category] += 1;
        if (index + 1) % 100_000 == 0 {
            eprintln!("completed {} generated cases", index + 1);
        }
    }
    println!("{{\"protocol\":\"fieldkin-qualification-v1\",\"seed\":{seed},\"cases\":{cases},\"passed\":true,\"elapsed_seconds\":{:.6},\"categories\":{{\"normalization\":{},\"malformed_schemas\":{},\"configuration_and_limits\":{},\"default_report_invariants\":{},\"assignment_oracle\":{}}}}}", started.elapsed().as_secs_f64(), counts[0], counts[1], counts[2], counts[3], counts[4]);
    Ok(())
}

fn main() {
    let mut args = std::env::args().skip(1);
    let mut cases = 1_000_000;
    let mut seed = 20_261_003;
    let parsed = (|| -> Result<(), String> {
        while let Some(flag) = args.next() {
            let value = args
                .next()
                .ok_or("argument requires a value")?
                .parse::<u64>()
                .map_err(|_| "expected unsigned integer")?;
            match flag.as_str() {
                "--cases" => cases = value,
                "--seed" => seed = value,
                _ => return Err("use --cases N --seed N".into()),
            }
        }
        run(cases, seed)
    })();
    if let Err(error) = parsed {
        eprintln!("qualification failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exhaustive_oracle_permits_unmatched_and_avoids_greedy_trap() {
        assert_eq!(exhaustive(&[vec![90, 80], vec![89, 0]], 0, 0, 0), 169);
        assert_eq!(exhaustive(&[vec![0, 0], vec![0, 0]], 0, 0, 0), 0);
        assert_eq!(exhaustive(&[vec![24, 0], vec![0, 25]], 25, 0, 0), 25);
    }
    #[test]
    fn smoke_all_generated_categories() {
        run(500, 20_261_003).unwrap();
    }
}
