//! Run the live Valentine comparison corpus. See scripts/run_valentine_parity.py.
use fieldkin::algorithms::{
    Coma, ComaConfig, Cupid, DistributionBased, JaccardConfig, JaccardDistanceMatcher,
    SimilarityFlooding,
};
use fieldkin::{DataType, Matcher, Schema, Table, valentine_match};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, fs, path::PathBuf, time::Instant};

#[derive(Deserialize)]
struct Dataset {
    name: String,
    source: Schema,
    target: Schema,
}

fn canonical_schema(mut schema: Schema) -> Schema {
    for field in &mut schema.fields {
        field.data_type = match &field.data_type {
            DataType::Decimal => DataType::Float,
            DataType::Timestamp => DataType::Date,
            DataType::Unknown | DataType::Boolean => DataType::Text,
            other => other.clone(),
        };
        // Match the public table API's treatment of missing instance values.
        field.samples.retain(|value| !value.is_empty());
    }
    schema
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/parity/rust_results.json"));
    let mut paths = Vec::new();
    for directory in ["eval", "eval_realworld"] {
        for entry in fs::read_dir(directory)? {
            let path = entry?.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                paths.push(path);
            }
        }
    }
    paths.sort();
    let mut datasets = Vec::new();
    for path in paths {
        let input = fs::read(&path)?;
        let dataset: Dataset = serde_json::from_slice(&input)?;
        let tables = [
            Table::from_schema("source", &canonical_schema(dataset.source)),
            Table::from_schema("target", &canonical_schema(dataset.target)),
        ];
        let matchers: Vec<(&str, Box<dyn Matcher>)> = vec![
            ("coma", Box::new(Coma::default())),
            (
                "coma_instances",
                Box::new(Coma::new(ComaConfig {
                    use_instances: true,
                    ..Default::default()
                })?),
            ),
            ("cupid", Box::new(Cupid::default())),
            ("distribution", Box::new(DistributionBased::default())),
            ("flooding", Box::new(SimilarityFlooding::default())),
            (
                "jaccard",
                Box::new(JaccardDistanceMatcher::new(JaccardConfig::default())?),
            ),
        ];
        let mut algorithms = BTreeMap::new();
        for (name, matcher) in matchers {
            let started = Instant::now();
            let result = valentine_match(&tables, matcher.as_ref());
            let elapsed = started.elapsed().as_secs_f64();
            let report = match result {
                Ok(matches) => {
                    let mut entries: Vec<_> = matches.iter().collect();
                    entries.sort_by_key(|(pair, _)| (*pair).clone());
                    let entries: Vec<Value> = entries
                        .into_iter()
                        .map(|(pair, score)| json!({"pair": pair, "score": score}))
                        .collect();
                    eprintln!(
                        "{} {name}: {} pairs in {elapsed:.3}s",
                        path.display(),
                        entries.len()
                    );
                    json!({"matches":entries,"error":null,"elapsed_seconds":elapsed})
                }
                Err(error) => {
                    eprintln!("{} {name}: ERROR {error}", path.display());
                    json!({"matches":[],"error":{"type":"fieldkin::Error","message":error.to_string()},"elapsed_seconds":elapsed})
                }
            };
            algorithms.insert(name, report);
        }
        datasets.push(json!({
            "path":path.to_string_lossy().replace('\\', "/"),
            "name":dataset.name,
            "input_sha256":format!("{:x}",Sha256::digest(&input)),
            "source_columns":tables[0].columns.len(),
            "target_columns":tables[1].columns.len(),
            "algorithms":algorithms,
        }));
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        // Checkpoint completed datasets so a long comparison remains inspectable.
        fs::write(
            &output,
            serde_json::to_vec_pretty(&json!({
                "version":env!("CARGO_PKG_VERSION"),
                "dtype_mapping":{"integer":"int","float":"float","decimal":"float","date":"date","timestamp":"date","text":"varchar","unknown":"varchar","boolean":"varchar"},
                "sampling":"all nonempty supplied values, no sample cap",
                "datasets":datasets,
            }))?,
        )?;
    }
    eprintln!("Wrote {}", output.display());
    Ok(())
}
