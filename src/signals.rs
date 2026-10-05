//! Built-in, deterministic name, declared-type and sampled-value signals.

use std::collections::{BTreeMap, BTreeSet};

use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

use crate::semantic_name::{normalize_word_forms, NameScope, DEFAULT_ALIASES};
use crate::{DataType, Evidence, Field, InputError, MatchError, Matcher, SampleValue};

/// Split a name at separators, case/acronym boundaries and letter-digit boundaries.
///
/// Unicode NFKC normalization happens before tokenization and Unicode lowercasing.
/// For example, `SKUCode_2024` becomes `["sku", "code", "2024"]`. Punctuation and
/// whitespace separate tokens. This function does not expand abbreviations, remove
/// accents, stem words or translate names.
pub fn normalize_name(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.nfkc().collect();
    let mut tokens = Vec::new();
    let mut token = String::new();
    for (index, &current) in chars.iter().enumerate() {
        if !current.is_alphanumeric() && !is_combining_mark(current) {
            if !token.is_empty() {
                tokens.push(std::mem::take(&mut token));
            }
            continue;
        }
        let previous = index.checked_sub(1).and_then(|i| chars.get(i)).copied();
        let next = chars.get(index + 1).copied();
        let boundary = previous.is_some_and(|previous| {
            (previous.is_lowercase() && current.is_uppercase())
                || (previous.is_uppercase()
                    && current.is_uppercase()
                    && next.is_some_and(char::is_lowercase))
                || (previous.is_alphabetic() && current.is_numeric())
                || (previous.is_numeric() && current.is_alphabetic())
        });
        if boundary && !token.is_empty() {
            tokens.push(std::mem::take(&mut token));
        }
        // A leading combining mark has no useful name evidence on its own.
        if !is_combining_mark(current) || !token.is_empty() {
            token.extend(current.to_lowercase());
        }
    }
    if !token.is_empty() {
        tokens.push(token);
    }
    tokens
}

/// Name similarity combining token overlap (65%) and Jaro-Winkler similarity (35%).
///
/// The two default aliases are `amt` → `amount` and `trans` → `transaction`.
/// They are conventions, not semantic guarantees. In particular, names do not
/// establish whether a monetary field is gross, net, taxable or in the right currency.
/// Names exceeding 1024 bytes after normalization and alias expansion (including
/// one space between tokens) return an error before string similarity is computed.
/// An engine with contextual evidence enabled also applies a small shared set of
/// event word forms in declared or sampled Boolean/temporal contexts. Direct
/// evaluation and engines with the default configuration retain ordinary names.
#[derive(Clone, Debug)]
pub struct NameMatcher {
    /// One-pass token substitutions, applied after normalization. Keys and values
    /// must each be a single normalized token. Used replacement values are validated
    /// and limited to 256 bytes. No recursive expansion is performed.
    /// Clear this map to disable all aliases; add domain aliases deliberately.
    pub aliases: BTreeMap<String, String>,
}

impl Default for NameMatcher {
    fn default() -> Self {
        Self {
            aliases: DEFAULT_ALIASES
                .into_iter()
                .map(|(from, to)| (from.to_owned(), to.to_owned()))
                .collect(),
        }
    }
}

pub(crate) struct PreparedName {
    value: Result<NameTokens, String>,
}

struct NameTokens {
    distinct: Vec<String>,
    joined: String,
    substitutions: usize,
    applied_aliases: BTreeSet<String>,
    word_forms: BTreeSet<String>,
}

impl NameMatcher {
    /// Add a caller-verified token alias after validating both sides. Each side
    /// must be one already-normalized token of at most 256 bytes. Aliases apply
    /// to every occurrence; they do not establish units or identifier scope.
    pub fn with_alias(
        mut self,
        from: impl Into<String>,
        to: impl Into<String>,
    ) -> Result<Self, MatchError> {
        let from = from.into();
        let to = to.into();
        for token in [&from, &to] {
            if token.len() > 256 || normalize_name(token).as_slice() != [token.as_str()] {
                return Err(MatchError::InvalidInput(InputError::Alias));
            }
        }
        self.aliases.insert(from, to);
        Ok(self)
    }
    fn tokens(&self, name: &str) -> Result<(Vec<String>, usize, BTreeSet<String>), String> {
        let mut tokens = normalize_name(name);
        let mut substitutions = 0;
        let mut applied_aliases = BTreeSet::new();
        let mut expanded_bytes = 0usize;
        for (index, token) in tokens.iter_mut().enumerate() {
            if let Some(replacement) = self.aliases.get(token) {
                if replacement.len() > 256
                    || normalize_name(replacement).as_slice() != [replacement.as_str()]
                {
                    return Err(
                        "NameMatcher alias targets must be single normalized tokens of at most 256 bytes"
                            .to_owned(),
                    );
                }
                if replacement != token {
                    applied_aliases.insert(format!("{token} -> {replacement}"));
                    *token = replacement.clone();
                    substitutions += 1;
                }
            }
            // Saturation also handles direct calls with unvalidated input names.
            expanded_bytes = expanded_bytes
                .saturating_add(token.len())
                .saturating_add(usize::from(index > 0));
            if expanded_bytes > 1024 {
                return Err(
                    "Normalized, alias-expanded names must be at most 1024 bytes".to_owned(),
                );
            }
        }
        Ok((tokens, substitutions, applied_aliases))
    }

    pub(crate) fn prepare(&self, name: &str) -> PreparedName {
        self.prepare_scoped(name, None)
    }

    pub(crate) fn prepare_contextual(&self, field: &Field) -> PreparedName {
        self.prepare_scoped(&field.name, Some(NameScope::for_field(field)))
    }

    fn prepare_scoped(&self, name: &str, scope: Option<NameScope>) -> PreparedName {
        PreparedName {
            value: self
                .tokens(name)
                .and_then(|(mut tokens, substitutions, applied_aliases)| {
                    let word_forms = scope
                        .map(|scope| normalize_word_forms(&mut tokens, scope))
                        .unwrap_or_default();
                    // Keep original token order for character similarity and exact
                    // equality, and a sorted set for allocation-free intersections.
                    let joined = tokens.join(" ");
                    if joined.len() > 1024 {
                        return Err(
                            "Normalized, alias-expanded names must be at most 1024 bytes"
                                .to_owned(),
                        );
                    }
                    tokens.sort_unstable();
                    tokens.dedup();
                    Ok(NameTokens {
                        distinct: tokens,
                        joined,
                        substitutions,
                        applied_aliases,
                        word_forms,
                    })
                }),
        }
    }

    pub(crate) fn evaluate_prepared(
        &self,
        source: &PreparedName,
        target: &PreparedName,
    ) -> Result<Evidence, String> {
        let source = source.value.as_ref().map_err(Clone::clone)?;
        let target = target.value.as_ref().map_err(Clone::clone)?;
        if source.distinct.is_empty() || target.distinct.is_empty() {
            return Ok(Evidence {
                score: None,
                explanation: "At least one name has no alphanumeric tokens".to_owned(),
            });
        }
        let mut source_tokens = source.distinct.iter().peekable();
        let mut target_tokens = target.distinct.iter().peekable();
        let mut overlap = 0;
        while let (Some(source), Some(target)) = (source_tokens.peek(), target_tokens.peek()) {
            match source.cmp(target) {
                std::cmp::Ordering::Less => {
                    source_tokens.next();
                }
                std::cmp::Ordering::Greater => {
                    target_tokens.next();
                }
                std::cmp::Ordering::Equal => {
                    overlap += 1;
                    source_tokens.next();
                    target_tokens.next();
                }
            }
        }
        let union = source.distinct.len() + target.distinct.len() - overlap;
        let token_score = overlap as f64 / union as f64;
        let character_score = strsim::jaro_winkler(&source.joined, &target.joined);
        let score = if source.joined == target.joined {
            1.0
        } else {
            0.65 * token_score + 0.35 * character_score
        };
        let mut explanation = format!(
            "Token Jaccard {token_score:.3} (weight 0.65); Jaro-Winkler \
                 {character_score:.3} (weight 0.35); {} alias substitutions. \
                 Lexical agreement does not establish semantic equivalence",
            source.substitutions + target.substitutions,
        );
        let aliases: BTreeSet<_> = source
            .applied_aliases
            .iter()
            .chain(&target.applied_aliases)
            .collect();
        if !aliases.is_empty() {
            explanation.push_str(". Applied token aliases: ");
            for (index, alias) in aliases.into_iter().enumerate() {
                if explanation.len() + alias.len() > 2800 {
                    explanation
                        .push_str("; further alias details omitted to bound explanation size");
                    break;
                }
                if index > 0 {
                    explanation.push_str("; ");
                }
                explanation.push_str(alias);
            }
        }
        let word_forms: BTreeSet<_> = source.word_forms.iter().chain(&target.word_forms).collect();
        if !word_forms.is_empty() {
            explanation.push_str(". Contextual word forms: ");
            for (index, form) in word_forms.into_iter().enumerate() {
                if index > 0 {
                    explanation.push_str("; ");
                }
                explanation.push_str(form);
            }
        }
        Ok(Evidence {
            score: Some(score),
            explanation,
        })
    }
}

impl Matcher for NameMatcher {
    fn name(&self) -> &str {
        "name"
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        self.evaluate_prepared(&self.prepare(&source.name), &self.prepare(&target.name))
    }
}

/// Compatibility of declared logical types; no value conversion is attempted.
#[derive(Clone, Copy, Debug, Default)]
pub struct TypeMatcher;

pub(crate) fn type_compatibility(source: DataType, target: DataType) -> Option<f64> {
    use DataType::{Date, Decimal, Float, Integer, Timestamp, Unknown};
    match (source, target) {
        (Unknown, _) | (_, Unknown) => None,
        (source, target) if source == target => Some(1.0),
        (Integer | Float | Decimal, Integer | Float | Decimal) => Some(0.85),
        (Date, Timestamp) | (Timestamp, Date) => Some(0.7),
        _ => Some(0.0),
    }
}

impl Matcher for TypeMatcher {
    fn name(&self) -> &str {
        "type"
    }

    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        let score = type_compatibility(source.data_type, target.data_type);
        let explanation = match score {
            None => "At least one declared type is unknown",
            Some(0.0) => "Known declared types are incompatible",
            Some(1.0) => "Declared logical types agree; this does not establish field meaning",
            Some(_) => "Related declared logical types; lossless conversion is not guaranteed",
        };
        Ok(Evidence {
            score,
            explanation: explanation.to_owned(),
        })
    }
}

/// Reliability policy applied to exact sample overlap.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SampleReliability {
    /// Stage 1 behavior: observation sufficiency and null coverage only.
    /// Repeated constants can produce full agreement; use for historical comparisons.
    Legacy,
    /// Attenuate by `min((lower distinct count - 1) / 2, 1)` in addition to
    /// observation sufficiency and coverage. Constants score zero; two-value
    /// columns receive half weight; three or more distinct values receive full
    /// weight. This is a conservative heuristic, not a reliability probability.
    #[default]
    Distinct,
}

/// Exact typed-value set overlap, attenuated by null coverage and distinct support.
///
/// Text is case-sensitive and untrimmed. Numeric `-0.0` equals `0.0`; no coercion
/// between numeric kinds, booleans and text is performed. Repetition affects sample
/// sufficiency and coverage but cannot increase distinct support. Shared values can be misleading
/// for low-cardinality fields; absent overlap does not prove unrelated meaning.
#[derive(Clone, Copy, Debug)]
pub struct SampleMatcher {
    /// Required non-null observations on each side, including repetitions.
    /// Must be positive. The default is three; this is not a statistical guarantee.
    pub min_non_null: usize,
    /// Low-cardinality attenuation. The default is `Distinct`; `Legacy` is available
    /// for reproducing the original heuristic, with its constant-column limitation.
    pub reliability: SampleReliability,
}

impl Default for SampleMatcher {
    fn default() -> Self {
        Self {
            min_non_null: 3,
            reliability: SampleReliability::Distinct,
        }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum SampleKey<'a> {
    Boolean(bool),
    Number(u64),
    Integer(i128),
    Decimal(i128, u8),
    Text(&'a str),
}

fn sample_set(samples: &[SampleValue]) -> Result<(BTreeSet<SampleKey<'_>>, usize), String> {
    let mut values = BTreeSet::new();
    let mut non_null = 0;
    for sample in samples {
        let key = match sample {
            SampleValue::Null => continue,
            SampleValue::Boolean(value) => SampleKey::Boolean(*value),
            SampleValue::Integer(value) => SampleKey::Integer(*value),
            SampleValue::Decimal(value) => SampleKey::Decimal(value.coefficient(), value.scale()),
            SampleValue::Number(value) => {
                if !value.is_finite() {
                    return Err("Sample contains a non-finite number".to_owned());
                }
                SampleKey::Number(if *value == 0.0 { 0 } else { value.to_bits() })
            }
            SampleValue::Text(value) => SampleKey::Text(value),
        };
        non_null += 1;
        values.insert(key);
    }
    Ok((values, non_null))
}

// Deliberately no Debug: prepared keys borrow the caller's sample values.
pub(crate) struct PreparedSamples<'a> {
    observations: Option<usize>,
    values: Result<(BTreeSet<SampleKey<'a>>, usize), String>,
}

impl SampleMatcher {
    pub(crate) fn prepare<'a>(&self, field: &'a Field) -> PreparedSamples<'a> {
        PreparedSamples {
            observations: field.samples.as_ref().map(Vec::len),
            values: sample_set(field.samples.as_deref().unwrap_or_default()),
        }
    }

    fn absent_evidence(
        &self,
        source_observations: Option<usize>,
        target_observations: Option<usize>,
    ) -> Result<Option<Evidence>, String> {
        if self.min_non_null == 0 {
            return Err("SampleMatcher.min_non_null must be positive".to_owned());
        }
        let (Some(source_observations), Some(target_observations)) =
            (source_observations, target_observations)
        else {
            return Ok(Some(Evidence {
                score: None,
                explanation: "At least one field has no sample data available".to_owned(),
            }));
        };
        if source_observations == 0 || target_observations == 0 {
            return Ok(Some(Evidence {
                score: None,
                explanation: format!(
                    "Observed empty sample: source {source_observations} and target \
                     {target_observations} observations",
                ),
            }));
        }
        Ok(None)
    }

    pub(crate) fn evaluate_prepared(
        &self,
        source: &PreparedSamples<'_>,
        target: &PreparedSamples<'_>,
    ) -> Result<Evidence, String> {
        if let Some(evidence) = self.absent_evidence(source.observations, target.observations)? {
            return Ok(evidence);
        }
        let (source_values, source_non_null) = source.values.as_ref().map_err(Clone::clone)?;
        let (target_values, target_non_null) = target.values.as_ref().map_err(Clone::clone)?;
        if *source_non_null < self.min_non_null || *target_non_null < self.min_non_null {
            return Ok(Evidence {
                score: None,
                explanation: format!(
                    "Insufficient non-null samples: source {source_non_null}, \
                     target {target_non_null}; requires {} per field",
                    self.min_non_null,
                ),
            });
        }
        let overlap = source_values.intersection(target_values).count();
        let union = source_values.len() + target_values.len() - overlap;
        let jaccard = overlap as f64 / union as f64;
        // Missing and empty observations returned above. The values are always
        // present and positive here, without requiring input-triggered panics.
        let source_observations = source.observations.unwrap_or_default();
        let target_observations = target.observations.unwrap_or_default();
        let coverage = (*source_non_null as f64 / source_observations as f64)
            .min(*target_non_null as f64 / target_observations as f64);
        if self.reliability == SampleReliability::Distinct {
            let support = (source_values
                .len()
                .min(target_values.len())
                .saturating_sub(1) as f64
                / 2.0)
                .min(1.0);
            return Ok(Evidence {
                score: Some(jaccard * coverage * support),
                explanation: format!(
                    "Exact typed-value Jaccard {jaccard:.3}; lower non-null coverage {coverage:.3}; distinct support {support:.3}; source {source_non_null}/{source_observations} and target {target_non_null}/{target_observations} non-null observations; source {} and target {} distinct; {overlap}/{union} shared/union distinct values. Constants and low-cardinality samples are attenuated; overlap does not establish meaning",
                    source_values.len(), target_values.len(),
                ),
            });
        }
        Ok(Evidence {
            score: Some(jaccard * coverage),
            explanation: format!(
                "Exact typed-value Jaccard {jaccard:.3}; lower non-null coverage \
                 {coverage:.3}; source {source_non_null}/{} and target \
                 {target_non_null}/{} non-null samples; {overlap}/{union} shared/union \
                 distinct values. Sample overlap does not establish field meaning",
                source_observations, target_observations,
            ),
        })
    }
}

impl Matcher for SampleMatcher {
    fn name(&self) -> &str {
        "samples"
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        if let Some(evidence) = self.absent_evidence(
            source.samples.as_ref().map(Vec::len),
            target.samples.as_ref().map(Vec::len),
        )? {
            return Ok(evidence);
        }
        self.evaluate_prepared(&self.prepare(source), &self.prepare(target))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field(name: &str) -> Field {
        Field::new("id", name, DataType::Text)
    }

    #[test]
    fn normalization_handles_case_acronyms_digits_and_unicode() {
        assert_eq!(
            normalize_name("SKUCode_2024-v2"),
            ["sku", "code", "2024", "v", "2"]
        );
        assert_eq!(normalize_name("XMLHttpRequest"), ["xml", "http", "request"]);
        assert_eq!(normalize_name("GrossAmt"), ["gross", "amt"]);
        assert_eq!(
            normalize_name("  Ｆｉｅｌｄ＿Ｃｏｄｅ  "),
            ["field", "code"]
        );
        assert_eq!(normalize_name("CAFÉ_Code"), ["café", "code"]);
        assert_eq!(normalize_name("Cafe\u{301}_Code"), ["café", "code"]);
        assert!(normalize_name("---_ \u{301}").is_empty());
    }

    #[test]
    fn conventional_aliases_can_be_disabled() {
        let source = field("TransDate");
        let target = field("transaction_date");
        let mut matcher = NameMatcher::default();
        assert_eq!(matcher.evaluate(&source, &target).unwrap().score, Some(1.0));
        matcher.aliases.clear();
        assert!(matcher.evaluate(&source, &target).unwrap().score.unwrap() < 1.0);
    }

    #[test]
    fn invalid_used_aliases_return_errors() {
        let mut matcher = NameMatcher::default();
        for replacement in ["", "two tokens", "UPPER", &"a".repeat(257)] {
            matcher
                .aliases
                .insert("amt".to_owned(), replacement.to_owned());
            assert!(matcher.evaluate(&field("amt"), &field("amount")).is_err());
        }
    }

    #[test]
    fn expanded_names_are_bounded_before_similarity() {
        let mut matcher = NameMatcher::default();
        matcher.aliases.insert("a".to_owned(), "b".repeat(256));
        assert!(matcher.evaluate(&field("a_a_a"), &field("target")).is_ok());
        assert!(matcher
            .evaluate(&field("a_a_a_a"), &field("target"))
            .is_err());
        assert!(matcher
            .evaluate(&field(&"b".repeat(1024)), &field("target"))
            .is_ok());
        assert!(matcher
            .evaluate(&field(&"b".repeat(1025)), &field("target"))
            .is_err());
    }

    #[test]
    fn names_empty_or_with_extra_qualifiers_do_not_match_exactly() {
        let matcher = NameMatcher::default();
        assert_eq!(
            matcher.evaluate(&field(""), &field("---")).unwrap().score,
            None
        );
        let score = matcher
            .evaluate(&field("GrossAmt"), &field("amount"))
            .unwrap()
            .score
            .unwrap();
        assert!(score > 0.0 && score < 0.7);
    }

    #[test]
    fn type_relations_distinguish_missing_from_incompatible() {
        assert_eq!(type_compatibility(DataType::Unknown, DataType::Text), None);
        assert_eq!(
            type_compatibility(DataType::Integer, DataType::Decimal),
            Some(0.85)
        );
        assert_eq!(
            type_compatibility(DataType::Date, DataType::Timestamp),
            Some(0.7)
        );
        assert_eq!(
            type_compatibility(DataType::Boolean, DataType::Integer),
            Some(0.0)
        );
    }

    #[test]
    fn samples_missing_empty_and_insufficient_are_absent_evidence() {
        let matcher = SampleMatcher::default();
        let enough = field("").with_samples(vec![SampleValue::Number(1.0); 3]);
        for missing in [
            field(""),
            field("").with_samples(vec![]),
            field("").with_samples(vec![SampleValue::Null; 10]),
            field("").with_samples(vec![SampleValue::Number(1.0); 2]),
        ] {
            assert_eq!(matcher.evaluate(&missing, &enough).unwrap().score, None);
        }
        let missing = matcher.evaluate(&field(""), &enough).unwrap();
        let empty = matcher
            .evaluate(&field("").with_samples(vec![]), &enough)
            .unwrap();
        let nulls = matcher
            .evaluate(&field("").with_samples(vec![SampleValue::Null; 3]), &enough)
            .unwrap();
        assert!(missing.explanation.contains("available"));
        assert!(empty.explanation.contains("Observed empty sample"));
        assert!(nulls.explanation.contains("Insufficient non-null samples"));
    }

    #[test]
    fn samples_attenuate_null_heavy_data_without_exposing_values() {
        let values: Vec<_> = (0..3)
            .map(|i| SampleValue::Text(format!("private sample value{i}")))
            .collect();
        let full = field("").with_samples(values.clone());
        let mut sparse_values = vec![SampleValue::Null; 27];
        sparse_values.extend(values);
        let sparse = field("").with_samples(sparse_values);
        let matcher = SampleMatcher::default();
        assert_eq!(matcher.evaluate(&full, &full).unwrap().score, Some(1.0));
        let evidence = matcher.evaluate(&full, &sparse).unwrap();
        assert!((evidence.score.unwrap() - 0.1).abs() < f64::EPSILON);
        assert!(!evidence.explanation.contains("private sample value"));
    }

    #[test]
    fn samples_compare_typed_values_without_coercion_and_canonicalize_zero() {
        let matcher = SampleMatcher {
            reliability: SampleReliability::Legacy,
            ..SampleMatcher::default()
        };
        let numbers = field("").with_samples(vec![SampleValue::Number(0.0); 3]);
        let negative_zero = field("").with_samples(vec![SampleValue::Number(-0.0); 3]);
        let text = field("").with_samples(vec![SampleValue::Text("0".to_owned()); 3]);
        assert_eq!(
            matcher.evaluate(&numbers, &negative_zero).unwrap().score,
            Some(1.0)
        );
        assert_eq!(matcher.evaluate(&numbers, &text).unwrap().score, Some(0.0));
        let upper = field("").with_samples(vec![SampleValue::Text("A".to_owned()); 3]);
        let lower = field("").with_samples(vec![SampleValue::Text("a".to_owned()); 3]);
        assert_eq!(matcher.evaluate(&upper, &lower).unwrap().score, Some(0.0));
    }

    #[test]
    fn direct_sample_calls_reject_invalid_configuration_and_non_finite_numbers() {
        let invalid = field("").with_samples(vec![SampleValue::Number(f64::NAN); 3]);
        let valid = field("").with_samples(vec![SampleValue::Number(1.0); 3]);
        assert!(SampleMatcher::default().evaluate(&invalid, &valid).is_err());
        assert!(SampleMatcher {
            min_non_null: 0,
            ..SampleMatcher::default()
        }
        .evaluate(&valid, &valid)
        .is_err());
    }

    #[test]
    fn prepared_names_preserve_order_duplicates_unicode_and_alias_errors() {
        let mut matcher = NameMatcher::default();
        matcher
            .aliases
            .insert("invalid".to_owned(), "two tokens".to_owned());
        let names = [
            "TransDate",
            "transaction_date",
            "date_transaction",
            "transaction_transaction_date",
            "CAFÉ_Code",
            "Cafe\u{301}_Code",
            "SKUCode_2024",
            "sku-code-2024",
            "",
            "---",
            "invalid",
        ];
        for source in names {
            for target in names {
                let prepared =
                    matcher.evaluate_prepared(&matcher.prepare(source), &matcher.prepare(target));
                assert_eq!(prepared, matcher.evaluate(&field(source), &field(target)));
                // Check the cached representation against the original ordered
                // tokens and set semantics, including repeated and reordered tokens.
                if source != "invalid" && target != "invalid" {
                    let (source_tokens, _, _) = matcher.tokens(source).unwrap();
                    let (target_tokens, _, _) = matcher.tokens(target).unwrap();
                    let expected = if source_tokens.is_empty() || target_tokens.is_empty() {
                        None
                    } else if source_tokens == target_tokens {
                        Some(1.0)
                    } else {
                        let source_set: BTreeSet<_> = source_tokens.iter().collect();
                        let target_set: BTreeSet<_> = target_tokens.iter().collect();
                        let overlap = source_set.intersection(&target_set).count();
                        let union = source_set.union(&target_set).count();
                        Some(
                            0.65 * (overlap as f64 / union as f64)
                                + 0.35
                                    * strsim::jaro_winkler(
                                        &source_tokens.join(" "),
                                        &target_tokens.join(" "),
                                    ),
                        )
                    };
                    assert_eq!(prepared.unwrap().score, expected);
                } else {
                    // Alias errors precede an otherwise absent empty-name signal.
                    assert!(prepared.unwrap_err().contains("alias targets"));
                }
            }
        }
        let oversized = "a".repeat(1025);
        let result =
            matcher.evaluate_prepared(&matcher.prepare(&oversized), &matcher.prepare("invalid"));
        assert_eq!(
            result.unwrap_err(),
            "Normalized, alias-expanded names must be at most 1024 bytes",
        );
    }

    #[test]
    fn prepared_samples_preserve_absence_and_invalid_input_precedence() {
        let cases = [
            field(""),
            field("").with_samples(vec![]),
            field("").with_samples(vec![SampleValue::Null; 5]),
            field("").with_samples(vec![SampleValue::Number(1.0)]),
            field("").with_samples(vec![SampleValue::Number(0.0); 3]),
            field("").with_samples(vec![SampleValue::Number(-0.0); 3]),
            field("").with_samples(vec![SampleValue::Text("0".to_owned()); 3]),
            field("").with_samples(vec![SampleValue::Boolean(false); 3]),
            field("").with_samples(vec![SampleValue::Number(f64::NAN)]),
            field("").with_samples(vec![SampleValue::Number(f64::INFINITY)]),
            field("").with_samples(vec![SampleValue::Number(f64::NEG_INFINITY)]),
        ];
        for min_non_null in [0, 3] {
            let matcher = SampleMatcher {
                min_non_null,
                ..SampleMatcher::default()
            };
            for source in &cases {
                for target in &cases {
                    let prepared = matcher
                        .evaluate_prepared(&matcher.prepare(source), &matcher.prepare(target));
                    assert_eq!(prepared, matcher.evaluate(source, target));
                    if min_non_null == 0 {
                        assert_eq!(
                            prepared.unwrap_err(),
                            "SampleMatcher.min_non_null must be positive"
                        );
                    } else if source.samples.is_none() || target.samples.is_none() {
                        assert_eq!(
                            prepared.unwrap(),
                            Evidence {
                                score: None,
                                explanation: "At least one field has no sample data available"
                                    .to_owned(),
                            }
                        );
                    } else if source.samples.as_ref().is_some_and(Vec::is_empty)
                        || target.samples.as_ref().is_some_and(Vec::is_empty)
                    {
                        let evidence = prepared.unwrap();
                        assert_eq!(evidence.score, None);
                        assert!(evidence.explanation.starts_with("Observed empty sample:"));
                    } else if source.samples.iter().chain(&target.samples).flatten().any(
                        |value| matches!(value, SampleValue::Number(number) if !number.is_finite()),
                    ) {
                        assert_eq!(prepared.unwrap_err(), "Sample contains a non-finite number");
                    }
                }
            }
        }
    }

    #[test]
    #[ignore = "manual preparation cost measurement"]
    fn prepare_original_128_cost() {
        use std::{hint::black_box, time::Instant};

        let fields: Vec<_> = (0..2)
            .flat_map(|side| {
                (0..128).map(move |index| {
                    let name = if side == 0 {
                        format!("Metric{index:04}Count")
                    } else {
                        format!("metric_{index:04}_count")
                    };
                    Field::new(format!("{side}-{index}"), name, DataType::Integer).with_samples(
                        (0..16)
                            .map(|sample| {
                                if sample % 5 == 0 {
                                    SampleValue::Null
                                } else {
                                    SampleValue::Number(f64::from(index * 100 + sample))
                                }
                            })
                            .collect(),
                    )
                })
            })
            .collect();
        let names = NameMatcher::default();
        let samples = SampleMatcher::default();
        let prepare = || {
            black_box(
                fields
                    .iter()
                    .map(|field| names.prepare(&field.name))
                    .collect::<Vec<_>>(),
            );
            black_box(
                fields
                    .iter()
                    .map(|field| samples.prepare(field))
                    .collect::<Vec<_>>(),
            );
        };
        for _ in 0..5 {
            prepare();
        }
        for round in 1..=5 {
            let started = Instant::now();
            for _ in 0..100 {
                prepare();
            }
            println!(
                "preparation_original_128 round={round} microseconds_per_call={:.3}",
                started.elapsed().as_secs_f64() * 1_000_000.0 / 100.0
            );
        }
    }
}
