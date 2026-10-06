//! Bounded sample-format fallback for the opt-in contextual policy.
//!
//! Scores are fixed development heuristics, not calibrated probabilities. This
//! adds edges only to previously unsupported rows and unclaimed targets. The
//! normal constraint, ambiguity and assignment paths still own every decision.

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    BudgetKind, Candidate, CandidateIssue, Config, CountKind, DataType, Field, MatchError,
    SampleValue,
};

use super::{FieldFeatures, TopTwo};

const FORMAT_FLOOR: f64 = 0.90;
const NAME_FLOOR: f64 = 0.40;
const MARGIN: f64 = 0.20;
const PROPOSAL_FLOOR: f64 = 0.90;

// Private and deliberately not Debug: a derived prefix may contain user data.
// Summarize once per field, not once per source/target pair.
struct Profile {
    email: bool,
    prefix: Option<String>,
    code_prefix: Option<String>,
    shape: Option<String>,
    initialism: Option<Initialism>,
    code_shapes: Option<BTreeSet<String>>,
    alpha: bool,
    digit: bool,
    structured: bool,
    distinct: usize,
}

impl Profile {
    fn new(field: &Field) -> Option<Self> {
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
                // Do not improve apparent reliability by dropping malformed data.
                _ => return None,
            }
        }
        let distinct = values.iter().copied().collect::<BTreeSet<_>>().len();
        if distinct < 3 || values.len() < samples.len().div_ceil(4) {
            return None;
        }
        Some(Self {
            email: values.iter().all(|value| looks_email(value)),
            prefix: meaningful_prefix(&common_prefix(&values)),
            code_prefix: stable_code_prefix(&values),
            shape: dominant_shape(&values),
            initialism: Initialism::new(&field.name),
            code_shapes: code_shapes(&values),
            alpha: values
                .iter()
                .all(|v| v.chars().any(|c| c.is_ascii_alphabetic())),
            digit: values.iter().all(|v| v.chars().any(|c| c.is_ascii_digit())),
            structured: values
                .iter()
                .all(|v| v.chars().any(|c| !c.is_ascii_alphanumeric())),
            distinct,
        })
    }
}

// A whole-field initialism, not a synonym dictionary. Keep every word in the
// expansion: prefixes, suffixes and qualifiers cannot be silently dropped.
enum Initialism {
    Short(String),
    Expanded(String),
}

impl Initialism {
    fn new(name: &str) -> Option<Self> {
        let tokens = crate::normalize_name(name);
        if tokens.len() == 1 {
            let token = &tokens[0];
            if (3..=8).contains(&token.len()) && token.bytes().all(|b| b.is_ascii_lowercase()) {
                return Some(Self::Short(token.clone()));
            }
        } else if (3..=8).contains(&tokens.len())
            && tokens
                .iter()
                .all(|token| token.len() >= 2 && token.bytes().all(|b| b.is_ascii_lowercase()))
        {
            return Some(Self::Expanded(
                tokens.iter().map(|t| t.as_bytes()[0] as char).collect(),
            ));
        }
        None
    }
}

// Retain all observed code shapes, not only a majority shape. This evidence is
// used ONLY with a complete initialism/expansion relation. Require ASCII codes
// containing letters, digits and a delimiter; never treat free text as a code.
// Shape strings and prefixes remain private and never enter explanations.
fn code_shapes(values: &[&str]) -> Option<BTreeSet<String>> {
    let valid = |value: &&str| {
        let bytes = value.as_bytes();
        bytes.first().is_some_and(u8::is_ascii_alphanumeric)
            && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
            && bytes.iter().any(u8::is_ascii_alphabetic)
            && bytes.iter().any(u8::is_ascii_digit)
            && bytes.iter().any(|b| matches!(*b, b'-' | b'_' | b':'))
            && bytes
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || matches!(*b, b'-' | b'_' | b':'))
    };
    if values.is_empty() || !values.iter().all(valid) {
        return None;
    }
    Some(values.iter().map(|value| skeleton(value)).collect())
}

fn initialism_support(source: Option<&Profile>, target: Option<&Profile>) -> bool {
    let (Some(source), Some(target)) = (source, target) else {
        return false;
    };
    let names_agree = match (&source.initialism, &target.initialism) {
        (Some(Initialism::Short(a)), Some(Initialism::Expanded(b)))
        | (Some(Initialism::Expanded(b)), Some(Initialism::Short(a))) => a == b,
        _ => false,
    };
    names_agree && source.code_shapes.is_some() && source.code_shapes == target.code_shapes
}

// This is observable spelling evidence, not proof of an identifier namespace.
// Require the exact same case-sensitive leading letters AND delimiter in every
// usable value. Serial digits are not part of the prefix; free text, pure
// numbers, single-letter prefixes and mixed prefix populations do not qualify.
fn code_prefix(value: &str) -> Option<&str> {
    let bytes = value.as_bytes();
    let end = bytes
        .iter()
        .take_while(|byte| byte.is_ascii_alphabetic())
        .count();
    if end < 2 || !matches!(bytes.get(end).copied(), Some(b'-' | b'_' | b':')) {
        return None;
    }
    let suffix = &bytes[end + 1..];
    if !suffix.first().is_some_and(u8::is_ascii_alphanumeric)
        || !suffix.iter().any(u8::is_ascii_digit)
        || !suffix
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(*byte, b'-' | b'_' | b':'))
    {
        return None;
    }
    // All bytes before this boundary are ASCII, so this is a UTF-8 boundary.
    Some(&value[..end + 1])
}

fn stable_code_prefix(values: &[&str]) -> Option<String> {
    let prefix = code_prefix(values.first().copied()?)?;
    values
        .iter()
        .all(|value| code_prefix(value) == Some(prefix))
        .then(|| prefix.to_owned())
}

fn code_prefix_agrees(source: Option<&Profile>, target: Option<&Profile>) -> bool {
    let (Some(source), Some(target)) = (source, target) else {
        return false;
    };
    source.code_prefix.is_some() && source.code_prefix == target.code_prefix
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
    let mut prefix = (*first).to_owned();
    for value in &values[1..] {
        let bytes: usize = prefix
            .chars()
            .zip(value.chars())
            .take_while(|(a, b)| a == b)
            .map(|(c, _)| c.len_utf8())
            .sum();
        prefix.truncate(bytes);
        if prefix.is_empty() {
            break;
        }
    }
    prefix
}

fn meaningful_prefix(prefix: &str) -> Option<String> {
    let trimmed = prefix.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    (trimmed.chars().filter(|c| c.is_ascii_alphabetic()).count() >= 2)
        .then(|| trimmed.to_ascii_lowercase())
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

fn dominant_shape(values: &[&str]) -> Option<String> {
    let mut counts = BTreeMap::<String, usize>::new();
    for value in values {
        *counts.entry(skeleton(value)).or_default() += 1;
    }
    let (shape, count) = counts
        .into_iter()
        .max_by(|a, b| a.1.cmp(&b.1).then_with(|| b.0.cmp(&a.0)))?;
    (count >= values.len().div_ceil(2)).then_some(shape)
}

fn format_score(source: Option<&Profile>, target: Option<&Profile>) -> f64 {
    let (Some(s), Some(t)) = (source, target) else {
        return 0.0;
    };
    if (s.email && t.email) || (s.prefix.is_some() && s.prefix == t.prefix) {
        return 1.0;
    }
    if s.shape.is_some() && s.shape == t.shape {
        if s.alpha && t.alpha && s.digit && t.digit {
            return if s.structured && t.structured {
                0.90
            } else {
                0.80
            };
        }
        // Match the audited probe's conservative all-numeric tier; never admitted
        // by the fixed 0.90 floor in this integration.
        if !(s.alpha && t.alpha) && s.digit && t.digit {
            return 0.70;
        }
    }
    0.0
}

fn hard_blocked(candidate: &Candidate) -> bool {
    candidate.issues.iter().any(|issue| {
        matches!(
            issue,
            CandidateIssue::IncompatibleTypes
                | CandidateIssue::NameConflict(_)
                | CandidateIssue::SemanticConflict(_)
        )
    })
}

fn clear_margin(score: f64, other: Option<f64>) -> bool {
    other.map_or(score >= MARGIN, |other| {
        score > other && score - other >= MARGIN
    })
}

#[derive(Clone, Copy)]
struct Winner {
    target: usize,
    format: f64,
    name: f64,
    combined: f64,
    initialism: bool,
}

pub(super) fn apply(
    config: &Config,
    sources: &[&Field],
    targets: &[&Field],
    candidates: &mut [Vec<Candidate>],
    features: (&[FieldFeatures], &[FieldFeatures]),
    name_signal: usize,
    explanation_bytes: &mut usize,
) -> Result<(), MatchError> {
    if sources.is_empty() || targets.is_empty() {
        return Ok(());
    }
    // Snapshot support before adding any edge. This is deliberately stricter
    // than the additive probe: do not compete with ANY original eligible edge,
    // including an edge in an originally ambiguous row.
    let supported_rows: Vec<_> = candidates
        .iter()
        .map(|row| row.iter().any(|candidate| candidate.eligible))
        .collect();
    let mut supported_targets = vec![false; targets.len()];
    for row in candidates.iter() {
        for (j, candidate) in row.iter().enumerate() {
            supported_targets[j] |= candidate.eligible;
        }
    }
    if supported_rows.iter().all(|supported| *supported)
        || supported_targets.iter().all(|supported| *supported)
    {
        return Ok(());
    }
    let source_profiles: Vec<_> = sources.iter().map(|field| Profile::new(field)).collect();
    let target_profiles: Vec<_> = targets.iter().map(|field| Profile::new(field)).collect();
    let mut rows = vec![TopTwo::default(); sources.len()];
    let mut columns = vec![TopTwo::default(); targets.len()];
    let mut winners: Vec<Option<Winner>> = vec![None; sources.len()];
    for (i, row) in candidates.iter().enumerate() {
        for (j, candidate) in row.iter().enumerate() {
            if sources[i].data_type != DataType::Text
                || targets[j].data_type != DataType::Text
                || hard_blocked(candidate)
            {
                continue;
            }
            // Supplied by the engine's concrete built-in identity, not a
            // caller-chosen signal name. Original signals and weights are kept.
            let name = candidate.signals[name_signal].evidence.score.unwrap_or(0.0);
            let initialism =
                initialism_support(source_profiles[i].as_ref(), target_profiles[j].as_ref());
            let raw_format = format_score(source_profiles[i].as_ref(), target_profiles[j].as_ref());
            let format = if initialism {
                raw_format.max(FORMAT_FLOOR)
            } else {
                raw_format
            };
            // A complete initialism with matching code-shape sets supplies
            // derived name agreement for contrast only. Raw signal reports stay
            // unchanged. All competing expansions receive the same treatment.
            let contrast_name = if initialism { 1.0 } else { name };
            let combined = 0.75 * format + 0.25 * contrast_name;
            rows[i].insert(j, combined);
            columns[j].insert(i, combined);
            if winners[i].is_none_or(|winner| combined > winner.combined) {
                winners[i] = Some(Winner {
                    target: j,
                    name,
                    format,
                    combined,
                    initialism,
                });
            }
        }
    }
    for (i, winner) in winners.into_iter().enumerate() {
        let Some(winner) = winner else {
            continue;
        };
        let j = winner.target;
        let literal_prefix =
            code_prefix_agrees(source_profiles[i].as_ref(), target_profiles[j].as_ref());
        // A reliable exact code prefix can supplement weak names. The format
        // floor, scores, competitors and margins are unchanged; a different
        // prefix is NOT removed from the competition to manufacture certainty.
        if supported_rows[i] || supported_targets[j]
            || winner.format < FORMAT_FLOOR
            || (winner.name < NAME_FLOOR && !literal_prefix && !winner.initialism)
            || !clear_margin(winner.combined, rows[i].competing_score(j))
            || !clear_margin(winner.combined, columns[j].competing_score(i))
            // Timestamp-like Text is not an identifier-format rescue route.
            || features.0[i].temporal || features.1[j].temporal
            || !features.0[i].units_agree(&features.1[j])
        {
            continue;
        }
        let candidate = &mut candidates[i][j];
        // An explicit corroboration requirement is NOT weakened by this fallback.
        if candidate.issues.iter().any(|issue| {
            matches!(
                issue,
                CandidateIssue::InsufficientNameSupport | CandidateIssue::InsufficientSampleSupport
            )
        }) {
            continue;
        }
        let score = candidate.score.max(PROPOSAL_FLOOR);
        if score < config.min_score {
            continue;
        }
        let mut warning = format!(
            "Sample-format fallback: format {:.3}, name {:.3}, combined {:.3}; both contrast margins >= {MARGIN:.2}; distinct text {}/{}. Original row and target had no eligible edge. Fixed scores are heuristics, not confidence or proof of shared meaning.",
            winner.format, winner.name, winner.combined,
            source_profiles[i].as_ref().map_or(0, |p| p.distinct),
            target_profiles[j].as_ref().map_or(0, |p| p.distinct),
        );
        if winner.name < NAME_FLOOR && literal_prefix {
            warning.push_str(" Additional support: exact literal code prefix; prefix contents omitted. This does not establish entity scope.");
        }
        if winner.initialism {
            warning.push_str(" Additional support: whole-name initialism plus equal observed code-shape sets; contrast uses derived name agreement 1.0. Raw name signals are unchanged. Sample shapes and values omitted; this is not proof of shared meaning.");
        }
        let charged = explanation_bytes
            .checked_add(warning.len())
            .ok_or(MatchError::CountOverflow(CountKind::ExplanationBytes))?;
        if charged > config.limits.max_explanation_bytes {
            return Err(MatchError::BudgetExceeded(BudgetKind::ExplanationBytes));
        }
        *explanation_bytes = charged;
        if score > candidate.score
            && !candidate
                .issues
                .contains(&CandidateIssue::ContextualScoreAdjustment)
        {
            candidate
                .issues
                .push(CandidateIssue::ContextualScoreAdjustment);
        }
        candidate.score = score;
        candidate.issues.retain(|issue| {
            !matches!(
                issue,
                CandidateIssue::InsufficientScore | CandidateIssue::InsufficientContextSupport
            )
        });
        candidate.eligible = true;
        candidate.warnings.push(warning);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(values: &[&str]) -> Field {
        Field::new("s", "code", DataType::Text).with_samples(
            values
                .iter()
                .map(|value| SampleValue::Text((*value).to_owned()))
                .collect(),
        )
    }

    #[test]
    fn format_reliability_rejects_sentinels_two_values_and_mixed_kinds() {
        for values in [vec!["UNKNOWN"; 4], vec!["INV-001", "INV-002", "INV-001"]] {
            assert!(Profile::new(&field(&values)).is_none());
        }
        let mut mixed = field(&["INV-001", "INV-002", "INV-003"]);
        mixed
            .samples
            .as_mut()
            .unwrap()
            .push(SampleValue::Boolean(true));
        assert!(Profile::new(&mixed).is_none());
    }

    #[test]
    fn shape_classification_and_coverage_match_the_audited_rule() {
        let mut source = field(&["INV-001", "INV-002", "INV-003"]);
        let target = field(&["INV-101", "INV-102", "INV-103"]);
        assert_eq!(
            format_score(
                Profile::new(&source).as_ref(),
                Profile::new(&target).as_ref()
            ),
            0.9
        );
        source
            .samples
            .as_mut()
            .unwrap()
            .extend(vec![SampleValue::Null; 9]);
        assert!(Profile::new(&source).is_some());
        source.samples.as_mut().unwrap().push(SampleValue::Null);
        assert!(Profile::new(&source).is_none());
        let source = field(&["ABCDEFGHI", "JKLMNOPQR", "STUVWXYZA"]);
        let target = field(&["BCDEFGHIJ", "KLMNOPQRS", "TUVWXYZAB"]);
        assert_eq!(
            format_score(
                Profile::new(&source).as_ref(),
                Profile::new(&target).as_ref()
            ),
            0.0
        );
    }

    #[test]
    fn close_or_tied_competitors_never_gain_an_edge() {
        assert!(!clear_margin(0.85, Some(0.8475)));
        assert!(!clear_margin(0.9, Some(0.9)));
        assert!(!clear_margin(0.8, Some(0.9)));
        assert!(clear_margin(0.9, Some(0.6)));
    }
    #[test]
    fn literal_prefix_is_bounded_by_a_delimiter_not_a_serial_digit() {
        assert_eq!(code_prefix("INV-0001"), Some("INV-"));
        assert_eq!(code_prefix("INV-9001"), Some("INV-"));
        assert_eq!(code_prefix("AB:X9-01"), Some("AB:"));
        assert_eq!(code_prefix("ab_X901"), Some("ab_"));
        for value in [
            "",
            "1",
            "C-001",
            "INV001",
            "INV-ABC",
            "INV-",
            "Invoice 001",
            " INV-001",
            "INV-001@host",
            "\u{00c9}T-001",
        ] {
            assert_eq!(code_prefix(value), None, "{value}");
        }
        assert_eq!(
            stable_code_prefix(&["INV-001", "INV-002", "INV-003"]),
            Some("INV-".into())
        );
        assert_eq!(stable_code_prefix(&["INV-001", "OTH-002", "INV-003"]), None);
        assert_eq!(stable_code_prefix(&["INV-001", "inv-002", "INV-003"]), None);
        assert_eq!(stable_code_prefix(&["INV-001", "INV_002", "INV-003"]), None);
    }
}
