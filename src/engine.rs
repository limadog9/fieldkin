use std::collections::BTreeSet;

use crate::signals::type_compatibility;
use crate::{
    assignment, Candidate, Config, Decision, Field, FieldMatch, MatchError, MatchReport, Matcher,
    NameMatcher, SampleMatcher, SampleValue, Schema, SignalReport, TypeMatcher,
};

/// A signal and its relative weight. Zero-weight signals are disabled.
pub struct WeightedMatcher {
    /// Relative weight, finite and nonnegative.
    pub weight: f64,
    /// Signal implementation.
    pub matcher: Box<dyn Matcher>,
}

impl WeightedMatcher {
    /// Wrap a built-in or caller-defined signal.
    pub fn new(weight: f64, matcher: impl Matcher + 'static) -> Self {
        Self {
            weight,
            matcher: Box::new(matcher),
        }
    }
}

/// Stateless matcher configuration, reusable for many independent schema pairs.
pub struct MatchEngine {
    config: Config,
    matchers: Vec<WeightedMatcher>,
}

impl MatchEngine {
    /// Use names (0.65), declared types (0.20), and sampled overlap (0.15).
    pub fn new(config: Config) -> Result<Self, MatchError> {
        Self::with_matchers(
            config,
            vec![
                WeightedMatcher::new(0.65, NameMatcher::default()),
                WeightedMatcher::new(0.20, TypeMatcher),
                WeightedMatcher::new(0.15, SampleMatcher::default()),
            ],
        )
    }

    /// Use custom signals. Weights are normalized once; missing evidence contributes
    /// zero and does not redistribute weight. At most 64 signals are accepted.
    pub fn with_matchers(
        config: Config,
        mut matchers: Vec<WeightedMatcher>,
    ) -> Result<Self, MatchError> {
        validate_config(&config)?;
        if matchers.is_empty() || matchers.len() > 64 {
            return Err(MatchError("expected between 1 and 64 matchers".into()));
        }
        let mut names = BTreeSet::new();
        let mut total = 0.0;
        for signal in &matchers {
            if !signal.weight.is_finite() || signal.weight < 0.0 {
                return Err(MatchError(
                    "signal weights must be finite and nonnegative".into(),
                ));
            }
            let name = signal.matcher.name();
            if name.is_empty() || name.len() > 256 || !names.insert(name.to_owned()) {
                return Err(MatchError(
                    "signal names must be nonempty, unique, and at most 256 bytes".into(),
                ));
            }
            total += signal.weight;
        }
        if !total.is_finite() || total <= 0.0 {
            return Err(MatchError(
                "total signal weight must be positive and finite".into(),
            ));
        }
        for signal in &mut matchers {
            signal.weight /= total;
        }
        Ok(Self { config, matchers })
    }

    /// Rank every pair, abstain when appropriate, and optionally compute a partial
    /// maximum-weight assignment. No I/O or data rewriting is performed.
    ///
    /// Candidate truncation affects presentation only. Eligibility, ambiguity, and
    /// assignment use all pairs. Returned fields and unmatched IDs are sorted by ID.
    pub fn match_schemas(
        &self,
        source: &Schema,
        target: &Schema,
    ) -> Result<MatchReport, MatchError> {
        let mut total_bytes = 0usize;
        self.validate_schema(source, &mut total_bytes)?;
        self.validate_schema(target, &mut total_bytes)?;
        let pairs = source
            .fields
            .len()
            .checked_mul(target.fields.len())
            .ok_or_else(|| MatchError("pair count overflow".into()))?;
        if pairs > self.config.limits.max_pairs {
            return Err(MatchError("source-target pair budget exceeded".into()));
        }
        let active_signals = self.matchers.iter().filter(|s| s.weight > 0.0).count();
        if pairs
            .checked_mul(active_signals)
            .is_none_or(|count| count > self.config.limits.max_signal_evaluations)
        {
            return Err(MatchError("signal evaluation budget exceeded".into()));
        }
        let mut explanation_bytes = 0usize;
        let mut sources: Vec<_> = source.fields.iter().collect();
        let mut targets: Vec<_> = target.fields.iter().collect();
        sources.sort_by(|a, b| a.id.cmp(&b.id));
        targets.sort_by(|a, b| a.id.cmp(&b.id));
        let mut fields = Vec::with_capacity(sources.len());
        let mut matrix = Vec::with_capacity(sources.len());
        for source in sources {
            let mut candidates = targets
                .iter()
                .map(|target| self.evaluate(source, target, &mut explanation_bytes))
                .collect::<Result<Vec<_>, _>>()?;
            let best_score = candidates
                .iter()
                .filter(|c| c.eligible)
                .map(|c| c.score)
                .max_by(f64::total_cmp);
            let alternatives: Vec<_> = candidates
                .iter()
                .filter(|c| {
                    c.eligible
                        && best_score
                            .is_some_and(|best| best - c.score <= self.config.ambiguity_margin)
                })
                .map(|c| c.target.clone())
                .collect();
            let abstain = self.config.abstain_on_ambiguity && alternatives.len() > 1;
            matrix.push(
                candidates
                    .iter()
                    .map(|c| (c.eligible && !abstain).then_some(c.score))
                    .collect::<Vec<_>>(),
            );
            candidates.sort_by(|a, b| b.score.total_cmp(&a.score).then(a.target.cmp(&b.target)));
            let selected = if abstain {
                None
            } else {
                candidates.iter().find(|c| c.eligible).cloned()
            };
            let decision = if abstain {
                Decision::Ambiguous
            } else if selected.is_some() {
                Decision::Proposed
            } else {
                Decision::BelowThreshold
            };
            fields.push(FieldMatch {
                source: source.id.clone(),
                candidates,
                alternatives,
                selected,
                decision,
            });
        }
        if self.config.one_to_one {
            let selected = assignment::solve(&matrix);
            let contention: Vec<_> = (0..targets.len())
                .map(|j| matrix.iter().filter(|row| row[j].is_some()).count() > 1)
                .collect();
            for (i, field) in fields.iter_mut().enumerate() {
                field.selected = selected[i].and_then(|j| {
                    field
                        .candidates
                        .iter()
                        .find(|c| c.target == targets[j].id)
                        .cloned()
                });
                if let Some(candidate) = &mut field.selected {
                    if selected[i].is_some_and(|j| contention[j]) {
                        candidate.warnings.push("Target has competing eligible sources; global ties are resolved deterministically, not by additional evidence.".into());
                    }
                }
                if field.selected.is_none() && field.decision == Decision::Proposed {
                    field.decision = Decision::AssignmentConflict;
                }
            }
        }
        let used: BTreeSet<_> = fields
            .iter()
            .filter_map(|f| f.selected.as_ref().map(|c| c.target.clone()))
            .collect();
        let unmatched_sources = fields
            .iter()
            .filter(|f| f.selected.is_none())
            .map(|f| f.source.clone())
            .collect();
        let unmatched_targets = targets
            .iter()
            .filter(|f| !used.contains(&f.id))
            .map(|f| f.id.clone())
            .collect();
        for field in &mut fields {
            field.candidates.truncate(self.config.max_candidates);
        }
        Ok(MatchReport {
            fields,
            unmatched_sources,
            unmatched_targets,
            one_to_one: self.config.one_to_one,
        })
    }

    fn evaluate(
        &self,
        source: &Field,
        target: &Field,
        explanation_bytes: &mut usize,
    ) -> Result<Candidate, MatchError> {
        let mut signals = Vec::with_capacity(self.matchers.len());
        let mut score = 0.0;
        for signal in &self.matchers {
            if signal.weight == 0.0 {
                continue;
            }
            let evidence = signal.matcher.evaluate(source, target).map_err(|_| {
                MatchError(format!(
                    "matcher '{}' failed (details suppressed to protect sample values)",
                    signal.matcher.name()
                ))
            })?;
            if evidence
                .score
                .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
            {
                return Err(MatchError(format!(
                    "matcher '{}' returned a score outside [0, 1]",
                    signal.matcher.name()
                )));
            }
            if evidence.explanation.len() > 4096 {
                return Err(MatchError("matcher explanation exceeds 4096 bytes".into()));
            }
            *explanation_bytes = explanation_bytes
                .checked_add(evidence.explanation.len())
                .ok_or_else(|| MatchError("explanation byte count overflow".into()))?;
            if *explanation_bytes > self.config.limits.max_explanation_bytes {
                return Err(MatchError(
                    "aggregate explanation byte budget exceeded".into(),
                ));
            }
            score += signal.weight * evidence.score.unwrap_or(0.0);
            signals.push(SignalReport {
                name: signal.matcher.name().to_owned(),
                weight: signal.weight,
                evidence,
            });
        }
        let score = score.clamp(0.0, 1.0);
        let incompatible = type_compatibility(source.data_type, target.data_type) == Some(0.0);
        let mut warnings = Vec::new();
        if incompatible {
            warnings.push("Declared types are incompatible; no conversion is inferred.".into());
        }
        if signals.iter().any(|s| s.evidence.score.is_none()) {
            warnings.push(
                "Some evidence is unavailable or insufficient; missing weight contributes zero."
                    .into(),
            );
        }
        Ok(Candidate {
            target: target.id.clone(),
            score,
            eligible: score > 0.0
                && score >= self.config.min_score
                && !(incompatible && self.config.reject_incompatible_types),
            signals,
            warnings,
        })
    }

    fn validate_schema(&self, schema: &Schema, total_bytes: &mut usize) -> Result<(), MatchError> {
        let limits = &self.config.limits;
        if schema.fields.len() > limits.max_fields {
            return Err(MatchError("field budget exceeded".into()));
        }
        let mut ids = BTreeSet::new();
        for field in &schema.fields {
            if field.id.0.is_empty() || !ids.insert(&field.id) {
                return Err(MatchError(
                    "field IDs must be nonempty and unique within each schema".into(),
                ));
            }
            if field.name.len() > limits.max_name_bytes || field.id.0.len() > limits.max_name_bytes
            {
                return Err(MatchError("field name or ID byte budget exceeded".into()));
            }
            if let Some(samples) = &field.samples {
                if samples.len() > limits.max_samples_per_field {
                    return Err(MatchError("sample count budget exceeded".into()));
                }
                for value in samples {
                    match value {
                        SampleValue::Number(n) if !n.is_finite() => {
                            return Err(MatchError("numeric samples must be finite".into()))
                        }
                        SampleValue::Text(s) => {
                            if s.len() > limits.max_sample_bytes {
                                return Err(MatchError("text sample byte budget exceeded".into()));
                            }
                            *total_bytes = total_bytes
                                .checked_add(s.len())
                                .ok_or_else(|| MatchError("sample byte count overflow".into()))?;
                            if *total_bytes > limits.max_total_sample_bytes {
                                return Err(MatchError("total sample byte budget exceeded".into()));
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
        Ok(())
    }
}

fn validate_config(config: &Config) -> Result<(), MatchError> {
    if !config.min_score.is_finite()
        || !(0.0..=1.0).contains(&config.min_score)
        || !config.ambiguity_margin.is_finite()
        || !(0.0..=1.0).contains(&config.ambiguity_margin)
    {
        return Err(MatchError(
            "threshold and ambiguity margin must be finite and in [0, 1]".into(),
        ));
    }
    if config.max_candidates == 0 {
        return Err(MatchError("max_candidates must be positive".into()));
    }
    Ok(())
}
