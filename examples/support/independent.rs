//! Provenance checks for the independently sourced benchmark, before predictions.
use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path},
};

use fieldkin::{Field, Schema};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::labeled::{Answer, EvalCase};

#[derive(Deserialize)]
struct Manifest {
    resources: BTreeMap<String, Resource>,
    views: BTreeMap<String, View>,
    datasets: BTreeMap<String, Dataset>,
}

#[derive(Deserialize)]
struct Resource {
    path: String,
    url: String,
    publisher_revision: String,
    retrieved_utc: String,
    license: String,
    sha256: String,
}

#[derive(Deserialize)]
struct View {
    resource: String,
    format: String,
    fields: Vec<Field>,
    rows: usize,
    type_evidence: Vec<String>,
    original_source_urls: Vec<String>,
}

#[derive(Deserialize)]
struct Dataset {
    source_view: String,
    target_view: String,
    evidence: BTreeMap<String, Evidence>,
}

#[derive(Deserialize)]
struct Evidence {
    decision: String,
    target: Option<String>,
    #[serde(default)]
    targets: Vec<String>,
    explanation: String,
    references: Vec<String>,
}

fn is_url(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://")
}

fn verify_resource(root: &Path, id: &str, resource: &Resource) -> Result<(), String> {
    let path = Path::new(&resource.path);
    if !path.starts_with("eval_independent/sources")
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        || !is_url(&resource.url)
        || resource.publisher_revision.len() != 40
        || !resource
            .publisher_revision
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || resource.retrieved_utc.is_empty()
        || resource.license.trim().is_empty()
    {
        return Err(format!("{id}: invalid resource provenance"));
    }
    let bytes = fs::read(root.join(path)).map_err(|error| format!("{}: {error}", resource.path))?;
    if format!("{:x}", Sha256::digest(bytes)) != resource.sha256 {
        return Err(format!(
            "{}: publisher snapshot checksum changed",
            resource.path
        ));
    }
    Ok(())
}

fn verify_view(
    root: &Path,
    view: &View,
    schema: &Schema,
    resources: &BTreeMap<String, Resource>,
) -> Result<(), String> {
    if view.rows == 0
        || view.fields.len() != schema.fields.len()
        || view.type_evidence.is_empty()
        || view
            .type_evidence
            .iter()
            .any(|reference| !resources.contains_key(reference))
        || view.original_source_urls.is_empty()
        || !view.original_source_urls.iter().all(|url| is_url(url))
    {
        return Err("invalid publisher view or type evidence".into());
    }
    let resource = resources
        .get(&view.resource)
        .ok_or("unknown view resource")?;
    for (declared, field) in view.fields.iter().zip(&schema.fields) {
        if declared.name != field.name
            || declared.data_type != field.data_type
            || field.samples.len() != view.rows.min(16)
        {
            return Err(format!(
                "{}: changed field, declared type or sample count",
                field.name
            ));
        }
    }
    match view.format.as_str() {
        "rda" => Ok(()), // Archived native bytes + case fingerprints; optional importer verifies decoding.
        "csv" => {
            let mut reader =
                csv::Reader::from_path(root.join(&resource.path)).map_err(|e| e.to_string())?;
            let headers = reader.headers().map_err(|e| e.to_string())?.clone();
            let positions: Vec<_> = schema
                .fields
                .iter()
                .map(|field| {
                    headers
                        .iter()
                        .position(|name| name == field.name)
                        .ok_or_else(|| format!("{}: missing publisher CSV field", field.name))
                })
                .collect::<Result<_, _>>()?;
            let count = view.rows.min(16);
            let mut selected = 0;
            let mut rows = 0;
            for (index, row) in reader.records().enumerate() {
                let row = row.map_err(|e| e.to_string())?;
                rows += 1;
                if selected == count || index != selected * (view.rows - 1) / (count - 1).max(1) {
                    continue;
                }
                for (field, position) in schema.fields.iter().zip(&positions) {
                    let value = &row[*position];
                    let expected = if value == "NA" { "" } else { value };
                    if field.samples[selected] != expected {
                        return Err(format!(
                            "{}: sample {selected} differs from publisher CSV",
                            field.name
                        ));
                    }
                }
                selected += 1;
            }
            if rows != view.rows || selected != count {
                return Err("publisher CSV row count changed".into());
            }
            Ok(())
        }
        _ => Err("unsupported publisher view format".into()),
    }
}

pub fn verify(root: &Path, raw: &str, cases: &BTreeMap<String, EvalCase>) -> Result<(), String> {
    let manifest: Manifest = serde_json::from_str(raw).map_err(|error| error.to_string())?;
    if manifest.resources.is_empty() || manifest.views.is_empty() {
        return Err("missing independent corpus provenance".into());
    }
    let mut paths = std::collections::BTreeSet::new();
    for (id, resource) in &manifest.resources {
        if !paths.insert(&resource.path) {
            return Err(format!("{id}: duplicate resource path"));
        }
        verify_resource(root, id, resource)?;
    }
    for (id, case) in cases {
        let dataset = manifest
            .datasets
            .get(id)
            .ok_or_else(|| format!("{id}: missing label evidence"))?;
        for (name, schema) in [
            (&dataset.source_view, &case.source),
            (&dataset.target_view, &case.target),
        ] {
            let view = manifest
                .views
                .get(name)
                .ok_or_else(|| format!("{id}: unknown publisher view {name}"))?;
            verify_view(root, view, schema, &manifest.resources)
                .map_err(|e| format!("{id} / {name}: {e}"))?;
        }
        if dataset.evidence.len() != case.answers.len() {
            return Err(format!("{id}: incomplete label evidence"));
        }
        for answer in &case.answers {
            let source = answer.source();
            let evidence = dataset
                .evidence
                .get(source)
                .ok_or_else(|| format!("{id} / {source}: missing label evidence"))?;
            let consistent = match answer {
                Answer::Match { target, .. } => {
                    evidence.decision == "match"
                        && evidence.target.as_ref() == Some(target)
                        && evidence.targets.is_empty()
                }
                Answer::NoMatch { .. } => {
                    evidence.decision == "no_match"
                        && evidence.target.is_none()
                        && evidence.targets.is_empty()
                }
                Answer::Ambiguous { targets, .. } => {
                    evidence.decision == "ambiguous"
                        && evidence.target.is_none()
                        && &evidence.targets == targets
                }
            };
            if !consistent
                || evidence.explanation.trim().is_empty()
                || evidence.references.is_empty()
                || evidence
                    .references
                    .iter()
                    .any(|reference| !manifest.resources.contains_key(reference))
            {
                return Err(format!(
                    "{id} / {source}: invalid or inconsistent label evidence"
                ));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn changed_missing_or_untracked_publisher_samples_are_rejected() {
        let root = std::env::temp_dir().join(format!(
            "fieldkin-publisher-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let path = "eval_independent/sources/test.csv";
        fs::create_dir_all(root.join("eval_independent/sources")).unwrap();
        let bytes = b"name\noriginal\nNA\n";
        fs::write(root.join(path), bytes).unwrap();
        let mut resource = Resource {
            path: path.into(),
            url: "https://example.org/test.csv".into(),
            publisher_revision: "0".repeat(40),
            retrieved_utc: "2026-10-10".into(),
            license: "test fixture".into(),
            sha256: format!("{:x}", Sha256::digest(bytes)),
        };
        verify_resource(&root, "test", &resource).unwrap();
        resource.path = "eval_independent/sources/../../../outside.csv".into();
        assert!(
            verify_resource(&root, "test", &resource)
                .unwrap_err()
                .contains("invalid resource provenance")
        );
        resource.path = path.into();
        let resources = BTreeMap::from([("test".into(), resource)]);
        let mut schema = Schema {
            fields: vec![Field {
                name: "name".into(),
                data_type: fieldkin::DataType::Text,
                samples: vec!["original".into(), "".into()],
            }],
        };
        let mut view = View {
            resource: "test".into(),
            format: "csv".into(),
            fields: schema.fields.clone(),
            rows: 2,
            type_evidence: vec!["test".into()],
            original_source_urls: vec!["https://example.org/original".into()],
        };
        verify_view(&root, &view, &schema, &resources).unwrap();
        schema.fields[0].samples[0] = "invented".into();
        assert!(
            verify_view(&root, &view, &schema, &resources)
                .unwrap_err()
                .contains("sample 0 differs")
        );
        schema.fields[0].samples[0] = "original".into();
        view.rows = 3;
        assert!(verify_view(&root, &view, &schema, &resources).is_err());
        view.rows = 0;
        assert!(
            verify_view(&root, &view, &schema, &resources)
                .unwrap_err()
                .contains("invalid publisher view")
        );
        fs::write(root.join(path), "name\nchanged\n").unwrap();
        assert!(
            verify_resource(&root, "test", &resources["test"])
                .unwrap_err()
                .contains("checksum changed")
        );
        fs::remove_file(root.join(path)).unwrap();
        assert!(
            verify_resource(&root, "test", &resources["test"])
                .unwrap_err()
                .contains(path)
        );
        fs::remove_dir_all(root).unwrap();
    }
}
