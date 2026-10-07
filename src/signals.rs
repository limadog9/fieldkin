use std::collections::BTreeSet;

use strsim::jaro_winkler;

use crate::synonyms::canonical_token;
use crate::DataType;

pub fn exact_name_match(left: &str, right: &str) -> bool {
    normalize_name(left) == normalize_name(right)
}

pub fn name_score(left: &str, right: &str) -> f64 {
    let left_normalized = normalize_name(left);
    let right_normalized = normalize_name(right);

    if left_normalized == right_normalized {
        return 1.0;
    }

    // Compare the overall strings.
    let string_score = jaro_winkler(&left_normalized, &right_normalized);

    // Also compare individual words, including our existing synonyms.
    let left_tokens = tokenize_name(left);
    let right_tokens = tokenize_name(right);

    let shared_tokens = left_tokens.intersection(&right_tokens).count();

    let token_score = if left_tokens.is_empty() || right_tokens.is_empty() {
        0.0
    } else {
        shared_tokens as f64 / left_tokens.len().min(right_tokens.len()) as f64
    };

    // Keep the existing behavior:
    // use whichever form gives stronger name evidence.
    string_score.max(token_score)
}

pub fn type_score(left: &DataType, right: &DataType) -> f64 {
    use DataType::*;

    if left == right {
        return 1.0;
    }

    if matches!(left, Unknown) || matches!(right, Unknown) {
        return 0.5;
    }

    let both_numeric = matches!(left, Integer | Float | Decimal)
        && matches!(right, Integer | Float | Decimal);

    if both_numeric {
        return 0.75;
    }

    if matches!((left, right), (Date, Timestamp) | (Timestamp, Date)) {
        return 0.65;
    }

    0.0
}

pub fn sample_score(left: &[String], right: &[String]) -> Option<f64> {
    // No samples on either side means sample evidence is unavailable.
    if left.is_empty() || right.is_empty() {
        return None;
    }

    // Normalize values and remove duplicates.
    let left: BTreeSet<String> = left.iter().map(|v| normalize_sample(v)).collect();
    let right: BTreeSet<String> = right.iter().map(|v| normalize_sample(v)).collect();

    // NEW: repeating one value is not enough to earn sample credit.
    //
    // ["us", "us", "us"] becomes just {"us"}.
    //
    // Return a score of zero, not missing evidence, so the engine
    // does not redistribute the sample weight to names and types.
    if left.len() < 2 || right.len() < 2 {
        return Some(0.0);
    }

    // Otherwise, use the same overlap calculation as before.
    let overlap = left.intersection(&right).count() as f64;
    let denominator = left.len().max(right.len()) as f64;

    Some(overlap / denominator)
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
    fn single_distinct_value_on_either_side_gets_no_credit() {
        let constant = vec!["USD".into(), "USD".into()];
        let varied = vec!["USD".into(), "EUR".into(), "GBP".into()];

        assert_eq!(sample_score(&constant, &varied), Some(0.0));
        assert_eq!(sample_score(&varied, &constant), Some(0.0));
    }

    #[test]
    fn missing_samples_remain_unavailable() {
        let empty: Vec<String> = vec![];
        let values = vec!["A".into(), "B".into()];

        assert_eq!(sample_score(&empty, &values), None);
        assert_eq!(sample_score(&values, &empty), None);
    }

    #[test]
    fn varied_samples_keep_the_existing_overlap_score() {
        let left = vec!["A".into(), "B".into(), "C".into()];
        let right = vec!["A".into(), "B".into(), "D".into(), "E".into()];

        // Two shared values divided by four distinct values
        // in the larger set.
        assert_eq!(sample_score(&left, &right), Some(0.5));
    }
}