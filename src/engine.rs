use crate::signals::{abbreviation_score, exact_name_match, name_score, sample_score, type_score};
use crate::{Candidate, Decision, Error, Field, FieldResult, MatchReport, Schema};

const NAME_WEIGHT: f64 = 0.60;
const TYPE_WEIGHT: f64 = 0.25;
const SAMPLE_WEIGHT: f64 = 0.15;

const EXACT_NAME_SCORE_FLOOR: f64 = 0.98;

/// Decision thresholds and the number of candidate explanations to return.
#[derive(Clone, Copy, Debug)]
pub struct Config {
    /// Minimum score for a match or ambiguous candidate; defaults to `0.72`.
    /// Must be finite. Scores are heuristic values in `0.0..=1.0`.
    pub min_score: f64,
    /// Maximum score gap from the best candidate to consider ambiguous.
    /// Must be finite and nonnegative; defaults to `0.05`.
    pub ambiguity_margin: f64,
    /// Maximum candidates returned per source field; defaults to `5`.
    /// This limit does not affect the decision. Zero omits candidate details.
    pub max_candidates: usize,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_score: 0.72,
            ambiguity_margin: 0.05,
            max_candidates: 5,
        }
    }
}

/// Suggest correspondences for every source field against all target fields.
///
/// Source fields are evaluated independently, so several may select the same
/// target. Names identify results; duplicate and empty names are accepted.
/// Abbreviation matching runs only after normal matching returns `NoMatch`.
/// Decisions use all candidates before the returned details are truncated.
///
/// # Panics
///
/// Panics if either threshold is nonfinite or `ambiguity_margin` is negative.
/// Use [`try_match_schemas`] to receive an error for invalid configuration.
pub fn match_schemas(source: &Schema, target: &Schema, config: Config) -> MatchReport {
    try_match_schemas(source, target, config).unwrap_or_else(|error| panic!("{error}"))
}

/// Suggest correspondences with the same behavior as [`match_schemas`], returning
/// [`Error::InvalidConfig`] for nonfinite thresholds or a negative ambiguity margin.
/// Finite minimum scores outside `[0, 1]` are accepted, as in the original API.
/// Empty schemas and duplicate/empty field names retain their original behavior.
pub fn try_match_schemas(
    source: &Schema,
    target: &Schema,
    config: Config,
) -> Result<MatchReport, Error> {
    if !config.min_score.is_finite() {
        return Err(Error::InvalidConfig("min_score must be finite".into()));
    }
    if !config.ambiguity_margin.is_finite() || config.ambiguity_margin < 0.0 {
        return Err(Error::InvalidConfig(
            "ambiguity_margin must be finite and nonnegative".into(),
        ));
    }
    let mut fields = Vec::with_capacity(source.fields.len());

    for source_field in &source.fields {
        let normal_candidates = build_candidates(source_field, target, false);

        let normal_decision = decide(&normal_candidates, config);

        let should_try_abbreviation_fallback = matches!(&normal_decision, Decision::NoMatch { .. });

        let (mut candidates, decision) = if should_try_abbreviation_fallback {
            // Retry with abbreviation evidence only after NoMatch.
            let fallback_candidates = build_candidates(source_field, target, true);

            let fallback_decision = decide(&fallback_candidates, config);

            (fallback_candidates, fallback_decision)
        } else {
            (normal_candidates, normal_decision)
        };

        candidates.truncate(config.max_candidates);

        fields.push(FieldResult {
            source: source_field.name.clone(),
            candidates,
            decision,
        });
    }

    Ok(MatchReport { fields })
}

fn build_candidates(
    source_field: &Field,
    target: &Schema,
    use_abbreviation_fallback: bool,
) -> Vec<Candidate> {
    let raw_candidates = target
        .fields
        .iter()
        .map(|target_field| {
            let normal_name = name_score(&source_field.name, &target_field.name);

            let name = if use_abbreviation_fallback {
                normal_name.max(abbreviation_score(&source_field.name, &target_field.name))
            } else {
                normal_name
            };

            let data_type = type_score(&source_field.data_type, &target_field.data_type);

            let samples = sample_score(&source_field.samples, &target_field.samples);

            let exact_name = exact_name_match(&source_field.name, &target_field.name);

            (target_field, name, data_type, samples, exact_name)
        })
        .collect::<Vec<_>>();

    // If several candidates share the strongest positive sample score,
    // that sample evidence is less discriminating.
    let best_sample_score = raw_candidates
        .iter()
        .filter_map(|(_, _, _, samples, _)| *samples)
        .reduce(f64::max);

    let best_sample_count = best_sample_score.map_or(0, |best| {
        raw_candidates
            .iter()
            .filter(|(_, _, _, samples, _)| {
                samples.is_some_and(|score| (score - best).abs() < 1e-12)
            })
            .count()
    });

    let sample_tie = best_sample_score.is_some_and(|best| best > 0.0) && best_sample_count > 1;

    let mut candidates = raw_candidates
        .into_iter()
        .map(|(target_field, name, data_type, samples, exact_name)| {
            let effective_samples = match (samples, best_sample_score, sample_tie) {
                (Some(score), Some(best), true) if (score - best).abs() < 1e-12 => {
                    Some(score * 0.5)
                }

                _ => samples,
            };

            let mut score = combined_score(name, data_type, effective_samples);

            // Exact same normalized name + compatible type
            // gets strong preference.
            if exact_name && data_type >= 0.75 {
                score = score.max(EXACT_NAME_SCORE_FLOOR);
            }

            Candidate {
                target: target_field.name.clone(),
                score,
                name_score: name,
                type_score: data_type,
                sample_score: effective_samples,
            }
        })
        .collect::<Vec<_>>();

    candidates.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.target.cmp(&b.target))
    });

    candidates
}

fn combined_score(name: f64, data_type: f64, samples: Option<f64>) -> f64 {
    let mut numerator = (name * NAME_WEIGHT) + (data_type * TYPE_WEIGHT);

    let mut denominator = NAME_WEIGHT + TYPE_WEIGHT;

    if let Some(samples) = samples {
        numerator += samples * SAMPLE_WEIGHT;
        denominator += SAMPLE_WEIGHT;
    }

    numerator / denominator
}

fn decide(candidates: &[Candidate], config: Config) -> Decision {
    let Some(best) = candidates.first() else {
        return Decision::NoMatch { best_score: None };
    };

    if best.score < config.min_score {
        return Decision::NoMatch {
            best_score: Some(best.score),
        };
    }

    let ambiguous_targets = candidates
        .iter()
        .take_while(|candidate| {
            candidate.score >= config.min_score
                && (best.score - candidate.score) <= config.ambiguity_margin
        })
        .map(|candidate| candidate.target.clone())
        .collect::<Vec<_>>();

    if ambiguous_targets.len() > 1 {
        return Decision::Ambiguous {
            targets: ambiguous_targets,
            best_score: best.score,
        };
    }

    Decision::Match {
        target: best.target.clone(),
        score: best.score,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::DataType;

    fn field(name: &str, data_type: DataType, samples: &[&str]) -> Field {
        Field {
            name: name.into(),
            data_type,
            samples: samples.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    #[test]
    fn obvious_match_is_selected() {
        let source = Schema {
            fields: vec![field(
                "customer_id",
                DataType::Integer,
                &["101", "102", "103"],
            )],
        };

        let target = Schema {
            fields: vec![
                field("cust_id", DataType::Integer, &["101", "102", "103"]),
                field("email", DataType::Text, &["a@x.com", "b@x.com"]),
            ],
        };

        let report = match_schemas(&source, &target, Config::default());

        assert!(matches!(
            &report.fields[0].decision,
            Decision::Match { target, .. }
                if target == "cust_id"
        ));
    }

    #[test]
    fn exact_name_beats_near_identical_sibling() {
        let source = Schema {
            fields: vec![field(
                "citymarketid_1",
                DataType::Integer,
                &["100", "200", "300"],
            )],
        };

        let target = Schema {
            fields: vec![
                field("citymarketid_1", DataType::Integer, &["100", "200", "300"]),
                field("citymarketid_2", DataType::Integer, &["200", "300", "400"]),
            ],
        };

        let report = match_schemas(&source, &target, Config::default());

        assert!(matches!(
            &report.fields[0].decision,
            Decision::Match { target, .. }
                if target == "citymarketid_1"
        ));
    }

    #[test]
    fn abbreviation_fallback_can_rescue_no_match() {
        let source = Schema {
            fields: vec![field("wind_spd", DataType::Float, &["1.1", "2.2", "3.3"])],
        };

        let target = Schema {
            fields: vec![
                field("wspd", DataType::Float, &["1.1", "2.2", "3.3"]),
                field("temperature", DataType::Float, &["10", "20", "30"]),
            ],
        };

        let report = match_schemas(&source, &target, Config::default());

        assert!(matches!(
            &report.fields[0].decision,
            Decision::Match { target, .. }
                if target == "wspd"
        ));
    }

    #[test]
    fn normal_match_prevents_fallback_interference() {
        let source = Schema {
            fields: vec![field(
                "payment_status",
                DataType::Text,
                &["open", "paid", "scheduled"],
            )],
        };

        let target = Schema {
            fields: vec![
                field("status", DataType::Text, &["open", "paid", "scheduled"]),
                field("payment_state_code", DataType::Text, &["x", "y", "z"]),
            ],
        };

        let report = match_schemas(&source, &target, Config::default());

        assert!(matches!(
            &report.fields[0].decision,
            Decision::Match { target, .. }
                if target == "status"
        ));
    }
}
