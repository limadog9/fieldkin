//! Optional bounded JSON export and application-owned review persistence.
//!
//! Reports are export-only display records, never imported as trusted reports or
//! confirmations. Review documents contain explicit caller decisions and opaque
//! application revisions. Revision equality is not authentication or automatic
//! schema-change detection. No schema, sample or semantic-label serialization is
//! implemented. Stable IDs remain visible; arbitrary report text is opt-in.

use std::fmt;
use std::io::{self, Write};

use serde::ser::{SerializeSeq, Serializer};
use serde::{Deserialize, Serialize};

use crate::{
    AssignmentAlternative, AssignmentChange, AssignmentDiagnosticStatus, AssignmentDiagnostics,
    Candidate, CandidateIssue, Decision, FieldDiagnostic, FieldId, FieldMatch, FieldPair,
    MatchConstraints, MatchReport, NameConflictKind, SemanticAxis, SignalReport, TargetCompetition,
};

/// Bounds for JSON input, output and record traversal. Increasing them opts into
/// more work. Matching still enforces its separate engine limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct JsonLimits {
    /// Maximum input or encoded output bytes (default: 8 MiB).
    pub max_bytes: usize,
    /// Maximum visited records and string values (default: 262,144).
    /// Repeated review directives count separately and are never deduplicated.
    pub max_items: usize,
    /// Maximum UTF-8 bytes in any emitted or accepted caller string (default: 4,096).
    pub max_string_bytes: usize,
}

impl Default for JsonLimits {
    fn default() -> Self {
        Self {
            max_bytes: 8 * 1024 * 1024,
            max_items: 262_144,
            max_string_bytes: 4_096,
        }
    }
}

/// Explicit report export choices. Free-form text is omitted by default because
/// custom matchers can place private data in names, explanations or warnings.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ReportJsonOptions {
    /// Resource limits for validation and encoding.
    pub limits: JsonLimits,
    /// Include matcher names, explanations and warning strings. The caller must
    /// review their privacy; Fieldkin cannot identify private text automatically.
    pub include_text: bool,
}

/// Expected schema revisions supplied by the application from trusted state.
/// Change revisions whenever prior review decisions need reconsideration,
/// including changes of field meaning, metadata or matching policy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReviewContext<'a> {
    /// Application-owned source schema revision; nonempty.
    pub source_revision: &'a str,
    /// Application-owned target schema revision; nonempty.
    pub target_revision: &'a str,
}

/// JSON resource limit that stopped processing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum JsonLimit {
    /// Encoded output or supplied input exceeds the byte limit.
    Bytes,
    /// Record/string traversal exceeds the item limit.
    Items,
    /// An emitted or accepted caller string exceeds its UTF-8 byte limit.
    StringBytes,
}

/// Privacy-safe failure; no parser message or supplied string is retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum JsonError {
    /// Malformed JSON, unknown/duplicate keys or a shape outside the review format.
    InvalidDocument,
    /// The document identifies a different format.
    UnsupportedFormat,
    /// The review format version is unsupported.
    UnsupportedVersion,
    /// Document revisions differ from trusted application revisions.
    ContextMismatch,
    /// Empty revisions or field IDs in a review document.
    InvalidReview,
    /// A report contains invalid IDs, nonfinite numbers or out-of-range scores.
    InvalidReport,
    /// An explicit resource limit was exceeded.
    LimitExceeded(JsonLimit),
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidDocument => "invalid review JSON document",
            Self::UnsupportedFormat => "unsupported JSON document format",
            Self::UnsupportedVersion => "unsupported review JSON version",
            Self::ContextMismatch => "review schema revisions do not match",
            Self::InvalidReview => "review context and field IDs must be nonempty",
            Self::InvalidReport => "report contains invalid identifiers or numeric values",
            Self::LimitExceeded(JsonLimit::Bytes) => "JSON byte limit exceeded",
            Self::LimitExceeded(JsonLimit::Items) => "JSON item limit exceeded",
            Self::LimitExceeded(JsonLimit::StringBytes) => "JSON string byte limit exceeded",
        })
    }
}

impl std::error::Error for JsonError {}

struct Budget<'a> {
    limits: &'a JsonLimits,
    items: usize,
}

impl Budget<'_> {
    fn item(&mut self) -> Result<(), JsonError> {
        self.items = self
            .items
            .checked_add(1)
            .ok_or(JsonError::LimitExceeded(JsonLimit::Items))?;
        if self.items > self.limits.max_items {
            return Err(JsonError::LimitExceeded(JsonLimit::Items));
        }
        Ok(())
    }
    fn string(&mut self, value: &str) -> Result<(), JsonError> {
        self.item()?;
        if value.len() > self.limits.max_string_bytes {
            return Err(JsonError::LimitExceeded(JsonLimit::StringBytes));
        }
        Ok(())
    }
    fn id(&mut self, value: &str, error: JsonError) -> Result<(), JsonError> {
        self.string(value)?;
        if value.is_empty() {
            return Err(error);
        }
        Ok(())
    }
}

struct Buffer {
    bytes: Vec<u8>,
    max: usize,
}
impl Write for Buffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > self.max.saturating_sub(self.bytes.len()) {
            return Err(io::Error::other("JSON byte limit exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn encode(value: &impl Serialize, limits: &JsonLimits) -> Result<Vec<u8>, JsonError> {
    let mut output = Buffer {
        bytes: Vec::new(),
        max: limits.max_bytes,
    };
    serde_json::to_writer(&mut output, value)
        .map_err(|_| JsonError::LimitExceeded(JsonLimit::Bytes))?;
    Ok(output.bytes)
}

fn context_budget(context: ReviewContext<'_>, budget: &mut Budget<'_>) -> Result<(), JsonError> {
    budget.id(context.source_revision, JsonError::InvalidReview)?;
    budget.id(context.target_revision, JsonError::InvalidReview)
}

fn review_budget(review: &MatchConstraints, budget: &mut Budget<'_>) -> Result<(), JsonError> {
    for pair in review.confirmed.iter().chain(&review.forbidden) {
        budget.item()?;
        budget.id(&pair.source.0, JsonError::InvalidReview)?;
        budget.id(&pair.target.0, JsonError::InvalidReview)?;
    }
    for id in &review.unmatched_sources {
        budget.id(&id.0, JsonError::InvalidReview)?;
    }
    Ok(())
}

#[derive(Serialize)]
struct ContextWire<'a> {
    source_revision: &'a str,
    target_revision: &'a str,
}

/// Export explicit caller review directives with application revisions.
/// Order and duplicate entries are preserved. This does not validate schema IDs,
/// semantic compatibility or contradictions; matching remains the authority.
/// Returns complete bytes or an error, never a partially encoded document.
pub fn review_to_json(
    review: &MatchConstraints,
    context: ReviewContext<'_>,
    limits: &JsonLimits,
) -> Result<Vec<u8>, JsonError> {
    let mut budget = Budget { limits, items: 0 };
    context_budget(context, &mut budget)?;
    review_budget(review, &mut budget)?;
    #[derive(Serialize)]
    struct Document<'a> {
        format: &'static str,
        version: u32,
        context: ContextWire<'a>,
        confirmed: View<'a, [FieldPair]>,
        forbidden: View<'a, [FieldPair]>,
        unmatched_sources: View<'a, [FieldId]>,
    }
    let options = ReportJsonOptions {
        limits: *limits,
        include_text: false,
    };
    encode(
        &Document {
            format: "fieldkin.review",
            version: 1,
            context: ContextWire {
                source_revision: context.source_revision,
                target_revision: context.target_revision,
            },
            confirmed: View(&review.confirmed, &options),
            forbidden: View(&review.forbidden, &options),
            unmatched_sources: View(&review.unmatched_sources, &options),
        },
        limits,
    )
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnedContext {
    source_revision: String,
    target_revision: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OwnedPair {
    source: String,
    target: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewDocument {
    format: String,
    version: u32,
    context: OwnedContext,
    confirmed: Vec<OwnedPair>,
    forbidden: Vec<OwnedPair>,
    unmatched_sources: Vec<String>,
}

/// Parse explicit review decisions, requiring trusted expected schema revisions.
/// Input bytes are checked before parsing; JSON's bounded recursion guard stays
/// enabled. Item/string validation follows parsing, whose allocation is bounded
/// by the input byte cap. Parser details are suppressed to protect private values.
///
/// The returned directives are unvalidated matching input: pass them to
/// [`crate::MatchEngine::match_schemas_with_constraints`] for the normal complete
/// schema, budget, conflict, type and semantic validation. This function neither
/// invokes matchers nor authenticates the reviewer. It never imports reports.
pub fn review_from_json(
    bytes: &[u8],
    expected: ReviewContext<'_>,
    limits: &JsonLimits,
) -> Result<MatchConstraints, JsonError> {
    if bytes.len() > limits.max_bytes {
        return Err(JsonError::LimitExceeded(JsonLimit::Bytes));
    }
    let mut budget = Budget { limits, items: 0 };
    context_budget(expected, &mut budget)?;
    let document: ReviewDocument =
        serde_json::from_slice(bytes).map_err(|_| JsonError::InvalidDocument)?;
    if document.format != "fieldkin.review" {
        return Err(JsonError::UnsupportedFormat);
    }
    if document.version != 1 {
        return Err(JsonError::UnsupportedVersion);
    }
    if document.context.source_revision != expected.source_revision
        || document.context.target_revision != expected.target_revision
    {
        return Err(JsonError::ContextMismatch);
    }
    let pair = |pair: OwnedPair| FieldPair::new(pair.source, pair.target);
    let review = MatchConstraints {
        confirmed: document.confirmed.into_iter().map(pair).collect(),
        forbidden: document.forbidden.into_iter().map(pair).collect(),
        unmatched_sources: document
            .unmatched_sources
            .into_iter()
            .map(FieldId::from)
            .collect(),
    };
    review_budget(&review, &mut budget)?;
    Ok(review)
}

fn score(value: f64) -> Result<(), JsonError> {
    if value.is_finite() && (0.0..=1.0).contains(&value) {
        Ok(())
    } else {
        Err(JsonError::InvalidReport)
    }
}
fn objective(value: f64) -> Result<(), JsonError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(JsonError::InvalidReport)
    }
}

fn candidate_budget(
    candidate: &Candidate,
    options: &ReportJsonOptions,
    budget: &mut Budget<'_>,
) -> Result<(), JsonError> {
    budget.item()?;
    budget.id(&candidate.target.0, JsonError::InvalidReport)?;
    score(candidate.score)?;
    for signal in &candidate.signals {
        budget.item()?;
        score(signal.weight)?;
        if let Some(value) = signal.evidence.score {
            score(value)?;
        }
        if options.include_text {
            budget.string(&signal.name)?;
            budget.string(&signal.evidence.explanation)?;
        }
    }
    for _ in &candidate.issues {
        budget.item()?;
    }
    if options.include_text {
        for warning in &candidate.warnings {
            budget.string(warning)?;
        }
    }
    Ok(())
}

/// Export a version-1 display report. No import into [`MatchReport`] or automatic
/// conversion to confirmations exists. Arbitrary text is omitted by default.
/// The export preserves report order; engine-produced reports already use stable
/// ID order. Validation rejects invalid numbers/IDs but does not certify that a
/// caller-mutated report was produced by matching or is internally consistent.
/// Returns complete bounded bytes; no I/O or partial document escapes on failure.
pub fn report_to_json(
    report: &MatchReport,
    options: &ReportJsonOptions,
) -> Result<Vec<u8>, JsonError> {
    let mut budget = Budget {
        limits: &options.limits,
        items: 0,
    };
    for field in &report.fields {
        budget.item()?;
        budget.id(&field.source.0, JsonError::InvalidReport)?;
        for candidate in &field.candidates {
            candidate_budget(candidate, options, &mut budget)?;
        }
        if let Some(candidate) = &field.selected {
            candidate_budget(candidate, options, &mut budget)?;
        }
        for id in &field.alternatives {
            budget.id(&id.0, JsonError::InvalidReport)?;
        }
        for reason in &field.diagnostics {
            budget.item()?;
            if let FieldDiagnostic::TargetCompetition(id)
            | FieldDiagnostic::Displaced {
                preferred_target: id,
            } = reason
            {
                budget.id(&id.0, JsonError::InvalidReport)?;
            }
        }
    }
    for id in report
        .unmatched_sources
        .iter()
        .chain(&report.unmatched_targets)
    {
        budget.id(&id.0, JsonError::InvalidReport)?;
    }
    for competition in &report.target_competition {
        budget.item()?;
        budget.id(&competition.target.0, JsonError::InvalidReport)?;
        for id in &competition.sources {
            budget.id(&id.0, JsonError::InvalidReport)?;
        }
    }
    if let Some(value) = report.assignment_diagnostics.base_objective {
        objective(value)?;
    }
    for alternative in &report.assignment_diagnostics.alternatives {
        budget.item()?;
        objective(alternative.objective)?;
        objective(alternative.gap)?;
        for change in &alternative.changes {
            budget.item()?;
            budget.id(&change.source.0, JsonError::InvalidReport)?;
            for id in change
                .selected_target
                .iter()
                .chain(&change.alternative_target)
            {
                budget.id(&id.0, JsonError::InvalidReport)?;
            }
        }
    }
    #[derive(Serialize)]
    struct Document<'a> {
        format: &'static str,
        version: u32,
        text_included: bool,
        report: View<'a, MatchReport>,
    }
    encode(
        &Document {
            format: "fieldkin.report",
            version: 1,
            text_included: options.include_text,
            report: View(report, options),
        },
        &options.limits,
    )
}

struct View<'a, T: ?Sized>(&'a T, &'a ReportJsonOptions);
impl<T> Serialize for View<'_, [T]>
where
    for<'a> View<'a, T>: Serialize,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut seq = serializer.serialize_seq(Some(self.0.len()))?;
        for value in self.0 {
            seq.serialize_element(&View(value, self.1))?;
        }
        seq.end()
    }
}
impl Serialize for View<'_, FieldId> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0 .0)
    }
}
impl Serialize for View<'_, FieldPair> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Pair<'a> {
            source: &'a str,
            target: &'a str,
        }
        Pair {
            source: &self.0.source.0,
            target: &self.0.target.0,
        }
        .serialize(serializer)
    }
}
impl Serialize for View<'_, SignalReport> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Signal<'a> {
            weight: f64,
            score: Option<f64>,
            #[serde(skip_serializing_if = "Option::is_none")]
            name: Option<&'a str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            explanation: Option<&'a str>,
        }
        Signal {
            weight: self.0.weight,
            score: self.0.evidence.score,
            name: self.1.include_text.then_some(self.0.name.as_str()),
            explanation: self
                .1
                .include_text
                .then_some(self.0.evidence.explanation.as_str()),
        }
        .serialize(serializer)
    }
}
impl Serialize for View<'_, Candidate> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct CandidateWire<'a> {
            target: &'a str,
            score: f64,
            eligible: bool,
            signals: View<'a, [SignalReport]>,
            issues: View<'a, [CandidateIssue]>,
            #[serde(skip_serializing_if = "Option::is_none")]
            warnings: Option<&'a [String]>,
        }
        CandidateWire {
            target: &self.0.target.0,
            score: self.0.score,
            eligible: self.0.eligible,
            signals: View(&self.0.signals, self.1),
            issues: View(&self.0.issues, self.1),
            warnings: self.1.include_text.then_some(self.0.warnings.as_slice()),
        }
        .serialize(serializer)
    }
}
impl Serialize for View<'_, CandidateIssue> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let (code, axis, kind) = match self.0 {
            CandidateIssue::ForbiddenByCaller => ("forbidden_by_caller", None, None),
            CandidateIssue::SourceExcludedByCaller => ("source_excluded_by_caller", None, None),
            CandidateIssue::TargetConfirmedByCaller => ("target_confirmed_by_caller", None, None),
            CandidateIssue::IncompatibleTypes => ("incompatible_types", None, None),
            CandidateIssue::MissingEvidence => ("missing_evidence", None, None),
            CandidateIssue::InsufficientScore => ("insufficient_score", None, None),
            CandidateIssue::InsufficientNameSupport => ("insufficient_name_support", None, None),
            CandidateIssue::InsufficientSampleSupport => {
                ("insufficient_sample_support", None, None)
            }
            CandidateIssue::InsufficientContextSupport => {
                ("insufficient_context_support", None, None)
            }
            CandidateIssue::ContextualScoreAdjustment => {
                ("contextual_score_adjustment", None, None)
            }
            CandidateIssue::ContextualReason(reason) => (
                match reason {
                    crate::ContextualReason::Contradiction => "contextual_contradiction",
                    crate::ContextualReason::SupportedEquivalence => {
                        "contextual_supported_equivalence"
                    }
                    crate::ContextualReason::UnresolvedRelationship => {
                        "contextual_unresolved_relationship"
                    }
                    crate::ContextualReason::MissingIdentifierSamples => {
                        "contextual_missing_identifier_samples"
                    }
                    crate::ContextualReason::InsufficientSampleSupport => {
                        "contextual_insufficient_sample_support"
                    }
                    crate::ContextualReason::CompetingCandidate => "contextual_competing_candidate",
                    crate::ContextualReason::InsufficientInformativeName => {
                        "contextual_insufficient_informative_name"
                    }
                    crate::ContextualReason::RepresentationConflict => {
                        "contextual_representation_conflict"
                    }
                },
                None,
                None,
            ),
            CandidateIssue::NameConflict(kind) => ("name_conflict", None, Some(kind)),
            CandidateIssue::SemanticConflict(axis) => ("semantic_conflict", Some(axis), None),
            CandidateIssue::SemanticAgreement(axis) => ("semantic_agreement", Some(axis), None),
            CandidateIssue::SemanticMissing(axis) => ("semantic_missing", Some(axis), None),
            CandidateIssue::TargetCompetition => ("target_competition", None, None),
        };
        let axis = axis.map(|axis| match axis {
            SemanticAxis::Unit => "unit",
            SemanticAxis::Currency => "currency",
            SemanticAxis::IdentifierScope => "identifier_scope",
        });
        let kind = kind.map(|kind| match kind {
            NameConflictKind::Qualifier => "qualifier",
            NameConflictKind::Unit => "unit",
        });
        #[derive(Serialize)]
        struct Reason {
            code: &'static str,
            #[serde(skip_serializing_if = "Option::is_none")]
            axis: Option<&'static str>,
            #[serde(skip_serializing_if = "Option::is_none")]
            kind: Option<&'static str>,
        }
        Reason { code, axis, kind }.serialize(serializer)
    }
}
impl Serialize for View<'_, FieldDiagnostic> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let (code, target) = match self.0 {
            FieldDiagnostic::ConfirmedByCaller => ("confirmed_by_caller", None),
            FieldDiagnostic::ExcludedByCaller => ("excluded_by_caller", None),
            FieldDiagnostic::NoEligibleTarget => ("no_eligible_target", None),
            FieldDiagnostic::InsufficientEvidence => ("insufficient_evidence", None),
            FieldDiagnostic::LocalAmbiguity => ("local_ambiguity", None),
            FieldDiagnostic::TargetCompetition(id) => ("target_competition", Some(id.0.as_str())),
            FieldDiagnostic::Displaced { preferred_target } => {
                ("displaced", Some(preferred_target.0.as_str()))
            }
            FieldDiagnostic::UnassignedByGlobalConstraint => {
                ("unassigned_by_global_constraint", None)
            }
        };
        #[derive(Serialize)]
        struct Reason<'a> {
            code: &'static str,
            #[serde(skip_serializing_if = "Option::is_none")]
            target: Option<&'a str>,
        }
        Reason { code, target }.serialize(serializer)
    }
}
impl Serialize for View<'_, FieldMatch> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let decision = match self.0.decision {
            Decision::Confirmed => "confirmed",
            Decision::ExcludedByCaller => "excluded_by_caller",
            Decision::Proposed => "proposed",
            Decision::Ambiguous => "ambiguous",
            Decision::BelowThreshold => "below_threshold",
            Decision::InsufficientEvidence => "insufficient_evidence",
            Decision::AssignmentConflict => "assignment_conflict",
        };
        #[derive(Serialize)]
        struct FieldWire<'a> {
            source: &'a str,
            decision: &'static str,
            candidates: View<'a, [Candidate]>,
            alternatives: View<'a, [FieldId]>,
            selected: Option<View<'a, Candidate>>,
            diagnostics: View<'a, [FieldDiagnostic]>,
        }
        FieldWire {
            source: &self.0.source.0,
            decision,
            candidates: View(&self.0.candidates, self.1),
            alternatives: View(&self.0.alternatives, self.1),
            selected: self.0.selected.as_ref().map(|value| View(value, self.1)),
            diagnostics: View(&self.0.diagnostics, self.1),
        }
        .serialize(serializer)
    }
}
impl Serialize for View<'_, TargetCompetition> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Competition<'a> {
            target: &'a str,
            sources: View<'a, [FieldId]>,
        }
        Competition {
            target: &self.0.target.0,
            sources: View(&self.0.sources, self.1),
        }
        .serialize(serializer)
    }
}
impl Serialize for View<'_, AssignmentChange> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Change<'a> {
            source: &'a str,
            selected_target: Option<&'a str>,
            alternative_target: Option<&'a str>,
        }
        Change {
            source: &self.0.source.0,
            selected_target: self.0.selected_target.as_ref().map(|id| id.0.as_str()),
            alternative_target: self.0.alternative_target.as_ref().map(|id| id.0.as_str()),
        }
        .serialize(serializer)
    }
}
impl Serialize for View<'_, AssignmentAlternative> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Alternative<'a> {
            objective: f64,
            gap: f64,
            changes: View<'a, [AssignmentChange]>,
        }
        Alternative {
            objective: self.0.objective,
            gap: self.0.gap,
            changes: View(&self.0.changes, self.1),
        }
        .serialize(serializer)
    }
}
impl Serialize for View<'_, AssignmentDiagnostics> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let status = match self.0.status {
            AssignmentDiagnosticStatus::Disabled => "disabled",
            AssignmentDiagnosticStatus::NotApplicable => "not_applicable",
            AssignmentDiagnosticStatus::Complete => "complete",
            AssignmentDiagnosticStatus::BudgetExhausted => "budget_exhausted",
        };
        #[derive(Serialize)]
        struct Diagnostics<'a> {
            status: &'static str,
            base_objective: Option<f64>,
            solves_used: usize,
            work_used: usize,
            alternatives: View<'a, [AssignmentAlternative]>,
        }
        Diagnostics {
            status,
            base_objective: self.0.base_objective,
            solves_used: self.0.solves_used,
            work_used: self.0.work_used,
            alternatives: View(&self.0.alternatives, self.1),
        }
        .serialize(serializer)
    }
}
impl Serialize for View<'_, MatchReport> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        #[derive(Serialize)]
        struct Report<'a> {
            fields: View<'a, [FieldMatch]>,
            unmatched_sources: View<'a, [FieldId]>,
            unmatched_targets: View<'a, [FieldId]>,
            one_to_one: bool,
            target_competition: View<'a, [TargetCompetition]>,
            assignment_diagnostics: View<'a, AssignmentDiagnostics>,
        }
        Report {
            fields: View(&self.0.fields, self.1),
            unmatched_sources: View(&self.0.unmatched_sources, self.1),
            unmatched_targets: View(&self.0.unmatched_targets, self.1),
            one_to_one: self.0.one_to_one,
            target_competition: View(&self.0.target_competition, self.1),
            assignment_diagnostics: View(&self.0.assignment_diagnostics, self.1),
        }
        .serialize(serializer)
    }
}
