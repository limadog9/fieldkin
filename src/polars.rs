use crate::{DataType, Error, Field, Table};
use polars_core::prelude::{AnyValue, DataFrame, DataType as PolarsType, TimeUnit};

// Polars 0.55 formats dates through Chrono, whose supported calendar spans
// -262143-01-01 through +262142-12-31. These are its inclusive Unix epoch days.
// Polars' temporal Display panics outside this range, even for valid storage types.
const TEMPORAL_DAYS: std::ops::RangeInclusive<i64> = -96_465_292..=95_026_236;

impl Table {
    /// Copy scalar columns from a native Polars frame, preserving declared types
    /// and omitting null/NaN values. Lists, structs, binary and object columns
    /// are rejected rather than stringified into misleading instance values.
    /// Numeric samples preserve scalar precision and ignore Polars display settings.
    /// Temporal values outside Polars' supported calendar range and invalid
    /// timezones return an error.
    pub fn from_polars(name: impl Into<String>, frame: &DataFrame) -> Result<Self, Error> {
        let mut columns = Vec::with_capacity(frame.width());
        for column in frame.columns() {
            let data_type = match column.dtype() {
                PolarsType::Boolean => DataType::Boolean,
                dtype if dtype.is_integer() => DataType::Integer,
                PolarsType::Float32 | PolarsType::Float64 => DataType::Float,
                PolarsType::Date => DataType::Date,
                PolarsType::Datetime(_, timezone) => {
                    if let Some(timezone) = timezone {
                        timezone.to_chrono().map_err(|error| {
                            Error::InvalidInput(format!(
                                "invalid Polars timezone {timezone} in column {}: {error}",
                                column.name()
                            ))
                        })?;
                    }
                    DataType::Timestamp
                }
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
                    // Primitive Debug uses round-trip formatting at the original
                    // float width and retains the `.0` on integral float values.
                    AnyValue::Float32(f) => format!("{f:?}"),
                    AnyValue::Float64(f) => format!("{f:?}"),
                    AnyValue::Boolean(v) => v.to_string(),
                    AnyValue::UInt8(v) => v.to_string(),
                    AnyValue::UInt16(v) => v.to_string(),
                    AnyValue::UInt32(v) => v.to_string(),
                    AnyValue::UInt64(v) => v.to_string(),
                    AnyValue::UInt128(v) => v.to_string(),
                    AnyValue::Int8(v) => v.to_string(),
                    AnyValue::Int16(v) => v.to_string(),
                    AnyValue::Int32(v) => v.to_string(),
                    AnyValue::Int64(v) => v.to_string(),
                    AnyValue::Int128(v) => v.to_string(),
                    AnyValue::String(s) => s.to_owned(),
                    AnyValue::StringOwned(s) => s.to_string(),
                    AnyValue::Date(days) => {
                        if !TEMPORAL_DAYS.contains(&i64::from(days)) {
                            return Err(Error::InvalidInput(format!(
                                "out-of-range Polars date {days} in column {} at row {index}",
                                column.name()
                            )));
                        }
                        AnyValue::Date(days).to_string()
                    }
                    value @ (AnyValue::Datetime(timestamp, unit, _)
                    | AnyValue::DatetimeOwned(timestamp, unit, _)) => {
                        let units_per_day = match unit {
                            TimeUnit::Milliseconds => 86_400_000,
                            TimeUnit::Microseconds => 86_400_000_000,
                            TimeUnit::Nanoseconds => 86_400_000_000_000,
                        };
                        if !TEMPORAL_DAYS.contains(&timestamp.div_euclid(units_per_day)) {
                            return Err(Error::InvalidInput(format!(
                                "out-of-range Polars timestamp {timestamp} ({unit:?}) in column {} at row {index}",
                                column.name()
                            )));
                        }
                        // Preserve calendar values, subseconds and timezones,
                        // independently of numeric display settings.
                        value.to_string()
                    }
                    value => {
                        return Err(Error::InvalidInput(format!(
                            "unsupported Polars type {} in {}",
                            value.dtype(),
                            column.name()
                        )));
                    }
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
