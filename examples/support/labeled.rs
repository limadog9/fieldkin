use std::collections::BTreeSet;

use fieldkin::Schema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EvalCase {
    pub name: String,
    pub source: Schema,
    pub target: Schema,
    pub answers: Vec<Answer>,
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
