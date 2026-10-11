use std::collections::BTreeSet;

use fuzzy_matcher::{FuzzyMatcher, skim::SkimMatcherV2};
use rapidfuzz::distance::jaro_winkler;

use crate::DataType;
use crate::synonyms::canonical_token;

pub fn exact_name_match(left: &str, right: &str) -> bool {
    normalize_name(left) == normalize_name(right)
}

pub fn name_score(left: &str, right: &str) -> f64 {
    let left_normalized = normalize_name(left);
    let right_normalized = normalize_name(right);

    if left_normalized == right_normalized {
        return 1.0;
    }

    let string_score =
        jaro_winkler::normalized_similarity(left_normalized.chars(), right_normalized.chars());

    let left_tokens = tokenize_name(left);
    let right_tokens = tokenize_name(right);

    let shared_tokens = left_tokens.intersection(&right_tokens).count();

    let token_score = if left_tokens.is_empty() || right_tokens.is_empty() {
        0.0
    } else {
        shared_tokens as f64 / left_tokens.len().min(right_tokens.len()) as f64
    };

    string_score.max(token_score)
}

// Used only after normal matching returns NoMatch.
pub(crate) fn abbreviation_score(left: &str, right: &str) -> f64 {
    let left = normalize_name(left);
    let right = normalize_name(right);

    if left.is_empty() || right.is_empty() {
        return 0.0;
    }

    skim_similarity(&left, &right)
}

pub fn type_score(left: &DataType, right: &DataType) -> f64 {
    use DataType::*;

    if left == right {
        return 1.0;
    }

    if matches!(left, Unknown) || matches!(right, Unknown) {
        return 0.5;
    }

    let both_numeric =
        matches!(left, Integer | Float | Decimal) && matches!(right, Integer | Float | Decimal);

    if both_numeric {
        return 0.75;
    }

    if matches!((left, right), (Date, Timestamp) | (Timestamp, Date)) {
        return 0.65;
    }

    0.0
}

pub fn sample_score(left: &[String], right: &[String]) -> Option<f64> {
    if left.is_empty() || right.is_empty() {
        return None;
    }

    let left: BTreeSet<String> = left
        .iter()
        .map(|v| normalize_sample(v))
        .filter(|value| !value.is_empty())
        .collect();

    let right: BTreeSet<String> = right
        .iter()
        .map(|v| normalize_sample(v))
        .filter(|value| !value.is_empty())
        .collect();

    if left.is_empty() || right.is_empty() {
        return None;
    }

    // One repeated value is not useful sample evidence.
    if left.len() < 2 || right.len() < 2 {
        return Some(0.0);
    }

    let overlap = left.intersection(&right).count() as f64;
    let denominator = left.len().max(right.len()) as f64;

    Some(overlap / denominator)
}

fn skim_similarity(left: &str, right: &str) -> f64 {
    // Treat the shorter name as the abbreviation/pattern.
    let (pattern, choice) = if left.chars().count() <= right.chars().count() {
        (left, right)
    } else {
        (right, left)
    };

    let matcher = SkimMatcherV2::default();

    let Some(actual_score) = matcher.fuzzy_match(choice, pattern) else {
        return 0.0;
    };

    // Skim gives a ranking score rather than a normalized 0-1 score.
    // Compare it with the score for matching the abbreviation to itself.
    let Some(perfect_score) = matcher.fuzzy_match(pattern, pattern) else {
        return 0.0;
    };

    if perfect_score <= 0 {
        return 0.0;
    }

    (actual_score as f64 / perfect_score as f64).clamp(0.0, 1.0)
}

fn normalize_name(value: &str) -> String {
    value
        .chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

fn tokenize_name(value: &str) -> BTreeSet<String> {
    value
        .split(|c: char| !c.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(|token| token.to_lowercase())
        .map(|token| canonical_token(&token).to_string())
        .collect()
}

fn normalize_sample(value: &str) -> String {
    value.trim().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_names_score_one() {
        assert_eq!(name_score("customer_id", "customer-id"), 1.0);
    }

    #[test]
    fn exact_name_match_detects_formatting_variants() {
        assert!(exact_name_match("customer_id", "customer-id"));
    }

    #[test]
    fn contained_words_score_highly() {
        assert_eq!(name_score("due_date", "payment_due_date"), 1.0);
        assert_eq!(name_score("payment_status", "status"), 1.0);
    }

    #[test]
    fn partial_word_overlap_is_not_perfect() {
        assert!(name_score("due_date", "document_date") < 1.0);
    }

    #[test]
    fn skim_recognizes_abbreviation() {
        assert!(abbreviation_score("wind_spd", "wspd") > 0.0);
        assert!(abbreviation_score("dominant_wpd", "dpd") > 0.0);
    }

    #[test]
    fn skim_scores_are_normalized() {
        let score = abbreviation_score("mean_wave_dir", "mwd");
        assert!((0.0..=1.0).contains(&score));
    }

    #[test]
    fn incompatible_types_score_zero() {
        assert_eq!(type_score(&DataType::Text, &DataType::Boolean), 0.0);
    }

    #[test]
    fn identical_samples_score_one() {
        let left = vec!["A".into(), "B".into()];
        let right = vec!["a".into(), "b".into()];

        assert_eq!(sample_score(&left, &right), Some(1.0));
    }

    #[test]
    fn repeated_single_value_gets_no_sample_credit() {
        let left = vec!["us".into(), "us".into(), "us".into()];
        let right = vec!["US".into(), " us ".into()];

        assert_eq!(sample_score(&left, &right), Some(0.0));
    }

    #[test]
    fn missing_values_do_not_create_distinct_sample_evidence() {
        let left = vec!["us".into(), "".into(), "us".into()];
        let right = vec!["US".into(), " \t ".into()];

        assert_eq!(sample_score(&left, &right), Some(0.0));

        let blank = vec!["".into(), " \t ".into()];
        assert_eq!(sample_score(&blank, &right), None);
        assert_eq!(sample_score(&left, &blank), None);
        assert_eq!(sample_score(&blank, &blank), None);

        let observed = vec!["us".into(), "ca".into()];
        let with_blanks = vec![" US ".into(), "".into(), "CA".into(), " ".into()];
        assert_eq!(sample_score(&observed, &with_blanks), Some(1.0));
        assert_eq!(sample_score(&with_blanks, &observed), Some(1.0));
    }

    #[test]
    fn missing_samples_remain_unavailable() {
        let empty: Vec<String> = vec![];
        let values = vec!["A".into(), "B".into()];

        assert_eq!(sample_score(&empty, &values), None);
        assert_eq!(sample_score(&values, &empty), None);
    }
}
