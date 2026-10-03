use std::fmt;

/// Stable caller-supplied identity, unique within one schema. Names need not be unique.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FieldId(
    /// The caller-supplied identity string.
    pub String,
);

impl From<&str> for FieldId {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for FieldId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl fmt::Display for FieldId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// Coarse logical types; compatibility does not promise a lossless conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DataType {
    /// No declared type; this is absent evidence, not a universal match.
    Unknown,
    /// Boolean values.
    Boolean,
    /// Integral numbers.
    Integer,
    /// Floating point numbers.
    Float,
    /// Decimal logical type; precision/scale constraints of the field are unspecified.
    Decimal,
    /// Unicode text.
    Text,
    /// Calendar date.
    Date,
    /// Date and time; timezone semantics are outside v0.1.
    Timestamp,
    /// Opaque bytes.
    Binary,
}

/// Optional sample evidence. Samples are never logged by the built-in matchers.
#[derive(Clone, PartialEq)]
pub enum SampleValue {
    /// Missing value; excluded from overlap but counted in coverage.
    Null,
    /// Boolean sample.
    Boolean(bool),
    /// Numeric sample (non-finite values are rejected).
    Number(f64),
    /// Exact signed integer. Kept distinct from floating point, decimal and text samples.
    Integer(i128),
    /// Exact base-ten sample. Trailing decimal zeros are canonicalized; no float coercion.
    Decimal(crate::ExactDecimal),
    /// Text sample, also suitable for caller-encoded dates and timestamps.
    Text(String),
}

impl fmt::Debug for SampleValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Null => "Null",
            Self::Boolean(_) => "Boolean(<redacted>)",
            Self::Number(_) => "Number(<redacted>)",
            Self::Integer(_) => "Integer(<redacted>)",
            Self::Decimal(_) => "Decimal(<redacted>)",
            Self::Text(_) => "Text(<redacted>)",
        })
    }
}

impl SampleValue {
    /// Convert an unsigned integer without truncation. Values greater than
    /// `i128::MAX` return an error; use an explicit caller-owned text representation
    /// if the signed exact-integer range does not cover your domain.
    pub fn from_unsigned(value: u128) -> Result<Self, MatchError> {
        i128::try_from(value).map(Self::Integer).map_err(|_| {
            MatchError("unsigned sample exceeds the exact signed integer range".into())
        })
    }
}

/// One flat field. Input order is immaterial; stable IDs determine output order.
#[derive(Clone, Debug, PartialEq)]
pub struct Field {
    /// Unique identity within its schema.
    pub id: FieldId,
    /// Human-readable name, possibly empty or duplicated.
    pub name: String,
    /// Declared logical type.
    pub data_type: DataType,
    /// None means unavailable; an empty vector means an observed empty sample.
    pub samples: Option<Vec<SampleValue>>,
    /// Optional caller-verified semantics. Conflicting supplied hints exclude a pair.
    pub hints: crate::SemanticHints,
}

impl Field {
    /// Construct a field without sample evidence.
    pub fn new(id: impl Into<FieldId>, name: impl Into<String>, data_type: DataType) -> Self {
        Self {
            id: id.into(),
            name: name.into(),
            data_type,
            samples: None,
            hints: crate::SemanticHints::default(),
        }
    }

    /// Attach a caller-selected, ordered sample. Selection is the caller's responsibility.
    pub fn with_samples(mut self, samples: Vec<SampleValue>) -> Self {
        self.samples = Some(samples);
        self
    }

    /// Attach verified metadata. Labels compare exactly; Fieldkin does not infer
    /// units, currency, identifier namespaces or conversion rules.
    pub fn with_hints(mut self, hints: crate::SemanticHints) -> Self {
        self.hints = hints;
        self
    }
}

/// An in-memory flat schema; validated when matching, so duplicate IDs return errors.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Schema {
    /// Fields in any order.
    pub fields: Vec<Field>,
}

impl Schema {
    /// Construct from fields. Matching performs validation.
    pub fn new(fields: Vec<Field>) -> Self {
        Self { fields }
    }
}

/// Input/evaluation budgets are checked before invoking matchers; output budgets
/// are enforced while collecting their results.
#[derive(Clone, Debug)]
pub struct Limits {
    /// Maximum fields in either input (also bounds cubic assignment work).
    pub max_fields: usize,
    /// Maximum number of source-target pairs evaluated.
    pub max_pairs: usize,
    /// Maximum active signal evaluations across all pairs.
    pub max_signal_evaluations: usize,
    /// Aggregate bytes in signal explanations across all evaluated pairs.
    pub max_explanation_bytes: usize,
    /// Maximum ID or name length in bytes.
    pub max_name_bytes: usize,
    /// Maximum sample values per field; oversize inputs are rejected, never silently truncated.
    pub max_samples_per_field: usize,
    /// Maximum bytes in any text sample.
    pub max_sample_bytes: usize,
    /// Aggregate text-sample bytes across both schemas.
    pub max_total_sample_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_fields: 128,
            max_pairs: 16_384,
            max_signal_evaluations: 65_536,
            max_explanation_bytes: 16 * 1024 * 1024,
            max_name_bytes: 256,
            max_samples_per_field: 256,
            max_sample_bytes: 1024,
            max_total_sample_bytes: 8 * 1024 * 1024,
        }
    }
}

/// Match controls. Scores are heuristic scores, never calibrated probabilities.
#[derive(Clone, Debug)]
pub struct Config {
    /// Inclusive score threshold for eligibility. Zero-score pairs are always excluded.
    pub min_score: f64,
    /// Candidates within this absolute distance of the best are ambiguous.
    pub ambiguity_margin: f64,
    /// Maximum ranked candidates retained per source (must be positive).
    pub max_candidates: usize,
    /// Compute maximum-total-score, partial one-to-one assignment over all eligible pairs.
    pub one_to_one: bool,
    /// Exclude locally ambiguous sources from selection/assignment.
    pub abstain_on_ambiguity: bool,
    /// Exclude pairs with incompatible known declared types, even with strong names.
    pub reject_incompatible_types: bool,
    /// Resource budgets.
    pub limits: Limits,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            min_score: 0.70,
            ambiguity_margin: 0.08,
            max_candidates: 5,
            one_to_one: false,
            abstain_on_ambiguity: true,
            reject_incompatible_types: true,
            limits: Limits::default(),
        }
    }
}

/// One matcher's bounded score and privacy-safe explanation.
#[derive(Clone, Debug, PartialEq)]
pub struct Evidence {
    /// None means missing/insufficient evidence. Missing signals contribute zero without
    /// redistributing their configured weight to other signals.
    pub score: Option<f64>,
    /// Explanation. Custom matchers must avoid placing sample values here.
    pub explanation: String,
}

/// Extension point: a deterministic, side-effect-free scoring signal.
///
/// Implementations receive bounded fields but must bound their own work, avoid logging
/// values, and return finite scores in [0, 1]. Errors become a match error. Fieldkin does
/// not catch panics, impose timeouts, or sandbox third-party implementations.
pub trait Matcher: Send + Sync {
    /// Stable unique signal name.
    fn name(&self) -> &str;
    /// Evaluate one pair. Do not treat absence of evidence as evidence of agreement.
    fn evaluate(&self, source: &Field, target: &Field) -> Result<Evidence, String>;

    /// Internal optimization hook for concrete built-in matchers. Custom matchers
    /// should keep the default; no preparation interface is required of them.
    #[doc(hidden)]
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        None
    }
}

/// Contribution retained with each candidate.
#[derive(Clone, Debug, PartialEq)]
pub struct SignalReport {
    /// Signal identity.
    pub name: String,
    /// Normalized configured weight.
    pub weight: f64,
    /// Evidence, including absence reason.
    pub evidence: Evidence,
}

/// A ranked pair; eligibility is distinct from its heuristic score.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    /// Target identity.
    pub target: FieldId,
    /// Weighted sum in [0, 1], not a probability.
    pub score: f64,
    /// Passes threshold, type veto and semantic-hint constraints; ambiguity is source-level.
    pub eligible: bool,
    /// Individual scores, missing evidence and contributions.
    pub signals: Vec<SignalReport>,
    /// Reasons for caution, without sample values.
    pub warnings: Vec<String>,
}

/// Outcome for a source field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    /// A proposal exists; still requires the caller's review.
    Proposed,
    /// Near-tied candidates caused abstention.
    Ambiguous,
    /// No candidate meets evidence requirements.
    BelowThreshold,
    /// Eligible candidates were consumed by other sources in one-to-one assignment.
    AssignmentConflict,
}

/// Ranked evidence and selection for one source.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldMatch {
    /// Source identity.
    pub source: FieldId,
    /// Highest scores first, ties by target ID. May include ineligible candidates.
    pub candidates: Vec<Candidate>,
    /// All eligible alternatives within the ambiguity margin, before candidate truncation.
    /// Contains the best too; length > 1 means ambiguous.
    pub alternatives: Vec<FieldId>,
    /// Selected proposal, if any. Retained even if global assignment selects outside top-k.
    pub selected: Option<Candidate>,
    /// Why a proposal was or was not selected.
    pub decision: Decision,
}

/// Reproducible report; collections are ordered by stable field ID.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchReport {
    /// One report per source, including sources with no candidates.
    pub fields: Vec<FieldMatch>,
    /// Sources with no selected proposal.
    pub unmatched_sources: Vec<FieldId>,
    /// Targets with no selected proposal (also in independent ranking mode).
    pub unmatched_targets: Vec<FieldId>,
    /// Whether optional global one-to-one assignment was requested.
    pub one_to_one: bool,
}

/// Validation or extension failure. Built-in errors never contain sample values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MatchError(
    /// Human-readable reason, without sample values.
    pub String,
);

impl fmt::Display for MatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl std::error::Error for MatchError {}
