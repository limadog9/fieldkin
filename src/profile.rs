//! Optional, deliberately weak evidence from the shape of observed sample values.

use std::collections::BTreeSet;

use crate::{Evidence, Field, Matcher, SampleValue};

const MAX_OBSERVATIONS: usize = 65_536;
const MAX_TEXT_BYTES: usize = 1024;
const RELIABLE_OBSERVATIONS: f64 = 8.0;
const RELIABLE_DISTINCT: f64 = 8.0;
const TEXT_KIND: usize = 4;

/// Optional sample-shape agreement, without parsing or comparing values across fields.
///
/// The matcher compares proportions of five runtime sample kinds: Boolean, Integer,
/// Decimal, Number and Text. The shared text proportion is further multiplied by
/// histogram intersection over Unicode character-length bins `0`, `1..=4`,
/// `5..=12`, `13..=32`, and `33..`. Character count means Unicode scalar values,
/// not bytes or grapheme clusters. Exact numeric kinds remain distinct.
///
/// The resulting shape agreement is multiplied by three conservative factors:
/// the lower non-null coverage, `min(lower_non_null_count / 8, 1)`, and
/// `min((lower_distinct_count - 1) / 7, 1)`. Constant columns therefore score zero;
/// repeating a constant never makes it reliable. These reference counts are
/// heuristic choices, not statistical confidence guarantees. Distinctness uses
/// exact typed values within each field, with floating-point signed zero equal.
///
/// Missing, empty, or insufficient samples provide absent evidence. Similar
/// shapes can occur in entirely unrelated fields, and numeric magnitudes, units,
/// currencies, text contents, and identifier scope are not compared. This matcher
/// is **not enabled by default**; evaluate it on application-specific fixtures.
///
/// When evaluating two nonempty sample sets, direct calls reject more than
/// 65,536 samples per field, text samples longer
/// than 1,024 bytes, and non-finite floating-point samples. The engine's configured
/// limits apply as well and are usually much smaller. Samples are never truncated,
/// logged, or included in explanations; private per-call preparation retains only
/// aggregate counts.
#[derive(Clone, Copy, Debug)]
pub struct SampleProfileMatcher {
    /// Required non-null observations on each side, including repetitions.
    /// Must be in `1..=65_536`. The default is three.
    pub min_non_null: usize,
}

impl Default for SampleProfileMatcher {
    fn default() -> Self {
        Self { min_non_null: 3 }
    }
}

#[derive(Default)]
struct Profile {
    non_null: usize,
    distinct: usize,
    kinds: [usize; 5],
    text_lengths: [usize; 5],
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
enum DistinctKey<'a> {
    Boolean(bool),
    Integer(i128),
    Decimal(i128, u8),
    Number(u64),
    Text(&'a str),
}

// No raw sample values survive preparation, and no Debug implementation can
// accidentally expose the temporary exact-value keys used for distinct counts.
pub(crate) struct PreparedProfile {
    observations: Option<usize>,
    summary: Result<Profile, String>,
}

fn summarize(samples: &[SampleValue]) -> Result<Profile, String> {
    if samples.len() > MAX_OBSERVATIONS {
        return Err("SampleProfileMatcher accepts at most 65,536 samples per field".to_owned());
    }
    let mut profile = Profile::default();
    let mut distinct = BTreeSet::new();
    for sample in samples {
        let (kind, key) = match sample {
            SampleValue::Null => continue,
            SampleValue::Boolean(value) => (0, DistinctKey::Boolean(*value)),
            SampleValue::Integer(value) => (1, DistinctKey::Integer(*value)),
            SampleValue::Decimal(value) => {
                (2, DistinctKey::Decimal(value.coefficient(), value.scale()))
            }
            SampleValue::Number(value) => {
                if !value.is_finite() {
                    return Err("Sample contains a non-finite number".to_owned());
                }
                (
                    3,
                    DistinctKey::Number(if *value == 0.0 { 0 } else { value.to_bits() }),
                )
            }
            SampleValue::Text(value) => {
                if value.len() > MAX_TEXT_BYTES {
                    return Err(
                        "SampleProfileMatcher accepts text samples of at most 1,024 bytes"
                            .to_owned(),
                    );
                }
                let length_bin = match value.chars().count() {
                    0 => 0,
                    1..=4 => 1,
                    5..=12 => 2,
                    13..=32 => 3,
                    _ => 4,
                };
                profile.text_lengths[length_bin] += 1;
                (TEXT_KIND, DistinctKey::Text(value))
            }
        };
        profile.non_null += 1;
        profile.kinds[kind] += 1;
        distinct.insert(key);
    }
    profile.distinct = distinct.len();
    Ok(profile)
}

fn histogram_intersection<const N: usize>(
    source: &[usize; N],
    source_count: usize,
    target: &[usize; N],
    target_count: usize,
) -> f64 {
    if source_count == 0 || target_count == 0 {
        return 0.0;
    }
    source
        .iter()
        .zip(target)
        .map(|(source, target)| {
            (*source as f64 / source_count as f64).min(*target as f64 / target_count as f64)
        })
        .sum::<f64>()
        // Rounding across a histogram can exceed 1 by a few ulps.
        .clamp(0.0, 1.0)
}

impl SampleProfileMatcher {
    pub(crate) fn prepare(&self, field: &Field) -> PreparedProfile {
        PreparedProfile {
            observations: field.samples.as_ref().map(Vec::len),
            summary: summarize(field.samples.as_deref().unwrap_or_default()),
        }
    }

    fn absent_evidence(
        &self,
        source_observations: Option<usize>,
        target_observations: Option<usize>,
    ) -> Result<Option<Evidence>, String> {
        if !(1..=MAX_OBSERVATIONS).contains(&self.min_non_null) {
            return Err("SampleProfileMatcher.min_non_null must be in 1..=65,536".to_owned());
        }
        let (Some(source_count), Some(target_count)) = (source_observations, target_observations)
        else {
            return Ok(Some(Evidence {
                score: None,
                explanation: "Sample profile unavailable: at least one field has no sample data"
                    .to_owned(),
            }));
        };
        if source_count == 0 || target_count == 0 {
            return Ok(Some(Evidence {
                score: None,
                explanation: format!(
                    "Sample profile has observed empty samples: source {source_count} and \
                     target {target_count} observations",
                ),
            }));
        }
        Ok(None)
    }

    pub(crate) fn evaluate_prepared(
        &self,
        source: &PreparedProfile,
        target: &PreparedProfile,
    ) -> Result<Evidence, String> {
        if let Some(evidence) = self.absent_evidence(source.observations, target.observations)? {
            return Ok(evidence);
        }
        let source_profile = source.summary.as_ref().map_err(Clone::clone)?;
        let target_profile = target.summary.as_ref().map_err(Clone::clone)?;
        if source_profile.non_null < self.min_non_null
            || target_profile.non_null < self.min_non_null
        {
            return Ok(Evidence {
                score: None,
                explanation: format!(
                    "Insufficient non-null samples for a profile: source {}, target {}; \
                     requires {} per field",
                    source_profile.non_null, target_profile.non_null, self.min_non_null,
                ),
            });
        }
        let source_observations = source.observations.unwrap_or_default();
        let target_observations = target.observations.unwrap_or_default();
        let kind_agreement = histogram_intersection(
            &source_profile.kinds,
            source_profile.non_null,
            &target_profile.kinds,
            target_profile.non_null,
        );
        let shared_text = (source_profile.kinds[TEXT_KIND] as f64 / source_profile.non_null as f64)
            .min(target_profile.kinds[TEXT_KIND] as f64 / target_profile.non_null as f64);
        let text_agreement = histogram_intersection(
            &source_profile.text_lengths,
            source_profile.kinds[TEXT_KIND],
            &target_profile.text_lengths,
            target_profile.kinds[TEXT_KIND],
        );
        let shape = (kind_agreement - shared_text + shared_text * text_agreement).clamp(0.0, 1.0);
        let coverage = (source_profile.non_null as f64 / source_observations as f64)
            .min(target_profile.non_null as f64 / target_observations as f64);
        let observation_reliability = (source_profile.non_null.min(target_profile.non_null) as f64
            / RELIABLE_OBSERVATIONS)
            .min(1.0);
        let distinct_reliability = (source_profile
            .distinct
            .min(target_profile.distinct)
            .saturating_sub(1) as f64
            / (RELIABLE_DISTINCT - 1.0))
            .min(1.0);
        Ok(Evidence {
            score: Some(shape * coverage * observation_reliability * distinct_reliability),
            explanation: format!(
                "Kind histogram intersection {kind_agreement:.3}; text-length-bin intersection \
                 {text_agreement:.3} over shared text mass {shared_text:.3}; adjusted shape \
                 agreement {shape:.3}; lower non-null coverage {coverage:.3}; observation \
                 reliability {observation_reliability:.3} (8 reference); distinct reliability \
                 {distinct_reliability:.3} (1 gives zero, 8 reference); source {}/{} non-null \
                 with {} distinct, target {}/{} non-null with {} distinct. Similar sample \
                 shapes do not establish field meaning or compatible units",
                source_profile.non_null,
                source_observations,
                source_profile.distinct,
                target_profile.non_null,
                target_observations,
                target_profile.distinct,
            ),
        })
    }
}

impl Matcher for SampleProfileMatcher {
    fn name(&self) -> &str {
        "sample_profile"
    }

    fn as_any(&self) -> Option<&dyn std::any::Any> {
        Some(self)
    }

    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String> {
        if let Some(evidence) = self.absent_evidence(
            source.samples.as_ref().map(Vec::len),
            target.samples.as_ref().map(Vec::len),
        )? {
            return Ok(evidence);
        }
        self.evaluate_prepared(&self.prepare(source), &self.prepare(target))
    }
}
