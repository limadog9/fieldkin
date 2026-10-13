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

#[path = "support/performance.rs"]
mod performance;
use performance::results_digest;

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
    let mut args = std::env::args().skip(1);
    let output = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/parity/rust_results.json"));
    let mut requested = Vec::new();
    let mut selected = Vec::new();
    let (mut repetitions, mut warmups) = (1usize, 0usize);
    let mut direct = false;
    while let Some(flag) = args.next() {
        if flag == "--direct" {
            direct = true;
            continue;
        }
        let value = args
            .next()
            .ok_or_else(|| format!("missing value for {flag}"))?;
        match flag.as_str() {
            "--dataset" => requested.push(PathBuf::from(value)),
            "--algorithm" => selected.push(value),
            "--repetitions" => repetitions = value.parse()?,
            "--warmups" => warmups = value.parse()?,
            _ => return Err(format!("unknown option {flag}").into()),
        }
    }
    if repetitions == 0 || warmups.checked_add(repetitions).is_none() {
        return Err("repetitions must be positive; warmups + repetitions must fit usize".into());
    }
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
    if !requested.is_empty() {
        paths = requested;
    }
    paths.sort();
    let mut datasets = Vec::new();
    for path in paths {
        let loading_started = Instant::now();
        let input = fs::read(&path)?;
        let dataset: Dataset = serde_json::from_slice(&input)?;
        let tables = [
            Table::from_schema("source", &canonical_schema(dataset.source)),
            Table::from_schema("target", &canonical_schema(dataset.target)),
        ];
        let input_loading_seconds = loading_started.elapsed().as_secs_f64();
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
        if let Some(unknown) = selected
            .iter()
            .find(|name| !matchers.iter().any(|(known, _)| known == name))
        {
            return Err(format!("unknown algorithm {unknown}").into());
        }
        let mut algorithms = BTreeMap::new();
        for (name, matcher) in matchers {
            if !selected.is_empty() && !selected.iter().any(|selected| selected == name) {
                continue;
            }
            let mut measured = Vec::new();
            let mut warmup_seconds = Vec::new();
            let mut previous = None;
            for iteration in 0..warmups + repetitions {
                let started = Instant::now();
                let result = if direct {
                    matcher.get_matches(&tables[0], &tables[1])
                } else {
                    valentine_match(&tables, matcher.as_ref())
                };
                let elapsed = started.elapsed().as_secs_f64();
                let mut report = match result {
                    Ok(matches) => {
                        let output_sha256 = results_digest(&matches)?;
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
                        json!({"matches":entries,"output_sha256":output_sha256,"error":null,"elapsed_seconds":elapsed})
                    }
                    Err(error) => {
                        eprintln!("{} {name}: ERROR {error}", path.display());
                        json!({"matches":[],"error":{"type":"fieldkin::Error","message":error.to_string()},"elapsed_seconds":elapsed})
                    }
                };
                let identity = Sha256::digest(serde_json::to_vec(&(
                    &report["matches"],
                    &report["output_sha256"],
                    &report["error"],
                ))?);
                if previous
                    .as_ref()
                    .is_some_and(|previous| previous != &identity)
                {
                    return Err(
                        format!("nondeterministic output: {} {name}", path.display()).into(),
                    );
                }
                previous = Some(identity);
                if iteration < warmups {
                    warmup_seconds.push(elapsed);
                } else {
                    measured.push(elapsed);
                }
                report["matching_seconds"] = json!(measured);
                report["warmup_seconds"] = json!(warmup_seconds);
                algorithms.insert(name, report);
            }
        }
        datasets.push(json!({
            "path":path.to_string_lossy().replace('\\', "/"),
            "name":dataset.name,
            "input_sha256":format!("{:x}",Sha256::digest(&input)),
            "input_loading_seconds": input_loading_seconds,
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
                "peak_rss_kib": fs::read_to_string("/proc/self/status").ok().and_then(|status|
                    status.lines().find_map(|line| line.strip_prefix("VmHWM:")?
                        .split_whitespace().next()?.parse::<u64>().ok())),
                "matching_api": if direct { "get_matches" } else { "valentine_match" },
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
