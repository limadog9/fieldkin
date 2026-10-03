use std::collections::BTreeSet;

use fieldkin::{DataType, Field, SampleValue, Schema};
use serde_json::Value;

use crate::model::{Case, Family, InputField, LabelKind};

pub const PROTOCOL: &str = include_str!("../protocol.json");
pub const FILES: [(&str, &str); 4] = [
    (
        "product_import",
        include_str!("../corpus/product_import.json"),
    ),
    (
        "financial_records",
        include_str!("../corpus/financial_records.json"),
    ),
    ("crm_contacts", include_str!("../corpus/crm_contacts.json")),
    (
        "operational_telemetry",
        include_str!("../corpus/operational_telemetry.json"),
    ),
];

pub fn protocol() -> Result<Value, String> {
    serde_json::from_str(PROTOCOL).map_err(|e| format!("invalid evaluation protocol: {e}"))
}

pub fn families() -> Result<Vec<Family>, String> {
    let mut result = Vec::new();
    let mut ids = BTreeSet::new();
    for (domain, text) in FILES {
        let group: Vec<Family> =
            serde_json::from_str(text).map_err(|e| format!("invalid {domain} corpus: {e}"))?;
        if group.len() != 10 {
            return Err(format!("{domain} must have 10 independent families"));
        }
        for family in group {
            if family.domain != domain || !ids.insert(family.id.clone()) {
                return Err("duplicate family ID or wrong domain".into());
            }
            validate_family(&family)?;
            result.push(family);
        }
    }
    result.sort_by(|a, b| a.id.cmp(&b.id));
    let protocol = protocol()?;
    let held_out = protocol["holdout_families"]
        .as_array()
        .ok_or("missing holdout partition")?;
    let mut split_ids = BTreeSet::new();
    for id in held_out {
        let id = id.as_str().ok_or("invalid holdout ID")?;
        if !ids.contains(id) || !split_ids.insert(id) {
            return Err("unknown or duplicate holdout family".into());
        }
    }
    if split_ids.len() != 8 {
        return Err("exactly 8 of 40 families must be held out".into());
    }
    for (domain, _) in FILES {
        if result
            .iter()
            .filter(|f| f.domain == domain && split_ids.contains(f.id.as_str()))
            .count()
            != 2
        {
            return Err("each domain must have two held-out families".into());
        }
    }
    Ok(result)
}

pub fn validate_family(family: &Family) -> Result<(), String> {
    if family.id.is_empty()
        || family.title.trim().is_empty()
        || family.rationale.trim().is_empty()
        || family.tags.is_empty()
        || family.tags.iter().any(|s| s.trim().is_empty())
    {
        return Err("family identity, tags and rationale are required".into());
    }
    if family.source.len() != 6 || !(5..=9).contains(&family.target.len()) {
        return Err(format!(
            "{} must have six sources and 5-9 targets",
            family.id
        ));
    }
    let source = field_ids(&family.source)?;
    let target = field_ids(&family.target)?;
    let mut labeled = BTreeSet::new();
    for label in &family.labels {
        if !source.contains(label.source.as_str())
            || !labeled.insert(label.source.as_str())
            || label.rationale.trim().is_empty()
        {
            return Err(format!(
                "{} contains missing/duplicate/invalid source labels",
                family.id
            ));
        }
        let unique: BTreeSet<_> = label.targets.iter().collect();
        if unique.len() != label.targets.len()
            || label.targets.iter().any(|id| !target.contains(id.as_str()))
        {
            return Err(format!(
                "{} label references duplicate or absent targets",
                family.id
            ));
        }
        let valid = match label.kind {
            LabelKind::Match => label.targets.len() == 1,
            LabelKind::NoMatch => label.targets.is_empty(),
            LabelKind::Ambiguous => label.targets.len() >= 2,
        };
        if !valid {
            return Err(format!("{} has inconsistent label cardinality", family.id));
        }
    }
    if source != labeled {
        return Err(format!("{} does not label every source", family.id));
    }
    Ok(())
}

fn field_ids(fields: &[InputField]) -> Result<BTreeSet<&str>, String> {
    let mut ids = BTreeSet::new();
    for field in fields {
        if field.id.is_empty() || field.concept.trim().is_empty() || !ids.insert(field.id.as_str())
        {
            return Err("field IDs/concepts must be present; IDs unique within a schema".into());
        }
        convert_field(field)?;
    }
    Ok(ids)
}

pub fn schema(fields: &[InputField]) -> Result<Schema, String> {
    fields
        .iter()
        .map(convert_field)
        .collect::<Result<Vec<_>, _>>()
        .map(Schema::new)
}

fn convert_field(input: &InputField) -> Result<Field, String> {
    let kind = match input.data_type.as_str() {
        "Unknown" => DataType::Unknown,
        "Boolean" => DataType::Boolean,
        "Integer" => DataType::Integer,
        "Float" => DataType::Float,
        "Decimal" => DataType::Decimal,
        "Text" => DataType::Text,
        "Date" => DataType::Date,
        "Timestamp" => DataType::Timestamp,
        "Binary" => DataType::Binary,
        _ => return Err("unknown declared data type".into()),
    };
    let mut field = Field::new(input.id.clone(), input.name.clone(), kind);
    field.samples = input
        .samples
        .as_ref()
        .map(|values| {
            values
                .iter()
                .map(|value| match value {
                    Value::Null => Ok(SampleValue::Null),
                    Value::Bool(value) => Ok(SampleValue::Boolean(*value)),
                    Value::Number(value) => value
                        .as_f64()
                        .filter(|v| v.is_finite())
                        .map(SampleValue::Number)
                        .ok_or_else(|| "invalid numeric sample".to_owned()),
                    Value::String(value) => Ok(SampleValue::Text(value.clone())),
                    _ => Err("only primitive sample values are supported".to_owned()),
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    Ok(field)
}

pub fn cases(families: &[Family]) -> Result<Vec<Case>, String> {
    let protocol = protocol()?;
    let seed = protocol["generator_seed"]
        .as_u64()
        .ok_or("missing generator seed")?;
    let variants = protocol["variants"].as_array().ok_or("missing variants")?;
    let holdout: BTreeSet<_> = protocol["holdout_families"]
        .as_array()
        .ok_or("missing holdout")?
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let mut result = Vec::new();
    for (index, family) in families.iter().enumerate() {
        for (variant_index, variant) in variants.iter().enumerate() {
            let variant = variant.as_str().ok_or("invalid variant")?;
            let case_seed = seed + (index as u64) * 100 + variant_index as u64;
            let mut source = family.source.clone();
            let mut target = family.target.clone();
            match variant {
                "base" => {}
                "reordered" => {
                    shuffle(&mut source, case_seed);
                    shuffle(&mut target, case_seed ^ 0x9e37_79b9);
                }
                "without_samples" => {
                    for field in source.iter_mut().chain(&mut target) {
                        field.samples = None;
                    }
                }
                "null_heavy" => {
                    for field in source.iter_mut().chain(&mut target) {
                        if let Some(samples) = &mut field.samples {
                            // Existing emptiness remains empty. For observed values append three
                            // missing observations per original sample; never create agreement.
                            let missing = samples.len() * 3;
                            samples.extend(std::iter::repeat(Value::Null).take(missing));
                        }
                    }
                }
                "separator_noise" => {
                    for field in &mut source {
                        field.name = format!("__{}--", field.name.replace(['_', '-', ' '], "."));
                    }
                    for field in &mut target {
                        field.name = format!("  {}  ", field.name.replace(['-', '.', ' '], "_"));
                    }
                }
                _ => return Err("unrecognized corpus variant".into()),
            }
            let mut tags = family.tags.clone();
            tags.push(variant.to_owned());
            if source.len() != target.len() {
                tags.push("unequal_sizes".into());
            }
            tags.sort();
            tags.dedup();
            result.push(Case {
                id: format!("{}/{variant}", family.id),
                family_id: family.id.clone(),
                domain: family.domain.clone(),
                tags,
                variant: variant.to_owned(),
                split: if holdout.contains(family.id.as_str()) {
                    "holdout"
                } else {
                    "development"
                }
                .into(),
                seed: case_seed,
                source,
                target,
                labels: family.labels.clone(),
            });
        }
    }
    Ok(result)
}

fn shuffle<T>(items: &mut [T], mut state: u64) {
    for end in (1..items.len()).rev() {
        // Fixed wrapping LCG and Fisher-Yates: reproducible on every supported platform.
        state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        items.swap(end, (state % (end as u64 + 1)) as usize);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corpus_has_full_labels_balanced_domains_and_disjoint_families() {
        let families = families().unwrap();
        let cases = cases(&families).unwrap();
        assert_eq!(families.len(), 40);
        assert_eq!(cases.len(), 200);
        assert_eq!(cases.iter().map(|c| c.labels.len()).sum::<usize>(), 1200);
        assert_eq!(cases.iter().filter(|c| c.split == "holdout").count(), 40);
        let dev: BTreeSet<_> = cases
            .iter()
            .filter(|c| c.split == "development")
            .map(|c| &c.family_id)
            .collect();
        let holdout: BTreeSet<_> = cases
            .iter()
            .filter(|c| c.split == "holdout")
            .map(|c| &c.family_id)
            .collect();
        assert_eq!(dev.len(), 32);
        assert_eq!(holdout.len(), 8);
        assert!(dev.is_disjoint(&holdout));
        assert_eq!(
            serde_json::to_vec(&cases).unwrap(),
            serde_json::to_vec(&super::cases(&families).unwrap()).unwrap()
        );
    }

    #[test]
    fn malformed_ground_truth_is_rejected() {
        let family = families().unwrap().remove(0);
        let mut bad = family.clone();
        bad.labels.pop();
        assert!(validate_family(&bad).is_err());
        let mut bad = family.clone();
        bad.labels[0].targets.push("absent".into());
        assert!(validate_family(&bad).is_err());
        let mut bad = family.clone();
        bad.source[1].id = bad.source[0].id.clone();
        assert!(validate_family(&bad).is_err());
        let mut bad = family;
        bad.labels.push(bad.labels[0].clone());
        assert!(validate_family(&bad).is_err());
    }

    #[test]
    fn variants_preserve_labels_and_reordering_preserves_inputs_by_id() {
        let families = families().unwrap();
        for group in cases(&families).unwrap().chunks(5) {
            let base = &group[0];
            for case in group {
                assert_eq!(
                    serde_json::to_value(&base.labels).unwrap(),
                    serde_json::to_value(&case.labels).unwrap()
                );
                assert_eq!(case.split, base.split);
            }
            let mut base_source = base.source.clone();
            let mut reordered = group[1].source.clone();
            base_source.sort_by(|a, b| a.id.cmp(&b.id));
            reordered.sort_by(|a, b| a.id.cmp(&b.id));
            assert_eq!(
                serde_json::to_value(base_source).unwrap(),
                serde_json::to_value(reordered).unwrap()
            );
            assert!(group[2]
                .source
                .iter()
                .chain(&group[2].target)
                .all(|f| f.samples.is_none()));
        }
    }
}
