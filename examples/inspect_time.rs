use std::{
    collections::BTreeSet,
    env,
    fs,
    io,
};

use chrono::DateTime;
use fieldkin::Schema;
use serde::Deserialize;

#[derive(Deserialize)]
struct EvalCase {
    source: Schema,
    target: Schema,
}

fn find_field<'a>(
    schema: &'a Schema,
    field_name: &str,
) -> Result<&'a fieldkin::Field, io::Error> {
    schema
        .fields
        .iter()
        .find(|field| field.name == field_name)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("field '{field_name}' was not found"),
            )
        })
}

fn timestamp_to_epoch_millis(value: &str) -> Option<i64> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|dt| dt.timestamp_millis())
}

fn inspect_pair(
    source_schema: &Schema,
    target_schema: &Schema,
    source_name: &str,
    target_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let source = find_field(source_schema, source_name)?;
    let target = find_field(target_schema, target_name)?;

    let source_millis = source
        .samples
        .iter()
        .filter_map(|value| timestamp_to_epoch_millis(value))
        .collect::<BTreeSet<_>>();

    let target_millis = target
        .samples
        .iter()
        .filter_map(|value| value.parse::<i64>().ok())
        .collect::<BTreeSet<_>>();

    let overlap = source_millis
        .intersection(&target_millis)
        .copied()
        .collect::<Vec<_>>();

    let source_containment = if source_millis.is_empty() {
        0.0
    } else {
        overlap.len() as f64 / source_millis.len() as f64
    };

    println!();
    println!("{}", "=".repeat(70));
    println!("{source_name}  ->  {target_name}");
    println!();

    println!("SOURCE");
    println!("  declared type: {:?}", source.data_type);
    println!("  stored samples: {}", source.samples.len());
    println!("  distinct converted timestamps: {}", source_millis.len());

    println!();

    println!("TARGET");
    println!("  declared type: {:?}", target.data_type);
    println!("  stored samples: {}", target.samples.len());
    println!("  distinct integer timestamps: {}", target_millis.len());

    println!();

    println!("COMPARISON");
    println!("  exact matching timestamps: {}", overlap.len());
    println!(
        "  source values found in target: {:.1}%",
        source_containment * 100.0
    );

    println!();

    println!("Example conversions:");

    for source_value in source.samples.iter().take(10) {
        match timestamp_to_epoch_millis(source_value) {
            Some(ms) => {
                let found = if target_millis.contains(&ms) {
                    "MATCH"
                } else {
                    "not found"
                };

                println!("  {source_value} -> {ms} -> {found}");
            }
            None => {
                println!("  {source_value} -> could not parse");
            }
        }
    }

    Ok(())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args()
        .nth(1)
        .unwrap_or_else(|| "eval_realworld/usgs_earthquakes.json".to_string());

    let raw = fs::read_to_string(&path)?;
    let case: EvalCase = serde_json::from_str(&raw)?;

    println!("FILE: {path}");

    inspect_pair(
        &case.source,
        &case.target,
        "time",
        "properties.time",
    )?;

    inspect_pair(
        &case.source,
        &case.target,
        "updated",
        "properties.updated",
    )?;

    Ok(())
}