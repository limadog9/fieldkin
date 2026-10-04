use std::fmt;

/// Invalid engine configuration, independent of schema or sample contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConfigurationError {
    /// Threshold or local ambiguity margin is not finite or outside `[0, 1]`.
    ThresholdOrMargin,
    /// At least one candidate must be retained for each source.
    MaxCandidates,
    /// The engine requires between one and 64 matchers.
    MatcherCount,
    /// A matcher weight is negative or non-finite.
    SignalWeight,
    /// Matcher names must be nonempty, distinct and at most 256 bytes.
    SignalNames,
    /// The sum of matcher weights must be finite and positive.
    TotalWeight,
    /// Options for global assignment diagnostics are invalid.
    GlobalDiagnostics,
    /// Corroboration requires a finite name floor in `[0, 1]` and a finite
    /// sample floor in `(0, 1]`.
    Corroboration,
}

impl fmt::Display for ConfigurationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::ThresholdOrMargin => {
                "threshold and ambiguity margin must be finite and in [0, 1]"
            }
            Self::MaxCandidates => "max_candidates must be positive",
            Self::MatcherCount => "expected between 1 and 64 matchers",
            Self::SignalWeight => "signal weights must be finite and nonnegative",
            Self::SignalNames => "signal names must be nonempty, unique, and at most 256 bytes",
            Self::TotalWeight => "total signal weight must be positive and finite",
            Self::GlobalDiagnostics => "invalid global assignment diagnostic configuration",
            Self::Corroboration => {
                "corroboration floors must be finite: name in [0, 1], samples in (0, 1]"
            }
        })
    }
}

impl std::error::Error for ConfigurationError {}

/// Invalid schema, sample or explicitly supplied matching metadata.
///
/// Reasons intentionally omit field IDs, sample values, aliases and hint labels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum InputError {
    /// A schema contains an empty or repeated stable field ID.
    InvalidFieldIds,
    /// A floating-point sample is infinite or NaN.
    NonFiniteSample,
    /// A semantic label is empty, untrimmed or exceeds 128 UTF-8 bytes.
    SemanticHint,
    /// An unsigned integer sample exceeds the representable `i128` range.
    UnsignedRange,
    /// An exact decimal sample was constructed with scale greater than 38.
    DecimalScale,
    /// An alias is not a single normalized token of at most 256 bytes.
    Alias,
}

impl fmt::Display for InputError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidFieldIds => "field IDs must be nonempty and unique within each schema",
            Self::NonFiniteSample => "numeric samples must be finite",
            Self::SemanticHint => {
                "semantic hints must be nonempty, trimmed labels of at most 128 bytes"
            }
            Self::UnsignedRange => "unsigned sample exceeds the exact signed integer range",
            Self::DecimalScale => "exact decimal scale must be at most 38",
            Self::Alias => "aliases must be single normalized tokens of at most 256 bytes",
        })
    }
}

impl std::error::Error for InputError {}

/// Resource limit responsible for a rejected matching call.
///
/// Limits reject work explicitly; no candidate or sample is silently discarded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum BudgetKind {
    /// Number of fields in either schema.
    Fields,
    /// Number of source-target pairs.
    Pairs,
    /// Number of pair evaluations across active matchers.
    SignalEvaluations,
    /// Aggregate UTF-8 bytes in signal explanations.
    ExplanationBytes,
    /// Hard 4096-byte bound on one signal explanation.
    SignalExplanation,
    /// UTF-8 bytes in a field name or stable ID.
    NameBytes,
    /// Number of supplied samples, including nulls, in one field.
    SamplesPerField,
    /// UTF-8 bytes in one text sample.
    TextSampleBytes,
    /// Aggregate UTF-8 bytes in text samples from both schemas.
    TotalSampleBytes,
}

impl fmt::Display for BudgetKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Fields => "field budget exceeded",
            Self::Pairs => "source-target pair budget exceeded",
            Self::SignalEvaluations => "signal evaluation budget exceeded",
            Self::ExplanationBytes => "aggregate explanation byte budget exceeded",
            Self::SignalExplanation => "matcher explanation exceeds 4096 bytes",
            Self::NameBytes => "field name or ID byte budget exceeded",
            Self::SamplesPerField => "sample count budget exceeded",
            Self::TextSampleBytes => "text sample byte budget exceeded",
            Self::TotalSampleBytes => "total sample byte budget exceeded",
        })
    }
}

impl std::error::Error for BudgetKind {}

/// Checked accounting operation that exceeded `usize` capacity.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum CountKind {
    /// Product of source and target field counts.
    Pairs,
    /// Sum of bytes in signal explanations.
    ExplanationBytes,
    /// Sum of bytes in supplied text samples.
    SampleBytes,
}

impl fmt::Display for CountKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Pairs => "pair count overflow",
            Self::ExplanationBytes => "explanation byte count overflow",
            Self::SampleBytes => "sample byte count overflow",
        })
    }
}

impl std::error::Error for CountKind {}

/// Structured validation, resource-limit or matcher failure.
///
/// Match the variants when handling failures programmatically; human-readable
/// [`Display`](fmt::Display) text is for diagnostics, not parsing. These enums
/// are non-exhaustive so callers should retain a fallback arm.
///
/// Built-in errors never retain samples, field IDs, hint values or arbitrary
/// extension error text. Matcher failures retain the configured matcher name,
/// which is application metadata and must not be used to carry sample values.
///
/// ```
/// use fieldkin::{BudgetKind, Config, DataType, Field, MatchEngine, MatchError, Schema};
///
/// let mut config = Config::default();
/// config.limits.max_fields = 0;
/// let engine = MatchEngine::new(config)?;
/// let schema = Schema::new(vec![Field::new("id", "name", DataType::Text)]);
/// match engine.match_schemas(&schema, &schema) {
///     Err(MatchError::BudgetExceeded(BudgetKind::Fields)) => {
///         // Ask the application to provide a smaller schema or an explicit limit.
///     }
///     other => panic!("unexpected outcome: {other:?}"),
/// }
/// # Ok::<(), MatchError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum MatchError {
    /// An engine configuration or matcher registration is invalid.
    InvalidConfiguration(ConfigurationError),
    /// A schema, sample or caller-supplied alias/hint is invalid.
    InvalidInput(InputError),
    /// An explicit resource limit was exceeded.
    BudgetExceeded(BudgetKind),
    /// Checked resource accounting overflowed.
    CountOverflow(CountKind),
    /// An extension returned an error; its arbitrary details are suppressed.
    MatcherFailed {
        /// Validated registered matcher name; no extension error text.
        name: String,
    },
    /// A matcher returned a non-finite score or a score outside `[0, 1]`.
    InvalidMatcherScore {
        /// Validated registered matcher name; no rejected score or input values.
        name: String,
    },
}

impl fmt::Display for MatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidConfiguration(reason) => reason.fmt(f),
            Self::InvalidInput(reason) => reason.fmt(f),
            Self::BudgetExceeded(reason) => reason.fmt(f),
            Self::CountOverflow(reason) => reason.fmt(f),
            Self::MatcherFailed { name } => write!(
                f,
                "matcher '{name}' failed (details suppressed to protect sample values)"
            ),
            Self::InvalidMatcherScore { name } => {
                write!(f, "matcher '{name}' returned a score outside [0, 1]")
            }
        }
    }
}

impl std::error::Error for MatchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InvalidConfiguration(reason) => Some(reason),
            Self::InvalidInput(reason) => Some(reason),
            Self::BudgetExceeded(reason) => Some(reason),
            Self::CountOverflow(reason) => Some(reason),
            Self::MatcherFailed { .. } | Self::InvalidMatcherScore { .. } => None,
        }
    }
}
