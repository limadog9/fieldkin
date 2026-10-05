//! Caller-owned review decisions applied to one schema pair.

use std::collections::BTreeSet;

use crate::signals::type_compatibility;
use crate::{BudgetKind, Config, ConstraintError, Field, FieldId, MatchError, SemanticAxis};

/// A source-target pair expressed using stable IDs, not display names.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FieldPair {
    /// Source identity in the supplied source schema.
    pub source: FieldId,
    /// Target identity in the supplied target schema.
    pub target: FieldId,
}

impl FieldPair {
    /// Construct a pair. IDs are validated against the schemas when matching.
    pub fn new(source: impl Into<FieldId>, target: impl Into<FieldId>) -> Self {
        Self {
            source: source.into(),
            target: target.into(),
        }
    }
}

/// Review decisions supplied for one matching call. Fieldkin stores no review state.
///
/// Identical repeated directives are idempotent, but still count toward input
/// budgets. Each of `confirmed` and `unmatched_sources` is bounded by
/// [`crate::Limits::max_fields`]; `forbidden` is bounded by
/// [`crate::Limits::max_pairs`]. Referenced IDs obey `max_name_bytes` and must
/// exist in the corresponding schema. Schema/version ownership stays with the caller.
///
/// Confirmations override score, corroboration, lexical name conflicts and local
/// ambiguity, but never conflicting supplied semantics or an enabled declared-type
/// veto. They do not change evidence scores or turn those scores into confidence
/// probabilities.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MatchConstraints {
    /// Caller-confirmed pairs. One source cannot be confirmed to distinct targets.
    /// In one-to-one mode, distinct sources cannot confirm the same target.
    pub confirmed: Vec<FieldPair>,
    /// Pairs the caller excludes from automatic selection.
    /// A pair cannot be both forbidden and confirmed.
    pub forbidden: Vec<FieldPair>,
    /// Sources explicitly kept unmatched, even when strong evidence exists.
    /// A source cannot be both confirmed and kept unmatched.
    pub unmatched_sources: Vec<FieldId>,
}

pub(crate) struct ValidatedConstraints {
    pub(crate) confirmed: Vec<Option<usize>>,
    pub(crate) unmatched: Vec<bool>,
    pub(crate) reserved: Vec<Option<usize>>,
    forbidden: BTreeSet<(usize, usize)>,
}

impl ValidatedConstraints {
    pub(crate) fn forbidden(&self, source: usize, target: usize) -> bool {
        self.forbidden.contains(&(source, target))
    }
}

impl MatchConstraints {
    pub(crate) fn validate(
        &self,
        source: &[&Field],
        target: &[&Field],
        config: &Config,
    ) -> Result<Option<ValidatedConstraints>, MatchError> {
        if self.confirmed.len() > config.limits.max_fields
            || self.unmatched_sources.len() > config.limits.max_fields
            || self.forbidden.len() > config.limits.max_pairs
        {
            return Err(MatchError::BudgetExceeded(BudgetKind::Constraints));
        }
        if self.confirmed.is_empty()
            && self.forbidden.is_empty()
            && self.unmatched_sources.is_empty()
        {
            return Ok(None);
        }
        let locate = |fields: &[&Field], id: &FieldId, error| {
            if id.0.len() > config.limits.max_name_bytes {
                return Err(MatchError::BudgetExceeded(BudgetKind::NameBytes));
            }
            fields
                .binary_search_by(|field| field.id.cmp(id))
                .map_err(|_| MatchError::InvalidConstraints(error))
        };
        let mut result = ValidatedConstraints {
            confirmed: vec![None; source.len()],
            unmatched: vec![false; source.len()],
            reserved: vec![None; target.len()],
            forbidden: BTreeSet::new(),
        };
        for pair in &self.forbidden {
            let source = locate(source, &pair.source, ConstraintError::UnknownSource)?;
            let target = locate(target, &pair.target, ConstraintError::UnknownTarget)?;
            result.forbidden.insert((source, target));
        }
        for id in &self.unmatched_sources {
            let index = locate(source, id, ConstraintError::UnknownSource)?;
            result.unmatched[index] = true;
        }
        for pair in &self.confirmed {
            let i = locate(source, &pair.source, ConstraintError::UnknownSource)?;
            let j = locate(target, &pair.target, ConstraintError::UnknownTarget)?;
            if result.confirmed[i].is_some_and(|existing| existing != j) {
                return Err(MatchError::InvalidConstraints(
                    ConstraintError::ConflictingSource,
                ));
            }
            if config.one_to_one && result.reserved[j].is_some_and(|existing| existing != i) {
                return Err(MatchError::InvalidConstraints(
                    ConstraintError::ConflictingTarget,
                ));
            }
            if result.forbidden.contains(&(i, j)) {
                return Err(MatchError::InvalidConstraints(
                    ConstraintError::ConfirmedForbidden,
                ));
            }
            if result.unmatched[i] {
                return Err(MatchError::InvalidConstraints(
                    ConstraintError::ConfirmedUnmatched,
                ));
            }
            if config.reject_incompatible_types
                && type_compatibility(source[i].data_type, target[j].data_type) == Some(0.0)
            {
                return Err(MatchError::InvalidConstraints(
                    ConstraintError::IncompatibleTypes,
                ));
            }
            for (axis, source, target) in [
                (
                    SemanticAxis::Unit,
                    &source[i].hints.unit,
                    &target[j].hints.unit,
                ),
                (
                    SemanticAxis::Currency,
                    &source[i].hints.currency,
                    &target[j].hints.currency,
                ),
                (
                    SemanticAxis::IdentifierScope,
                    &source[i].hints.identifier_scope,
                    &target[j].hints.identifier_scope,
                ),
            ] {
                if matches!((source, target), (Some(a), Some(b)) if a != b) {
                    return Err(MatchError::InvalidConstraints(
                        ConstraintError::SemanticConflict(axis),
                    ));
                }
            }
            result.confirmed[i] = Some(j);
            if config.one_to_one {
                result.reserved[j] = Some(i);
            }
        }
        Ok(Some(result))
    }
}
