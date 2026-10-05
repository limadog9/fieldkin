//! Development prototype using only names, types, hints and sampled input values.
//! It has no label, concept, family, rationale or holdout-specific inputs.

use std::collections::{BTreeMap, BTreeSet};

use fieldkin::{
    normalize_name, CandidateIssue, Config, DataType, Evidence, Field, FieldId, FieldPair, Limits,
    MatchConstraints, MatchEngine, MatchReport, Matcher, NameConflictKind, NameConflictRule,
    SampleValue, Schema, WeightedMatcher,
};

const MIN_DISTINCT: usize = 3;
const MIN_COVERAGE: f64 = 0.25;
const MIN_JACCARD: f64 = 0.9;
const SAMPLE_MARGIN: f64 = 0.1;

/// Frozen development policy versions with an independent identifier requirement.
#[derive(Clone, Copy, Debug, Default)]
pub struct Policy {
    /// Require distinctive samples even when informative identifier names agree.
    pub require_identifier_samples: bool,
    /// V2 requires observed support, representation qualification and temporal
    /// role agreement. False preserves the original two development policies.
    pub strict_observed_support: bool,
    /// V3 uses adequate pair support for exact informative non-identifier names,
    /// treats null-only observations as unavailable and distinguishes declared
    /// integral counts from unqualified floating measurements.
    pub refined_lexical_support: bool,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum SampleKey<'a> {
    Boolean(bool),
    Number(u64),
    Integer(i128),
    Decimal(i128, u8),
    Text(&'a str),
}

struct FieldFeatures {
    core: BTreeSet<String>,
    informative: usize,
    informative_core: BTreeSet<String>,
    identifier: bool,
    observed: bool,
    non_null: usize,
    distinct: usize,
    coverage: f64,
    units: Vec<u16>,
    numeric: bool,
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
        Self {
            core,
            informative: informative_core.len(),
            informative_core,
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
            numeric,
            representation_sensitive: numeric && !declared_integral,
            boolean: field.data_type == DataType::Boolean
                || (non_null > 0
                    && field
                        .samples
                        .as_deref()
                        .unwrap_or_default()
                        .iter()
                        .all(|value| matches!(value, SampleValue::Null | SampleValue::Boolean(_)))),
            temporal: matches!(field.data_type, DataType::Date | DataType::Timestamp)
                || (non_null > 0
                    && field.samples.as_deref().unwrap_or_default().iter().all(
                        |value| match value {
                            SampleValue::Null => true,
                            SampleValue::Text(text) => temporal_text(text),
                            _ => false,
                        },
                    )),
        }
    }

    fn units_agree(&self, target: &Self) -> bool {
        self.units
            .iter()
            .zip(&target.units)
            .all(|(source, target)| (*source == 0 && *target == 0) || (*source & *target != 0))
    }
}

#[derive(Clone, Copy, Default)]
struct PairFeatures {
    score: f64,
    jaccard: f64,
    hard_compatible: bool,
    sample_adequate: bool,
}

struct CachedScore {
    score: f64,
    explanation: String,
}

struct CachedMatcher {
    // Stable IDs address evidence computed from input data; they are never features.
    source_index: BTreeMap<FieldId, usize>,
    target_index: BTreeMap<FieldId, usize>,
    scores: Vec<Vec<CachedScore>>,
}

impl Matcher for CachedMatcher {
    fn name(&self) -> &str {
        "context_policy"
    }

    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        let source = *self.source_index.get(&source.id).ok_or("uncached source")?;
        let target = *self.target_index.get(&target.id).ok_or("uncached target")?;
        let cached = &self.scores[source][target];
        Ok(Evidence {
            score: Some(cached.score),
            explanation: cached.explanation.clone(),
        })
    }
}

fn sample_distinctive(
    pairs: &[Vec<PairFeatures>],
    source: usize,
    target: usize,
    equivalent_sources: Option<&[Vec<bool>]>,
) -> bool {
    let candidate = pairs[source][target];
    candidate.hard_compatible
        && candidate.sample_adequate
        && pairs[source].iter().enumerate().all(|(other, pair)| {
            other == target
                || !pair.hard_compatible
                || candidate.jaccard >= pair.jaccard + SAMPLE_MARGIN
        })
        && pairs.iter().enumerate().all(|(other, row)| {
            other == source
                || equivalent_sources.is_some_and(|same| same[source][other])
                || !row[target].hard_compatible
                || candidate.jaccard >= row[target].jaccard + SAMPLE_MARGIN
        })
}

/// Match using a frozen development policy, without trusted-answer inputs.
/// All derived exclusions are prototype policy decisions, not human confirmations.
pub fn match_schemas(
    source: &Schema,
    target: &Schema,
    policy: Policy,
    name_conflicts: Vec<NameConflictRule>,
    one_to_one: bool,
) -> Result<MatchReport, String> {
    let limits = Limits::default();
    let pairs_count = source
        .fields
        .len()
        .checked_mul(target.fields.len())
        .ok_or("context policy pair count overflow")?;
    let first_evaluations = pairs_count
        .checked_mul(3)
        .ok_or("context policy signal count overflow")?;
    let total_evaluations = first_evaluations
        .checked_add(pairs_count)
        .ok_or("context policy signal count overflow")?;
    if total_evaluations > limits.max_signal_evaluations {
        return Err("context policy combined signal evaluation budget exceeded".into());
    }
    let raw = MatchEngine::new(Config {
        min_score: 0.0,
        max_candidates: limits.max_fields,
        abstain_on_ambiguity: false,
        name_conflicts: name_conflicts.clone(),
        ..Config::default()
    })
    .map_err(|error| error.to_string())?
    .match_schemas(source, target)
    .map_err(|error| error.to_string())?;
    // Validation above precedes feature preparation, including sample finiteness,
    // unique IDs, canonical rule phrases and all schema resource budgets.
    let mut sources: Vec<_> = source.fields.iter().collect();
    let mut targets: Vec<_> = target.fields.iter().collect();
    sources.sort_unstable_by(|a, b| a.id.cmp(&b.id));
    targets.sort_unstable_by(|a, b| a.id.cmp(&b.id));
    let source_index: BTreeMap<_, _> = sources
        .iter()
        .enumerate()
        .map(|(index, field)| (field.id.clone(), index))
        .collect();
    let target_index: BTreeMap<_, _> = targets
        .iter()
        .enumerate()
        .map(|(index, field)| (field.id.clone(), index))
        .collect();
    let units = unit_phrases(&name_conflicts);
    let source_features: Vec<_> = sources
        .iter()
        .map(|field| FieldFeatures::new(field, &units))
        .collect();
    let target_features: Vec<_> = targets
        .iter()
        .map(|field| FieldFeatures::new(field, &units))
        .collect();
    let equivalent_sources = policy.strict_observed_support.then(|| {
        sources
            .iter()
            .map(|source| {
                sources
                    .iter()
                    .map(|other| {
                        source.name == other.name
                            && source.data_type == other.data_type
                            && source.samples == other.samples
                            && source.hints == other.hints
                    })
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    });
    let mut target_core_counts = BTreeMap::new();
    for feature in &target_features {
        *target_core_counts.entry(&feature.core).or_insert(0usize) += 1;
    }
    let mut explanation_bytes = 0usize;
    let mut pairs = vec![vec![PairFeatures::default(); targets.len()]; sources.len()];
    for (source, field) in raw.fields.iter().enumerate() {
        for candidate in &field.candidates {
            let target = *target_index
                .get(&candidate.target)
                .ok_or("uncached raw target")?;
            for signal in &candidate.signals {
                explanation_bytes = explanation_bytes
                    .checked_add(signal.evidence.explanation.len())
                    .ok_or("context policy explanation count overflow")?;
            }
            let coverage = source_features[source]
                .coverage
                .min(target_features[target].coverage);
            let sample = candidate
                .signals
                .iter()
                .find(|signal| signal.name == "samples")
                .and_then(|signal| signal.evidence.score);
            let jaccard = if coverage > 0.0 {
                (sample.unwrap_or(0.0) / coverage).clamp(0.0, 1.0)
            } else {
                0.0
            };
            pairs[source][target] = PairFeatures {
                score: candidate.score,
                jaccard,
                hard_compatible: !candidate.issues.iter().any(|issue| {
                    matches!(
                        issue,
                        CandidateIssue::IncompatibleTypes
                            | CandidateIssue::SemanticConflict(_)
                            | CandidateIssue::NameConflict(_)
                    )
                }),
                sample_adequate: source_features[source].distinct >= MIN_DISTINCT
                    && target_features[target].distinct >= MIN_DISTINCT
                    && coverage >= MIN_COVERAGE
                    && jaccard >= MIN_JACCARD,
            };
        }
    }
    let mut forbidden = Vec::new();
    let mut scores = Vec::with_capacity(sources.len());
    for (source, feature) in source_features.iter().enumerate() {
        let mut row = Vec::with_capacity(targets.len());
        for (target, target_feature) in target_features.iter().enumerate() {
            let pair = pairs[source][target];
            let distinctive =
                sample_distinctive(&pairs, source, target, equivalent_sources.as_deref());
            let informed_exact = feature.informative > 0
                && feature.core == target_feature.core
                && !feature.core.is_empty();
            let identifier = feature.identifier || target_feature.identifier;
            let identifier_support = !identifier
                || distinctive
                || (!policy.require_identifier_samples
                    && (!feature.observed || !target_feature.observed));
            let refined_non_identifier = policy.refined_lexical_support && !identifier;
            let both_observed = if refined_non_identifier {
                feature.non_null > 0 && target_feature.non_null > 0
            } else {
                feature.observed && target_feature.observed
            };
            let observed_support = !policy.strict_observed_support
                || !both_observed
                || (feature.boolean && target_feature.boolean)
                || if refined_non_identifier {
                    pair.sample_adequate
                } else {
                    distinctive
                };
            let lexical = informed_exact && identifier_support && observed_support;
            let duplicate_generic = feature.informative == 0
                && target_feature.informative == 0
                && !feature.core.is_empty()
                && feature.core == target_feature.core
                && target_core_counts[&target_feature.core] >= 2;
            let temporal_role_agrees = !policy.strict_observed_support
                || !(feature.temporal || target_feature.temporal)
                || feature.core == target_feature.core
                || !feature
                    .informative_core
                    .is_disjoint(&target_feature.informative_core);
            let recovered = distinctive
                && temporal_role_agrees
                && (feature.informative > 0 || target_feature.informative > 0 || duplicate_generic);
            let unqualified_numeric = policy.strict_observed_support
                && !identifier
                && if policy.refined_lexical_support {
                    feature.representation_sensitive || target_feature.representation_sensitive
                } else {
                    feature.numeric || target_feature.numeric
                }
                && feature.core.len() == 1
                && target_feature.core.len() == 1
                && feature.units.iter().all(|meaning| *meaning == 0)
                && target_feature.units.iter().all(|meaning| *meaning == 0);
            let allowed = pair.hard_compatible
                && !unqualified_numeric
                && feature.units_agree(target_feature)
                && (lexical || recovered);
            if !allowed {
                forbidden.push(FieldPair::new(
                    sources[source].id.clone(),
                    targets[target].id.clone(),
                ));
            }
            // Sample support still ranks policy-excluded retrieval candidates.
            // Contrast and semantic restrictions govern proposals separately.
            let score = pair
                .score
                .max(if lexical { 0.9 } else { 0.0 })
                .max(if pair.sample_adequate { 0.95 } else { 0.0 })
                .clamp(0.0, 1.0);
            row.push(CachedScore {
                score,
                explanation: format!(
                    "Input-derived cached score; informative tokens {}/{}; distinct samples {}/{}; lexical support {}; distinctive sample support {}; policy allowed {}",
                    feature.informative,
                    target_feature.informative,
                    feature.distinct,
                    target_feature.distinct,
                    lexical,
                    distinctive,
                    allowed,
                ),
            });
        }
        scores.push(row);
    }
    let mut remaining = limits;
    remaining.max_signal_evaluations -= first_evaluations;
    remaining.max_explanation_bytes = remaining
        .max_explanation_bytes
        .checked_sub(explanation_bytes)
        .ok_or("context policy combined explanation budget exceeded")?;
    MatchEngine::with_matchers(
        Config {
            one_to_one,
            name_conflicts,
            limits: remaining,
            ..Config::default()
        },
        vec![WeightedMatcher::new(
            1.0,
            CachedMatcher {
                source_index,
                target_index,
                scores,
            },
        )],
    )
    .map_err(|error| error.to_string())?
    .match_schemas_with_constraints(
        source,
        target,
        &MatchConstraints {
            forbidden,
            ..MatchConstraints::default()
        },
    )
    .map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use fieldkin::{DataType, Decision};

    use super::*;

    fn field(id: &str, name: &str, values: Option<&[i128]>) -> Field {
        let mut field = Field::new(id, name, DataType::Unknown);
        if let Some(values) = values {
            field.samples = Some(values.iter().copied().map(SampleValue::Integer).collect());
        }
        field
    }

    fn run(source: Vec<Field>, target: Vec<Field>, strict: bool) -> MatchReport {
        match_schemas(
            &Schema::new(source),
            &Schema::new(target),
            Policy {
                require_identifier_samples: strict,
                strict_observed_support: false,
                refined_lexical_support: false,
            },
            Vec::new(),
            false,
        )
        .unwrap()
    }

    fn run_v2(source: Vec<Field>, target: Vec<Field>, one_to_one: bool) -> MatchReport {
        match_schemas(
            &Schema::new(source),
            &Schema::new(target),
            Policy {
                require_identifier_samples: false,
                strict_observed_support: true,
                refined_lexical_support: false,
            },
            Vec::new(),
            one_to_one,
        )
        .unwrap()
    }

    fn run_v3(source: Vec<Field>, target: Vec<Field>, strict_identifiers: bool) -> MatchReport {
        match_schemas(
            &Schema::new(source),
            &Schema::new(target),
            Policy {
                require_identifier_samples: strict_identifiers,
                strict_observed_support: true,
                refined_lexical_support: true,
            },
            Vec::new(),
            false,
        )
        .unwrap()
    }

    #[test]
    fn prototype_v3_exact_scopes_can_share_values_but_identifiers_require_contrast() {
        let source = vec![
            field("first", "account_balance", Some(&[10, 20, 30])),
            field("second", "savings_balance", Some(&[10, 20, 30])),
        ];
        let target = vec![
            field("account", "account_balance", Some(&[10, 20, 30])),
            field("savings", "savings_balance", Some(&[10, 20, 30])),
        ];
        assert!(run_v2(source.clone(), target.clone(), false)
            .fields
            .iter()
            .all(|field| field.selected.is_none()));
        let refined = run_v3(source, target, true);
        assert_eq!(
            refined.fields[0].selected.as_ref().unwrap().target.0,
            "account"
        );
        assert_eq!(
            refined.fields[1].selected.as_ref().unwrap().target.0,
            "savings"
        );
        let identifiers = run_v3(
            vec![
                field("first", "customer_id", Some(&[10, 20, 30])),
                field("second", "employee_id", Some(&[10, 20, 30])),
            ],
            vec![
                field("customer", "customer_id", Some(&[10, 20, 30])),
                field("employee", "employee_id", Some(&[10, 20, 30])),
            ],
            true,
        );
        assert!(identifiers
            .fields
            .iter()
            .all(|field| field.selected.is_none()));
    }

    #[test]
    fn prototype_v3_null_only_is_unavailable_but_constants_are_insufficient() {
        for unavailable in [Vec::new(), vec![SampleValue::Null; 3]] {
            let source = Field::new("source", "account_balance", DataType::Unknown)
                .with_samples(unavailable.clone());
            let target = field("target", "account_balance", Some(&[10, 20, 30]));
            assert!(
                run_v2(vec![source.clone()], vec![target.clone()], false).fields[0]
                    .selected
                    .is_none()
            );
            assert!(run_v3(vec![source], vec![target], true).fields[0]
                .selected
                .is_some());
            for strict in [false, true] {
                let identifier = |id| {
                    Field::new(id, "customer_id", DataType::Unknown)
                        .with_samples(unavailable.clone())
                };
                assert!(run_v3(
                    vec![identifier("source")],
                    vec![identifier("target")],
                    strict
                )
                .fields[0]
                    .selected
                    .is_none());
            }
        }
        for samples in [&[0, 0, 0][..], &[100, 200, 300][..]] {
            assert!(run_v3(
                vec![field("source", "account_balance", Some(&[10, 20, 30]))],
                vec![field("target", "account_balance", Some(samples))],
                true,
            )
            .fields[0]
                .selected
                .is_none());
        }
        assert!(run_v3(
            vec![field("source", "customer_id", None)],
            vec![field("target", "customer_id", None)],
            true,
        )
        .fields[0]
            .selected
            .is_none());
    }

    #[test]
    fn prototype_v3_integral_declarations_preserve_counts_without_float_unit_assumptions() {
        let numeric = |id: &str, data_type, values: &[f64]| {
            Field::new(id, "quantity", data_type)
                .with_samples(values.iter().copied().map(SampleValue::Number).collect())
        };
        let source = numeric("source", DataType::Integer, &[1.0, 2.0, 3.0]);
        let target = numeric("target", DataType::Integer, &[1.0, 2.0, 3.0]);
        assert!(
            run_v2(vec![source.clone()], vec![target.clone()], false).fields[0]
                .selected
                .is_none()
        );
        assert!(run_v3(vec![source], vec![target], true).fields[0]
            .selected
            .is_some());
        for data_type in [DataType::Float, DataType::Decimal, DataType::Unknown] {
            assert!(run_v3(
                vec![numeric("source", data_type, &[1.0, 2.0, 3.0])],
                vec![numeric("target", data_type, &[1.0, 2.0, 3.0])],
                true,
            )
            .fields[0]
                .selected
                .is_none());
        }
        let fractional = run_v3(
            vec![numeric("source", DataType::Integer, &[1.0, 2.5, 3.0])],
            vec![numeric("target", DataType::Integer, &[1.0, 2.5, 3.0])],
            true,
        );
        assert!(fractional.fields[0].selected.is_none());
        let mixed = |id| {
            Field::new(id, "quantity", DataType::Integer).with_samples(vec![
                SampleValue::Number(1.0),
                SampleValue::Number(2.0),
                SampleValue::Text("three".into()),
            ])
        };
        assert!(
            run_v3(vec![mixed("source")], vec![mixed("target")], true).fields[0]
                .selected
                .is_none()
        );
    }

    #[test]
    fn prototype_v2_observed_samples_can_veto_lexical_agreement() {
        for target_samples in [&[100, 200, 300][..], &[10, 10, 10][..], &[][..]] {
            let source = vec![field("source", "account_balance", Some(&[10, 20, 30]))];
            let target = vec![field("target", "account_balance", Some(target_samples))];
            assert!(run(source.clone(), target.clone(), false).fields[0]
                .selected
                .is_some());
            assert!(run_v2(source, target, false).fields[0].selected.is_none());
        }
        let missing = run_v2(
            vec![field("source", "account_balance", None)],
            vec![field("target", "account_balance", Some(&[10, 20, 30]))],
            false,
        );
        assert!(missing.fields[0].selected.is_some());
        let boolean = |id: &str| {
            Field::new(id, "account_active", DataType::Boolean)
                .with_samples(vec![SampleValue::Boolean(true); 3])
        };
        let flags = run_v2(vec![boolean("source")], vec![boolean("target")], false);
        assert!(flags.fields[0].selected.is_some());
    }

    #[test]
    fn prototype_v2_numeric_representation_needs_qualified_names() {
        let source = field("source", "distance", Some(&[10, 20, 30]));
        let target = field("target", "distance", Some(&[10, 20, 30]));
        assert!(
            run(vec![source.clone()], vec![target.clone()], false).fields[0]
                .selected
                .is_some()
        );
        assert!(run_v2(vec![source], vec![target], false).fields[0]
            .selected
            .is_none());
        let scoped = run_v2(
            vec![field("source", "journey_distance", Some(&[10, 20, 30]))],
            vec![field("target", "journey_distance", Some(&[10, 20, 30]))],
            false,
        );
        assert!(scoped.fields[0].selected.is_some());
        let unit_qualified = match_schemas(
            &Schema::new(vec![field("source", "kilograms", Some(&[10, 20, 30]))]),
            &Schema::new(vec![field("target", "kilograms", Some(&[10, 20, 30]))]),
            Policy {
                strict_observed_support: true,
                ..Policy::default()
            },
            vec![NameConflictRule {
                kind: NameConflictKind::Unit,
                alternatives: vec![vec!["kilograms".into()], vec!["pounds".into()]],
            }],
            false,
        )
        .unwrap();
        assert!(unit_qualified.fields[0].selected.is_some());
    }

    #[test]
    fn prototype_v2_coincident_temporal_samples_need_event_role_agreement() {
        let temporal = |id: &str, name: &str, data_type| {
            Field::new(id, name, data_type).with_samples(
                ["2030-01-11", "2030-02-12", "2030-03-13"]
                    .into_iter()
                    .map(|value| SampleValue::Text(value.into()))
                    .collect(),
            )
        };
        for data_type in [DataType::Date, DataType::Text] {
            let source = vec![temporal("source", "activation_at", data_type)];
            let target = vec![temporal("target", "deactivation_on", data_type)];
            assert!(run(source.clone(), target.clone(), false).fields[0]
                .selected
                .is_some());
            assert!(run_v2(source, target, false).fields[0].selected.is_none());
            let shared_role = run_v2(
                vec![temporal("source", "dispatch_at", data_type)],
                vec![temporal("target", "shipment_dispatch_on", data_type)],
                false,
            );
            assert!(shared_role.fields[0].selected.is_some());
        }
        assert!(temporal_text("2030-01-11T12:30:20.123+02:00"));
        assert!(!temporal_text("2030-99-11"));
        assert!(!temporal_text("2030-01-11T32:30:20Z"));
        assert!(!temporal_text("unrelated-format"));
    }

    #[test]
    fn prototype_v2_copied_sources_share_targets_but_target_copies_still_abstain() {
        let source = vec![
            field("first", "customer_id", Some(&[10, 20, 30])),
            field("second", "customer_id", Some(&[10, 20, 30])),
        ];
        let target = vec![field("target", "customer_id", Some(&[10, 20, 30]))];
        assert!(run(source.clone(), target.clone(), false)
            .fields
            .iter()
            .all(|field| field.selected.is_none()));
        let independent = run_v2(source.clone(), target.clone(), false);
        assert!(independent
            .fields
            .iter()
            .all(|field| field.selected.is_some()));
        let one_to_one = run_v2(source, target, true);
        assert_eq!(
            one_to_one
                .fields
                .iter()
                .filter(|field| field.selected.is_some())
                .count(),
            1
        );
        assert_eq!(one_to_one.target_competition.len(), 1);
        let target_copies = run_v2(
            vec![field("source", "customer_id", Some(&[10, 20, 30]))],
            vec![
                field("first", "customer_id", Some(&[10, 20, 30])),
                field("second", "customer_id", Some(&[10, 20, 30])),
            ],
            false,
        );
        assert!(target_copies.fields[0].selected.is_none());
    }

    #[test]
    fn prototype_informative_identifier_missing_samples_distinguishes_the_two_policies() {
        for (strict, selected) in [(false, true), (true, false)] {
            let report = run(
                vec![field("source", "customer_id", None)],
                vec![field("target", "customer_key", None)],
                strict,
            );
            assert_eq!(report.fields[0].selected.is_some(), selected);
        }
    }

    #[test]
    fn prototype_distinctive_values_recover_opaque_names_but_not_solitary_generic_names() {
        let opaque = run(
            vec![field("source", "x", Some(&[10, 20, 30]))],
            vec![field("target", "product_sku", Some(&[10, 20, 30]))],
            true,
        );
        assert_eq!(opaque.fields[0].decision, Decision::Proposed);
        for name in ["id", "name", "amount"] {
            let generic = run(
                vec![field("source", name, Some(&[10, 20, 30]))],
                vec![field("target", name, Some(&[10, 20, 30]))],
                false,
            );
            assert!(generic.fields[0].selected.is_none());
            assert!(generic.fields[0].candidates[0].score >= 0.95);
        }
    }

    #[test]
    fn prototype_duplicate_generic_cores_need_distinctive_values_and_matching_core() {
        let targets = vec![
            field("first", "amount", Some(&[10, 20, 30])),
            field("second", "amount", Some(&[100, 200, 300])),
        ];
        let selected = run(
            vec![field("source", "amount", Some(&[10, 20, 30]))],
            targets.clone(),
            false,
        );
        assert_eq!(
            selected.fields[0].selected.as_ref().unwrap().target.0,
            "first"
        );
        let unrelated = run(
            vec![field("source", "price", Some(&[10, 20, 30]))],
            targets,
            false,
        );
        assert!(unrelated.fields[0].selected.is_none());
    }

    #[test]
    fn prototype_identifier_samples_require_both_row_and_column_contrast() {
        let values = Some(&[10, 20, 30][..]);
        let tied_targets = run(
            vec![field("source", "customer_id", values)],
            vec![
                field("first", "customer_id", values),
                field("second", "opaque", values),
            ],
            false,
        );
        assert!(tied_targets.fields[0].selected.is_none());
        let tied_sources = run(
            vec![
                field("first", "customer_id", values),
                field("second", "opaque", values),
            ],
            vec![field("target", "customer_id", values)],
            false,
        );
        assert!(tied_sources
            .fields
            .iter()
            .all(|field| field.selected.is_none()));
    }

    #[test]
    fn prototype_missing_units_and_longest_phrase_conflicts_remain_excluded() {
        let rules = vec![NameConflictRule {
            kind: NameConflictKind::Unit,
            alternatives: vec![
                vec!["k pa".to_owned(), "kpa".to_owned()],
                vec!["pa".to_owned()],
            ],
        }];
        for target_name in ["chamber_pressure", "chamber_pressure_Pa"] {
            let report = match_schemas(
                &Schema::new(vec![field(
                    "source",
                    "chamber_pressure_kPa",
                    Some(&[10, 20, 30]),
                )]),
                &Schema::new(vec![field("target", target_name, Some(&[10, 20, 30]))]),
                Policy {
                    require_identifier_samples: false,
                    strict_observed_support: false,
                    refined_lexical_support: false,
                },
                rules.clone(),
                false,
            )
            .unwrap();
            assert!(report.fields[0].selected.is_none());
        }
    }
}
