//! Small, evidence-scoped word-form normalization shared by contextual names.

use std::collections::BTreeSet;

use crate::{DataType, Field, SampleValue};

// Keep the ordinary name matcher's public defaults unchanged. Contextual cores
// use this same vocabulary rather than independently duplicating aliases.
pub(crate) const DEFAULT_ALIASES: [(&str, &str); 2] = [("amt", "amount"), ("trans", "transaction")];

#[derive(Clone, Copy, Default)]
pub(crate) struct NameScope {
    pub boolean: bool,
    pub temporal: bool,
}

impl NameScope {
    pub(crate) fn for_field(field: &Field) -> Self {
        let samples = field.samples.as_deref().unwrap_or_default();
        let observed = samples
            .iter()
            .any(|value| !matches!(value, SampleValue::Null));
        Self {
            boolean: field.data_type == DataType::Boolean
                || (observed
                    && samples
                        .iter()
                        .all(|value| matches!(value, SampleValue::Null | SampleValue::Boolean(_)))),
            temporal: matches!(field.data_type, DataType::Date | DataType::Timestamp)
                || (observed
                    && samples.iter().all(|value| match value {
                        SampleValue::Null => true,
                        SampleValue::Text(text) => temporal_text(text),
                        _ => false,
                    })),
        }
    }
}

// These are word forms of the same event, not a general-purpose stemmer or a
// full-field alias table. The scope prevents, for example, reversing a numeric
// amount or treating a settlement document as a settled Boolean state.
pub(crate) fn normalize_word_forms(tokens: &mut [String], scope: NameScope) -> BTreeSet<String> {
    let mut applied = BTreeSet::new();
    for token in tokens {
        let canonical = match token.as_str() {
            "expiry" if scope.temporal => "expiration",
            "settled" if scope.temporal || scope.boolean => "settlement",
            "reversed" if scope.temporal || scope.boolean => "reversal",
            _ => continue,
        };
        applied.insert(format!("{token} -> {canonical}"));
        *token = canonical.to_owned();
    }
    applied
}

pub(crate) fn identifier_token(token: &str) -> bool {
    matches!(
        token,
        "id" | "identifier" | "key" | "code" | "ref" | "reference" | "num" | "number"
    )
}

pub(crate) fn structural_token(token: &str, index: usize, length: usize, scope: NameScope) -> bool {
    identifier_token(token)
        || (scope.boolean
            && ((index == 0 && token == "is") || (index + 1 == length && token == "flag")))
        || (scope.temporal
            && index + 1 == length
            && matches!(token, "at" | "on" | "date" | "timestamp"))
}

fn ascii_number(value: &str, minimum: u32, maximum: u32) -> bool {
    value.bytes().all(|byte| byte.is_ascii_digit())
        && value
            .parse::<u32>()
            .is_ok_and(|number| (minimum..=maximum).contains(&number))
}

pub(crate) fn temporal_text(value: &str) -> bool {
    // Recognize clear calendar/ISO timestamp formats without coercing samples.
    if !value.is_ascii() || value.len() < 10 {
        return false;
    }
    let date = &value[..10];
    if date.as_bytes()[4] != b'-'
        || date.as_bytes()[7] != b'-'
        || !ascii_number(&date[..4], 0, 9999)
        || !ascii_number(&date[5..7], 1, 12)
        || !ascii_number(&date[8..10], 1, 31)
    {
        return false;
    }
    if value.len() == 10 {
        return true;
    }
    if value.len() < 19
        || !matches!(value.as_bytes()[10], b'T' | b' ')
        || value.as_bytes()[13] != b':'
        || value.as_bytes()[16] != b':'
        || !ascii_number(&value[11..13], 0, 23)
        || !ascii_number(&value[14..16], 0, 59)
        || !ascii_number(&value[17..19], 0, 60)
    {
        return false;
    }
    let mut suffix = &value[19..];
    if let Some(fraction) = suffix.strip_prefix('.') {
        let digits = fraction
            .bytes()
            .take_while(|byte| byte.is_ascii_digit())
            .count();
        if digits == 0 {
            return false;
        }
        suffix = &fraction[digits..];
    }
    suffix.is_empty()
        || suffix == "Z"
        || (suffix.len() == 6
            && matches!(suffix.as_bytes()[0], b'+' | b'-')
            && suffix.as_bytes()[3] == b':'
            && ascii_number(&suffix[1..3], 0, 23)
            && ascii_number(&suffix[4..], 0, 59))
}
