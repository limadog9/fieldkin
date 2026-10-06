//! Development-only audit of sample-format evidence.
//! Does not modify the matcher, dataset, or historical results.
//! Independent additive simulation only; not a production selector or qualification.
//! Labels are used for scoring after the original matcher and proposed rule decisions.

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

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::PathBuf;

use fieldkin::{
    CandidateIssue, Config, ContextualEvidence, DataType, Decision, Field, MatchEngine, NameConflictKind,
    NameConflictRule, SampleValue, Schema,
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

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
struct Counts {
    unique: usize,
    unique_proposals: usize,
    proposals: usize,
    correct: usize,
    wrong_or_unsafe: usize,
}

impl Counts {
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
        max_candidates: 32,
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

fn cases(fresh_path: &std::path::Path) -> Result<Vec<ProbeCase>> {
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

fn name_score(candidate: &fieldkin::Candidate) -> f64 {
    candidate
        .signals
        .iter()
        .find(|s| s.name == "name")
        .and_then(|s| s.evidence.score)
        .unwrap_or(0.0)
}


const MIN_DISTINCT_TEXT: usize = 3;
const MIN_TEXT_COVERAGE: f64 = 0.25;

fn reliable_text_values(field: &Field) -> Option<Vec<&str>> {
    if field.data_type != DataType::Text {
        return None;
    }
    let samples = field.samples.as_deref()?;
    if samples.is_empty() {
        return None;
    }
    let mut values = Vec::new();
    for sample in samples {
        match sample {
            SampleValue::Null => {}
            SampleValue::Text(value) if !value.is_empty() => values.push(value.as_str()),
            // Do not silently discard mixed runtime sample kinds or empty strings.
            _ => return None,
        }
    }
    let distinct: BTreeSet<_> = values.iter().copied().collect();
    if distinct.len() < MIN_DISTINCT_TEXT
        || (values.len() as f64 / samples.len() as f64) < MIN_TEXT_COVERAGE
    {
        return None;
    }
    Some(values)
}

fn distinct_text_count(field: &Field) -> usize {
    field.samples.as_deref().unwrap_or_default().iter().filter_map(|s| {
        match s {
            SampleValue::Text(value) => Some(value.as_str()),
            _ => None,
        }
    }).collect::<BTreeSet<_>>().len()
}

fn looks_email(value: &str) -> bool {
    let Some((local, domain)) = value.split_once('@') else {
        return false;
    };
    !local.is_empty() && domain.contains('.') && !domain.ends_with('.')
}

fn common_prefix(values: &[&str]) -> String {
    let Some(first) = values.first() else {
        return String::new();
    };
    let mut prefix = first.to_string();
    for value in &values[1..] {
        let mut n = 0usize;
        for (a, b) in prefix.chars().zip(value.chars()) {
            if a != b {
                break;
            }
            n += a.len_utf8();
        }
        prefix.truncate(n);
        if prefix.is_empty() {
            break;
        }
    }
    prefix
}

fn meaningful_prefix(prefix: &str) -> Option<String> {
    let trimmed = prefix.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    let letters = trimmed.chars().filter(|c| c.is_ascii_alphabetic()).count();
    if letters >= 2 {
        Some(trimmed.to_ascii_lowercase())
    } else {
        None
    }
}

fn skeleton(value: &str) -> String {
    let mut out = String::new();
    let mut last = '\0';
    let mut run = 0usize;

    let flush = |out: &mut String, last: char, run: usize| {
        if run > 0 {
            out.push(last);
            out.push_str(&run.min(99).to_string());
        }
    };

    for c in value.chars() {
        let class = if c.is_ascii_alphabetic() {
            'A'
        } else if c.is_ascii_digit() {
            '9'
        } else if c.is_whitespace() {
            ' '
        } else {
            c
        };
        if class == last {
            run += 1;
        } else {
            flush(&mut out, last, run);
            last = class;
            run = 1;
        }
    }
    flush(&mut out, last, run);
    out
}

fn dominant_skeleton(values: &[&str]) -> Option<String> {
    if values.len() < 2 {
        return None;
    }
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for value in values {
        *counts.entry(skeleton(value)).or_default() += 1;
    }
    let (shape, count) = counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))?;
    (count * 2 >= values.len()).then_some(shape)
}

fn format_score(source: &Field, target: &Field) -> f64 {
    let Some(source_values) = reliable_text_values(source) else { return 0.0; };
    let Some(target_values) = reliable_text_values(target) else { return 0.0; };

    let source_email = source_values.iter().all(|v| looks_email(v));
    let target_email = target_values.iter().all(|v| looks_email(v));
    if source_email && target_email {
        return 1.0;
    }

    let sp = meaningful_prefix(&common_prefix(&source_values));
    let tp = meaningful_prefix(&common_prefix(&target_values));
    if sp.is_some() && sp == tp {
        return 1.0;
    }

    let ss = dominant_skeleton(&source_values);
    let ts = dominant_skeleton(&target_values);
    if let (Some(ss), Some(ts)) = (ss, ts) {
        if ss == ts {
            // Inspect actual character classes, not digits used to encode run
            // lengths: a nine-letter word has skeleton A9 but contains no digit.
            let both_values = source_values.iter().chain(&target_values);
            let has_alpha = both_values.clone().all(|v| v.chars().any(|c| c.is_ascii_alphabetic()));
            let has_digit = both_values.clone().all(|v| v.chars().any(|c| c.is_ascii_digit()));
            let has_structure = both_values.clone().all(|v| v.chars().any(|c| !c.is_ascii_alphanumeric()));
            if has_alpha && has_digit && has_structure {
                return 0.90;
            }
            if has_alpha && has_digit {
                return 0.80;
            }
            if !has_alpha && has_digit {
                return 0.70;
            }
        }
    }

    0.0
}


#[derive(Clone, Copy, Debug)]
struct Rule {
    min_format: f64,
    min_name: f64,
    row_margin: f64,
    col_margin: f64,
}

#[derive(Clone, Debug)]
struct Edge {
    source: usize,
    target: usize,
    format: f64,
    name: f64,
    combined: f64,
}

struct CachedCase {
    case: ProbeCase,
    // One original selection per source, expressed as an index into case.target.
    selected: Vec<Option<usize>>,
    ambiguous: Vec<bool>,
    rows: Vec<Vec<Edge>>,
    columns: Vec<Vec<Edge>>,
}

fn counts_from_official(c: &metrics::Counts) -> Counts {
    Counts {
        unique: c.unique_fields,
        unique_proposals: c.unique_proposals,
        proposals: c.proposals,
        correct: c.correct_proposals,
        wrong_or_unsafe: c.wrong_unique_proposals + c.no_match_proposals + c.ambiguous_proposals,
    }
}

fn add_counts(dst: &mut Counts, src: &Counts) {
    dst.unique += src.unique;
    dst.unique_proposals += src.unique_proposals;
    dst.proposals += src.proposals;
    dst.correct += src.correct;
    dst.wrong_or_unsafe += src.wrong_or_unsafe;
}

fn prepare(all: Vec<ProbeCase>) -> Result<(Vec<CachedCase>, BTreeMap<String, Counts>)> {
    let matcher = engine()?;
    let mut cache = Vec::new();
    let mut official = BTreeMap::<String, Counts>::new();
    for case in all {
        if case.target.fields.len() > 32 {
            return Err("probe needs all target candidates; schema exceeds display bound".into());
        }
        // Match exactly once per schema pair. No per-threshold matcher reruns.
        let report = matcher.match_schemas(&case.source, &case.target)
            .map_err(|e| e.to_string())?;
        let official_counts = metrics::score_case(&case.labels, &report)?;
        add_counts(official.entry(case.corpus_name.clone()).or_default(),
            &counts_from_official(&official_counts));

        let source_index: BTreeMap<_, _> = case.source.fields.iter().enumerate()
            .map(|(i, f)| (f.id.0.as_str(), i)).collect();
        let target_index: BTreeMap<_, _> = case.target.fields.iter().enumerate()
            .map(|(i, f)| (f.id.0.as_str(), i)).collect();
        let mut selected = vec![None; case.source.fields.len()];
        let mut ambiguous = vec![false; case.source.fields.len()];
        let mut rows = vec![Vec::new(); case.source.fields.len()];
        let mut columns = vec![Vec::new(); case.target.fields.len()];
        for result in &report.fields {
            let i = *source_index.get(result.source.0.as_str()).ok_or("missing source")?;
            if result.candidates.len() != case.target.fields.len() {
                return Err(format!("{}: incomplete candidate matrix", case.id));
            }
            selected[i] = result.selected.as_ref().map(|s| {
                target_index.get(s.target.0.as_str()).copied().ok_or("unknown selected target")
            }).transpose()?;
            ambiguous[i] = result.decision == Decision::Ambiguous;
            for candidate in &result.candidates {
                let j = *target_index.get(candidate.target.0.as_str()).ok_or("missing target")?;
                let source = &case.source.fields[i];
                let target = &case.target.fields[j];
                if hard_blocked(candidate) || source.data_type != target.data_type {
                    continue;
                }
                let format = format_score(source, target);
                let name = name_score(candidate);
                let edge = Edge { source: i, target: j, format, name,
                    combined: 0.75 * format + 0.25 * name };
                // Include every hard-compatible same-type competitor, even below
                // the later admission thresholds. Filtering before the margin test
                // would hide close alternatives and fabricate certainty.
                rows[i].push(edge.clone());
                columns[j].push(edge);
            }
        }
        for row in &mut rows {
            row.sort_by(|a, b| b.combined.total_cmp(&a.combined).then(a.target.cmp(&b.target)));
        }
        for column in &mut columns {
            column.sort_by(|a, b| b.combined.total_cmp(&a.combined).then(a.source.cmp(&b.source)));
        }
        cache.push(CachedCase { case, selected, ambiguous, rows, columns });
    }
    Ok((cache, official))
}

fn clear_margin(best: f64, other: Option<f64>, minimum: f64) -> bool {
    match other {
        Some(other) => best > other && best - other >= minimum,
        None => best >= minimum,
    }
}

fn admitted(row: &[Edge], columns: &[Vec<Edge>], rule: Rule) -> Option<usize> {
    let best = row.first()?;
    if best.format < rule.min_format || best.name < rule.min_name
        || !clear_margin(best.combined, row.get(1).map(|e| e.combined), rule.row_margin)
    {
        return None;
    }
    let column = &columns[best.target];
    let col_best = column.first()?;
    if col_best.source != best.source
        || !clear_margin(col_best.combined, column.get(1).map(|e| e.combined), rule.col_margin)
    {
        return None;
    }
    Some(best.target)
}

#[derive(Default)]
struct Evaluation {
    by_corpus: BTreeMap<String, Counts>,
    errors: BTreeMap<String, usize>,
    additions: BTreeMap<String, usize>,
}

fn evaluate(cache: &[CachedCase], rule: Option<Rule>, details: bool) -> Result<Evaluation> {
    let mut output = Evaluation::default();
    for entry in cache {
        let case = &entry.case;
        let label_by_source: BTreeMap<_, _> = case.labels.iter()
            .map(|l| (l.source.as_str(), l)).collect();
        for (i, source) in case.source.fields.iter().enumerate() {
            let label = label_by_source.get(source.id.0.as_str()).ok_or("source lacks label")?;
            let mut selected = entry.selected[i];
            let mut added = false;
            // Preserve original selections and original declared ambiguity.
            // This is intentionally an additive independent simulation, not a
            // reimplementation of Fieldkin's global assignment or reranking.
            if selected.is_none() && !entry.ambiguous[i] {
                if let Some(rule) = rule {
                    selected = admitted(&entry.rows[i], &entry.columns, rule);
                    added = selected.is_some();
                }
            }
            let counts = output.by_corpus.entry(case.corpus_name.clone()).or_default();
            if label.kind == model::LabelKind::Match { counts.unique += 1; }
            if let Some(j) = selected {
                let target = &case.target.fields[j];
                counts.proposals += 1;
                if label.kind == model::LabelKind::Match { counts.unique_proposals += 1; }
                let correct = label.kind == model::LabelKind::Match
                    && label.targets.first() == Some(&target.id.0);
                if correct { counts.correct += 1; } else { counts.wrong_or_unsafe += 1; }
                if added && details {
                    let family_id = case.id.split('/').next().unwrap_or(&case.id);
                    let summary = format!(
                        "{} / {} :: {} -> {} :: gold={:?}; distinct text={}/{}",
                        case.corpus_name, family_id, source.name, target.name, label.kind,
                        distinct_text_count(source), distinct_text_count(target));
                    *output.additions.entry(summary.clone()).or_default() += 1;
                    if !correct { *output.errors.entry(summary).or_default() += 1; }
                }
            }
        }
    }
    Ok(output)
}

fn meets_precision(c: &Counts) -> bool {
    c.proposals > 0 && c.correct as u128 * 100 >= c.proposals as u128 * 95
}

fn all_precision_pass(eval: &Evaluation) -> bool {
    ["original_development", "stage3_extension", "corrective_development",
     "examined_qualification", "fresh_development"].iter().all(|key| {
        eval.by_corpus.get(*key).is_some_and(meets_precision)
    })
}

// Reporting filter only. The simulation is additive and never removes an
// original selection, so unchanged error counts mean zero newly added errors.
// Check every corpus separately: improvements on one cannot hide damage to another.
fn adds_no_errors(eval: &Evaluation, baseline: &BTreeMap<String, Counts>) -> bool {
    eval.by_corpus.len() == baseline.len()
        && baseline.iter().all(|(name, before)| {
            eval.by_corpus.get(name).is_some_and(|after| {
                after.unique == before.unique
                    && after.correct >= before.correct
                    && after.wrong_or_unsafe == before.wrong_or_unsafe
            })
        })
}

fn pct(value: Option<f64>) -> String {
    value.map_or_else(|| "n/a".into(), |v| format!("{:.2}%", v * 100.0))
}

fn print_table(eval: &Evaluation, baseline: &BTreeMap<String, Counts>) {
    println!("| Corpus | Correct/proposed | Precision | Unique coverage | Correct recall | Added correct | Added errors | >=95% precision |");
    println!("| --- | ---: | ---: | ---: | ---: | ---: | ---: | --- |");
    for (name, c) in &eval.by_corpus {
        let base = baseline.get(name).copied().unwrap_or_default();
        let recall = (c.unique > 0).then(|| c.correct as f64 / c.unique as f64);
        println!("| {name} | {}/{} | {} | {} | {} | {} | {} | {} |", c.correct, c.proposals,
            pct(c.precision()), pct(c.coverage()), pct(recall),
            c.correct.saturating_sub(base.correct), c.wrong_or_unsafe.saturating_sub(base.wrong_or_unsafe),
            meets_precision(c));
    }
}

fn print_errors(eval: &Evaluation) {
    println!("\n### Added wrong/unsafe pairs (all corpora; grouped across related variants)");
    if eval.errors.is_empty() { println!("None."); }
    for (error, count) in &eval.errors { println!("{count} related decision(s): {error}"); }
}

fn rule_label(r: Rule) -> String {
    format!("format >= {:.2}, name >= {:.2}, row margin >= {:.2}, column margin >= {:.2}",
        r.min_format, r.min_name, r.row_margin, r.col_margin)
}

fn run() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let path = PathBuf::from(args.next().ok_or("usage: sample_format_probe PATH_TO_EXAMINED_CORPUS")?);
    if args.next().is_some() { return Err("unexpected additional arguments".into()); }
    let all = cases(&path)?;
    let (cache, official) = prepare(all)?;
    let baseline = evaluate(&cache, None, false)?;
    if baseline.by_corpus != official {
        return Err("NO-OP CONTROL FAILED: probe counters disagree with the existing official scorer".into());
    }
    println!("# Sample-format probe: error-tradeoff comparison\n");
    println!("Examined development data only. Independent additive simulation; no new qualification claim.");
    println!("Matcher, input corpus and historical outputs were not modified.");
    println!("Requires >=3 distinct text values per field and >=25% usable coverage; rejects constants and mixed types.");
    println!("All same-type hard-compatible competitors participate in margins; equal-score ties do not win.");
    println!("Original selections and declared ambiguities are preserved. Actual selector integration and one-to-one remain untested.\n");
    println!("## No-op control: agrees with the official scorer\n");
    print_table(&baseline, &official);
    if !all_precision_pass(&baseline) {
        println!("\nSTOP: the unchanged baseline is already below 95% precision on at least one corpus. No sweep was run.");
        return Ok(());
    }

    // Also show an explicit conservative diagnostic, even if the sweep has no
    // passing rules. These settings were already present in the previous grid.
    let conservative = Rule { min_format: 1.0, min_name: 0.4, row_margin: 0.2, col_margin: 0.2 };
    let audit = evaluate(&cache, Some(conservative), true)?;
    println!("\n## Conservative reliability-controlled rule\n{}\n", rule_label(conservative));
    print_table(&audit, &official);
    print_errors(&audit);

    let baseline_fresh = official.get("fresh_development").ok_or("missing fresh-development control")?;
    let mut passing = Vec::<(Rule, Counts)>::new();
    let mut zero_added_error_rules = Vec::<(Rule, Counts)>::new();
    let mut tested = 0;
    let mut safe_count = 0;
    for min_format in [0.70, 0.80, 0.90, 1.00] {
        for min_name in [0.00, 0.10, 0.20, 0.30, 0.40] {
            for row_margin in [0.00, 0.05, 0.10, 0.15, 0.20] {
                for col_margin in [0.00, 0.05, 0.10, 0.15, 0.20] {
                    tested += 1;
                    let rule = Rule { min_format, min_name, row_margin, col_margin };
                    let result = evaluate(&cache, Some(rule), false)?;
                    if all_precision_pass(&result) {
                        safe_count += 1;
                        let fresh = result.by_corpus.get("fresh_development").copied()
                            .ok_or("missing fresh-development score")?;
                        if fresh.correct > baseline_fresh.correct {
                            passing.push((rule, fresh));
                            if adds_no_errors(&result, &official) {
                                zero_added_error_rules.push((rule, fresh));
                            }
                        }
                    }
                }
            }
        }
    }
    // Rank only already-passing rules, using correct recoveries rather than
    // unique coverage (which can include wrong-target proposals).
    passing.sort_by(|a, b| b.1.correct.cmp(&a.1.correct)
        .then(a.1.wrong_or_unsafe.cmp(&b.1.wrong_or_unsafe))
        .then(b.0.min_name.total_cmp(&a.0.min_name))
        .then(b.0.min_format.total_cmp(&a.0.min_format))
        .then(b.0.row_margin.total_cmp(&a.0.row_margin))
        .then(b.0.col_margin.total_cmp(&a.0.col_margin)));
    zero_added_error_rules.sort_by(|a, b| b.1.correct.cmp(&a.1.correct)
        .then(a.1.wrong_or_unsafe.cmp(&b.1.wrong_or_unsafe))
        .then(b.0.min_name.total_cmp(&a.0.min_name))
        .then(b.0.min_format.total_cmp(&a.0.min_format))
        .then(b.0.row_margin.total_cmp(&a.0.row_margin))
        .then(b.0.col_margin.total_cmp(&a.0.col_margin)));
    println!("\n## Grid summary\nTested {tested} rules. {safe_count} preserved >=95% precision on ALL FIVE examined corpora.");
    println!("{} of those also recovered additional correct fresh-development decisions.", passing.len());
    println!("{} of those useful rules added ZERO errors on every corpus.", zero_added_error_rules.len());
    if let Some((rule, _)) = passing.first() {
        let best = evaluate(&cache, Some(*rule), true)?;
        println!("\n## Best tested development-only rule\n{}\n", rule_label(*rule));
        print_table(&best, &official);
        print_errors(&best);
        println!("\n### Fresh-development additions (grouped; not independent observations)");
        for (addition, count) in &best.additions {
            if addition.starts_with("fresh_development /") { println!("{count} related decision(s): {addition}"); }
        }
    } else {
        println!("No useful passing rule in this corrected grid. This does not rule out other feature designs.");
    }

    println!("\n## Best tested rule with zero added errors on every corpus");
    println!("This is an additional development comparison, not a new release threshold.");
    println!("The original >=95% precision gate and every tested threshold remain unchanged.\n");
    if let Some((rule, fresh)) = zero_added_error_rules.first() {
        let best = evaluate(&cache, Some(*rule), true)?;
        if !adds_no_errors(&best, &official) || !best.errors.is_empty() {
            return Err("zero-added-error report disagrees with detailed evaluation".into());
        }
        println!("{}\n", rule_label(*rule));
        print_table(&best, &official);
        print_errors(&best);
        let needed_for_sixty_percent = (fresh.unique as u128 * 60).div_ceil(100);
        println!("\nFresh-development correct decisions: {} of {} ({}).",
            fresh.correct, fresh.unique,
            pct((fresh.unique > 0).then(|| fresh.correct as f64 / fresh.unique as f64)));
        println!("At zero wrong unique proposals, 60% coverage requires {} correct decisions; {} more would be needed.",
            needed_for_sixty_percent,
            needed_for_sixty_percent.saturating_sub(fresh.correct as u128));
        if let Some((_, best_fresh)) = passing.first() {
            println!("Compared with the best >=95%-precision rule, the zero-added-error rule gives up {} fresh correct decisions.",
                best_fresh.correct.saturating_sub(fresh.correct));
        }
        println!("\n### Fresh-development additions for the zero-added-error rule");
        for (addition, count) in &best.additions {
            if addition.starts_with("fresh_development /") {
                println!("{count} related decision(s): {addition}");
            }
        }
        println!("\n### Leading zero-added-error settings (same grid; descriptive only)");
        println!("| Format floor | Name floor | Row margin | Column margin | Fresh correct | Fresh correct recall |");
        println!("| ---: | ---: | ---: | ---: | ---: | ---: |");
        for (rule, counts) in zero_added_error_rules.iter().take(10) {
            println!("| {:.2} | {:.2} | {:.2} | {:.2} | {} | {} |",
                rule.min_format, rule.min_name, rule.row_margin, rule.col_margin, counts.correct,
                pct((counts.unique > 0).then(|| counts.correct as f64 / counts.unique as f64)));
        }
    } else {
        println!("No tested rule both improved fresh correct recall and added zero errors across all corpora.");
    }
    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("sample-format audit failed: {error}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod audit_tests {
    use super::*;

    fn text_field(name: &str, values: &[&str]) -> Field {
        Field::new("id", name, DataType::Text).with_samples(values.iter()
            .map(|v| SampleValue::Text((*v).to_owned())).collect())
    }

    #[test]
    fn repeated_sentinels_do_not_supply_format_evidence() {
        for value in ["NEW", "UNKNOWN", "ACTIVE", "example@test.invalid", "SKU-001"] {
            let source = text_field("a", &[value, value, value, value]);
            let target = text_field("b", &[value, value, value]);
            assert_eq!(format_score(&source, &target), 0.0);
        }
    }

    #[test]
    fn two_distinct_values_do_not_meet_reliability_floor() {
        let source = text_field("a", &["INV-001", "INV-002", "INV-001"]);
        let target = text_field("b", &["INV-101", "INV-102", "INV-101"]);
        assert_eq!(format_score(&source, &target), 0.0);
    }

    #[test]
    fn three_distinct_structured_codes_can_supply_evidence() {
        let source = text_field("a", &["INV-001", "INV-002", "INV-003"]);
        let target = text_field("b", &["INV-101", "INV-102", "INV-103"]);
        assert!(format_score(&source, &target) >= 0.9);
    }

    #[test]
    fn skeleton_run_length_digits_are_not_value_digits() {
        let source = text_field("a", &["ABCDEFGHI", "JKLMNOPQR", "STUVWXYZA"]);
        let target = text_field("b", &["BCDEFGHIJ", "KLMNOPQRS", "TUVWXYZAB"]);
        assert_eq!(format_score(&source, &target), 0.0);
    }

    #[test]
    fn malformed_sample_kinds_are_not_silently_filtered() {
        let mut source = text_field("a", &["INV-001", "INV-002", "INV-003"]);
        source.samples.as_mut().unwrap().push(SampleValue::Boolean(true));
        let target = text_field("b", &["INV-101", "INV-102", "INV-103"]);
        assert_eq!(format_score(&source, &target), 0.0);
    }

    #[test]
    fn nulls_do_not_create_diversity_or_erase_coverage_checks() {
        let mut source = text_field("a", &["INV-001", "INV-002", "INV-003"]);
        let target = text_field("b", &["INV-101", "INV-102", "INV-103"]);
        source.samples.as_mut().unwrap().extend(vec![SampleValue::Null; 9]);
        assert!(format_score(&source, &target) > 0.0); // exactly 25%
        source.samples.as_mut().unwrap().push(SampleValue::Null);
        assert_eq!(format_score(&source, &target), 0.0);
    }

    #[test]
    fn below_admission_threshold_competitor_still_blocks_a_close_win() {
        let row = vec![
            Edge { source: 0, target: 0, format: 1.0, name: 0.4, combined: 0.85 },
            Edge { source: 0, target: 1, format: 1.0, name: 0.39, combined: 0.8475 },
        ];
        let columns = vec![vec![row[0].clone()], vec![row[1].clone()]];
        let rule = Rule { min_format: 1.0, min_name: 0.4, row_margin: 0.05, col_margin: 0.05 };
        assert!(admitted(&row, &columns, rule).is_none());
    }

    #[test]
    fn zero_error_filter_allows_baseline_errors_but_no_new_ones() {
        let before = Counts { unique: 100, unique_proposals: 92,
            proposals: 95, correct: 92, wrong_or_unsafe: 3 };
        let mut eval = Evaluation::default();
        let baseline = BTreeMap::from([("example".to_owned(), before)]);
        eval.by_corpus.insert("example".to_owned(), Counts {
            unique_proposals: 96, proposals: 99, correct: 96, ..before
        });
        assert!(adds_no_errors(&eval, &baseline));
        eval.by_corpus.get_mut("example").unwrap().wrong_or_unsafe += 1;
        assert!(!adds_no_errors(&eval, &baseline));
    }

    #[test]
    fn zero_error_filter_checks_each_corpus_not_a_pooled_total() {
        let before = Counts { unique: 100, unique_proposals: 90,
            proposals: 92, correct: 90, wrong_or_unsafe: 2 };
        let baseline = BTreeMap::from([
            ("a".to_owned(), before), ("b".to_owned(), before),
        ]);
        let mut eval = Evaluation::default();
        eval.by_corpus = baseline.clone();
        eval.by_corpus.get_mut("a").unwrap().wrong_or_unsafe = 1;
        eval.by_corpus.get_mut("b").unwrap().wrong_or_unsafe = 3;
        assert!(!adds_no_errors(&eval, &baseline));
    }

    #[test]
    fn zero_error_filter_rejects_missing_corpus_and_changed_denominator() {
        let before = Counts { unique: 100, unique_proposals: 90,
            proposals: 90, correct: 90, wrong_or_unsafe: 0 };
        let baseline = BTreeMap::from([("example".to_owned(), before)]);
        let mut eval = Evaluation::default();
        assert!(!adds_no_errors(&eval, &baseline));
        eval.by_corpus.insert("example".to_owned(), before);
        assert!(adds_no_errors(&eval, &baseline));
        eval.by_corpus.get_mut("example").unwrap().unique -= 1;
        assert!(!adds_no_errors(&eval, &baseline));
    }

    #[test]
    fn exact_ties_do_not_win_even_at_zero_margin() {
        assert!(!clear_margin(0.9, Some(0.9), 0.0));
    }
}
