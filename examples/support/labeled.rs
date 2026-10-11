use std::collections::BTreeSet;

use fieldkin::{DataType, Field, Schema};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvalCase {
    pub name: String,
    #[serde(deserialize_with = "deserialize_schema")]
    pub source: Schema,
    #[serde(deserialize_with = "deserialize_schema")]
    pub target: Schema,
    pub answers: Vec<Answer>,
}

// Evaluation inputs must not silently discard misspelled evidence fields.
// Keep the public Schema/Field deserializers and fingerprint serialization intact.
fn deserialize_schema<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Schema, D::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct InputSchema {
        fields: Vec<InputField>,
    }

    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct InputField {
        name: String,
        data_type: DataType,
        #[serde(default)]
        samples: Vec<String>,
    }

    let input = InputSchema::deserialize(deserializer)?;
    Ok(Schema {
        fields: input
            .fields
            .into_iter()
            .map(|field| Field {
                name: field.name,
                data_type: field.data_type,
                samples: field.samples,
            })
            .collect(),
    })
}

#[derive(Debug, Deserialize, Serialize, Clone)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Answer {
    Match {
        source: String,
        target: String,
    },
    NoMatch {
        source: String,
    },
    Ambiguous {
        source: String,
        targets: Vec<String>,
    },
}

impl Answer {
    pub fn source(&self) -> &str {
        match self {
            Self::Match { source, .. }
            | Self::NoMatch { source }
            | Self::Ambiguous { source, .. } => source,
        }
    }

    pub fn display(&self) -> String {
        match self {
            Self::Match { target, .. } => {
                format!("match -> {target}")
            }

            Self::NoMatch { .. } => "no_match".to_string(),

            Self::Ambiguous { targets, .. } => {
                format!("ambiguous -> [{}]", targets.join(", "))
            }
        }
    }
}

pub fn parse_case(raw: &str) -> Result<EvalCase, String> {
    let case: EvalCase = serde_json::from_str(raw).map_err(|error| error.to_string())?;
    if case.name.trim().is_empty() || case.source.fields.is_empty() || case.answers.is_empty() {
        return Err("dataset must have a name, source fields, and answers".into());
    }
    for (side, schema) in [("source", &case.source), ("target", &case.target)] {
        let mut names = BTreeSet::new();
        for field in &schema.fields {
            if field.name.trim().is_empty() || !names.insert(&field.name) {
                return Err(format!(
                    "{side}: empty or duplicate field name {:?}",
                    field.name
                ));
            }
        }
    }
    let sources: BTreeSet<_> = case
        .source
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    let targets: BTreeSet<_> = case
        .target
        .fields
        .iter()
        .map(|field| field.name.as_str())
        .collect();
    let mut labeled = BTreeSet::new();
    for answer in &case.answers {
        let source = answer.source();
        if !sources.contains(source) || !labeled.insert(source) {
            return Err(format!("{source}: unknown or duplicate answer source"));
        }
        let valid_targets = match answer {
            Answer::Match { target, .. } => targets.contains(target.as_str()),
            Answer::NoMatch { .. } => true,
            Answer::Ambiguous {
                targets: expected, ..
            } => {
                expected.len() >= 2
                    && expected.iter().collect::<BTreeSet<_>>().len() == expected.len()
                    && expected
                        .iter()
                        .all(|target| targets.contains(target.as_str()))
            }
        };
        if !valid_targets {
            return Err(format!(
                "{source}: invalid answer targets: {}",
                answer.display()
            ));
        }
    }
    if labeled != sources {
        return Err(format!(
            "missing answers for source fields: {:?}",
            sources.difference(&labeled).collect::<Vec<_>>()
        ));
    }
    Ok(case)
}

pub fn fingerprint(case: &EvalCase) -> Result<String, String> {
    // Hash parsed data so checkout line endings/JSON whitespace do not matter.
    let data = serde_json::to_vec(case).map_err(|error| error.to_string())?;
    Ok(format!("{:x}", Sha256::digest(data)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn case_json() -> serde_json::Value {
        json!({
            "name": "strict evaluation input",
            "source": {"fields": [{"name": "a", "data_type": "text"}]},
            "target": {"fields": [{"name": "b", "data_type": "text"}]},
            "answers": [{"kind": "match", "source": "a", "target": "b"}]
        })
    }

    #[test]
    fn evaluation_schemas_reject_unknown_keys_instead_of_dropping_samples() {
        for (pointer, key, value) in [
            ("/source", "fieldz", json!([])),
            ("/target", "fieldz", json!([])),
            ("/source/fields/0", "sample", json!(["lost source value"])),
            ("/target/fields/0", "sample", json!(["lost target value"])),
        ] {
            let mut raw = case_json();
            raw.pointer_mut(pointer).unwrap()[key] = value;
            let error = parse_case(&raw.to_string()).unwrap_err();
            assert!(error.contains(&format!("unknown field `{key}`")), "{error}");
        }
    }

    #[test]
    fn omitted_samples_keep_the_same_normalization_and_fingerprint() {
        let mut raw = case_json();
        let omitted = parse_case(&raw.to_string()).unwrap();
        assert!(omitted.source.fields[0].samples.is_empty());
        assert!(omitted.target.fields[0].samples.is_empty());
        raw["source"]["fields"][0]["samples"] = json!([]);
        raw["target"]["fields"][0]["samples"] = json!([]);
        let explicit = parse_case(&raw.to_string()).unwrap();
        assert_eq!(
            fingerprint(&omitted).unwrap(),
            fingerprint(&explicit).unwrap()
        );
        assert_eq!(serde_json::to_value(&omitted).unwrap(), raw);
    }
}
