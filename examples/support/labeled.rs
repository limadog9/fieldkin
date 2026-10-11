use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
};

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

// Read corpus identities without coupling the two evaluators' metrics.
#[derive(Deserialize)]
pub struct FrozenCorpus {
    pub datasets: BTreeMap<String, FrozenDataset>,
}

#[derive(Deserialize)]
pub struct FrozenDataset {
    pub sha256: String,
}

pub fn load_cases(root: &Path, directories: &[&str]) -> Result<BTreeMap<String, EvalCase>, String> {
    let mut cases = BTreeMap::new();
    let mut names = BTreeSet::new();
    for directory in directories {
        let mut paths = fs::read_dir(root.join(directory))
            .and_then(|entries| {
                entries
                    .map(|entry| entry.map(|entry| entry.path()))
                    .collect::<Result<Vec<_>, _>>()
            })
            .map_err(|error| format!("{directory}: {error}"))?;
        paths.retain(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        });
        if paths.is_empty() {
            return Err(format!(
                "{directory}: no JSON evaluation datasets (empty evaluation corpus)"
            ));
        }
        paths.sort();
        for path in paths {
            let id = format!(
                "{directory}/{}",
                path.file_name().unwrap().to_string_lossy()
            );
            let raw = fs::read_to_string(&path).map_err(|error| format!("{id}: {error}"))?;
            let case = parse_case(&raw).map_err(|error| format!("{id}: {error}"))?;
            if !names.insert(case.name.clone()) {
                return Err(format!("{id}: duplicate dataset name {:?}", case.name));
            }
            if cases.insert(id.clone(), case).is_some() {
                return Err(format!("duplicate dataset identifier: {id}"));
            }
        }
    }
    if cases.is_empty() {
        return Err("empty evaluation corpus: no JSON evaluation datasets".into());
    }
    Ok(cases)
}

pub fn verify_freeze(
    cases: &BTreeMap<String, EvalCase>,
    frozen: &FrozenCorpus,
) -> Result<(), String> {
    if cases.is_empty() || frozen.datasets.is_empty() {
        return Err("empty evaluation corpus or frozen dataset list".into());
    }
    for id in frozen.datasets.keys() {
        if !cases.contains_key(id) {
            return Err(format!("{id}: missing frozen evaluation dataset"));
        }
    }
    for (id, case) in cases {
        let expected = frozen
            .datasets
            .get(id)
            .ok_or_else(|| format!("{id}: dataset absent from frozen corpus"))?;
        if fingerprint(case)? != expected.sha256 {
            return Err(format!("{id}: evaluation inputs or labels changed"));
        }
    }
    Ok(())
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
