//! Small adapter for independently frozen, heterogeneous source bundles.
//! Source-specific offline extractors are checked by validation/reliability/verify_extraction.py.
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path},
};

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::labeled::{EvalCase, FrozenCorpus, fingerprint, load_cases, parse_case, verify_freeze};

// Some runners reuse only the single-corpus loader rather than these tranches.
#[allow(dead_code)]
pub const MANIFEST: &str = "validation/reliability_corpus.json";
#[allow(dead_code)]
const FOLLOWUP_MANIFEST: &str = "validation/reliability_followup_corpus.json";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format_version: u32,
    protocol: String,
    bundles: BTreeMap<String, Bundle>,
    artifacts: BTreeMap<String, String>,
    datasets: BTreeMap<String, Dataset>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Bundle {
    frozen_at_utc: String,
    freeze_path: String,
    case_array: String,
    source_kind: String,
    evidence_paths: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Dataset {
    sha256: String,
    bundle: String,
    case_index: usize,
}

fn source_path(path: &str) -> bool {
    let path = Path::new(path);
    path.starts_with("eval_reliability/sources")
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

fn verify_artifacts(root: &Path, artifacts: &BTreeMap<String, String>) -> Result<(), String> {
    if artifacts.is_empty() {
        return Err("missing frozen reliability artifacts".into());
    }
    for (path, expected) in artifacts {
        if !source_path(path) {
            return Err(format!("{path}: invalid frozen artifact path"));
        }
        let bytes = fs::read(root.join(path)).map_err(|e| format!("{path}: {e}"))?;
        if format!("{:x}", Sha256::digest(bytes)) != *expected {
            return Err(format!(
                "{path}: frozen source, evidence or extractor changed"
            ));
        }
    }
    Ok(())
}

fn verify(root: &Path, raw: &str, cases: &BTreeMap<String, EvalCase>) -> Result<(), String> {
    let frozen: FrozenCorpus = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    verify_freeze(cases, &frozen)?;
    let manifest: Manifest = serde_json::from_str(raw).map_err(|e| e.to_string())?;
    if manifest.format_version != 1
        || manifest.protocol.trim().is_empty()
        || manifest.bundles.is_empty()
    {
        return Err("invalid reliability protocol".into());
    }
    verify_artifacts(root, &manifest.artifacts)?;
    let mut originals = BTreeMap::new();
    for (id, bundle) in &manifest.bundles {
        if bundle.frozen_at_utc.is_empty()
            || !matches!(
                bundle.source_kind.as_str(),
                "schema_only"
                    | "publisher_examples"
                    | "publisher_examples_and_schema"
                    | "actual_records"
            )
            || bundle.evidence_paths.is_empty()
            || [&bundle.freeze_path, &bundle.case_array]
                .into_iter()
                .chain(&bundle.evidence_paths)
                .any(|path| !manifest.artifacts.contains_key(path))
        {
            return Err(format!("{id}: incomplete frozen provenance"));
        }
        let raw = fs::read_to_string(root.join(&bundle.case_array)).map_err(|e| e.to_string())?;
        // Source teams may freeze one case object or an array of cases.
        let values = match serde_json::from_str(&raw).map_err(|e| e.to_string())? {
            serde_json::Value::Array(values) => values,
            value => vec![value],
        };
        if values.is_empty() {
            return Err(format!("{id}: empty original case array"));
        }
        for (index, value) in values.into_iter().enumerate() {
            let case = parse_case(&value.to_string())?;
            if bundle.source_kind == "schema_only"
                && case
                    .source
                    .fields
                    .iter()
                    .chain(&case.target.fields)
                    .any(|field| !field.samples.is_empty())
            {
                return Err(format!("{id}: schema-only case contains observations"));
            }
            originals.insert((id.as_str(), index), fingerprint(&case)?);
        }
    }
    let mut used = BTreeSet::new();
    for (id, dataset) in &manifest.datasets {
        let key = (dataset.bundle.as_str(), dataset.case_index);
        if !used.insert(key) || originals.get(&key) != Some(&dataset.sha256) {
            return Err(format!(
                "{id}: case differs from original pre-prediction freeze"
            ));
        }
    }
    if used.len() != originals.len() {
        return Err("reliability case selection omitted original frozen cases".into());
    }
    Ok(())
}

/// Verify one frozen corpus, its provenance, and its original case selection
/// before the caller can invoke a matcher. No source code is executed here.
pub fn load_verified_corpus(
    root: &Path,
    manifest: &str,
    directory: &str,
) -> Result<BTreeMap<String, EvalCase>, String> {
    let raw = fs::read_to_string(root.join(manifest)).map_err(|e| format!("{manifest}: {e}"))?;
    let cases = load_cases(root, &[directory])?;
    verify(root, &raw, &cases).map_err(|e| format!("{manifest}: {e}"))?;
    Ok(cases)
}

#[allow(dead_code)]
pub fn load_verified(root: &Path) -> Result<BTreeMap<String, EvalCase>, String> {
    let mut combined = BTreeMap::new();
    let mut names = BTreeSet::new();
    // Validate labels, every frozen input, the original case arrays, and source bytes
    // before the caller can invoke the matcher. No source code is executed here.
    for (manifest, directory) in [
        (MANIFEST, "eval_reliability"),
        (FOLLOWUP_MANIFEST, "eval_reliability_new"),
    ] {
        let cases = load_verified_corpus(root, manifest, directory)?;
        for (id, case) in cases {
            if !names.insert(case.name.clone()) || combined.insert(id.clone(), case).is_some() {
                return Err(format!("{id}: duplicate reliability case across tranches"));
            }
        }
    }
    Ok(combined)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frozen_reliability_labels_and_provenance_are_valid() {
        load_verified(Path::new(env!("CARGO_MANIFEST_DIR"))).unwrap();
    }

    #[test]
    fn frozen_semantic_corpus_has_valid_provenance_and_gold_ambiguity() {
        let cases = load_verified_corpus(
            Path::new(env!("CARGO_MANIFEST_DIR")),
            "validation/semantic_corpus.json",
            "eval_semantic",
        )
        .unwrap();
        assert!(
            cases
                .values()
                .flat_map(|case| &case.answers)
                .any(|answer| { matches!(answer, crate::labeled::Answer::Ambiguous { .. }) })
        );
    }

    #[test]
    fn changed_labels_or_source_samples_fail_before_predictions() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let raw = fs::read_to_string(root.join(MANIFEST)).unwrap();
        let mut cases = load_cases(root, &["eval_reliability"]).unwrap();
        let case = cases.values_mut().next().unwrap();
        case.source.fields[0]
            .samples
            .push("changed observation".into());
        assert!(
            verify(root, &raw, &cases)
                .unwrap_err()
                .contains("inputs or labels changed")
        );
        let mut cases = load_cases(root, &["eval_reliability"]).unwrap();
        let case = cases.values_mut().next().unwrap();
        let answer = case
            .answers
            .iter_mut()
            .find(|answer| matches!(answer, crate::labeled::Answer::Match { .. }))
            .unwrap();
        *answer = crate::labeled::Answer::NoMatch {
            source: answer.source().into(),
        };
        assert!(
            verify(root, &raw, &cases)
                .unwrap_err()
                .contains("inputs or labels changed")
        );
    }

    #[test]
    fn archived_source_mutation_and_escaping_paths_are_rejected() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR"));
        let raw = fs::read_to_string(root.join(MANIFEST)).unwrap();
        let manifest: Manifest = serde_json::from_str(&raw).unwrap();
        let path = manifest.artifacts.keys().next().unwrap();
        let bad = BTreeMap::from([(path.clone(), "0".repeat(64))]);
        assert!(
            verify_artifacts(root, &bad)
                .unwrap_err()
                .contains("changed")
        );
        assert!(!source_path("eval_reliability/sources/../../outside"));
        assert!(!source_path("/eval_reliability/sources/absolute"));
    }
}
