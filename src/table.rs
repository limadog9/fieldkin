use crate::{DataType, Field, Schema};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, fmt, io::Read};

/// Recoverable input, configuration, I/O, and algorithm failures.
#[derive(Debug)]
pub enum Error {
    InvalidInput(String),
    InvalidConfig(String),
    Algorithm(String),
    Io(std::io::Error),
    Csv(csv::Error),
    Json(serde_json::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidInput(s) | Self::InvalidConfig(s) | Self::Algorithm(s) => f.write_str(s),
            Self::Io(e) => e.fmt(f),
            Self::Csv(e) => e.fmt(f),
            Self::Json(e) => e.fmt(f),
        }
    }
}
impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(e) => Some(e),
            Self::Csv(e) => Some(e),
            Self::Json(e) => Some(e),
            _ => None,
        }
    }
}
impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e)
    }
}
impl From<csv::Error> for Error {
    fn from(e: csv::Error) -> Self {
        Self::Csv(e)
    }
}
impl From<serde_json::Error> for Error {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct MatchOptions {
    /// Maximum nonempty values per column; `None` keeps all values, zero clears samples.
    pub instance_sample_size: Option<usize>,
}
impl Default for MatchOptions {
    fn default() -> Self {
        Self {
            instance_sample_size: Some(1000),
        }
    }
}

/// An owned, framework-independent columnar table. Empty columns are supported.
///
/// Nulls are omitted from samples. Types can be supplied explicitly or inferred
/// by the row/CSV/JSON constructors. Column names and table names must be unique.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Table {
    pub name: String,
    pub columns: Vec<Field>,
}

impl Table {
    pub fn new(name: impl Into<String>, columns: Vec<Field>) -> Result<Self, Error> {
        let table = Self {
            name: name.into(),
            columns,
        };
        table.validate()?;
        Ok(table)
    }
    pub fn from_schema(name: impl Into<String>, schema: &Schema) -> Self {
        Self {
            name: name.into(),
            columns: schema.fields.clone(),
        }
    }
    pub fn validate(&self) -> Result<(), Error> {
        if self.name.trim().is_empty() {
            return Err(Error::InvalidInput("table name must be nonempty".into()));
        }
        let mut names = BTreeSet::new();
        for col in &self.columns {
            if col.name.trim().is_empty() {
                return Err(Error::InvalidInput("column name must be nonempty".into()));
            }
            if !names.insert(&col.name) {
                return Err(Error::InvalidInput(format!(
                    "duplicate column name: {}",
                    col.name
                )));
            }
        }
        Ok(())
    }

    /// Build a table from ordered headers and nullable string rows.
    pub fn from_rows(
        name: impl Into<String>,
        headers: Vec<String>,
        rows: impl IntoIterator<Item = Vec<Option<String>>>,
    ) -> Result<Self, Error> {
        let mut columns: Vec<_> = headers
            .into_iter()
            .map(|name| Field {
                name,
                data_type: DataType::Unknown,
                samples: Vec::new(),
            })
            .collect();
        for row in rows {
            if row.len() != columns.len() {
                return Err(Error::InvalidInput(
                    "row width does not match headers".into(),
                ));
            }
            for (column, value) in columns.iter_mut().zip(row) {
                if let Some(value) = value.filter(|v| !v.is_empty()) {
                    column.samples.push(value);
                }
            }
        }
        for column in &mut columns {
            column.data_type = infer_type(&column.samples);
        }
        Self::new(name, columns)
    }

    /// Read UTF-8 CSV with a header, quoting, and consistent row widths.
    pub fn from_csv(name: impl Into<String>, reader: impl Read) -> Result<Self, Error> {
        let mut csv = csv::Reader::from_reader(reader);
        let headers = csv.headers()?.iter().map(str::to_owned).collect();
        let rows: Result<Vec<_>, csv::Error> = csv
            .records()
            .map(|r| {
                r.map(|r| {
                    r.iter()
                        .map(|v| (!v.is_empty()).then(|| v.to_owned()))
                        .collect()
                })
            })
            .collect();
        Self::from_rows(name, headers, rows?)
    }

    /// Read an array of JSON objects. Missing keys and null values are omitted.
    /// Object keys are sorted for deterministic column order; nested values are rejected.
    pub fn from_json(name: impl Into<String>, reader: impl Read) -> Result<Self, Error> {
        let records: Vec<serde_json::Map<String, serde_json::Value>> =
            serde_json::from_reader(reader)?;
        let headers: Vec<_> = records
            .iter()
            .flat_map(|r| r.keys().cloned())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let mut rows = Vec::new();
        for record in records {
            let mut row = Vec::new();
            for header in &headers {
                let value = match record.get(header) {
                    None | Some(serde_json::Value::Null) => None,
                    Some(serde_json::Value::String(s)) => Some(s.clone()),
                    Some(serde_json::Value::Bool(b)) => Some(b.to_string()),
                    Some(serde_json::Value::Number(n)) => Some(n.to_string()),
                    _ => {
                        return Err(Error::InvalidInput(format!(
                            "nested JSON value in column {header}"
                        )));
                    }
                };
                row.push(value);
            }
            rows.push(row);
        }
        Self::from_rows(name, headers, rows)
    }

    pub(crate) fn sampled(&self, limit: Option<usize>) -> Self {
        let mut table = self.clone();
        for col in &mut table.columns {
            col.samples.retain(|v| !v.is_empty());
            if let Some(limit) = limit.filter(|n| *n < col.samples.len()) {
                let length = col.samples.len();
                col.samples = (0..limit)
                    .map(|i| col.samples[i * length / limit].clone())
                    .collect();
            }
        }
        table
    }
}

fn infer_type(values: &[String]) -> DataType {
    if values.is_empty() {
        return DataType::Unknown;
    }
    if values
        .iter()
        .all(|v| matches!(v.trim().to_ascii_lowercase().as_str(), "true" | "false"))
    {
        return DataType::Boolean;
    }
    if values.iter().all(|v| {
        let value = v.trim();
        value.parse::<i64>().is_ok() || value.parse::<u64>().is_ok()
    }) {
        return DataType::Integer;
    }
    if values
        .iter()
        .all(|v| v.trim().parse::<f64>().is_ok_and(f64::is_finite))
    {
        return DataType::Float;
    }
    DataType::Text
}
