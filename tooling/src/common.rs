//! Shared deterministic records and checked subprocess execution.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant};

use serde::{
    de::{MapAccess, SeqAccess, Visitor},
    Deserialize, Deserializer,
};
use serde_json::Value;
use sha2::{Digest, Sha256};

pub fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_owned()
}

pub fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn read(path: &Path) -> Result<Vec<u8>, String> {
    checked_path(path)?;
    fs::read(path).map_err(|error| format!("{}: {error}", path.display()))
}

pub fn json(path: &Path) -> Result<Value, String> {
    let bytes = read(path)?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("JSON exceeds its byte limit".into());
    }
    serde_json::from_slice::<UniqueJson>(&bytes)
        .map(|value| value.0)
        .map_err(|error| format!("{}: {error}", path.display()))
}

struct UniqueJson(Value);
impl<'de> Deserialize<'de> for UniqueJson {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct UniqueVisitor;
        impl<'de> Visitor<'de> for UniqueVisitor {
            type Value = UniqueJson;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("JSON without duplicate object keys")
            }
            fn visit_bool<E: serde::de::Error>(self, value: bool) -> Result<Self::Value, E> {
                Ok(UniqueJson(Value::Bool(value)))
            }
            fn visit_i64<E: serde::de::Error>(self, value: i64) -> Result<Self::Value, E> {
                Ok(UniqueJson(value.into()))
            }
            fn visit_u64<E: serde::de::Error>(self, value: u64) -> Result<Self::Value, E> {
                Ok(UniqueJson(value.into()))
            }
            fn visit_f64<E: serde::de::Error>(self, value: f64) -> Result<Self::Value, E> {
                serde_json::Number::from_f64(value)
                    .map(|value| UniqueJson(Value::Number(value)))
                    .ok_or_else(|| E::custom("non-finite number"))
            }
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Self::Value, E> {
                Ok(UniqueJson(value.into()))
            }
            fn visit_string<E: serde::de::Error>(self, value: String) -> Result<Self::Value, E> {
                Ok(UniqueJson(value.into()))
            }
            fn visit_unit<E: serde::de::Error>(self) -> Result<Self::Value, E> {
                Ok(UniqueJson(Value::Null))
            }
            fn visit_seq<A: SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> Result<Self::Value, A::Error> {
                let mut values = Vec::new();
                while let Some(value) = sequence.next_element::<UniqueJson>()? {
                    values.push(value.0);
                }
                Ok(UniqueJson(Value::Array(values)))
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut keys = BTreeSet::new();
                let mut values = serde_json::Map::new();
                while let Some(key) = map.next_key::<String>()? {
                    if !keys.insert(key.clone()) {
                        return Err(serde::de::Error::custom("duplicate JSON object key"));
                    }
                    values.insert(key, map.next_value::<UniqueJson>()?.0);
                }
                Ok(UniqueJson(Value::Object(values)))
            }
        }
        deserializer.deserialize_any(UniqueVisitor)
    }
}

pub fn write_new(path: &Path, value: &Value) -> Result<(), String> {
    if path.exists() {
        return Err(format!("refusing to overwrite {}", path.display()));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let text = serde_json::to_string_pretty(value).map_err(|error| error.to_string())? + "\n";
    checked_path(path)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.write_all(text.as_bytes())
        .map_err(|error| error.to_string())
}

pub fn command(program: &str, args: &[String], directory: &Path) -> Result<Output, String> {
    let output = capture(program, args, directory, Duration::from_secs(1800))?;
    if !output.status.success() {
        return Err(format!(
            "{program} exited {}; no success record written",
            output.status
        ));
    }
    Ok(output)
}

/// Bound both direct-child execution and pipe completion. Descendants are trusted
/// local processes; this is a timeout, not an operating-system sandbox.
pub fn capture(
    program: &str,
    args: &[String],
    directory: &Path,
    timeout: Duration,
) -> Result<Output, String> {
    let deadline = Instant::now() + timeout;
    let mut child = Command::new(program)
        .args(args)
        .current_dir(directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("could not run {program}: {error}"))?;
    fn capture(stream: impl Read) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        stream
            .take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|error| error.to_string())?;
        if bytes.len() > 64 * 1024 * 1024 {
            return Err("tool output exceeds its byte budget".into());
        }
        Ok(bytes)
    }
    let stdout = child.stdout.take().ok_or("stdout pipe missing")?;
    let stderr = child.stderr.take().ok_or("stderr pipe missing")?;
    let (send, receive) = std::sync::mpsc::channel();
    let out_send = send.clone();
    std::thread::spawn(move || {
        let _ = out_send.send((true, capture(stdout)));
    });
    std::thread::spawn(move || {
        let _ = send.send((false, capture(stderr)));
    });
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err("tool exceeded its time budget; no success record written".into());
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let mut stdout = None;
    let mut stderr = None;
    for _ in 0..2 {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .ok_or("tool pipes exceeded the time budget")?;
        let (is_stdout, bytes) = receive
            .recv_timeout(remaining)
            .map_err(|_| "tool pipes exceeded the time budget")?;
        if is_stdout {
            stdout = Some(bytes?);
        } else {
            stderr = Some(bytes?);
        }
    }
    Ok(Output {
        status,
        stdout: stdout.ok_or("stdout result missing")?,
        stderr: stderr.ok_or("stderr result missing")?,
    })
}

pub fn source_inventory(directory: &Path) -> Result<BTreeMap<String, String>, String> {
    fn walk(
        base: &Path,
        directory: &Path,
        entries: &mut BTreeMap<String, String>,
    ) -> Result<(), String> {
        for entry in fs::read_dir(directory).map_err(|error| error.to_string())? {
            let path = entry.map_err(|error| error.to_string())?.path();
            checked_path(&path)?;
            if path.is_dir() {
                walk(base, &path, entries)?;
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                entries.insert(
                    path.strip_prefix(base)
                        .map_err(|error| error.to_string())?
                        .to_string_lossy()
                        .replace('\\', "/"),
                    digest(&read(&path)?),
                );
            }
        }
        Ok(())
    }
    let mut entries = BTreeMap::new();
    walk(directory, directory, &mut entries)?;
    Ok(entries)
}

pub fn executable(directory: &Path, name: &str) -> PathBuf {
    directory.join(if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    })
}

pub fn checked_path(path: &Path) -> Result<(), String> {
    for component in path.ancestors() {
        match fs::symlink_metadata(component) {
            Ok(metadata) => {
                let link = metadata.file_type().is_symlink();
                #[cfg(windows)]
                let link = {
                    use std::os::windows::fs::MetadataExt;
                    link || metadata.file_attributes() & 0x400 != 0
                };
                if link {
                    return Err(
                        "symlinks and junctions are unsupported in recorded tooling paths".into(),
                    );
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn records_reject_duplicate_keys_at_every_depth() {
        for text in [
            r#"{"binary":"first","binary":"second"}"#,
            r#"{"inputs":[{"sha":"a","sha":"b"}]}"#,
        ] {
            assert!(serde_json::from_str::<UniqueJson>(text).is_err());
        }
        let value = serde_json::from_str::<UniqueJson>(
            r#"{"array":[null,true,1,-2,1.5,"text"],"nested":{"sha":"a"}}"#,
        )
        .unwrap()
        .0;
        assert_eq!(value["array"].as_array().unwrap().len(), 6);
    }
    #[test]
    fn already_exhausted_execution_budget_fails_closed() {
        assert!(capture(
            "rustc",
            &["+stable".into(), "-V".into()],
            &root(),
            Duration::ZERO
        )
        .is_err());
    }
}
