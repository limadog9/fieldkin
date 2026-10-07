use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataType {
    Unknown,
    Boolean,
    Integer,
    Float,
    Decimal,
    Text,
    Date,
    Timestamp,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Field {
    pub name: String,
    pub data_type: DataType,
    #[serde(default)]
    pub samples: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Schema {
    pub fields: Vec<Field>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Candidate {
    pub target: String,
    pub score: f64,
    pub name_score: f64,
    pub type_score: f64,
    pub sample_score: Option<f64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Decision {
    Match {
        target: String,
        score: f64,
    },
    Ambiguous {
        targets: Vec<String>,
        best_score: f64,
    },
    NoMatch {
        best_score: Option<f64>,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct FieldResult {
    pub source: String,
    pub candidates: Vec<Candidate>,
    pub decision: Decision,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MatchReport {
    pub fields: Vec<FieldResult>,
}
