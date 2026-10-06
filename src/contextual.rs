//! Opt-in contextual evidence derived from validated observable inputs.

#[path = "sample_format.rs"]
mod sample_format;

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    normalize_name, BudgetKind, Candidate, CandidateIssue, Config, CountKind, DataType, Field,
    MatchError, NameConflictKind, NameConflictRule, SampleValue,
};

const MIN_DISTINCT: usize = 3;
const MIN_COVERAGE: f64 = 0.25;
const MIN_JACCARD: f64 = 0.9;
const SAMPLE_MARGIN: f64 = 0.1;

/// Experimental, opt-in lexical and distinctive-sample evidence policy.
///
/// This policy requires active concrete built-in name and default distinct-aware
/// sample signals. It retains their original evidence, but transforms the score
/// to the maximum of its weighted sum, 0.90 for supported informative lexical
/// agreement, and 0.95 for adequate sample overlap. These fixed floors are
/// heuristic choices, never calibrated confidence. Selection additionally needs
/// contextual support; sample overlap alone does not establish shared meaning.
///
/// A bounded Text-only format fallback can supply a 0.90 score floor. It needs
/// three distinct nonempty text values, at least 25% non-null coverage, format
/// agreement >=0.90, name evidence >=0.40 and mutual margins >=0.20. It never
/// adds to an already-supported row or target, and preserves explicit semantic,
/// unit and corroboration vetoes. Similar formats do not prove shared meaning.
///
/// A physical-measurement name can also supply a 0.90 floor when its complete
/// quantity, qualifiers, explicit supported unit token and numeric type agree.
/// This route preserves already-supported rows and targets, performs no unit or
/// sample conversion, and remains subject to normal ambiguity and constraints.
///
/// A trailing Text-field name suffix can supply a 0.90 floor when complete
/// ordered role tokens agree and both sides have three distinct descriptive
/// text samples with at least 25% coverage. Numeric/code-like or missing values
/// do not qualify. Competing full-role names, existing support and explicit
/// semantic or corroboration vetoes are preserved.
///
/// Context uses all hard-compatible original pairs before caller review, local
/// ambiguity, assignment and display truncation. No labels, stored mappings or
/// caller confirmations inform the derived evidence. Defaults are unchanged
/// unless this policy is explicitly enabled in [`Config::contextual_evidence`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContextualEvidence {
    /// Require observed identifier support: distinctive value overlap or the
    /// bounded text-format fallback. Defaults to true. False also permits
    /// informative names when at least one field has no observed samples.
    pub strict_identifier_samples: bool,
    /// Require adequate nonempty observed samples for non-Boolean informative
    /// lexical matches, representation-qualified singleton measurements, and
    /// shared temporal roles. Known Integer fields with exclusively integral
    /// observations are exempt from the measurement restriction. Ignore exact
    /// duplicate source observations as sample competitors; target duplicates
    /// remain competing choices. Defaults to true.
    pub scoped_support: bool,
}

impl Default for ContextualEvidence {
    fn default() -> Self {
        Self {
            strict_identifier_samples: true,
            scoped_support: true,
        }
    }
}

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
    concrete_signals: (usize, usize),
    explanation_bytes: &mut usize,
) -> Result<(), MatchError> {
    let (name_signal, sample_signal) = concrete_signals;
    let units = unit_phrases(&config.name_conflicts);
    let source_features: Vec<_> = sources
        .iter()
        .map(|field| FieldFeatures::new(field, &units))
        .collect();
    let target_features: Vec<_> = targets
        .iter()
        .map(|field| FieldFeatures::new(field, &units))
        .collect();
    let groups = source_groups(sources, policy.scoped_support);
    let mut target_core_counts = BTreeMap::new();
    for feature in &target_features {
        *target_core_counts.entry(&feature.core).or_insert(0usize) += 1;
    }
    let mut pairs = vec![vec![PairFeatures::default(); targets.len()]; sources.len()];
    let mut row_best = vec![TopTwo::default(); sources.len()];
    let mut column_best = vec![TopTwo::default(); targets.len()];
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
            let pair = PairFeatures {
                jaccard,
                hard_compatible: !candidate.issues.iter().any(|issue| match issue {
                    CandidateIssue::IncompatibleTypes => config.reject_incompatible_types,
                    CandidateIssue::SemanticConflict(_) | CandidateIssue::NameConflict(_) => true,
                    _ => false,
                }),
                sample_adequate: source_features[source].distinct >= MIN_DISTINCT
                    && target_features[target].distinct >= MIN_DISTINCT
                    && coverage >= MIN_COVERAGE
                    && jaccard >= MIN_JACCARD,
            };
            pairs[source][target] = pair;
            if pair.hard_compatible {
                row_best[source].insert(target, jaccard);
                column_best[target].insert(groups[source], jaccard);
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
            let word_form_exact = !matches!(
                (sources[source].data_type, targets[target].data_type),
                (DataType::Date, DataType::Timestamp) | (DataType::Timestamp, DataType::Date)
            ) && feature.word_form_core.is_some()
                && feature.word_form_core == target_feature.word_form_core;
            let geo_role_exact = feature.geo_role_core.is_some()
                && feature.geo_role_core == target_feature.geo_role_core;
            let email_form_exact = feature.email_form_core.is_some()
                && feature.email_form_core == target_feature.email_form_core;
            let default_alias_exact = feature.core == target_feature.core
                && !feature.core.is_empty()
                && (feature.default_alias_applied || target_feature.default_alias_applied);
            let trusted_normalized_exact =
                word_form_exact || geo_role_exact || email_form_exact || default_alias_exact;
            let informed_exact = (feature.informative > 0
                && feature.core == target_feature.core
                && !feature.core.is_empty())
                || trusted_normalized_exact;
            let identifier = feature.identifier || target_feature.identifier;
            let identifier_support = !identifier
                || distinctive
                || (!policy.strict_identifier_samples
                    && (!feature.observed || !target_feature.observed));
            let both_observed = if identifier {
                feature.observed && target_feature.observed
            } else {
                feature.non_null > 0 && target_feature.non_null > 0
            };
            let observed_support = !policy.scoped_support
                || !both_observed
                || (feature.boolean && target_feature.boolean)
                || trusted_normalized_exact
                || if identifier {
                    distinctive
                } else {
                    pair.sample_adequate
                };
            let lexical = informed_exact && identifier_support && observed_support;
            let duplicate_generic = feature.informative == 0
                && target_feature.informative == 0
                && !feature.core.is_empty()
                && feature.core == target_feature.core
                && target_core_counts[&target_feature.core] >= 2;
            let temporal_role_agrees = !policy.scoped_support
                || !(feature.temporal || target_feature.temporal)
                || feature.core == target_feature.core
                || !feature
                    .informative_core
                    .is_disjoint(&target_feature.informative_core);
            let recovered = distinctive
                && temporal_role_agrees
                && (feature.informative > 0 || target_feature.informative > 0 || duplicate_generic);
            let unqualified_numeric = policy.scoped_support
                && !identifier
                && (feature.representation_sensitive || target_feature.representation_sensitive)
                && feature.core.len() == 1
                && target_feature.core.len() == 1
                && feature.units.iter().all(|meaning| *meaning == 0)
                && target_feature.units.iter().all(|meaning| *meaning == 0);
            let units_agree = feature.units_agree(target_feature);
            let allowed = pair.hard_compatible
                && !unqualified_numeric
                && units_agree
                && (lexical || recovered);
            let candidate = &mut candidates[source][target];
            let base = candidate.score;
            candidate.score = base
                .max(if lexical { 0.9 } else { 0.0 })
                .max(if pair.sample_adequate { 0.95 } else { 0.0 })
                .clamp(0.0, 1.0);
            candidate
                .issues
                .retain(|issue| *issue != CandidateIssue::InsufficientScore);
            if candidate.score <= 0.0 || candidate.score < config.min_score {
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
            candidate.eligible = allowed
                && candidate.score > 0.0
                && candidate.score >= config.min_score
                && !candidate.issues.iter().any(|issue| {
                    matches!(
                        issue,
                        CandidateIssue::InsufficientNameSupport
                            | CandidateIssue::InsufficientSampleSupport
                    )
                });
            let explanation = format!(
                "Contextual evidence: weighted score {base:.3}, transformed score {:.3}; informative tokens {}/{}; distinct samples {}/{}; lexical support {lexical}; distinctive sample support {distinctive}; unit representation agreement {units_agree}; unqualified numeric {unqualified_numeric}; temporal role agreement {temporal_role_agrees}; contextual support {allowed}. Fixed score floors are heuristics, not calibrated confidence.",
                candidate.score,
                feature.informative,
                target_feature.informative,
                feature.distinct,
                target_feature.distinct,
            );
            *explanation_bytes = explanation_bytes
                .checked_add(explanation.len())
                .ok_or(MatchError::CountOverflow(CountKind::ExplanationBytes))?;
            if *explanation_bytes > config.limits.max_explanation_bytes {
                return Err(MatchError::BudgetExceeded(BudgetKind::ExplanationBytes));
            }
            candidate.warnings.push(explanation);
        }
    }
    sample_format::apply(
        config,
        sources,
        targets,
        candidates,
        (&source_features, &target_features),
        name_signal,
        explanation_bytes,
    )?;
    apply_measurement_names(
        config,
        sources,
        targets,
        candidates,
        (&source_features, &target_features),
        explanation_bytes,
    )?;
    apply_display_name_suffix(
        config,
        sources,
        targets,
        candidates,
        (&source_features, &target_features),
        explanation_bytes,
    )
}

// A trailing "name" can describe an existing text label, but is not permission
// to drop arbitrary tokens or equate a stored code with a human-readable name.
fn display_name_key(field: &Field) -> Option<(Vec<String>, bool)> {
    if field.data_type != DataType::Text {
        return None;
    }
    let mut tokens = normalize_name(&field.name);
    let explicit_name = tokens.last().is_some_and(|token| token == "name");
    if explicit_name {
        tokens.pop();
    }
    if tokens.is_empty()
        || tokens.iter().any(|token| {
            structural_token(token) || matches!(token.as_str(), "uuid" | "guid" | "name")
        })
        || !tokens.iter().any(|token| {
            token.chars().count() > 2
                && !generic_token(token)
                && !token.chars().all(char::is_numeric)
        })
    {
        return None;
    }
    Some((tokens, explicit_name))
}

fn descriptive_name_samples(field: &Field, features: &FieldFeatures) -> bool {
    // Reuse the already-prepared reliability counts. Require observations here:
    // missing samples cannot establish that a bare field stores labels, not codes.
    features.distinct >= MIN_DISTINCT
        && features.coverage >= MIN_COVERAGE
        && field.samples.as_deref().is_some_and(|samples| {
            samples.iter().all(|sample| match sample {
                SampleValue::Null => true,
                SampleValue::Text(value) => {
                    value.chars().filter(|c| c.is_alphabetic()).count() >= 3
                        && value.chars().any(char::is_lowercase)
                        && value.chars().all(|c| {
                            c.is_alphabetic()
                                || c.is_whitespace()
                                || matches!(c, '-' | '\'' | '\u{2019}' | '.' | '&')
                        })
                }
                _ => false,
            })
        })
}

fn apply_display_name_suffix(
    config: &Config,
    sources: &[&Field],
    targets: &[&Field],
    candidates: &mut [Vec<Candidate>],
    features: (&[FieldFeatures], &[FieldFeatures]),
    explanation_bytes: &mut usize,
) -> Result<(), MatchError> {
    let supported_rows: Vec<_> = candidates
        .iter()
        .map(|row| row.iter().any(|candidate| candidate.eligible))
        .collect();
    let mut supported_targets = vec![false; targets.len()];
    for row in candidates.iter() {
        for (j, candidate) in row.iter().enumerate() {
            supported_targets[j] |= candidate.eligible;
        }
    }
    if supported_rows.iter().all(|supported| *supported)
        || supported_targets.iter().all(|supported| *supported)
    {
        return Ok(());
    }
    let source_names: Vec<_> = sources.iter().map(|f| display_name_key(f)).collect();
    let target_names: Vec<_> = targets.iter().map(|f| display_name_key(f)).collect();
    let mut source_counts = BTreeMap::<Vec<String>, usize>::new();
    let mut target_groups = BTreeMap::<Vec<String>, Vec<usize>>::new();
    for (key, _) in source_names.iter().flatten() {
        *source_counts.entry(key.clone()).or_default() += 1;
    }
    for (j, name) in target_names.iter().enumerate() {
        if let Some((key, _)) = name {
            target_groups.entry(key.clone()).or_default().push(j);
        }
    }
    // Count competing names BEFORE reliability, eligibility, review or top-k
    // filtering. Sparse/empty observations cannot make a competing column vanish.
    for (i, source_name) in source_names.iter().enumerate() {
        let Some((key, explicit_source_name)) = source_name else {
            continue;
        };
        let Some(destinations) = target_groups.get(key) else {
            continue;
        };
        if supported_rows[i] || source_counts.get(key) != Some(&1) || destinations.len() != 1 {
            continue;
        }
        let j = destinations[0];
        let Some((_, explicit_target_name)) = &target_names[j] else {
            continue;
        };
        if supported_targets[j]
            || explicit_source_name == explicit_target_name
            || !descriptive_name_samples(sources[i], &features.0[i])
            || !descriptive_name_samples(targets[j], &features.1[j])
            || !features.0[i].units_agree(&features.1[j])
        {
            continue;
        }
        let candidate = &mut candidates[i][j];
        if candidate.issues.iter().any(|issue| {
            matches!(
                issue,
                CandidateIssue::IncompatibleTypes
                    | CandidateIssue::NameConflict(_)
                    | CandidateIssue::SemanticConflict(_)
                    | CandidateIssue::InsufficientNameSupport
                    | CandidateIssue::InsufficientSampleSupport
            )
        }) {
            continue;
        }
        let score = candidate.score.max(0.90);
        if score < config.min_score {
            continue;
        }
        let warning = "Display-name suffix support: complete ordered role tokens agree after removing one trailing name token; both Text fields have reliable descriptive samples. Original row and target had no eligible edge; no competing full-role name was omitted. Samples are not required to overlap. The fixed score floor is a heuristic, not proof of shared meaning.";
        let charged = explanation_bytes
            .checked_add(warning.len())
            .ok_or(MatchError::CountOverflow(CountKind::ExplanationBytes))?;
        if charged > config.limits.max_explanation_bytes {
            return Err(MatchError::BudgetExceeded(BudgetKind::ExplanationBytes));
        }
        *explanation_bytes = charged;
        if score > candidate.score
            && !candidate
                .issues
                .contains(&CandidateIssue::ContextualScoreAdjustment)
        {
            candidate
                .issues
                .push(CandidateIssue::ContextualScoreAdjustment);
        }
        candidate.score = score;
        candidate.issues.retain(|issue| {
            !matches!(
                issue,
                CandidateIssue::InsufficientScore | CandidateIssue::InsufficientContextSupport
            )
        });
        candidate.eligible = true;
        candidate.warnings.push(warning.to_owned());
    }
    Ok(())
}

// Exact physical-measurement names are a separate evidence route, not permission
// to trust every identical numeric name. Keep all tokens and do no conversion.
fn measurement_name(field: &Field) -> Option<Vec<String>> {
    if !matches!(
        field.data_type,
        DataType::Integer | DataType::Float | DataType::Decimal
    ) || field
        .samples
        .as_deref()
        .unwrap_or_default()
        .iter()
        .any(|sample| match sample {
            SampleValue::Null | SampleValue::Integer(_) => false,
            SampleValue::Number(value) => {
                !value.is_finite() || (field.data_type == DataType::Integer && value.fract() != 0.0)
            }
            SampleValue::Decimal(_) => field.data_type == DataType::Integer,
            _ => true,
        })
    {
        return None;
    }
    let mut tokens = normalize_name(&field.name);
    if tokens.iter().any(|token| identifier_token(token)) {
        return None;
    }
    let unit = tokens.last()?.as_str();
    let thermal = matches!(unit, "c" | "f" | "k" | "celsius" | "fahrenheit" | "kelvin");
    // Interpret temp as temperature only with an explicit temperature unit.
    if thermal {
        for token in &mut tokens {
            if token == "temp" {
                *token = "temperature".to_owned();
            }
        }
    }
    let (unit, role) = tokens.split_last()?;
    let has = |names: &[&str]| role.iter().any(|token| names.contains(&token.as_str()));
    let explicit_quantity_and_unit = match unit.as_str() {
        "km" | "m" | "cm" | "mm" | "ft" | "inch" | "inches" => has(&[
            "distance",
            "length",
            "width",
            "height",
            "depth",
            "odometer",
            "altitude",
            "elevation",
            "radius",
            "diameter",
        ]),
        "g" | "kg" | "mg" | "lb" | "lbs" => has(&["weight", "mass"]),
        "c" | "f" | "k" | "celsius" | "fahrenheit" | "kelvin" => has(&["temperature"]),
        "pa" | "kpa" | "mpa" | "psi" | "bar" => has(&["pressure"]),
        "v" | "mv" | "volts" => has(&["voltage"]),
        "ns" | "us" | "ms" | "s" | "seconds" | "minutes" | "hours" => {
            has(&["duration", "latency", "elapsed", "timeout"])
        }
        _ => false,
    };
    explicit_quantity_and_unit.then_some(tokens)
}

fn apply_measurement_names(
    config: &Config,
    sources: &[&Field],
    targets: &[&Field],
    candidates: &mut [Vec<Candidate>],
    features: (&[FieldFeatures], &[FieldFeatures]),
    explanation_bytes: &mut usize,
) -> Result<(), MatchError> {
    // Preserve existing support before adding any edges, including ambiguous
    // rows and nonwinning eligible edges. New edges cannot take claimed targets.
    let supported_rows: Vec<_> = candidates
        .iter()
        .map(|row| row.iter().any(|candidate| candidate.eligible))
        .collect();
    let mut supported_targets = vec![false; targets.len()];
    for row in candidates.iter() {
        for (j, candidate) in row.iter().enumerate() {
            supported_targets[j] |= candidate.eligible;
        }
    }
    if supported_rows.iter().all(|supported| *supported)
        || supported_targets.iter().all(|supported| *supported)
    {
        return Ok(());
    }
    let source_names: Vec<_> = sources
        .iter()
        .map(|field| measurement_name(field))
        .collect();
    let target_names: Vec<_> = targets
        .iter()
        .map(|field| measurement_name(field))
        .collect();
    for (i, row) in candidates.iter_mut().enumerate() {
        let Some(name) = source_names[i].as_ref() else {
            continue;
        };
        if supported_rows[i] {
            continue;
        }
        for (j, candidate) in row.iter_mut().enumerate() {
            if supported_targets[j]
                || sources[i].data_type != targets[j].data_type
                || target_names[j].as_ref() != Some(name)
                || !features.0[i].units_agree(&features.1[j])
                || candidate.issues.iter().any(|issue| {
                    matches!(
                        issue,
                        CandidateIssue::IncompatibleTypes
                            | CandidateIssue::NameConflict(_)
                            | CandidateIssue::SemanticConflict(_)
                            | CandidateIssue::InsufficientNameSupport
                            | CandidateIssue::InsufficientSampleSupport
                    )
                })
            {
                continue;
            }
            let score = candidate.score.max(0.90);
            if score < config.min_score {
                continue;
            }
            let warning = "Explicit measurement-name support: complete quantity, qualifiers, unit token and declared numeric type agree. Sample values are not required to overlap. No value or unit conversion is performed. The fixed score floor is a heuristic, not proof of shared meaning.";
            let charged = explanation_bytes
                .checked_add(warning.len())
                .ok_or(MatchError::CountOverflow(CountKind::ExplanationBytes))?;
            if charged > config.limits.max_explanation_bytes {
                return Err(MatchError::BudgetExceeded(BudgetKind::ExplanationBytes));
            }
            *explanation_bytes = charged;
            if score > candidate.score
                && !candidate
                    .issues
                    .contains(&CandidateIssue::ContextualScoreAdjustment)
            {
                candidate
                    .issues
                    .push(CandidateIssue::ContextualScoreAdjustment);
            }
            candidate.score = score;
            candidate.issues.retain(|issue| {
                !matches!(
                    issue,
                    CandidateIssue::InsufficientScore | CandidateIssue::InsufficientContextSupport
                )
            });
            candidate.eligible = true;
            candidate.warnings.push(warning.to_owned());
        }
    }
    Ok(())
}

struct FieldFeatures {
    core: BTreeSet<String>,
    informative: usize,
    informative_core: BTreeSet<String>,
    word_form_core: Option<BTreeSet<String>>,
    geo_role_core: Option<BTreeSet<String>>,
    email_form_core: Option<BTreeSet<String>>,
    default_alias_applied: bool,
    identifier: bool,
    observed: bool,
    non_null: usize,
    distinct: usize,
    coverage: f64,
    units: Vec<u16>,
    representation_sensitive: bool,
    boolean: bool,
    temporal: bool,
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

fn identifier_token(token: &str) -> bool {
    matches!(
        token,
        "id" | "identifier" | "key" | "code" | "ref" | "reference" | "num" | "number"
    )
}

fn structural_token(token: &str) -> bool {
    identifier_token(token) || matches!(token, "is" | "has" | "flag" | "enabled" | "at" | "on")
}

fn scoped_word_form_core(
    tokens: &[String],
    boolean: bool,
    temporal: bool,
) -> Option<BTreeSet<String>> {
    if !boolean && !temporal {
        return None;
    }

    let mut applied = false;
    let mut core = BTreeSet::new();

    for (index, token) in tokens.iter().enumerate() {
        let canonical = match token.as_str() {
            "expiry" | "expiration" if temporal => {
                applied = true;
                "expiration"
            }
            "settled" | "settlement" if temporal || boolean => {
                applied = true;
                "settlement"
            }
            "reversed" | "reversal" if temporal || boolean => {
                applied = true;
                "reversal"
            }
            "approved" | "approval" if boolean => {
                applied = true;
                "approval"
            }
            "posting" | "post" if temporal => {
                applied = true;
                "post"
            }
            "sample" | "sampled" | "observation" if temporal => {
                applied = true;
                "observation"
            }
            "amt" => "amount",
            "trans" => "transaction",
            token => token,
        };

        if structural_token(canonical)
            || (temporal
                && index + 1 == tokens.len()
                && matches!(canonical, "time" | "date" | "timestamp" | "ts"))
        {
            continue;
        }

        core.insert(canonical.to_owned());
    }

    (applied && !core.is_empty()).then_some(core)
}

fn scoped_geo_role_core(tokens: &[String]) -> Option<BTreeSet<String>> {
    let coordinate = tokens.iter().any(|token| {
        matches!(
            token.as_str(),
            "latitude" | "longitude" | "lat" | "lon" | "lng"
        )
    });

    if !coordinate {
        return None;
    }

    let mut applied = false;
    let core: BTreeSet<_> = tokens
        .iter()
        .filter_map(|token| {
            let canonical = match token.as_str() {
                "start" | "origin" => {
                    applied = true;
                    "origin"
                }
                "amt" => "amount",
                "trans" => "transaction",
                token => token,
            };

            (!structural_token(canonical)).then(|| canonical.to_owned())
        })
        .collect();

    (applied && !core.is_empty()).then_some(core)
}

fn scoped_email_form_core(tokens: &[String], data_type: DataType) -> Option<BTreeSet<String>> {
    if data_type != DataType::Text || !tokens.iter().any(|token| token == "email") {
        return None;
    }

    let core: BTreeSet<_> = tokens
        .iter()
        .filter(|token| token.as_str() != "address")
        .map(|token| match token.as_str() {
            "amt" => "amount",
            "trans" => "transaction",
            token => token,
        })
        .filter(|token| !structural_token(token))
        .map(str::to_owned)
        .collect();

    (!core.is_empty()).then_some(core)
}

fn generic_token(token: &str) -> bool {
    matches!(
        token,
        "id" | "name"
            | "status"
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

fn ascii_number(value: &str, minimum: u32, maximum: u32) -> bool {
    value.bytes().all(|byte| byte.is_ascii_digit())
        && value
            .parse::<u32>()
            .is_ok_and(|number| (minimum..=maximum).contains(&number))
}

fn temporal_text(value: &str) -> bool {
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

impl FieldFeatures {
    fn new(field: &Field, unit_rules: &[Vec<UnitPhrase>]) -> Self {
        let tokens = normalize_name(&field.name);
        let default_alias_applied = tokens
            .iter()
            .any(|token| matches!(token.as_str(), "amt" | "trans"));
        let core: BTreeSet<_> = tokens
            .iter()
            .map(|token| match token.as_str() {
                "amt" => "amount",
                "trans" => "transaction",
                token => token,
            })
            .filter(|token| !structural_token(token))
            .map(str::to_owned)
            .collect();
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
        let boolean = field.data_type == DataType::Boolean
            || (non_null > 0
                && field
                    .samples
                    .as_deref()
                    .unwrap_or_default()
                    .iter()
                    .all(|value| matches!(value, SampleValue::Null | SampleValue::Boolean(_))));
        let temporal =
            matches!(field.data_type, DataType::Date | DataType::Timestamp)
                || (non_null > 0
                    && field.samples.as_deref().unwrap_or_default().iter().all(
                        |value| match value {
                            SampleValue::Null => true,
                            SampleValue::Text(text) => temporal_text(text),
                            _ => false,
                        },
                    ));
        let word_form_core = scoped_word_form_core(&tokens, boolean, temporal);
        let geo_role_core = scoped_geo_role_core(&tokens);
        let email_form_core = scoped_email_form_core(&tokens, field.data_type);
        Self {
            core,
            informative: informative_core.len(),
            informative_core,
            word_form_core,
            geo_role_core,
            email_form_core,
            default_alias_applied,
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
            boolean,
            temporal,
        }
    }

    fn units_agree(&self, target: &Self) -> bool {
        self.units
            .iter()
            .zip(&target.units)
            .all(|(source, target)| (*source == 0 && *target == 0) || (*source & *target != 0))
    }
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
