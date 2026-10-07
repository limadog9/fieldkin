use crate::signals::{exact_name_match, name_score, sample_score, type_score};
use crate::{Candidate, Decision, FieldResult, MatchReport, Schema};

const NAME_WEIGHT: f64 = 0.60;
const TYPE_WEIGHT: f64 = 0.25;
const SAMPLE_WEIGHT: f64 = 0.15;

// Only new behavior:
// exact same normalized name + compatible type gets a strong preference.
const EXACT_NAME_SCORE_FLOOR: f64 = 0.98;

#[derive(Clone, Copy, Debug)]
pub struct Config {
    pub min_score: f64,
    pub ambiguity_margin: f64,
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

pub fn match_schemas(source: &Schema, target: &Schema, config: Config) -> MatchReport {
    let mut fields = Vec::with_capacity(source.fields.len());

    for source_field in &source.fields {
        let raw_candidates = target
            .fields
            .iter()
            .map(|target_field| {
                let name = name_score(&source_field.name, &target_field.name);
                let data_type = type_score(&source_field.data_type, &target_field.data_type);
                let samples = sample_score(&source_field.samples, &target_field.samples);

                let exact_name =
                    exact_name_match(&source_field.name, &target_field.name);

                (target_field, name, data_type, samples, exact_name)
            })
            .collect::<Vec<_>>();

        // Existing behavior:
        // find the strongest sample overlap.
        let best_sample_score = raw_candidates
            .iter()
            .filter_map(|(_, _, _, samples, _)| *samples)
            .reduce(f64::max);

        // Existing behavior:
        // see whether multiple targets share that strongest sample overlap.
        let best_sample_count = best_sample_score.map_or(0, |best| {
            raw_candidates
                .iter()
                .filter(|(_, _, _, samples, _)| {
                    samples.is_some_and(|score| (score - best).abs() < 1e-12)
                })
                .count()
        });

        let sample_tie =
            best_sample_score.is_some_and(|best| best > 0.0)
                && best_sample_count > 1;

        let mut candidates = raw_candidates
            .into_iter()
            .map(
                |(target_field, name, data_type, samples, exact_name)| {
                    // Existing behavior:
                    // tied strongest sample matches only get half credit.
                    let effective_samples =
                        match (samples, best_sample_score, sample_tie) {
                            (Some(score), Some(best), true)
                                if (score - best).abs() < 1e-12 =>
                            {
                                Some(score * 0.5)
                            }

                            _ => samples,
                        };

                    let mut score =
                        combined_score(name, data_type, effective_samples);

                    // NEW:
                    // exact same name + compatible type gets strong preference.
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
                },
            )
            .collect::<Vec<_>>();

        candidates.sort_by(|a, b| {
            b.score
                .total_cmp(&a.score)
                .then_with(|| a.target.cmp(&b.target))
        });

        let decision = decide(&candidates, config);

        candidates.truncate(config.max_candidates);

        fields.push(FieldResult {
            source: source_field.name.clone(),
            candidates,
            decision,
        });
    }

    MatchReport { fields }
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
    use crate::{DataType, Field};

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
            Decision::Match { target, .. } if target == "cust_id"
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
                field(
                    "citymarketid_1",
                    DataType::Integer,
                    &["100", "200", "300"],
                ),
                field(
                    "citymarketid_2",
                    DataType::Integer,
                    &["200", "300", "400"],
                ),
            ],
        };

        let report = match_schemas(&source, &target, Config::default());

        assert!(matches!(
            &report.fields[0].decision,
            Decision::Match { target, .. } if target == "citymarketid_1"
        ));
    }

    #[test]
    fn tied_best_sample_matches_get_half_credit() {
        let source = Schema {
            fields: vec![field(
                "bank_account_id",
                DataType::Integer,
                &["501", "502", "503"],
            )],
        };

        let target = Schema {
            fields: vec![
                field(
                    "merchant_id",
                    DataType::Integer,
                    &["501", "502", "503"],
                ),
                field(
                    "batch_number",
                    DataType::Integer,
                    &["501", "502", "503"],
                ),
            ],
        };

        let report = match_schemas(&source, &target, Config::default());

        for candidate in &report.fields[0].candidates {
            assert_eq!(candidate.sample_score, Some(0.5));
        }
    }
}