//! String and name similarities shared by the native matchers.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::DataType;

/// Normalized, case-sensitive value similarities supported by Jaccard matching.
///
/// Embeddings use [`super::jaccard::EmbeddingProvider`] instead: a string alone
/// cannot determine a meaningful embedding similarity without a model.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StringDistanceFunction {
    #[default]
    Levenshtein,
    DamerauLevenshtein,
    Hamming,
    Jaro,
    JaroWinkler,
    Exact,
}

/// Compare Unicode strings, returning a normalized score in `[0, 1]`.
///
/// Hamming pads the shorter string, as RapidFuzz's default scorer does. Edit
/// distances and padding count Unicode characters, rather than UTF-8 bytes.
/// Two empty strings have similarity one for every distance function.
pub fn similarity(a: &str, b: &str, function: StringDistanceFunction) -> f64 {
    match function {
        StringDistanceFunction::Levenshtein => strsim::normalized_levenshtein(a, b),
        StringDistanceFunction::DamerauLevenshtein => strsim::normalized_damerau_levenshtein(a, b),
        StringDistanceFunction::Hamming => {
            let a: Vec<_> = a.chars().collect();
            let b: Vec<_> = b.chars().collect();
            let length = a.len().max(b.len());
            if length == 0 {
                1.0
            } else {
                let equal = a.iter().zip(&b).filter(|(a, b)| a == b).count();
                equal as f64 / length as f64
            }
        }
        StringDistanceFunction::Jaro => strsim::jaro(a, b),
        StringDistanceFunction::JaroWinkler => strsim::jaro_winkler(a, b),
        StringDistanceFunction::Exact => f64::from(a == b),
    }
}

/// Split camelCase, PascalCase, uppercase abbreviations, separators and digits.
///
/// The word rules intentionally follow Valentine's ASCII word regex; numeric
/// runs include Unicode decimal digits. Tokens are lowercased, deduplicated and
/// retained in their first-occurrence order.
pub fn tokens(name: &str) -> Vec<String> {
    let characters: Vec<char> = name.chars().collect();
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    let mut i = 0;
    while i < characters.len() {
        let start = i;
        if characters[i].is_ascii_uppercase() {
            let mut end = i + 1;
            while end < characters.len() && characters[end].is_ascii_uppercase() {
                end += 1;
            }
            if end - i > 1 && end < characters.len() && characters[end].is_ascii_lowercase() {
                // HTTPServer => HTTP, Server.
                i = end - 1;
            } else {
                i = end;
                while i < characters.len() && characters[i].is_ascii_lowercase() {
                    i += 1;
                }
            }
        } else if characters[i].is_ascii_lowercase() {
            i += 1;
            while i < characters.len() && characters[i].is_ascii_lowercase() {
                i += 1;
            }
        } else if is_decimal_digit(characters[i]) {
            i += 1;
            while i < characters.len() && is_decimal_digit(characters[i]) {
                i += 1;
            }
        } else {
            i += 1;
            continue;
        }
        let token: String = characters[start..i]
            .iter()
            .flat_map(|character| character.to_lowercase())
            .collect();
        if seen.insert(token.clone()) {
            result.push(token);
        }
    }
    result
}

fn is_decimal_digit(character: char) -> bool {
    // Unicode Nd blocks, whose digits occupy consecutive groups of ten.
    // Unlike is_numeric(), this excludes fractions and Roman numerals.
    const STARTS: &[u32] = &[
        0x30, 0x660, 0x6f0, 0x7c0, 0x966, 0x9e6, 0xa66, 0xae6, 0xb66, 0xbe6, 0xc66, 0xce6, 0xd66,
        0xde6, 0xe50, 0xed0, 0xf20, 0x1040, 0x1090, 0x17e0, 0x1810, 0x1946, 0x19d0, 0x1a80, 0x1a90,
        0x1b50, 0x1bb0, 0x1c40, 0x1c50, 0xa620, 0xa8d0, 0xa900, 0xa9d0, 0xa9f0, 0xaa50, 0xabf0,
        0xff10, 0x104a0, 0x10d30, 0x10d40, 0x11066, 0x110f0, 0x11136, 0x111d0, 0x112f0, 0x11450,
        0x114d0, 0x11650, 0x116c0, 0x116d0, 0x116da, 0x11730, 0x118e0, 0x11950, 0x11bf0, 0x11f50,
        0x16a60, 0x16ac0, 0x16b50, 0x1d7ce, 0x1d7d8, 0x1d7e2, 0x1d7ec, 0x1d7f6, 0x1e140, 0x1e2f0,
        0x1e4f0, 0x1e5f1, 0x1e950,
    ];
    let value = character as u32;
    STARTS
        .iter()
        .any(|&start| value >= start && value < start + 10)
}

/// Detect exact or abbreviated tokens using Valentine's prefix/subsequence rule.
pub fn is_abbreviation(a: &str, b: &str) -> bool {
    if a == b {
        return true;
    }
    let (short, long) = if a.chars().count() <= b.chars().count() {
        (a, b)
    } else {
        (b, a)
    };
    let short_len = short.chars().count();
    let long_len = long.chars().count();
    if short_len < 2
        || short.chars().next() != long.chars().next()
        || (short_len as f64 / long_len as f64) < 0.3
    {
        return false;
    }
    if long.starts_with(short) {
        return true;
    }
    let mut long_chars = long.chars();
    short
        .chars()
        .all(|character| long_chars.any(|candidate| candidate == character))
}

/// Dice similarity on name tokens, including generic abbreviations.
/// Each token is used at most once, with deterministic greedy matching.
pub fn tokens_similarity(a: &str, b: &str) -> f64 {
    let mut a = tokens(a);
    let mut b = tokens(b);
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    if a.len() > b.len() {
        std::mem::swap(&mut a, &mut b);
    }
    let mut used = vec![false; b.len()];
    let mut matched = 0;
    for token in &a {
        if let Some(index) = b
            .iter()
            .enumerate()
            .position(|(index, candidate)| !used[index] && is_abbreviation(token, candidate))
        {
            used[index] = true;
            matched += 1;
        }
    }
    2.0 * matched as f64 / (a.len() + b.len()) as f64
}

/// Case-insensitive multiset Dice similarity on unpadded character trigrams.
/// Strings shorter than three characters contribute a single whole-string gram.
pub fn trigram_similarity(a: &str, b: &str) -> f64 {
    if a.is_empty() || b.is_empty() {
        return f64::from(a.is_empty() && b.is_empty());
    }
    let a = a.to_lowercase();
    let b = b.to_lowercase();
    if a == b {
        return 1.0;
    }
    fn grams(value: &str) -> Vec<String> {
        let characters: Vec<char> = value.chars().collect();
        if characters.len() < 3 {
            vec![value.to_string()]
        } else {
            characters
                .windows(3)
                .map(|window| window.iter().collect())
                .collect()
        }
    }
    let a = grams(&a);
    let b = grams(&b);
    let mut counts = HashMap::new();
    for gram in &b {
        *counts.entry(gram).or_insert(0usize) += 1;
    }
    let mut shared = 0;
    for gram in &a {
        if let Some(count) = counts.get_mut(gram)
            && *count > 0
        {
            *count -= 1;
            shared += 1;
        }
    }
    2.0 * shared as f64 / (a.len() + b.len()) as f64
}

/// Valentine's declared-type compatibility, extended to Rust's decimal and
/// timestamp variants as floating-point and date aliases respectively.
pub fn type_similarity(a: &DataType, b: &DataType) -> f64 {
    if a == b {
        return 1.0;
    }
    fn family(value: &DataType) -> u8 {
        match value {
            DataType::Integer => 1,
            DataType::Float | DataType::Decimal => 2,
            DataType::Text => 3,
            DataType::Date | DataType::Timestamp => 4,
            DataType::Boolean => 5,
            DataType::Unknown => 6,
        }
    }
    match (family(a), family(b)) {
        (x, y) if x == y => 1.0,
        (1, 2) | (2, 1) => 0.5,
        (3, 4) | (4, 3) => 0.3,
        _ => 0.0,
    }
}
