use serde::{Deserialize, Serialize};

/// A field's declared type. The original suggestion API uses it as supplied;
/// table row/CSV/JSON constructors can infer boolean and numeric types.
///
/// Serialized as a snake_case string, such as `"integer"` or `"timestamp"`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataType {
    /// No declared type information.
    Unknown,
    /// A true or false value.
    Boolean,
    /// A whole number.
    Integer,
    /// A floating-point number.
    Float,
    /// A decimal number.
    Decimal,
    /// Text.
    Text,
    /// A date without a time.
    Date,
    /// A date and time.
    Timestamp,
}

/// A named field in a flat schema, with optional string sample values.
///
/// The original `Schema` API accepts empty and duplicate names. `Table`
/// requires nonempty, distinct names because results identify fields by name.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Field {
    /// The field name used for name similarity and result identification.
    pub name: String,
    /// The declared type used as compatibility evidence.
    pub data_type: DataType,
    /// Optional sample values; omitted samples deserialize as an empty vector.
    ///
    /// The original `match_schemas` API trims/lowercases values and gives no
    /// positive sample credit to a single distinct value. Interpretation in
    /// the Valentine matchers depends on the chosen algorithm.
    #[serde(default)]
    pub samples: Vec<String>,
}

/// A flat collection of fields to compare.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    /// Fields in input order. Empty schemas are accepted.
    pub fields: Vec<Field>,
}

/// A target field's heuristic score and the evidence contributing to it.
///
/// Scores are not probabilities or verified mapping confidence.
#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    /// The target field's name.
    pub target: String,
    /// The combined heuristic score, including any exact-name preference.
    pub score: f64,
    /// Name similarity, including abbreviation evidence when fallback runs.
    pub name_score: f64,
    /// Declared type compatibility; incompatible types do not veto a match.
    pub type_score: f64,
    /// Sample overlap, reduced when the strongest overlap is shared by targets.
    ///
    /// `None` means at least one field has no samples.
    pub sample_score: Option<f64>,
}

/// A suggested correspondence for one source field, requiring caller review.
#[derive(Clone, Debug, PartialEq)]
pub enum Decision {
    /// One candidate meets the threshold and is outside the ambiguity margin
    /// of every other qualifying candidate.
    Match {
        /// The selected target name.
        target: String,
        /// Its heuristic score.
        score: f64,
    },
    /// Multiple candidates meet the threshold and are within the ambiguity
    /// margin of the best score.
    Ambiguous {
        /// Qualifying target names in ranked order.
        targets: Vec<String>,
        /// The highest candidate score.
        best_score: f64,
    },
    /// No target meets the threshold, after any abbreviation fallback.
    NoMatch {
        /// The highest score, or `None` when the target schema is empty.
        best_score: Option<f64>,
    },
}

/// Ranked candidates and a decision for one source field.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldResult {
    /// The source field's name.
    pub source: String,
    /// Candidates sorted by descending score, then target name, and limited by
    /// [`Config::max_candidates`](crate::Config::max_candidates).
    pub candidates: Vec<Candidate>,
    /// The decision made using all candidates before truncation.
    pub decision: Decision,
}

/// Suggestions for each source field; target fields may be reused.
#[derive(Clone, Debug, PartialEq)]
pub struct MatchReport {
    /// Results in source schema order; empty when the source schema is empty.
    pub fields: Vec<FieldResult>,
}
