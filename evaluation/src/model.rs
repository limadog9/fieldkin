use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct InputField {
    pub id: String,
    pub name: String,
    pub data_type: String,
    pub concept: String,
    #[serde(default)]
    pub samples: Option<Vec<serde_json::Value>>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum LabelKind {
    Match,
    NoMatch,
    Ambiguous,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Label {
    pub source: String,
    pub kind: LabelKind,
    pub targets: Vec<String>,
    pub rationale: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Family {
    pub id: String,
    pub domain: String,
    pub title: String,
    pub tags: Vec<String>,
    pub rationale: String,
    pub source: Vec<InputField>,
    pub target: Vec<InputField>,
    pub labels: Vec<Label>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Case {
    pub id: String,
    pub family_id: String,
    pub domain: String,
    pub tags: Vec<String>,
    pub variant: String,
    pub split: String,
    pub seed: u64,
    pub source: Vec<InputField>,
    pub target: Vec<InputField>,
    pub labels: Vec<Label>,
}
