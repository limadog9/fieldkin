//! Offline reproduction from pinned licensed archives. Never executes a matcher.

use std::collections::{BTreeMap, BTreeSet};
use std::io::{Cursor, Read};
use std::path::Path;

use serde::Serialize;
use serde_json::{json, Value};
use sha2::Digest;
use zip::CompressionMethod;

use crate::common::{digest, read, root};

const NORTHIX_BYTES: usize = 1_240_631;
// Upstream hashes are checked independently of the generated fixture.
const NORTHIX_PIN: &str = "2b158f99e1a041311177e9519c0d93a3821400121b809cbf7cb4614c6635a838";
const MEMBER_CAP: usize = 512 * 1024;

fn options(args: &[String]) -> Result<bool, String> {
    if args.iter().any(|arg| arg != "--check") {
        return Err(
            "offline imports accept only --check; pinned archives are already vendored".into(),
        );
    }
    Ok(args.iter().any(|arg| arg == "--check"))
}

fn pinned(path: &Path, bytes: usize, sha: &str) -> Result<Vec<u8>, String> {
    if std::fs::metadata(path)
        .map_err(|error| error.to_string())?
        .len()
        != bytes as u64
    {
        return Err("pinned archive byte count differs".into());
    }
    let data = read(path)?;
    if digest(&data) != sha {
        return Err("pinned archive digest differs".into());
    }
    Ok(data)
}

fn write_or_check(path: &Path, bytes: &[u8], check: bool) -> Result<(), String> {
    if path.exists() {
        if read(path)? != bytes {
            return Err(format!("{} differs from its pinned inputs", path.display()));
        }
    } else if check {
        return Err(format!("missing {}", path.display()));
    } else {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        std::fs::write(path, bytes).map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn pretty(value: &Value) -> Result<Vec<u8>, String> {
    Ok(
        (serde_json::to_string_pretty(value).map_err(|error| error.to_string())? + "\n")
            .into_bytes(),
    )
}

fn sample_indices(count: usize, cap: usize) -> Result<Vec<usize>, String> {
    if cap == 0 {
        return Err("sample cap must be positive".into());
    }
    let size = count.min(cap);
    Ok(if size < 2 {
        (0..size).collect()
    } else {
        (0..size)
            .map(|index| index * (count - 1) / (size - 1))
            .collect()
    })
}

fn filename(name: &str) -> Option<(&str, &str, &str, &str)> {
    let (stem, extension) = name.rsplit_once('.')?;
    let parts: Vec<_> = stem.split('@').collect();
    let token = |text: &str| {
        !text.is_empty() && text.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
    };
    if parts.len() != 3
        || !token(parts[0])
        || !token(parts[1])
        || !matches!(parts[2], "1" | "2")
        || !matches!(extension, "dat" | "txt")
    {
        return None;
    }
    Some((parts[0], parts[1], parts[2], extension))
}

fn permitted_member(name: &str, directory: bool) -> bool {
    if name.starts_with('/') || name.contains("//") || name.contains(['\\', ':', '\0']) {
        return false;
    }
    let parts: Vec<_> = name.trim_end_matches('/').split('/').collect();
    if parts.iter().any(|part| matches!(*part, "" | "." | ".."))
        || parts.first() != Some(&"Northix")
    {
        return false;
    }
    directory
        || name == "Northix/ReadMe.txt"
        || (parts.len() == 3 && parts[1] == "InputData" && filename(parts[2]).is_some())
        || (parts.len() == 4 && parts[1] == "Classes" && filename(parts[3]).is_some())
}

fn northix_members(data: &[u8]) -> Result<BTreeMap<String, Vec<u8>>, String> {
    if data.len() > NORTHIX_BYTES {
        return Err("oversized Northix archive".into());
    }
    let mut archive = zip::ZipArchive::new(Cursor::new(data)).map_err(|error| error.to_string())?;
    // ZipArchive indexes by name. Check the bounded single-disk EOCD count too,
    // so duplicate names cannot disappear inside the library's map.
    let end = data
        .windows(4)
        .enumerate()
        .rev()
        .find_map(|(offset, signature)| {
            (signature == b"PK\x05\x06"
                && offset + 22 <= data.len()
                && offset
                    + 22
                    + u16::from_le_bytes([data[offset + 20], data[offset + 21]]) as usize
                    == data.len())
            .then_some(offset)
        })
        .ok_or("missing bounded ZIP end record")?;
    let count = u16::from_le_bytes([data[end + 10], data[end + 11]]) as usize;
    if count > 512
        || count != archive.len()
        || data[end + 4..end + 8] != [0, 0, 0, 0]
        || data[end + 8..end + 10] != data[end + 10..end + 12]
    {
        return Err("duplicate, oversized or multi-disk ZIP".into());
    }
    let mut expanded = 0_u64;
    let mut result = BTreeMap::new();
    let mut seen = BTreeSet::new();
    for index in 0..archive.len() {
        let member = archive.by_index(index).map_err(|error| error.to_string())?;
        let name = member.name().to_owned();
        if !permitted_member(&name, member.is_dir())
            || member.name_raw() != name.as_bytes()
            || !seen.insert(name.clone())
            || member.encrypted()
            || !matches!(
                member.compression(),
                CompressionMethod::Stored | CompressionMethod::Deflated
            )
            || member
                .unix_mode()
                .is_some_and(|mode| mode & 0o170000 == 0o120000)
        {
            return Err("unsafe, duplicated, encrypted or unsupported ZIP member".into());
        }
        expanded = expanded
            .checked_add(member.size())
            .ok_or("ZIP size overflow")?;
        if member.size() > MEMBER_CAP as u64 || expanded > 8 * 1024 * 1024 {
            return Err("ZIP expansion budget exceeded".into());
        }
        if member.is_dir() {
            continue;
        }
        let expected = member.size();
        let mut content = Vec::new();
        member
            .take((MEMBER_CAP + 1) as u64)
            .read_to_end(&mut content)
            .map_err(|error| error.to_string())?;
        if content.len() as u64 != expected {
            return Err("ZIP member size differs".into());
        }
        result.insert(name, content);
    }
    Ok(result)
}

fn northix_value(data: &[u8]) -> Result<Value, String> {
    let members = northix_members(data)?;
    let inputs: BTreeMap<_, _> = members
        .iter()
        .filter(|(path, _)| path.contains("/InputData/"))
        .map(|(path, content)| (path.rsplit('/').next().unwrap().to_owned(), (path, content)))
        .collect();
    if inputs.is_empty() {
        return Err("archive contains no input columns".into());
    }
    let mut labels = BTreeMap::new();
    let mut discrepancies = Vec::new();
    for (path, content) in members
        .iter()
        .filter(|(path, _)| path.contains("/Classes/"))
    {
        let parts: Vec<_> = path.split('/').collect();
        let id = parts[3];
        let (input_path, input) = inputs.get(id).ok_or("unknown labeled input column")?;
        if labels.insert(id.to_owned(),json!({"field":id,"class":parts[2],"class_path":path,"class_sha256":digest(content)})).is_some() {
            return Err("multiply labeled column".into());
        }
        if *input != content {
            discrepancies.push(json!({"field":id,"input_path":input_path,"input_sha256":digest(input),
                "class_path":path,"class_sha256":digest(content),"input_bytes":input.len(),"class_bytes":content.len()}));
        }
    }
    if inputs.keys().ne(labels.keys()) {
        return Err("not every column is labeled".into());
    }
    let mut grouped = BTreeMap::<(String, String), Vec<Value>>::new();
    let mut ascii = 0;
    for (id, (path, content)) in &inputs {
        let (name, table, database, extension) = filename(id).ok_or("invalid input filename")?;
        if extension != if database == "1" { "dat" } else { "txt" } {
            return Err("database and extension differ".into());
        }
        if content
            .iter()
            .any(|byte| (0x80..=0x9f).contains(byte) || *byte == 0)
        {
            return Err("encoding audit no longer holds".into());
        }
        ascii += usize::from(content.is_ascii());
        let text: String = content.iter().map(|byte| char::from(*byte)).collect();
        let text = text
            .replace("\r\n", "\n")
            .replace(['\r', '\u{b}', '\u{c}', '\u{1c}', '\u{1d}', '\u{1e}'], "\n");
        let lines: Vec<_> = text.split_terminator('\n').collect();
        let indices = sample_indices(lines.len(), 64)?;
        let blank = |line: &str| {
            line.chars()
                .all(|c| c.is_whitespace() || ('\u{1c}'..='\u{1f}').contains(&c))
        };
        let samples: Vec<_> = indices
            .iter()
            .map(|index| {
                if blank(lines[*index]) {
                    Value::Null
                } else {
                    json!(lines[*index])
                }
            })
            .collect();
        grouped.entry((database.into(),table.into())).or_default().push(json!({
            "id":id,"name":name,"samples":samples,"sample_row_indices":indices,"row_count":lines.len(),
            "input_path":path,"input_sha256":digest(content),"input_bytes":content.len(),
            "blank_rows":lines.iter().filter(|line|blank(line)).count()
        }));
    }
    let tables: Vec<_> = grouped
        .into_iter()
        .map(|((database, name), fields)| {
            json!({
                "id":format!("db{database}:{name}"),"database":database,"name":name,"fields":fields
            })
        })
        .collect();
    let mut pairs = Vec::new();
    for source in tables.iter().filter(|table| table["database"] == "1") {
        for target in tables.iter().filter(|table| table["database"] == "2") {
            pairs.push(json!({"id":format!("{}->{}",source["id"].as_str().unwrap(),target["id"].as_str().unwrap()),
                "source_table":source["id"],"target_table":target["id"]}));
        }
    }
    pairs.sort_by(|a, b| a["id"].as_str().cmp(&b["id"].as_str()));
    discrepancies.sort_by(|a, b| a["field"].as_str().cmp(&b["field"].as_str()));
    Ok(
        json!({"format":"fieldkin-northix-v1","tables":tables,"pairs":pairs,"labels":labels.into_values().collect::<Vec<_>>(),
        "discrepancies":discrepancies,"encoding_audit":{"encoding":"ISO-8859-1","ascii_files":ascii,"non_ascii_files":inputs.len()-ascii,"cp1252_equivalent":true}}),
    )
}

fn northix_inventory(value: &Value) -> Result<Value, String> {
    let labels: BTreeMap<_, _> = value["labels"]
        .as_array()
        .ok_or("missing labels")?
        .iter()
        .map(|label| {
            (
                label["field"].as_str().unwrap(),
                label["class"].as_str().unwrap(),
            )
        })
        .collect();
    let tables: BTreeMap<_, _> = value["tables"]
        .as_array()
        .ok_or("missing tables")?
        .iter()
        .map(|table| (table["id"].as_str().unwrap(), table))
        .collect();
    let mut counts = BTreeMap::from([
        (
            "table_pairs".to_owned(),
            value["pairs"].as_array().unwrap().len(),
        ),
        ("unique_fields".into(), labels.len()),
        (
            "label_classes".into(),
            labels.values().collect::<BTreeSet<_>>().len(),
        ),
        (
            "value_copy_discrepancies".into(),
            value["discrepancies"].as_array().unwrap().len(),
        ),
    ]);
    for table in tables.values() {
        let database = table["database"].as_str().unwrap();
        *counts
            .entry(format!("database_{database}_tables"))
            .or_default() += 1;
        *counts
            .entry(format!("database_{database}_fields"))
            .or_default() += table["fields"].as_array().unwrap().len();
    }
    for pair in value["pairs"].as_array().unwrap() {
        let source = tables[pair["source_table"].as_str().unwrap()];
        let target = tables[pair["target_table"].as_str().unwrap()];
        let mut edges = 0;
        for field in source["fields"].as_array().unwrap() {
            let class = labels[field["id"].as_str().unwrap()];
            let positives = target["fields"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|field| {
                    class != "UNCLASSED" && class == labels[field["id"].as_str().unwrap()]
                })
                .count();
            for (key, amount) in [
                ("source_field_occurrences", 1),
                ("positive_field_occurrences", usize::from(positives > 0)),
                ("no_match_field_occurrences", usize::from(positives == 0)),
                (
                    "explicit_unclassed_field_occurrences",
                    usize::from(class == "UNCLASSED"),
                ),
            ] {
                *counts.entry(key.into()).or_default() += amount;
            }
            edges += positives;
        }
        for (key, amount) in [
            ("positive_edges", edges),
            ("pairs_with_positive_edges", usize::from(edges > 0)),
            ("pairs_without_positive_edges", usize::from(edges == 0)),
        ] {
            *counts.entry(key.into()).or_default() += amount;
        }
    }
    serde_json::to_value(counts).map_err(|error| error.to_string())
}

pub fn northix(args: Vec<String>) -> Result<(), String> {
    let check = options(&args)?;
    let directory = root().join("evaluation/external/northix-v1");
    let value = northix_value(&pinned(
        &directory.join("northix.zip"),
        NORTHIX_BYTES,
        NORTHIX_PIN,
    )?)?;
    let data = pretty(&value)?;
    write_or_check(
        &root().join("evaluation/fixtures/northix-v1.json"),
        &data,
        check,
    )?;
    let provenance: Value = crate::common::json(&directory.join("provenance.json"))?;
    if provenance["archive"]["sha256"] != NORTHIX_PIN
        || provenance["derived_fixture"]["sha256"] != digest(&data)
        || provenance["inventory"] != northix_inventory(&value)?
    {
        return Err("Northix provenance differs".into());
    }
    println!("Northix fixture and licensed source provenance reproduced; no matcher executed.");
    Ok(())
}

#[derive(Serialize)]
struct Attribute {
    index: u64,
    name: String,
    positive_targets: Vec<String>,
}
#[derive(Serialize)]
struct Property {
    uri: String,
    name: String,
}
#[derive(Serialize)]
struct Class {
    uri: String,
    name: String,
    partition: String,
    properties: Vec<Property>,
}
#[derive(Serialize)]
struct Table {
    id: String,
    class_uri: String,
    fields: Vec<Attribute>,
}
#[derive(Serialize)]
struct Excluded {
    table_id: String,
    reason: String,
}

fn csv_rows(data: &[u8]) -> Result<Vec<csv::StringRecord>, String> {
    csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(data)
        .records()
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| error.to_string())
}

fn attributes(data: &[u8]) -> Result<Vec<Attribute>, String> {
    let mut fields = BTreeMap::<u64, Attribute>::new();
    for row in csv_rows(data)? {
        if row.len() != 4
            || row[0].is_empty()
            || !matches!(&row[2], "True" | "False" | "")
            || row[3].is_empty()
            || !row[3].bytes().all(|byte| byte.is_ascii_digit())
        {
            return Err("malformed attribute annotation".into());
        }
        let index = row[3].parse().map_err(|_| "attribute index overflow")?;
        let field = fields.entry(index).or_insert_with(|| Attribute {
            index,
            name: row[1].into(),
            positive_targets: Vec::new(),
        });
        if field.name != row[1] {
            return Err("inconsistent header for one index".into());
        }
        if !field
            .positive_targets
            .iter()
            .any(|target| target == &row[0])
        {
            field.positive_targets.push(row[0].into());
        }
    }
    for field in fields.values_mut() {
        field.positive_targets.sort();
    }
    Ok(fields.into_values().collect())
}

fn t2d_bytes(attributes_gz: &[u8], class_bytes: &[u8]) -> Result<Vec<u8>, String> {
    let mut class_map = BTreeMap::new();
    for row in csv_rows(class_bytes)? {
        if row.len() != 4 || !row[0].ends_with(".tar.gz") || row[2].is_empty() {
            return Err("malformed class annotation".into());
        }
        if class_map
            .insert(
                row[0].trim_end_matches(".tar.gz").to_owned(),
                (row[1].to_owned(), row[2].to_owned()),
            )
            .is_some()
        {
            return Err("duplicate class annotation".into());
        }
    }
    let decoder = flate2::read::GzDecoder::new(Cursor::new(attributes_gz));
    let mut archive = tar::Archive::new(decoder);
    let mut tables = Vec::new();
    let mut excluded = Vec::new();
    let mut classes = BTreeMap::<String, (String, BTreeSet<String>)>::new();
    let mut seen = BTreeSet::new();
    let mut expanded = 0;
    for item in archive.entries().map_err(|error| error.to_string())? {
        let mut member = item.map_err(|error| error.to_string())?;
        let name = std::str::from_utf8(&member.path_bytes())
            .map_err(|_| "non-UTF8 attribute path")?
            .to_owned();
        let id = name
            .strip_suffix(".csv")
            .ok_or("unexpected attribute archive member")?;
        if id.is_empty()
            || !id.bytes().all(|byte| byte.is_ascii_digit() || byte == b'_')
            || !member.header().entry_type().is_file()
            || !seen.insert(name.clone())
            || seen.len() > 2000
            || member.size() > 100_000
        {
            return Err("unsafe, duplicate or oversized attribute member".into());
        }
        expanded += member.size();
        if expanded > 16 * 1024 * 1024 {
            return Err("attribute expansion budget exceeded".into());
        }
        let mut data = Vec::new();
        member
            .by_ref()
            .take(100_001)
            .read_to_end(&mut data)
            .map_err(|error| error.to_string())?;
        if data.len() as u64 != member.size() {
            return Err("attribute size differs".into());
        }
        let fields = attributes(&data)?;
        let reason = if fields.is_empty() {
            Some("empty_attribute_annotations")
        } else if !class_map.contains_key(id) {
            Some("missing_class_annotation")
        } else {
            None
        };
        if let Some(reason) = reason {
            excluded.push(Excluded {
                table_id: id.into(),
                reason: reason.into(),
            });
            continue;
        }
        let (class_name, uri) = &class_map[id];
        let target = classes
            .entry(uri.clone())
            .or_insert_with(|| (class_name.clone(), BTreeSet::new()));
        if target.0 != *class_name {
            return Err("class URI has inconsistent names".into());
        }
        target.1.extend(
            fields
                .iter()
                .flat_map(|field| field.positive_targets.iter().cloned()),
        );
        tables.push(Table {
            id: id.into(),
            class_uri: uri.clone(),
            fields,
        });
    }
    let classes: Vec<_> = classes
        .into_iter()
        .map(|(uri, (name, properties))| {
            let partition = if sha2::Sha256::digest(uri.as_bytes())[0] % 5 == 0 {
                "holdout"
            } else {
                "development"
            };
            Class {
                uri,
                name,
                partition: partition.into(),
                properties: properties
                    .into_iter()
                    .map(|uri| {
                        let name = uri.rsplit(['/', '#']).next().unwrap().to_owned();
                        Property { uri, name }
                    })
                    .collect(),
            }
        })
        .collect();
    tables.sort_by(|a, b| a.id.cmp(&b.id));
    excluded.sort_by(|a, b| a.table_id.cmp(&b.table_id));
    fn lines<T: Serialize>(values: &[T]) -> Result<Vec<String>, String> {
        values
            .iter()
            .enumerate()
            .map(|(index, item)| {
                Ok(format!(
                    "    {}{}",
                    serde_json::to_string(item).map_err(|error| error.to_string())?,
                    if index + 1 < values.len() { "," } else { "" }
                ))
            })
            .collect()
    }
    let mut text =
        String::from("{\n  \"format\": \"fieldkin-t2d-annotations-v1\",\n  \"classes\": [\n");
    text.push_str(&lines(&classes)?.join("\n"));
    text.push_str("\n  ],\n  \"tables\": [\n");
    text.push_str(&lines(&tables)?.join("\n"));
    text.push_str("\n  ],\n  \"excluded\": [\n");
    text.push_str(&lines(&excluded)?.join("\n"));
    text.push_str("\n  ]\n}\n");
    Ok(text.into_bytes())
}

pub fn t2d(args: Vec<String>) -> Result<(), String> {
    let check = options(&args)?;
    let directory = root().join("evaluation/external/t2d-v1");
    let attributes = pinned(
        &directory.join("attributes_complete.tar.gz"),
        84_557,
        "c35dbeadab8908a72c3f58315a6710039b292a42a7e765ff4a3e4191f8255ba2",
    )?;
    let classes = pinned(
        &directory.join("classes_complete.csv"),
        154_844,
        "6a3724e4cce88e3b10ceb97bbdff6da3c397e7a4f9f1bd24b6ff6a7e47f6956c",
    )?;
    let data = t2d_bytes(&attributes, &classes)?;
    let path = directory.join("corpus.json");
    if path.exists()
        && String::from_utf8(read(&path)?)
            .map_err(|error| error.to_string())?
            .replace("\r\n", "\n")
            .as_bytes()
            == data
    {
        println!("T2D annotation-only fixture reproduced; no matcher executed.");
        return Ok(());
    }
    write_or_check(&path, &data, check)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn endpoint_samples_are_bounded_and_deterministic() {
        assert_eq!(sample_indices(0, 64).unwrap(), Vec::<usize>::new());
        assert_eq!(sample_indices(1, 64).unwrap(), vec![0]);
        let indices = sample_indices(1000, 64).unwrap();
        assert_eq!(indices.len(), 64);
        assert_eq!(indices[0], 0);
        assert_eq!(indices[63], 999);
        assert!(indices.windows(2).all(|window| window[0] < window[1]));
        assert!(sample_indices(4, 0).is_err());
    }
    #[test]
    fn archive_paths_reject_traversal_links_and_new_inputs() {
        for name in [
            "Northix/../escape",
            "/Northix/InputData/x@y@1.dat",
            "Northix//InputData/x@y@1.dat",
            "Northix\\InputData\\x@y@1.dat",
            "Northix/Other/x@y@1.dat",
            "Northix/InputData/x@y@3.dat",
        ] {
            assert!(!permitted_member(name, false), "{name}");
        }
        assert!(permitted_member("Northix/InputData/x@y@1.dat", false));
    }
    #[test]
    fn attributes_validate_ambiguity_and_quote_boundaries() {
        let fields = attributes(b"uri,\"a,b\",True,2\nother,\"a,b\",False,2\n").unwrap();
        assert_eq!(fields.len(), 1);
        assert_eq!(fields[0].positive_targets, vec!["other", "uri"]);
        assert!(attributes(b"uri,a,True,2\nother,b,False,2\n").is_err());
        assert!(attributes(b"uri,a,Maybe,2\n").is_err());
    }
}
