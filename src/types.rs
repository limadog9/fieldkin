use crate::{InputError, MatchError};
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
        i128::try_from(value)
            .map(Self::Integer)
            .map_err(|_| MatchError::InvalidInput(InputError::UnsignedRange))
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

/// Optional evidence requirements in addition to weighted scoring and hard constraints.
///
/// Only active concrete built-in [`crate::NameMatcher`] and distinct-aware
/// [`crate::SampleMatcher`] evidence can satisfy these requirements. Missing,
/// disabled, custom, profile and historical `Legacy` signals do not qualify.
/// Semantic-hint agreement does not substitute for sampled support.
/// This gate reuses evidence and never changes scores or candidate ranking.
/// It can change ambiguity and assignment by excluding eligible edges.
///
/// Shared samples still do not establish shared meaning. The default sample
/// floor admits fully overlapping two-value columns and loses coverage on
/// null-heavy, disjoint or unavailable samples. Thresholds are heuristics.
///
/// ```
/// use fieldkin::{Config, Corroboration, MatchEngine};
/// let engine = MatchEngine::new(Config {
///     corroboration: Some(Corroboration::default()),
///     ..Config::default()
/// })?;
/// # let _ = engine;
/// # Ok::<(), fieldkin::MatchError>(())
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Corroboration {
    /// Inclusive raw name-score floor in `[0, 1]`, before weighting. The score
    /// must also be positive, even when this floor is zero (the default).
    pub min_name_score: f64,
    /// Inclusive raw distinct-aware sample-score floor in `(0, 1]`, before
    /// weighting. Defaults to 0.5. Both floors must be finite.
    pub min_sample_score: f64,
}

impl Default for Corroboration {
    fn default() -> Self {
        Self {
            min_name_score: 0.0,
            min_sample_score: 0.5,
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
    /// Optional built-in name/sample support gate. Defaults to `None`, preserving
    /// weighted matching when samples are unavailable. See [`Corroboration`].
    pub corroboration: Option<Corroboration>,
    /// Optional bounded alternative-assignment analysis; never changes selection.
    pub global_diagnostics: crate::GlobalDiagnosticsConfig,
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
            corroboration: None,
            global_diagnostics: crate::GlobalDiagnosticsConfig::default(),
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
    /// Passes threshold, optional corroboration, type veto and semantic-hint
    /// constraints; ambiguity is source-level.
    pub eligible: bool,
    /// Individual scores, missing evidence and contributions.
    pub signals: Vec<SignalReport>,
    /// Reasons for caution, without sample values.
    pub warnings: Vec<String>,
    /// Structured counterparts of important caution and exclusion reasons.
    pub issues: Vec<CandidateIssue>,
}

/// Caller-verified semantic category; never contains a supplied label value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum SemanticAxis {
    /// Physical or application-defined unit.
    Unit,
    /// Currency vocabulary.
    Currency,
    /// Identifier namespace.
    IdentifierScope,
}

/// Machine-readable candidate reasons. These are not probability estimates.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CandidateIssue {
    /// Incompatible declared types, regardless of whether the type veto is enabled.
    IncompatibleTypes,
    /// At least one active signal returned absent evidence.
    MissingEvidence,
    /// Score is zero or below the configured threshold.
    InsufficientScore,
    /// The enabled corroboration gate requires a positive built-in name score
    /// meeting its floor; absent, disabled or custom evidence cannot qualify.
    InsufficientNameSupport,
    /// The enabled corroboration gate requires a distinct-aware built-in sample
    /// score meeting its floor; absent, disabled, Legacy or custom evidence cannot qualify.
    InsufficientSampleSupport,
    /// Supplied semantics conflict; this always excludes the pair.
    SemanticConflict(SemanticAxis),
    /// Supplied labels agree, without increasing the score.
    SemanticAgreement(SemanticAxis),
    /// Only one side supplied this category.
    SemanticMissing(SemanticAxis),
    /// Other assignable sources compete for the selected target.
    TargetCompetition,
}

/// Source-level reasons, independent of displayed candidate truncation.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum FieldDiagnostic {
    /// No candidate passes the score, corroboration and semantic/type constraints.
    NoEligibleTarget,
    /// Several eligible candidates are within the local ambiguity margin.
    LocalAmbiguity,
    /// Another assignable source also has an eligible edge to this target.
    TargetCompetition(FieldId),
    /// Global assignment selected a different target from the first ranked eligible one.
    Displaced {
        /// Highest ranked eligible target, before global assignment.
        preferred_target: FieldId,
    },
    /// Global constraints left this otherwise assignable source unmatched.
    UnassignedByGlobalConstraint,
}

/// Competing sources for a target in the actual assignment graph. Locally
/// ambiguous sources excluded by policy are not part of this graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetCompetition {
    /// Contested target identity.
    pub target: FieldId,
    /// Competing sources in stable ID order.
    pub sources: Vec<FieldId>,
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
    /// Structured local and global reasons, in deterministic order.
    pub diagnostics: Vec<FieldDiagnostic>,
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
    /// Competitions in the one-to-one graph, before display truncation.
    pub target_competition: Vec<TargetCompetition>,
    /// Bounded analysis of other eligible assignments, separate from selections.
    pub assignment_diagnostics: crate::AssignmentDiagnostics,
}
