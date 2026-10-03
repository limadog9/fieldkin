//! Built-in, deterministic name, declared-type and sampled-value signals.

use std::collections::{BTreeMap, BTreeSet};

use unicode_normalization::{char::is_combining_mark, UnicodeNormalization};

use crate::{DataType, Evidence, Field, Matcher, SampleValue};

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
            aliases: BTreeMap::from([
                ("amt".to_owned(), "amount".to_owned()),
                ("trans".to_owned(), "transaction".to_owned()),
            ]),
        }
    }
}

impl NameMatcher {
    fn tokens(&self, name: &str) -> Result<(Vec<String>, usize), String> {
        let mut tokens = normalize_name(name);
        let mut substitutions = 0;
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
        Ok((tokens, substitutions))
    }
}

impl Matcher for NameMatcher {
    fn name(&self) -> &str {
        "name"
    }

    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        let (source_tokens, source_aliases) = self.tokens(&source.name)?;
        let (target_tokens, target_aliases) = self.tokens(&target.name)?;
        if source_tokens.is_empty() || target_tokens.is_empty() {
            return Ok(Evidence {
                score: None,
                explanation: "At least one name has no alphanumeric tokens".to_owned(),
            });
        }
        let source_set: BTreeSet<&str> = source_tokens.iter().map(String::as_str).collect();
        let target_set: BTreeSet<&str> = target_tokens.iter().map(String::as_str).collect();
        let overlap = source_set.intersection(&target_set).count();
        let union = source_set.len() + target_set.len() - overlap;
        let token_score = overlap as f64 / union as f64;
        let character_score =
            strsim::jaro_winkler(&source_tokens.join(" "), &target_tokens.join(" "));
        let score = if source_tokens == target_tokens {
            1.0
        } else {
            0.65 * token_score + 0.35 * character_score
        };
        Ok(Evidence {
            score: Some(score),
            explanation: format!(
                "Token Jaccard {token_score:.3} (weight 0.65); Jaro-Winkler \
                 {character_score:.3} (weight 0.35); {} alias substitutions. \
                 Lexical agreement does not establish semantic equivalence",
                source_aliases + target_aliases,
            ),
        })
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

/// Exact typed-value set overlap, attenuated by the lower non-null coverage.
///
/// Text is case-sensitive and untrimmed. Numeric `-0.0` equals `0.0`; no coercion
/// between numbers, booleans and text is performed. Repetition affects sample
/// sufficiency and coverage but not set overlap. Shared values can be misleading
/// for low-cardinality fields; absent overlap does not prove unrelated meaning.
#[derive(Clone, Copy, Debug)]
pub struct SampleMatcher {
    /// Required non-null observations on each side, including repetitions.
    /// Must be positive. The default is three; this is not a statistical guarantee.
    pub min_non_null: usize,
}

impl Default for SampleMatcher {
    fn default() -> Self {
        Self { min_non_null: 3 }
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum SampleKey<'a> {
    Boolean(bool),
    Number(u64),
    Text(&'a str),
}

fn sample_set(samples: &[SampleValue]) -> Result<(BTreeSet<SampleKey<'_>>, usize), String> {
    let mut values = BTreeSet::new();
    let mut non_null = 0;
    for sample in samples {
        let key = match sample {
            SampleValue::Null => continue,
            SampleValue::Boolean(value) => SampleKey::Boolean(*value),
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

impl Matcher for SampleMatcher {
    fn name(&self) -> &str {
        "samples"
    }

    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        if self.min_non_null == 0 {
            return Err("SampleMatcher.min_non_null must be positive".to_owned());
        }
        let (Some(source_samples), Some(target_samples)) = (&source.samples, &target.samples)
        else {
            return Ok(Evidence {
                score: None,
                explanation: "At least one field has no sample data available".to_owned(),
            });
        };
        if source_samples.is_empty() || target_samples.is_empty() {
            return Ok(Evidence {
                score: None,
                explanation: format!(
                    "Observed empty sample: source {} and target {} observations",
                    source_samples.len(),
                    target_samples.len(),
                ),
            });
        }
        let (source_values, source_non_null) = sample_set(source_samples)?;
        let (target_values, target_non_null) = sample_set(target_samples)?;
        if source_non_null < self.min_non_null || target_non_null < self.min_non_null {
            return Ok(Evidence {
                score: None,
                explanation: format!(
                    "Insufficient non-null samples: source {source_non_null}, \
                     target {target_non_null}; requires {} per field",
                    self.min_non_null,
                ),
            });
        }
        let overlap = source_values.intersection(&target_values).count();
        let union = source_values.len() + target_values.len() - overlap;
        let jaccard = overlap as f64 / union as f64;
        let coverage = (source_non_null as f64 / source_samples.len() as f64)
            .min(target_non_null as f64 / target_samples.len() as f64);
        Ok(Evidence {
            score: Some(jaccard * coverage),
            explanation: format!(
                "Exact typed-value Jaccard {jaccard:.3}; lower non-null coverage \
                 {coverage:.3}; source {source_non_null}/{} and target \
                 {target_non_null}/{} non-null samples; {overlap}/{union} shared/union \
                 distinct values. Sample overlap does not establish field meaning",
                source_samples.len(),
                target_samples.len(),
            ),
        })
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
        let value = SampleValue::Text("private sample value".to_owned());
        let full = field("").with_samples(vec![value.clone(); 3]);
        let mut sparse_values = vec![SampleValue::Null; 27];
        sparse_values.extend(vec![value; 3]);
        let sparse = field("").with_samples(sparse_values);
        let matcher = SampleMatcher::default();
        assert_eq!(matcher.evaluate(&full, &full).unwrap().score, Some(1.0));
        let evidence = matcher.evaluate(&full, &sparse).unwrap();
        assert!((evidence.score.unwrap() - 0.1).abs() < f64::EPSILON);
        assert!(!evidence.explanation.contains("private sample value"));
    }

    #[test]
    fn samples_compare_typed_values_without_coercion_and_canonicalize_zero() {
        let matcher = SampleMatcher::default();
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
        assert!(SampleMatcher { min_non_null: 0 }
            .evaluate(&valid, &valid)
            .is_err());
    }
}
