use crate::{DataType, Error, Field, Table};
use polars_core::prelude::{AnyValue, DataFrame, DataType as PolarsType};

impl Table {
    /// Copy scalar columns from a native Polars frame, preserving declared types
    /// and omitting null/NaN values. Lists, structs, binary and object columns
    /// are rejected rather than stringified into misleading instance values.
    pub fn from_polars(name: impl Into<String>, frame: &DataFrame) -> Result<Self, Error> {
        let mut columns = Vec::with_capacity(frame.width());
        for column in frame.columns() {
            let data_type = match column.dtype() {
                PolarsType::Boolean => DataType::Boolean,
                dtype if dtype.is_integer() => DataType::Integer,
                PolarsType::Float32 | PolarsType::Float64 => DataType::Float,
                PolarsType::Date => DataType::Date,
                PolarsType::Datetime(_, _) => DataType::Timestamp,
                PolarsType::String => DataType::Text,
                PolarsType::Null => DataType::Unknown,
                dtype => {
                    return Err(Error::InvalidInput(format!(
                        "unsupported Polars type {dtype} in {}",
                        column.name()
                    )));
                }
            };
            let mut samples = Vec::new();
            for index in 0..frame.height() {
                let value = column
                    .get(index)
                    .map_err(|e| Error::InvalidInput(e.to_string()))?;
                let sample = match value {
                    AnyValue::Null => continue,
                    AnyValue::Float32(f) if f.is_nan() => continue,
                    AnyValue::Float64(f) if f.is_nan() => continue,
                    AnyValue::String(s) => s.to_owned(),
                    AnyValue::StringOwned(s) => s.to_string(),
                    value => value.to_string(),
                };
                samples.push(sample);
            }
            columns.push(Field {
                name: column.name().to_string(),
                data_type,
                samples,
            });
        }
        Self::new(name, columns)
    }
}
