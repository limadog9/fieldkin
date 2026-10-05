use std::collections::BTreeSet;

use crate::name_conflicts::{CompiledNameConflictRule, PreparedNameConflicts};
use crate::profile::PreparedProfile;
use crate::signals::{type_compatibility, PreparedName, PreparedSamples};
use crate::{
    assignment, AssignmentDiagnostics, BudgetKind, Candidate, CandidateIssue, Config,
    ConfigurationError, CountKind, Decision, Field, FieldDiagnostic, FieldMatch, InputError,
    MatchConstraints, MatchError, MatchReport, Matcher, NameConflictKind, NameMatcher,
    SampleMatcher, SampleProfileMatcher, SampleReliability, SampleValue, Schema, SemanticAxis,
    SignalReport, TargetCompetition, TypeMatcher,
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
    name_conflicts: Vec<CompiledNameConflictRule>,
    contextual_sample_signal: Option<usize>,
}

// This cache borrows validated fields and lives only for one match call. It is
// deliberately private: callers still pass ordinary schemas and custom matchers
// retain their original pair-by-pair evaluation order.
enum PreparedSignal<'a> {
    Names {
        matcher: &'a NameMatcher,
        source: Vec<PreparedName>,
        target: Vec<PreparedName>,
    },
    Samples {
        matcher: &'a SampleMatcher,
        source: Vec<PreparedSamples<'a>>,
        target: Vec<PreparedSamples<'a>>,
    },
    Profiles {
        matcher: &'a SampleProfileMatcher,
        source: Vec<PreparedProfile>,
        target: Vec<PreparedProfile>,
    },
    Direct,
}

impl<'a> PreparedSignal<'a> {
    fn new(signal: &'a WeightedMatcher, source: &[&'a Field], target: &[&'a Field]) -> Self {
        // Empty products and disabled signals never evaluate evidence, including
        // invalid aliases or matcher-specific settings.
        if signal.weight == 0.0 || source.is_empty() || target.is_empty() {
            return Self::Direct;
        }
        let concrete = signal.matcher.as_any();
        if let Some(matcher) = concrete.and_then(|m| m.downcast_ref::<NameMatcher>()) {
            Self::Names {
                matcher,
                source: source
                    .iter()
                    .map(|field| matcher.prepare(&field.name))
                    .collect(),
                target: target
                    .iter()
                    .map(|field| matcher.prepare(&field.name))
                    .collect(),
            }
        } else if let Some(matcher) = concrete.and_then(|m| m.downcast_ref::<SampleMatcher>()) {
            Self::Samples {
                matcher,
                source: source.iter().map(|field| matcher.prepare(field)).collect(),
                target: target.iter().map(|field| matcher.prepare(field)).collect(),
            }
        } else if let Some(matcher) =
            concrete.and_then(|m| m.downcast_ref::<SampleProfileMatcher>())
        {
            Self::Profiles {
                matcher,
                source: source.iter().map(|field| matcher.prepare(field)).collect(),
                target: target.iter().map(|field| matcher.prepare(field)).collect(),
            }
        } else {
            Self::Direct
        }
    }
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
            return Err(MatchError::InvalidConfiguration(
                ConfigurationError::MatcherCount,
            ));
        }
        let mut names = BTreeSet::new();
        let mut total = 0.0;
        for signal in &matchers {
            if !signal.weight.is_finite() || signal.weight < 0.0 {
                return Err(MatchError::InvalidConfiguration(
                    ConfigurationError::SignalWeight,
                ));
            }
            let name = signal.matcher.name();
            if name.is_empty() || name.len() > 256 || !names.insert(name.to_owned()) {
                return Err(MatchError::InvalidConfiguration(
                    ConfigurationError::SignalNames,
                ));
            }
            total += signal.weight;
        }
        if !total.is_finite() || total <= 0.0 {
            return Err(MatchError::InvalidConfiguration(
                ConfigurationError::TotalWeight,
            ));
        }
        for signal in &mut matchers {
            signal.weight /= total;
        }
        let contextual_sample_signal = if config.contextual_evidence.is_some() {
            let active = || matchers.iter().filter(|signal| signal.weight > 0.0);
            let names = active().any(|signal| {
                signal
                    .matcher
                    .as_any()
                    .is_some_and(|matcher| matcher.is::<NameMatcher>())
            });
            let samples = active().position(|signal| {
                signal
                    .matcher
                    .as_any()
                    .and_then(|matcher| matcher.downcast_ref::<SampleMatcher>())
                    .is_some_and(|matcher| {
                        matcher.reliability == SampleReliability::Distinct
                            && matcher.min_non_null == SampleMatcher::default().min_non_null
                    })
            });
            if !names || samples.is_none() {
                return Err(MatchError::InvalidConfiguration(
                    ConfigurationError::ContextualEvidence,
                ));
            }
            samples
        } else {
            None
        };
        let name_conflicts = config
            .name_conflicts
            .iter()
            .map(CompiledNameConflictRule::new)
            .collect();
        Ok(Self {
            config,
            matchers,
            name_conflicts,
            contextual_sample_signal,
        })
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
        self.match_schemas_with_constraints(source, target, &MatchConstraints::default())
    }

    /// Match while respecting caller-owned confirmations, forbidden pairs and
    /// explicitly unmatched sources. Empty constraints reproduce [`Self::match_schemas`].
    ///
    /// All original fields and all pairs are validated and evaluated, including
    /// excluded ones. Constraints do not reduce budgets or matcher callbacks.
    /// Confirmations override score, corroboration, name conflicts and ambiguity,
    /// but never the enabled type veto or conflicting semantic hints. Scores remain unchanged;
    /// [`Decision::Confirmed`] distinguishes a caller decision from a proposal.
    ///
    /// In one-to-one mode confirmed targets are reserved before automatic local
    /// ambiguity and assignment are computed. Global diagnostics analyze only
    /// the remaining automatic edges, excluding fixed confirmations from the
    /// objective and alternatives. Work is charged using the original dimensions.
    pub fn match_schemas_with_constraints(
        &self,
        source: &Schema,
        target: &Schema,
        constraints: &MatchConstraints,
    ) -> Result<MatchReport, MatchError> {
        let mut total_bytes = 0usize;
        self.validate_schema(source, &mut total_bytes)?;
        self.validate_schema(target, &mut total_bytes)?;
        let pairs = source
            .fields
            .len()
            .checked_mul(target.fields.len())
            .ok_or(MatchError::CountOverflow(CountKind::Pairs))?;
        if pairs > self.config.limits.max_pairs {
            return Err(MatchError::BudgetExceeded(BudgetKind::Pairs));
        }
        let active_signals = self.matchers.iter().filter(|s| s.weight > 0.0).count();
        if pairs
            .checked_mul(active_signals)
            .is_none_or(|count| count > self.config.limits.max_signal_evaluations)
        {
            return Err(MatchError::BudgetExceeded(BudgetKind::SignalEvaluations));
        }
        let mut explanation_bytes = 0usize;
        let mut sources: Vec<_> = source.fields.iter().collect();
        let mut targets: Vec<_> = target.fields.iter().collect();
        sources.sort_unstable_by(|a, b| a.id.cmp(&b.id));
        targets.sort_unstable_by(|a, b| a.id.cmp(&b.id));
        let constraints = constraints.validate(&sources, &targets, &self.config)?;
        let prepared: Vec<_> = self
            .matchers
            .iter()
            .map(|signal| PreparedSignal::new(signal, &sources, &targets))
            .collect();
        let name_conflicts = (!self.name_conflicts.is_empty()).then(|| {
            let prepare = |fields: &[&Field]| {
                fields
                    .iter()
                    .map(|field| PreparedNameConflicts::new(&field.name, &self.name_conflicts))
                    .collect::<Vec<_>>()
            };
            (prepare(&sources), prepare(&targets))
        });
        // Schema context needs every pair before row decisions. The default
        // keeps streaming rows and never allocates this additional matrix.
        let mut contextual_rows = if let Some(policy) = self.config.contextual_evidence {
            let mut candidates = (0..sources.len())
                .map(|source| {
                    self.evaluate_row(
                        source,
                        &sources,
                        &targets,
                        &prepared,
                        name_conflicts.as_ref(),
                        &mut explanation_bytes,
                    )
                })
                .collect::<Result<Vec<_>, _>>()?;
            crate::contextual::apply(
                &self.config,
                policy,
                &sources,
                &targets,
                &mut candidates,
                self.contextual_sample_signal
                    .ok_or(MatchError::InvalidConfiguration(
                        ConfigurationError::ContextualEvidence,
                    ))?,
                &mut explanation_bytes,
            )?;
            Some(candidates.into_iter())
        } else {
            None
        };
        let mut fields = Vec::with_capacity(sources.len());
        let mut matrix = Vec::new();
        if self.config.one_to_one {
            matrix.reserve(sources.len());
        }
        for (source_index, source) in sources.iter().enumerate() {
            let mut candidates = if let Some(rows) = &mut contextual_rows {
                rows.next()
                    .ok_or(MatchError::CountOverflow(CountKind::Pairs))?
            } else {
                self.evaluate_row(
                    source_index,
                    &sources,
                    &targets,
                    &prepared,
                    name_conflicts.as_ref(),
                    &mut explanation_bytes,
                )?
            };
            for (target_index, candidate) in candidates.iter_mut().enumerate() {
                if let Some(constraints) = &constraints {
                    if constraints.forbidden(source_index, target_index) {
                        candidate.eligible = false;
                        candidate.issues.push(CandidateIssue::ForbiddenByCaller);
                        candidate
                            .warnings
                            .push("Caller forbids this pair; automatic selection excluded.".into());
                    }
                    if constraints.unmatched[source_index] {
                        candidate.eligible = false;
                        candidate
                            .issues
                            .push(CandidateIssue::SourceExcludedByCaller);
                        candidate.warnings.push(
                            "Caller keeps this source unmatched; automatic selection excluded."
                                .into(),
                        );
                    }
                    if constraints.reserved[target_index].is_some_and(|owner| owner != source_index)
                    {
                        candidate.eligible = false;
                        candidate
                            .issues
                            .push(CandidateIssue::TargetConfirmedByCaller);
                        candidate.warnings.push(
                                "Target is reserved by another caller-confirmed source; automatic selection excluded."
                                    .into(),
                            );
                    }
                }
            }
            let confirmed = constraints
                .as_ref()
                .and_then(|constraints| constraints.confirmed[source_index]);
            let excluded = constraints
                .as_ref()
                .is_some_and(|constraints| constraints.unmatched[source_index]);
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
            let insufficient_evidence = (self.config.corroboration.is_some()
                || self.config.contextual_evidence.is_some())
                && best_score.is_none()
                && confirmed.is_none()
                && !excluded
                && candidates.iter().any(|candidate| {
                    candidate.issues.iter().any(|issue| {
                        matches!(
                            issue,
                            CandidateIssue::InsufficientNameSupport
                                | CandidateIssue::InsufficientSampleSupport
                                | CandidateIssue::InsufficientContextSupport
                        )
                    }) && !candidate.issues.iter().any(|issue| match issue {
                        CandidateIssue::InsufficientScore
                        | CandidateIssue::NameConflict(_)
                        | CandidateIssue::SemanticConflict(_)
                        | CandidateIssue::ForbiddenByCaller
                        | CandidateIssue::SourceExcludedByCaller
                        | CandidateIssue::TargetConfirmedByCaller => true,
                        CandidateIssue::IncompatibleTypes => self.config.reject_incompatible_types,
                        _ => false,
                    })
                });
            let mut diagnostics = Vec::new();
            if best_score.is_none() && !excluded {
                diagnostics.push(FieldDiagnostic::NoEligibleTarget);
            }
            if insufficient_evidence {
                diagnostics.push(FieldDiagnostic::InsufficientEvidence);
            }
            if alternatives.len() > 1 {
                diagnostics.push(FieldDiagnostic::LocalAmbiguity);
            }
            if confirmed.is_some() {
                diagnostics.push(FieldDiagnostic::ConfirmedByCaller);
            } else if excluded {
                diagnostics.push(FieldDiagnostic::ExcludedByCaller);
            }
            if self.config.one_to_one {
                matrix.push(
                    candidates
                        .iter()
                        .map(|c| {
                            (c.eligible && !abstain && confirmed.is_none() && !excluded)
                                .then_some(c.score)
                        })
                        .collect::<Vec<_>>(),
                );
            }
            // IDs are unique, so this comparator defines a total order even when
            // scores tie; an unstable sort needs no auxiliary allocation.
            candidates
                .sort_unstable_by(|a, b| b.score.total_cmp(&a.score).then(a.target.cmp(&b.target)));
            // Clone before display truncation, including zero-score confirmations.
            // Eligibility still describes the automatic evidence policy.
            let selected = if let Some(target_index) = confirmed {
                let mut candidate = candidates
                    .iter()
                    .find(|candidate| candidate.target == targets[target_index].id)
                    .cloned();
                if let Some(candidate) = &mut candidate {
                    candidate.warnings.push(
                        "Caller-confirmed mapping: automatic evidence exclusions and ambiguity do not veto this selection; the heuristic score is unchanged."
                            .into(),
                    );
                }
                candidate
            } else if excluded || abstain || self.config.one_to_one {
                None
            } else {
                candidates.iter().find(|c| c.eligible).cloned()
            };
            let decision = if confirmed.is_some() {
                Decision::Confirmed
            } else if excluded {
                Decision::ExcludedByCaller
            } else if abstain {
                Decision::Ambiguous
            } else if best_score.is_some() {
                Decision::Proposed
            } else if insufficient_evidence {
                Decision::InsufficientEvidence
            } else {
                Decision::BelowThreshold
            };
            // Independent decisions are complete now. Drop undisplayed reports
            // before the next source, while charging all evaluated explanations.
            if !self.config.one_to_one {
                candidates.truncate(self.config.max_candidates);
            }
            fields.push(FieldMatch {
                source: source.id.clone(),
                candidates,
                alternatives,
                selected,
                decision,
                diagnostics,
            });
        }
        let mut target_competition = Vec::new();
        let mut assignment_diagnostics = if self.config.global_diagnostics.max_solves == 0 {
            AssignmentDiagnostics::default()
        } else {
            AssignmentDiagnostics::not_applicable()
        };
        if self.config.one_to_one {
            let selected = assignment::solve(&matrix);
            if self.config.global_diagnostics.max_solves > 0 {
                assignment_diagnostics = crate::diagnostics::analyze(
                    &matrix,
                    &selected,
                    &sources.iter().map(|f| f.id.clone()).collect::<Vec<_>>(),
                    &targets.iter().map(|f| f.id.clone()).collect::<Vec<_>>(),
                    &self.config.global_diagnostics,
                );
            }
            let contention: Vec<_> = (0..targets.len())
                .map(|j| matrix.iter().filter(|row| row[j].is_some()).count() > 1)
                .collect();
            for (j, target) in targets.iter().enumerate().filter(|(j, _)| contention[*j]) {
                target_competition.push(TargetCompetition {
                    target: target.id.clone(),
                    sources: sources
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| matrix[*i][j].is_some())
                        .map(|(_, source)| source.id.clone())
                        .collect(),
                });
            }
            for (i, field) in fields.iter_mut().enumerate() {
                if matches!(
                    field.decision,
                    Decision::Confirmed | Decision::ExcludedByCaller
                ) {
                    continue;
                }
                for (j, target) in targets.iter().enumerate() {
                    if contention[j] && matrix[i][j].is_some() {
                        field
                            .diagnostics
                            .push(FieldDiagnostic::TargetCompetition(target.id.clone()));
                    }
                }
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
                        candidate.issues.push(CandidateIssue::TargetCompetition);
                    }
                    if let Some(preferred) = field.candidates.iter().find(|c| c.eligible) {
                        if preferred.target != candidate.target {
                            field.diagnostics.push(FieldDiagnostic::Displaced {
                                preferred_target: preferred.target.clone(),
                            });
                        }
                    }
                }
                if field.selected.is_none() && field.decision == Decision::Proposed {
                    field.decision = Decision::AssignmentConflict;
                    field
                        .diagnostics
                        .push(FieldDiagnostic::UnassignedByGlobalConstraint);
                }
            }
        }
        let used: BTreeSet<_> = fields
            .iter()
            .filter_map(|f| f.selected.as_ref().map(|c| &c.target))
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
            target_competition,
            assignment_diagnostics,
        })
    }

    fn evaluate_row(
        &self,
        source_index: usize,
        sources: &[&Field],
        targets: &[&Field],
        prepared: &[PreparedSignal<'_>],
        name_conflicts: Option<&(Vec<PreparedNameConflicts>, Vec<PreparedNameConflicts>)>,
        explanation_bytes: &mut usize,
    ) -> Result<Vec<Candidate>, MatchError> {
        targets
            .iter()
            .enumerate()
            .map(|(target_index, target)| {
                self.evaluate(
                    sources[source_index],
                    target,
                    prepared,
                    (source_index, target_index),
                    name_conflicts
                        .map(|(source, target)| (&source[source_index], &target[target_index])),
                    explanation_bytes,
                )
            })
            .collect()
    }

    fn evaluate(
        &self,
        source: &Field,
        target: &Field,
        prepared: &[PreparedSignal<'_>],
        (source_index, target_index): (usize, usize),
        name_conflicts: Option<(&PreparedNameConflicts, &PreparedNameConflicts)>,
        explanation_bytes: &mut usize,
    ) -> Result<Candidate, MatchError> {
        let mut signals = Vec::with_capacity(self.matchers.len());
        let mut score = 0.0;
        let mut name_support = false;
        let mut sample_support = false;
        for (signal, prepared) in self.matchers.iter().zip(prepared) {
            if signal.weight == 0.0 {
                continue;
            }
            let evidence = match prepared {
                PreparedSignal::Names {
                    matcher,
                    source,
                    target,
                } => matcher.evaluate_prepared(&source[source_index], &target[target_index]),
                PreparedSignal::Samples {
                    matcher,
                    source,
                    target,
                } => matcher.evaluate_prepared(&source[source_index], &target[target_index]),
                PreparedSignal::Direct => signal.matcher.evaluate(source, target),
                PreparedSignal::Profiles {
                    matcher,
                    source,
                    target,
                } => matcher.evaluate_prepared(&source[source_index], &target[target_index]),
            }
            .map_err(|_| MatchError::MatcherFailed {
                name: signal.matcher.name().to_owned(),
            })?;
            if evidence
                .score
                .is_some_and(|v| !v.is_finite() || !(0.0..=1.0).contains(&v))
            {
                return Err(MatchError::InvalidMatcherScore {
                    name: signal.matcher.name().to_owned(),
                });
            }
            if evidence.explanation.len() > 4096 {
                return Err(MatchError::BudgetExceeded(BudgetKind::SignalExplanation));
            }
            *explanation_bytes = explanation_bytes
                .checked_add(evidence.explanation.len())
                .ok_or(MatchError::CountOverflow(CountKind::ExplanationBytes))?;
            if *explanation_bytes > self.config.limits.max_explanation_bytes {
                return Err(MatchError::BudgetExceeded(BudgetKind::ExplanationBytes));
            }
            if let Some(requirements) = self.config.corroboration {
                // Consume the evidence actually produced by concrete prepared
                // built-ins. Public matcher names are identifiers, not evidence
                // capabilities. No extra evaluation or scan is performed.
                match prepared {
                    PreparedSignal::Names { .. } => {
                        name_support |= evidence.score.is_some_and(|value| {
                            value > 0.0 && value >= requirements.min_name_score
                        });
                    }
                    PreparedSignal::Samples { matcher, .. }
                        if matcher.reliability == SampleReliability::Distinct =>
                    {
                        sample_support |= evidence.score.is_some_and(|value| {
                            value > 0.0 && value >= requirements.min_sample_score
                        });
                    }
                    _ => {}
                }
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
        let mut issues = Vec::new();
        if incompatible {
            warnings.push("Declared types are incompatible; no conversion is inferred.".into());
            issues.push(CandidateIssue::IncompatibleTypes);
        }
        if signals.iter().any(|s| s.evidence.score.is_none()) {
            warnings.push(
                "Some evidence is unavailable or insufficient; missing weight contributes zero."
                    .into(),
            );
            issues.push(CandidateIssue::MissingEvidence);
        }
        if score <= 0.0 || score < self.config.min_score {
            issues.push(CandidateIssue::InsufficientScore);
        }
        if let Some(requirements) = self.config.corroboration {
            if !name_support {
                issues.push(CandidateIssue::InsufficientNameSupport);
                warnings.push(format!(
                    "Corroboration requires a positive active built-in name score of at least {}; pair excluded.",
                    requirements.min_name_score,
                ));
            }
            if !sample_support {
                issues.push(CandidateIssue::InsufficientSampleSupport);
                warnings.push(format!(
                    "Corroboration requires an active distinct-aware built-in sample score of at least {}; pair excluded.",
                    requirements.min_sample_score,
                ));
            }
        }
        let mut name_conflict = false;
        if let Some((source, target)) = name_conflicts {
            for kind in source.conflicts_with(target, &self.name_conflicts) {
                name_conflict = true;
                let issue = CandidateIssue::NameConflict(kind);
                if !issues.contains(&issue) {
                    issues.push(issue);
                    warnings.push(match kind {
                        NameConflictKind::Qualifier => {
                            "Configured qualifier meanings conflict in normalized names; automatic pair excluded."
                                .into()
                        }
                        NameConflictKind::Unit => {
                            "Configured unit meanings conflict in normalized names; automatic pair excluded, no conversion inferred."
                                .into()
                        }
                    });
                }
            }
        }
        for (axis, source, target) in [
            (SemanticAxis::Unit, &source.hints.unit, &target.hints.unit),
            (
                SemanticAxis::Currency,
                &source.hints.currency,
                &target.hints.currency,
            ),
            (
                SemanticAxis::IdentifierScope,
                &source.hints.identifier_scope,
                &target.hints.identifier_scope,
            ),
        ] {
            match (source, target) {
                (Some(a), Some(b)) if a != b => issues.push(CandidateIssue::SemanticConflict(axis)),
                (Some(_), Some(_)) => issues.push(CandidateIssue::SemanticAgreement(axis)),
                (Some(_), None) | (None, Some(_)) => {
                    issues.push(CandidateIssue::SemanticMissing(axis))
                }
                (None, None) => {}
            }
        }
        let semantic_conflict = source.hints.compare(&target.hints, &mut warnings);
        Ok(Candidate {
            target: target.id.clone(),
            score,
            eligible: score > 0.0
                && score >= self.config.min_score
                && (self.config.corroboration.is_none() || (name_support && sample_support))
                && !name_conflict
                && !semantic_conflict
                && !(incompatible && self.config.reject_incompatible_types),
            signals,
            warnings,
            issues,
        })
    }

    fn validate_schema(&self, schema: &Schema, total_bytes: &mut usize) -> Result<(), MatchError> {
        let limits = &self.config.limits;
        if schema.fields.len() > limits.max_fields {
            return Err(MatchError::BudgetExceeded(BudgetKind::Fields));
        }
        let mut ids = BTreeSet::new();
        for field in &schema.fields {
            field.hints.validate()?;
            if field.id.0.is_empty() || !ids.insert(&field.id) {
                return Err(MatchError::InvalidInput(InputError::InvalidFieldIds));
            }
            if field.name.len() > limits.max_name_bytes || field.id.0.len() > limits.max_name_bytes
            {
                return Err(MatchError::BudgetExceeded(BudgetKind::NameBytes));
            }
            if let Some(samples) = &field.samples {
                if samples.len() > limits.max_samples_per_field {
                    return Err(MatchError::BudgetExceeded(BudgetKind::SamplesPerField));
                }
                for value in samples {
                    match value {
                        SampleValue::Number(n) if !n.is_finite() => {
                            return Err(MatchError::InvalidInput(InputError::NonFiniteSample))
                        }
                        SampleValue::Text(s) => {
                            if s.len() > limits.max_sample_bytes {
                                return Err(MatchError::BudgetExceeded(
                                    BudgetKind::TextSampleBytes,
                                ));
                            }
                            *total_bytes = total_bytes
                                .checked_add(s.len())
                                .ok_or(MatchError::CountOverflow(CountKind::SampleBytes))?;
                            if *total_bytes > limits.max_total_sample_bytes {
                                return Err(MatchError::BudgetExceeded(
                                    BudgetKind::TotalSampleBytes,
                                ));
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
    crate::name_conflicts::validate(&config.name_conflicts)?;
    if !config.min_score.is_finite()
        || !(0.0..=1.0).contains(&config.min_score)
        || !config.ambiguity_margin.is_finite()
        || !(0.0..=1.0).contains(&config.ambiguity_margin)
    {
        return Err(MatchError::InvalidConfiguration(
            ConfigurationError::ThresholdOrMargin,
        ));
    }
    if config.max_candidates == 0 {
        return Err(MatchError::InvalidConfiguration(
            ConfigurationError::MaxCandidates,
        ));
    }
    if config.corroboration.is_some_and(|requirements| {
        !requirements.min_name_score.is_finite()
            || !(0.0..=1.0).contains(&requirements.min_name_score)
            || !requirements.min_sample_score.is_finite()
            || requirements.min_sample_score <= 0.0
            || requirements.min_sample_score > 1.0
    }) {
        return Err(MatchError::InvalidConfiguration(
            ConfigurationError::Corroboration,
        ));
    }
    if !config.global_diagnostics.objective_margin.is_finite()
        || config.global_diagnostics.objective_margin < 0.0
        || config.global_diagnostics.max_solves > 1024
    {
        return Err(MatchError::InvalidConfiguration(
            ConfigurationError::GlobalDiagnostics,
        ));
    }
    Ok(())
}
