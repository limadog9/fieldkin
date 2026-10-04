//! Narrow comparison for historical behavior snapshots without rewriting provenance.

use serde_json::Value;

pub fn same_behavior_json(existing: &str, current: &str) -> Result<bool, String> {
    let strip_implementation_hashes = |text: &str| -> Result<Value, String> {
        let mut value: Value = serde_json::from_str(text).map_err(|error| error.to_string())?;
        let metadata = value["metadata"]
            .as_object_mut()
            .ok_or("snapshot metadata is missing")?;
        for key in ["engine_source_sha256", "evaluator_source_sha256"] {
            let hashes = metadata
                .remove(key)
                .ok_or("snapshot source hashes are missing")?;
            if !hashes.is_object() {
                return Err("snapshot source hashes must be maps".into());
            }
        }
        Ok(value)
    };
    Ok(strip_implementation_hashes(existing)? == strip_implementation_hashes(current)?)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn only_two_implementation_hash_maps_are_exempt() {
        let original = json!({"metadata":{
            "engine_source_sha256":{"engine.rs":"old"},
            "evaluator_source_sha256":{"release.rs":"old"},
            "protocol_sha256":"protocol","original_corpus_sha256":{"domain":"fixture"},
            "manifest_sha256":{"Cargo.toml":"manifest"},"cargo_lock":"full lock",
            "cargo_lock_sha256":"lock", "extension_sha256":"extension"
        },"rows":[{"score":0.7,"counts":{"proposals":4}}],
        "default_predictions":[{"report_sha256":"whole-report","selected":"id"}]});
        let mut changed = original.clone();
        changed["metadata"]["engine_source_sha256"] = json!({"engine.rs":"new","new.rs":"new"});
        changed["metadata"]["evaluator_source_sha256"] = json!({"release.rs":"new"});
        assert!(same_behavior_json(&original.to_string(), &changed.to_string()).unwrap());
        for key in [
            "protocol_sha256",
            "original_corpus_sha256",
            "manifest_sha256",
            "cargo_lock",
            "cargo_lock_sha256",
            "extension_sha256",
        ] {
            changed = original.clone();
            changed["metadata"][key] = json!("different");
            assert!(!same_behavior_json(&original.to_string(), &changed.to_string()).unwrap());
        }
        for (pointer, value) in [
            ("/rows/0/score", json!(0.8)),
            ("/rows/0/counts/proposals", json!(3)),
            ("/default_predictions/0/report_sha256", json!("different")),
            ("/default_predictions/0/selected", json!("other")),
        ] {
            changed = original.clone();
            *changed.pointer_mut(pointer).unwrap() = value;
            assert!(!same_behavior_json(&original.to_string(), &changed.to_string()).unwrap());
        }
        changed = original.clone();
        changed["metadata"]
            .as_object_mut()
            .unwrap()
            .remove("engine_source_sha256");
        assert!(same_behavior_json(&original.to_string(), &changed.to_string()).is_err());
        changed = original.clone();
        changed["metadata"]["evaluator_source_sha256"] = json!("wrong shape");
        assert!(same_behavior_json(&original.to_string(), &changed.to_string()).is_err());
    }
}
