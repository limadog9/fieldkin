//! Opt-in contextual evidence derived from validated observable inputs.

use std::collections::{BTreeMap, BTreeSet};

use crate::semantic_name::{
    identifier_token, normalize_word_forms, structural_token, temporal_text, NameScope,
};
use crate::{
    normalize_name, BudgetKind, Candidate, CandidateIssue, Config, CountKind, DataType, Field,
    MatchError, NameConflictKind, NameConflictRule, NameMatcher, SampleValue,
};

const MIN_DISTINCT: usize = 3;
const MIN_COVERAGE: f64 = 0.25;
const MIN_JACCARD: f64 = 0.9;
const SAMPLE_MARGIN: f64 = 0.1;

/// Experimental, opt-in lexical and distinctive-sample evidence policy.
///
/// This policy requires active concrete built-in name and default distinct-aware
/// sample signals. Contextual name preparation applies evidence-scoped event
/// word forms, reported in the name signal. It transforms the aggregate score
/// to the maximum of its weighted sum, 0.90 for supported informative lexical
/// agreement, and 0.95 for adequate sample overlap. These fixed floors are
/// heuristic choices, never calibrated confidence. Selection additionally needs
/// contextual support; sample overlap alone does not establish shared meaning.
/// The shared vocabulary equates `expiry`/`expiration` only in temporal
/// contexts, and `settled`/`settlement` and `reversed`/`reversal` in temporal
/// or Boolean contexts. Types or typed samples establish these contexts.
/// Boolean `is`/`flag` and temporal suffixes are structural only in those
/// contexts. `enabled` retains its meaning, but alone is a generic status
/// without informative entity or event support; it is never aliased to `active`.
///
/// By default identifiers, temporal events, Boolean predicates and named
/// consent/event roles require their complete canonical cores to agree. The
/// named [`Config::contextual_quality`] preset separates name agreement from
/// actual conflicts and recognizes paired regular identifier noun inflections.
/// Different business nouns remain unresolved even with exclusive identical
/// samples; this policy supplies no inferred entity synonyms. Explicit directions
/// also retain token order. Negation and estimated/actual qualifiers cannot be
/// erased by sampled agreement. Declared Date and Timestamp remain distinct
/// representations; this policy never supplies conversions. Configured unit,
/// qualifier and verified semantic conflicts continue to exclude pairs.
///
/// Context uses all hard-compatible original pairs before caller review, local
/// ambiguity, assignment and display truncation. No labels, stored mappings or
/// caller confirmations inform the derived evidence. Defaults are unchanged
/// unless this policy is explicitly enabled in [`Config::contextual_evidence`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContextualEvidence {
    /// Require distinctive samples for identifier fields even when samples are
    /// unavailable on the original lexical path. Defaults to true. False permits
    /// informative name agreement when at least one identifier field has no
    /// observed samples. The separately enabled independent-population path has
    /// its own informative-name, representation and separation requirements.
    pub strict_identifier_samples: bool,
    /// Require adequate nonempty observed samples for non-Boolean informative
    /// lexical matches, representation-qualified singleton measurements, and
    /// shared temporal roles. Known Integer fields with exclusively integral
    /// observations are exempt from the measurement restriction. Ignore exact
    /// duplicate source observations as sample competitors; target duplicates
    /// remain competing choices. Defaults to true.
    /// Setting false disables the observation/measurement heuristics, while full
    /// entity/event/predicate roles, explicit direction, polarity, qualifiers and
    /// declared Date/Timestamp representation remain safeguards.
    pub scoped_support: bool,
    /// Separate supported equivalence, unresolved names and visible conflict.
    /// Contradictory pairs do not participate in either sample-contrast axis.
    /// Unresolved pairs remain competitors regardless of their support score.
    /// Defaults to false, retaining the original contextual policy.
    pub distinguish_relationships: bool,
    /// Permit informative equivalent roles from independently sampled populations.
    /// Missing or disjoint values are inconclusive; names, representation and
    /// separation from plausible alternatives must support the correspondence.
    /// Samples never establish meaning by themselves. Defaults to false.
    pub independent_sample_populations: bool,
    /// Keep the weighted score for ranking and assignment. A nondistinctive
    /// competing target supporting the same complete role excludes automatic
    /// support before thresholds, caller review or top-k can narrow the choice.
    /// The original
    /// contextual floors still provide a separate acceptance score checked
    /// against `min_score`; they do not compress competing ranking scores.
    /// Defaults to false. Neither score is a calibrated probability.
    pub preserve_score_ranking: bool,
    /// Recognize paired regular English singular/plural nouns in informative
    /// identifier roles, such as `warehouse_id` versus `warehouses_code`.
    /// This does not stem arbitrary names or infer synonyms. Defaults to false.
    pub identifier_word_forms: bool,
}

impl Default for ContextualEvidence {
    fn default() -> Self {
        Self {
            strict_identifier_samples: true,
            scoped_support: true,
            distinguish_relationships: false,
            independent_sample_populations: false,
            preserve_score_ranking: false,
            identifier_word_forms: false,
        }
    }
}

/// Inspectable contextual relationship and support diagnostics.
/// Reasons can overlap: a pair may have unresolved wording, missing samples
/// and a plausible competitor. They never contain observed sample values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ContextualReason {
    /// Explicit meaning, qualifier, direction or representation disagrees.
    Contradiction,
    /// Informative canonical names support the same complete role.
    SupportedEquivalence,
    /// Different or uninformative names establish neither agreement nor conflict.
    UnresolvedRelationship,
    /// The strict identifier path lacks nonempty identifier observations.
    MissingIdentifierSamples,
    /// Available observations do not provide adequate corroboration.
    InsufficientSampleSupport,
    /// A noncontradictory competing pair prevents required separation.
    CompetingCandidate,
    /// Generic or opaque names do not establish a useful concept.
    InsufficientInformativeName,
    /// Declared temporal representation or explicit units differ or are missing.
    RepresentationConflict,
}

#[derive(Clone, Copy, Default, PartialEq, Eq)]
enum Relationship {
    Contradiction,
    Supported,
    #[default]
    Unresolved,
}

const ROLE_AXES: &[&[&str]] = &[
    &["customer", "supplier"],
    &[
        "customer", "supplier", "account", "order", "invoice", "product", "payment", "refund",
        "shipment", "coupon",
    ],
    &[
        "created",
        "updated",
        "modified",
        "received",
        "dispatched",
        "expiration",
        "settlement",
        "reversal",
        "settled",
        "reversed",
    ],
    &["active", "enabled"],
];

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum SampleKey<'a> {
    Null,
    Boolean(bool),
    Number(u64),
    Integer(i128),
    Decimal(i128, u8),
    Text(&'a str),
}

fn observation_sample(value: &SampleValue) -> SampleKey<'_> {
    match value {
        SampleValue::Null => SampleKey::Null,
        SampleValue::Boolean(value) => SampleKey::Boolean(*value),
        SampleValue::Number(value) => {
            SampleKey::Number(if *value == 0.0 { 0 } else { value.to_bits() })
        }
        SampleValue::Integer(value) => SampleKey::Integer(*value),
        SampleValue::Decimal(value) => SampleKey::Decimal(value.coefficient(), value.scale()),
        SampleValue::Text(value) => SampleKey::Text(value),
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct ObservationKey<'a> {
    name: &'a str,
    data_type: u8,
    samples: Option<Vec<SampleKey<'a>>>,
    unit: Option<&'a str>,
    currency: Option<&'a str>,
    identifier_scope: Option<&'a str>,
}

fn source_groups(sources: &[&Field], scoped: bool) -> Vec<usize> {
    if !scoped {
        return (0..sources.len()).collect();
    }
    let mut observations = BTreeMap::new();
    sources
        .iter()
        .map(|field| {
            let key = ObservationKey {
                name: &field.name,
                data_type: field.data_type as u8,
                samples: field
                    .samples
                    .as_ref()
                    .map(|values| values.iter().map(observation_sample).collect()),
                unit: field.hints.unit.as_deref(),
                currency: field.hints.currency.as_deref(),
                identifier_scope: field.hints.identifier_scope.as_deref(),
            };
            let next_group = observations.len();
            *observations.entry(key).or_insert(next_group)
        })
        .collect()
}

#[derive(Clone, Copy, Default)]
struct PairFeatures {
    jaccard: f64,
    hard_compatible: bool,
    sample_adequate: bool,
    relationship: Relationship,
}

// Keep the two best distinct competing identities. Source identities can be
// observation groups; target identities always remain individual columns.
#[derive(Clone, Copy, Default)]
struct TopTwo {
    first: Option<(usize, f64)>,
    second: Option<(usize, f64)>,
}

impl TopTwo {
    fn insert(&mut self, identity: usize, score: f64) {
        if let Some((key, value)) = &mut self.first {
            if *key == identity {
                *value = value.max(score);
                return;
            }
        }
        if let Some((key, value)) = &mut self.second {
            if *key == identity {
                *value = value.max(score);
                if self.first.is_none_or(|(_, first)| score > first) {
                    std::mem::swap(&mut self.first, &mut self.second);
                }
                return;
            }
        }
        if self.first.is_none_or(|(_, first)| score > first) {
            self.second = self.first;
            self.first = Some((identity, score));
        } else if self.second.is_none_or(|(_, second)| score > second) {
            self.second = Some((identity, score));
        }
    }

    fn competing_score(self, identity: usize) -> Option<f64> {
        if self.first.is_some_and(|(key, _)| key == identity) {
            self.second.map(|(_, score)| score)
        } else {
            self.first.map(|(_, score)| score)
        }
    }
}

/// Apply contextual support to every evaluated pair before caller constraints.
pub(crate) fn apply(
    config: &Config,
    policy: ContextualEvidence,
    sources: &[&Field],
    targets: &[&Field],
    candidates: &mut [Vec<Candidate>],
    signals: (&NameMatcher, usize),
    explanation_bytes: &mut usize,
) -> Result<(), MatchError> {
    // Disabled/empty products do not evaluate matcher configuration. In
    // particular, aliases on the nonempty side have not been validated yet.
    if sources.is_empty() || targets.is_empty() {
        return Ok(());
    }
    let (name_matcher, sample_signal) = signals;
    let repaired = policy.distinguish_relationships
        || policy.independent_sample_populations
        || policy.preserve_score_ranking
        || policy.identifier_word_forms;
    let units = unit_phrases(&config.name_conflicts);
    let source_features: Vec<_> = sources
        .iter()
        .map(|field| FieldFeatures::new(field, &units, name_matcher))
        .collect();
    let target_features: Vec<_> = targets
        .iter()
        .map(|field| FieldFeatures::new(field, &units, name_matcher))
        .collect();
    let groups = source_groups(sources, policy.scoped_support);
    let mut target_core_counts = BTreeMap::new();
    for feature in &target_features {
        *target_core_counts.entry(&feature.core).or_insert(0usize) += 1;
    }
    let mut pairs = vec![vec![PairFeatures::default(); targets.len()]; sources.len()];
    let mut row_best = vec![TopTwo::default(); sources.len()];
    let mut column_best = vec![TopTwo::default(); targets.len()];
    let mut row_names = vec![TopTwo::default(); sources.len()];
    let mut column_names = vec![TopTwo::default(); targets.len()];
    for (source, row) in candidates.iter().enumerate() {
        for (target, candidate) in row.iter().enumerate() {
            let coverage = source_features[source]
                .coverage
                .min(target_features[target].coverage);
            // The engine supplies the concrete prepared sample signal's report
            // position, never a caller-controlled name string.
            let sample = candidate.signals[sample_signal].evidence.score;
            let jaccard = if coverage > 0.0 {
                (sample.unwrap_or(0.0) / coverage).clamp(0.0, 1.0)
            } else {
                0.0
            };
            let hard_compatible = !candidate.issues.iter().any(|issue| match issue {
                CandidateIssue::IncompatibleTypes => config.reject_incompatible_types,
                CandidateIssue::SemanticConflict(_) | CandidateIssue::NameConflict(_) => true,
                _ => false,
            });
            let relationship = if !repaired {
                Relationship::Unresolved
            } else if hard_compatible {
                source_features[source].relationship(
                    &target_features[target],
                    sources[source].data_type,
                    targets[target].data_type,
                    policy.identifier_word_forms,
                )
            } else {
                Relationship::Contradiction
            };
            let pair = PairFeatures {
                jaccard,
                hard_compatible,
                sample_adequate: source_features[source].distinct >= MIN_DISTINCT
                    && target_features[target].distinct >= MIN_DISTINCT
                    && coverage >= MIN_COVERAGE
                    && jaccard >= MIN_JACCARD,
                relationship,
            };
            pairs[source][target] = pair;
            if pair.hard_compatible
                && (!policy.distinguish_relationships
                    || relationship != Relationship::Contradiction)
            {
                row_best[source].insert(target, jaccard);
                column_best[target].insert(groups[source], jaccard);
                if repaired {
                    let name_support = if relationship == Relationship::Supported {
                        1.0
                    } else {
                        source_features[source].core_support(&target_features[target])
                    };
                    row_names[source].insert(target, name_support);
                    column_names[target].insert(groups[source], name_support);
                }
            }
        }
    }
    for (source, feature) in source_features.iter().enumerate() {
        for (target, target_feature) in target_features.iter().enumerate() {
            let pair = pairs[source][target];
            let distinctive = pair.hard_compatible
                && pair.sample_adequate
                && row_best[source]
                    .competing_score(target)
                    .is_none_or(|other| pair.jaccard >= other + SAMPLE_MARGIN)
                && column_best[target]
                    .competing_score(groups[source])
                    .is_none_or(|other| pair.jaccard >= other + SAMPLE_MARGIN);
            let informed_exact = feature.informative > 0
                && (feature.core == target_feature.core
                    || (policy.identifier_word_forms
                        && pair.relationship == Relationship::Supported))
                && !feature.core.is_empty();
            let name_separated = row_names[source]
                .competing_score(target)
                .is_none_or(|other| 1.0 >= other + SAMPLE_MARGIN)
                && column_names[target]
                    .competing_score(groups[source])
                    .is_none_or(|other| 1.0 >= other + SAMPLE_MARGIN);
            // A name-supported population path cannot dismiss an opaque field
            // carrying equally strong sample evidence. Unresolved competitors
            // survive contrast even when they lack support for selection.
            let sample_separated = row_best[source]
                .competing_score(target)
                .is_none_or(|other| other < MIN_JACCARD || pair.jaccard >= other + SAMPLE_MARGIN)
                && column_best[target]
                    .competing_score(groups[source])
                    .is_none_or(|other| {
                        other < MIN_JACCARD || pair.jaccard >= other + SAMPLE_MARGIN
                    });
            let population_lexical = policy.independent_sample_populations
                && pair.relationship == Relationship::Supported
                && feature.name_informative
                && target_feature.name_informative
                && [feature, target_feature].iter().all(|field| {
                    field.non_null == 0
                        || field.boolean
                        || (field.distinct >= MIN_DISTINCT && field.coverage >= MIN_COVERAGE)
                })
                && (feature.sample_representation == 0
                    || target_feature.sample_representation == 0
                    || feature.sample_representation == target_feature.sample_representation)
                && name_separated
                && sample_separated;
            let identifier = feature.identifier || target_feature.identifier;
            let identifier_support = !identifier
                || distinctive
                || population_lexical
                || (!policy.strict_identifier_samples
                    && (!feature.observed || !target_feature.observed));
            let both_observed = if identifier {
                feature.observed && target_feature.observed
            } else {
                feature.non_null > 0 && target_feature.non_null > 0
            };
            let observed_support = !policy.scoped_support
                || population_lexical
                || !both_observed
                || (feature.boolean && target_feature.boolean)
                || if identifier {
                    distinctive
                } else {
                    pair.sample_adequate
                };
            // Sample overlap cannot erase an explicit entity, event or predicate
            // distinction. Temporal roles must agree completely: a shared
            // "shipment" token does not equate received and dispatched events.
            let scoped_role_agrees = !(identifier
                || feature.boolean
                || target_feature.boolean
                || feature.temporal
                || target_feature.temporal
                || feature.named_role
                || target_feature.named_role)
                || if policy.distinguish_relationships {
                    pair.relationship == Relationship::Supported
                } else {
                    feature.core == target_feature.core
                };
            let modifiers_agree = feature.modifiers == target_feature.modifiers;
            let directed_role_agrees = !(feature.directional || target_feature.directional)
                || feature.ordered_core == target_feature.ordered_core;
            let representation_agrees = !matches!(
                (sources[source].data_type, targets[target].data_type),
                (DataType::Date, DataType::Timestamp) | (DataType::Timestamp, DataType::Date)
            );
            let lexical = informed_exact && identifier_support && observed_support;
            let duplicate_generic = feature.informative == 0
                && target_feature.informative == 0
                && !feature.core.is_empty()
                && feature.core == target_feature.core
                && target_core_counts[&target_feature.core] >= 2;
            let recovered = distinctive
                && scoped_role_agrees
                && (feature.informative > 0 || target_feature.informative > 0 || duplicate_generic);
            let unqualified_numeric = policy.scoped_support
                && !identifier
                && (feature.representation_sensitive || target_feature.representation_sensitive)
                && feature.core.len() == 1
                && target_feature.core.len() == 1
                && feature.units.iter().all(|meaning| *meaning == 0)
                && target_feature.units.iter().all(|meaning| *meaning == 0);
            let units_agree = feature.units_agree(target_feature);
            // Equivalent role peers are evidence of an unresolved choice even
            // when different syntactic scores or a caller's threshold would
            // leave only one eligible edge. Derive the tie from all original
            // pairs; neither top-k nor caller exclusions can manufacture it away.
            let canonical_tie = repaired
                && pair.relationship == Relationship::Supported
                && !distinctive
                && row_names[source].competing_score(target) == Some(1.0);
            let allowed = pair.hard_compatible
                && !canonical_tie
                && (!policy.distinguish_relationships
                    || (pair.relationship != Relationship::Contradiction
                        && (feature.role_informative || target_feature.role_informative)
                        && feature.protected_scope == target_feature.protected_scope
                        && feature.numeric_qualifiers == target_feature.numeric_qualifiers))
                && scoped_role_agrees
                && modifiers_agree
                && directed_role_agrees
                && representation_agrees
                && !unqualified_numeric
                && units_agree
                && (lexical || recovered);
            let candidate = &mut candidates[source][target];
            let base = candidate.score;
            let acceptance_score = base
                .max(if lexical { 0.9 } else { 0.0 })
                .max(if pair.sample_adequate { 0.95 } else { 0.0 })
                .clamp(0.0, 1.0);
            candidate.score = if policy.preserve_score_ranking && !canonical_tie {
                base
            } else {
                acceptance_score
            };
            candidate
                .issues
                .retain(|issue| *issue != CandidateIssue::InsufficientScore);
            if candidate.score <= 0.0 || acceptance_score < config.min_score {
                candidate.issues.push(CandidateIssue::InsufficientScore);
            }
            if candidate.score > base {
                candidate
                    .issues
                    .push(CandidateIssue::ContextualScoreAdjustment);
            }
            if !allowed {
                candidate
                    .issues
                    .push(CandidateIssue::InsufficientContextSupport);
            }
            if repaired {
                candidate
                    .issues
                    .push(CandidateIssue::ContextualReason(match pair.relationship {
                        Relationship::Contradiction => ContextualReason::Contradiction,
                        Relationship::Supported => ContextualReason::SupportedEquivalence,
                        Relationship::Unresolved => ContextualReason::UnresolvedRelationship,
                    }));
                if identifier && (feature.non_null == 0 || target_feature.non_null == 0) {
                    candidate.issues.push(CandidateIssue::ContextualReason(
                        ContextualReason::MissingIdentifierSamples,
                    ));
                }
                if both_observed
                    && !pair.sample_adequate
                    && !(feature.boolean && target_feature.boolean)
                {
                    candidate.issues.push(CandidateIssue::ContextualReason(
                        ContextualReason::InsufficientSampleSupport,
                    ));
                }
                if !name_separated || (pair.sample_adequate && !distinctive) || !sample_separated {
                    candidate.issues.push(CandidateIssue::ContextualReason(
                        ContextualReason::CompetingCandidate,
                    ));
                }
                if !feature.name_informative || !target_feature.name_informative {
                    candidate.issues.push(CandidateIssue::ContextualReason(
                        ContextualReason::InsufficientInformativeName,
                    ));
                }
                if !representation_agrees || !units_agree || unqualified_numeric {
                    candidate.issues.push(CandidateIssue::ContextualReason(
                        ContextualReason::RepresentationConflict,
                    ));
                }
            }
            candidate.eligible = allowed
                && candidate.score > 0.0
                && acceptance_score >= config.min_score
                && !candidate.issues.iter().any(|issue| {
                    matches!(
                        issue,
                        CandidateIssue::InsufficientNameSupport
                            | CandidateIssue::InsufficientSampleSupport
                    )
                });
            let mut explanation = format!(
                "Contextual evidence: weighted score {base:.3}, transformed score {:.3}; informative tokens {}/{}; distinct samples {}/{}; lexical support {lexical}; distinctive sample support {distinctive}; unit representation agreement {units_agree}; unqualified numeric {unqualified_numeric}; scoped role agreement {scoped_role_agrees}; directed role agreement {directed_role_agrees}; modifier agreement {modifiers_agree}; temporal representation agreement {representation_agrees}; contextual support {allowed}. Fixed score floors are heuristics, not calibrated confidence.",
                candidate.score,
                feature.informative,
                target_feature.informative,
                feature.distinct,
                target_feature.distinct,
            );
            if repaired {
                use std::fmt::Write;
                let relationship = match pair.relationship {
                    Relationship::Contradiction => "contradiction",
                    Relationship::Supported => "supported equivalence",
                    Relationship::Unresolved => "unresolved",
                };
                // Writing to String is infallible; charge the complete expanded
                // explanation below, including explicitly stated assumptions.
                let _ = write!(explanation,
                    " Relationship {relationship}; separate acceptance score {acceptance_score:.3}; ranking preserved {}; canonical role tie {canonical_tie}; independent sampling {}; informative population support {population_lexical}; name separation {name_separated}; sample separation {sample_separated}.",
                    policy.preserve_score_ranking, policy.independent_sample_populations,
                );
            }
            *explanation_bytes = explanation_bytes
                .checked_add(explanation.len())
                .ok_or(MatchError::CountOverflow(CountKind::ExplanationBytes))?;
            if *explanation_bytes > config.limits.max_explanation_bytes {
                return Err(MatchError::BudgetExceeded(BudgetKind::ExplanationBytes));
            }
            candidate.warnings.push(explanation);
        }
    }
    Ok(())
}

struct FieldFeatures {
    core: BTreeSet<String>,
    ordered_core: Vec<String>,
    informative: usize,
    name_informative: bool,
    role_informative: bool,
    modifiers: BTreeSet<String>,
    protected_scope: BTreeSet<String>,
    named_role: bool,
    directional: bool,
    identifier: bool,
    observed: bool,
    non_null: usize,
    distinct: usize,
    coverage: f64,
    units: Vec<u16>,
    representation_sensitive: bool,
    boolean: bool,
    temporal: bool,
    sample_representation: u8,
    role_axes: [u16; 4],
    numeric_qualifiers: BTreeSet<String>,
}

struct UnitPhrase {
    tokens: Vec<String>,
    meaning: u16,
}

fn unit_phrases(rules: &[NameConflictRule]) -> Vec<Vec<UnitPhrase>> {
    rules
        .iter()
        .filter(|rule| rule.kind == NameConflictKind::Unit)
        .map(|rule| {
            let mut phrases: Vec<_> = rule
                .alternatives
                .iter()
                .enumerate()
                .flat_map(|(index, alternatives)| {
                    alternatives.iter().map(move |phrase| UnitPhrase {
                        tokens: phrase.split(' ').map(str::to_owned).collect(),
                        meaning: 1 << index,
                    })
                })
                .collect();
            phrases.sort_unstable_by(|a, b| {
                b.tokens
                    .len()
                    .cmp(&a.tokens.len())
                    .then(a.tokens.cmp(&b.tokens))
            });
            phrases
        })
        .collect()
}

fn recognized_units(tokens: &[String], phrases: &[UnitPhrase]) -> u16 {
    let mut longest = vec![0; tokens.len()];
    let mut meanings = 0;
    for phrase in phrases {
        let length = phrase.tokens.len();
        if length > tokens.len() {
            continue;
        }
        for (start, window) in tokens.windows(length).enumerate() {
            let span = &mut longest[start..start + length];
            if window == phrase.tokens && span.iter().all(|covered| *covered <= length) {
                meanings |= phrase.meaning;
                span.fill(length);
            }
        }
    }
    meanings
}

fn generic_token(token: &str) -> bool {
    matches!(
        token,
        "id" | "name"
            | "status"
            | "enabled"
            | "value"
            | "count"
            | "amount"
            | "price"
            | "region"
            | "location"
            | "time"
            | "date"
            | "timestamp"
            | "created"
            | "updated"
            | "modified"
            | "score"
            | "label"
            | "data"
            | "text"
            | "field"
            | "column"
            | "total"
    )
}

fn opaque_role_token(token: &str) -> bool {
    matches!(
        token,
        "opaque"
            | "unknown"
            | "attribute"
            | "attr"
            | "record"
            | "entry"
            | "object"
            | "entity"
            | "event"
            | "metric"
            | "measurement"
            | "col"
            | "fld"
    )
}

impl FieldFeatures {
    fn new(field: &Field, unit_rules: &[Vec<UnitPhrase>], name_matcher: &NameMatcher) -> Self {
        let tokens = normalize_name(&field.name);
        let scope = NameScope::for_field(field);
        let mut canonical = tokens.clone();
        for token in &mut canonical {
            // The prepared name signal already validates used replacements.
            // Honor the same one-pass aliases, including a caller-cleared map.
            if let Some(to) = name_matcher.aliases.get(token) {
                *token = to.clone();
            }
        }
        normalize_word_forms(&mut canonical, scope);
        let ordered_core: Vec<_> = canonical
            .iter()
            .enumerate()
            .filter(|(index, token)| !structural_token(token, *index, canonical.len(), scope))
            .map(|(_, token)| token.clone())
            .collect();
        let core: BTreeSet<_> = ordered_core.iter().cloned().collect();
        let informative_core: BTreeSet<_> = core
            .iter()
            .filter(|token| {
                token.chars().count() > 2
                    && !token.chars().all(char::is_numeric)
                    && !generic_token(token)
            })
            .cloned()
            .collect();
        let mut values = BTreeSet::new();
        let mut non_null = 0;
        for value in field.samples.as_deref().unwrap_or_default() {
            let key = match value {
                SampleValue::Null => continue,
                SampleValue::Boolean(value) => SampleKey::Boolean(*value),
                SampleValue::Number(value) => {
                    SampleKey::Number(if *value == 0.0 { 0 } else { value.to_bits() })
                }
                SampleValue::Integer(value) => SampleKey::Integer(*value),
                SampleValue::Decimal(value) => {
                    SampleKey::Decimal(value.coefficient(), value.scale())
                }
                SampleValue::Text(value) => SampleKey::Text(value),
            };
            values.insert(key);
            non_null += 1;
        }
        let observations = field.samples.as_ref().map_or(0, Vec::len);
        let numeric = matches!(
            field.data_type,
            DataType::Integer | DataType::Float | DataType::Decimal
        ) || field
            .samples
            .as_deref()
            .unwrap_or_default()
            .iter()
            .any(|value| {
                matches!(
                    value,
                    SampleValue::Number(_) | SampleValue::Integer(_) | SampleValue::Decimal(_)
                )
            });
        // JSON importers may represent integral observations as Number. A known
        // Integer declaration supports their count/dimensionless representation;
        // fractional, decimal or non-numeric observations do not gain that exemption.
        let declared_integral = field.data_type == DataType::Integer
            && field
                .samples
                .as_deref()
                .unwrap_or_default()
                .iter()
                .all(|value| match value {
                    SampleValue::Null | SampleValue::Integer(_) => true,
                    SampleValue::Number(value) => value.fract() == 0.0,
                    _ => false,
                });
        // A declared integral count supplies a concrete dimensionless role for
        // otherwise abstract container nouns. It does not supply an identifier
        // namespace, nor make a plain generic Count or opaque code informative.
        let typed_count_role = declared_integral
            && canonical
                .iter()
                .any(|token| matches!(token.as_str(), "count" | "counter" | "quantity" | "total"))
            && informative_core.iter().any(|token| {
                matches!(
                    token.as_str(),
                    "event" | "record" | "entry" | "object" | "entity" | "metric" | "measurement"
                )
            });
        Self {
            role_informative: typed_count_role
                || informative_core
                    .iter()
                    .any(|token| !opaque_role_token(token)),
            role_axes: std::array::from_fn(|index| {
                ROLE_AXES[index]
                    .iter()
                    .enumerate()
                    .fold(0, |mask, (bit, token)| {
                        mask | if core.contains(*token) { 1 << bit } else { 0 }
                    })
            }),
            numeric_qualifiers: core
                .iter()
                .filter(|token| token.chars().all(char::is_numeric))
                .cloned()
                .collect(),
            // Reject conventional opaque placeholders and mixed numeric codes
            // on the new name-only path. This is a lexical quality heuristic,
            // not a dictionary or proof that an arbitrary word has a meaning.
            name_informative: informative_core.iter().any(|token| {
                token.chars().count() >= 4
                    && token.chars().all(char::is_alphabetic)
                    && !opaque_role_token(token)
            }),
            core,
            ordered_core,
            informative: informative_core.len(),
            // Observed agreement does not justify dropping explicit polarity or
            // measurement qualifiers, including a qualifier missing on one side.
            modifiers: tokens
                .iter()
                .filter(|token| {
                    matches!(
                        token.as_str(),
                        "no" | "not"
                            | "non"
                            | "without"
                            | "actual"
                            | "estimated"
                            | "forecast"
                            | "predicted"
                    )
                })
                .cloned()
                .collect(),
            protected_scope: tokens
                .iter()
                .filter(|token| {
                    matches!(
                        token.as_str(),
                        "external"
                            | "internal"
                            | "parent"
                            | "child"
                            | "billing"
                            | "shipping"
                            | "primary"
                            | "secondary"
                    )
                })
                .cloned()
                .collect(),
            // Preserve named event and consent roles even when their values are
            // represented as counts or text instead of dates and Booleans.
            named_role: tokens
                .iter()
                .any(|token| matches!(token.as_str(), "consent" | "received" | "dispatched")),
            directional: tokens
                .iter()
                .any(|token| matches!(token.as_str(), "from" | "to" | "by" | "for")),
            identifier: tokens.iter().any(|token| identifier_token(token)),
            observed: field.samples.is_some(),
            non_null,
            distinct: values.len(),
            coverage: if observations == 0 {
                0.0
            } else {
                non_null as f64 / observations as f64
            },
            units: unit_rules
                .iter()
                .map(|phrases| recognized_units(&tokens, phrases))
                .collect(),
            representation_sensitive: numeric && !declared_integral,
            boolean: scope.boolean,
            temporal: scope.temporal,
            sample_representation: field.samples.as_deref().unwrap_or_default().iter().fold(
                0,
                |mask, value| {
                    mask | match value {
                        SampleValue::Null => 0,
                        SampleValue::Boolean(_) => 1,
                        SampleValue::Number(_)
                        | SampleValue::Integer(_)
                        | SampleValue::Decimal(_) => 2,
                        SampleValue::Text(text) if temporal_text(text) => {
                            if text.len() == 10 {
                                8
                            } else {
                                16
                            }
                        }
                        SampleValue::Text(_) => 4,
                    }
                },
            ),
        }
    }

    fn units_agree(&self, target: &Self) -> bool {
        self.units
            .iter()
            .zip(&target.units)
            .all(|(source, target)| (*source == 0 && *target == 0) || (*source & *target != 0))
    }

    fn core_support(&self, target: &Self) -> f64 {
        let union = self.core.union(&target.core).count();
        if union == 0 {
            0.0
        } else {
            self.core.intersection(&target.core).count() as f64 / union as f64
        }
    }

    fn relationship(
        &self,
        target: &Self,
        source_type: DataType,
        target_type: DataType,
        identifier_word_forms: bool,
    ) -> Relationship {
        let approximation =
            |feature: &Self| {
                u8::from(feature.modifiers.contains("actual"))
                    | if feature.modifiers.iter().any(|token| {
                        matches!(token.as_str(), "estimated" | "forecast" | "predicted")
                    }) {
                        2
                    } else {
                        0
                    }
            };
        let negated = |feature: &Self| {
            feature
                .modifiers
                .iter()
                .any(|token| matches!(token.as_str(), "no" | "not" | "non" | "without"))
        };
        let explicit_scope_conflict = [
            ("external", "internal"),
            ("parent", "child"),
            ("billing", "shipping"),
            ("primary", "secondary"),
        ]
        .iter()
        .any(|(left, right)| {
            (self.protected_scope.contains(*left) && target.protected_scope.contains(*right))
                || (self.protected_scope.contains(*right) && target.protected_scope.contains(*left))
        });
        let direction_conflict = self.directional
            && target.directional
            && ((self.core == target.core && self.ordered_core != target.ordered_core)
                || (((self.core.contains("from")
                    && !self.core.contains("to")
                    && target.core.contains("to")
                    && !target.core.contains("from"))
                    || (self.core.contains("to")
                        && !self.core.contains("from")
                        && target.core.contains("from")
                        && !target.core.contains("to")))
                    && self
                        .core
                        .iter()
                        .filter(|token| !matches!(token.as_str(), "from" | "to"))
                        .eq(target
                            .core
                            .iter()
                            .filter(|token| !matches!(token.as_str(), "from" | "to")))));
        let unit_conflict = self
            .units
            .iter()
            .zip(&target.units)
            .any(|(left, right)| *left != 0 && *right != 0 && left & right == 0);
        if (approximation(self) != 0
            && approximation(target) != 0
            && approximation(self) & approximation(target) == 0)
            || (self.boolean && target.boolean && negated(self) != negated(target))
            || explicit_scope_conflict
            || (!self.numeric_qualifiers.is_empty()
                && !target.numeric_qualifiers.is_empty()
                && self.numeric_qualifiers != target.numeric_qualifiers)
            || direction_conflict
            || unit_conflict
            || (self.sample_representation != 0
                && target.sample_representation != 0
                && self.sample_representation & target.sample_representation == 0)
            || matches!(
                (source_type, target_type),
                (DataType::Date, DataType::Timestamp) | (DataType::Timestamp, DataType::Date)
            )
            || self
                .role_axes
                .iter()
                .zip(target.role_axes)
                .any(|(source, target)| *source != 0 && target != 0 && source & target == 0)
        {
            Relationship::Contradiction
        } else if self.modifiers != target.modifiers
            || self.protected_scope != target.protected_scope
            || self.numeric_qualifiers != target.numeric_qualifiers
            || ((self.directional || target.directional)
                && self.ordered_core != target.ordered_core)
            || !self.units_agree(target)
        {
            Relationship::Unresolved
        } else if self.informative > 0
            && self.role_informative
            && target.role_informative
            && !self.core.is_empty()
            && (self.core == target.core
                || (identifier_word_forms
                    && self.identifier
                    && target.identifier
                    && self.name_informative
                    && target.name_informative
                    && !self.boolean
                    && !target.boolean
                    && !self.temporal
                    && !target.temporal
                    && !self.named_role
                    && !target.named_role
                    && regular_identifier_word_forms(&self.core, &target.core)))
        {
            Relationship::Supported
        } else {
            Relationship::Unresolved
        }
    }
}

fn regular_identifier_word_forms(source: &BTreeSet<String>, target: &BTreeSet<String>) -> bool {
    fn singular(token: &str) -> Option<&str> {
        (token.len() >= 5
            && token.is_ascii()
            && token.chars().all(char::is_alphabetic)
            && !["ss", "us", "is", "ics"]
                .iter()
                .any(|suffix| token.ends_with(suffix)))
        .then(|| token.strip_suffix('s'))
        .flatten()
    }
    source.len() == target.len()
        && source.iter().all(|token| {
            target.contains(token)
                || singular(token).is_some_and(|form| target.contains(form))
                || target
                    .iter()
                    .any(|other| singular(other) == Some(token.as_str()))
        })
        && target.iter().all(|token| {
            source.contains(token)
                || singular(token).is_some_and(|form| source.contains(form))
                || source
                    .iter()
                    .any(|other| singular(other) == Some(token.as_str()))
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contrast_maxima_match_exhaustive_competitor_search_with_duplicate_groups() {
        let identities = [0, 1, 0, 2, 1, 3];
        let scores = [0.0, 0.2, 0.9, 1.0];
        for mut scenario in 0..4usize.pow(identities.len() as u32) {
            let mut best = TopTwo::default();
            let mut observations = Vec::new();
            for identity in identities {
                let score = scores[scenario % scores.len()];
                scenario /= scores.len();
                observations.push((identity, score));
                best.insert(identity, score);
            }
            for excluded in 0..5 {
                let expected = observations
                    .iter()
                    .filter(|(identity, _)| *identity != excluded)
                    .map(|(_, score)| *score)
                    .max_by(f64::total_cmp);
                assert_eq!(best.competing_score(excluded), expected);
            }
        }
    }
}
