#![cfg(feature = "polars")]
use fieldkin::{DataType, Table};
use polars_core::prelude::*;

#[test]
fn typed_polars_columns_preserve_nulls_names_and_strings() {
    let frame = df!("id" => [Some(1i64),None,Some(3)], "name" => [Some("Alice"),Some("Bob"),None], "amount" => [1.0f64,f64::NAN,3.0], "active" => [true,false,true]).unwrap();
    let table = Table::from_polars("people", &frame).unwrap();
    assert_eq!(table.columns[0].data_type, DataType::Integer);
    assert_eq!(table.columns[0].samples, ["1", "3"]);
    assert_eq!(table.columns[1].samples, ["Alice", "Bob"]);
    assert_eq!(table.columns[2].samples, ["1.0", "3.0"]);
    assert_eq!(table.columns[3].data_type, DataType::Boolean);
}

#[test]
fn float_samples_round_trip_at_their_original_precision() {
    let precise = 1.2345678901234567_f64;
    let doubles = [
        precise,
        f64::from_bits(precise.to_bits() + 1),
        1.0000000000000002,
        f64::MIN_POSITIVE,
        f64::from_bits(1),
        f64::MAX,
        -f64::MAX,
        -0.0,
    ];
    let singles = [
        1.2345678f32,
        1.2345679,
        1.0000001,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
        f32::MAX,
        -f32::MAX,
        -0.0,
    ];
    let frame = df!("double" => doubles, "single" => singles).unwrap();
    let table = Table::from_polars("numbers", &frame).unwrap();
    assert_eq!(table.columns[0].samples.len(), doubles.len());
    assert_eq!(table.columns[1].samples.len(), singles.len());
    for (sample, expected) in table.columns[0].samples.iter().zip(doubles) {
        assert_eq!(
            sample.parse::<f64>().unwrap().to_bits(),
            expected.to_bits(),
            "f64 {expected:?} became {sample}"
        );
    }
    for (sample, expected) in table.columns[1].samples.iter().zip(singles) {
        assert_eq!(
            sample.parse::<f32>().unwrap().to_bits(),
            expected.to_bits(),
            "f32 {expected:?} became {sample}"
        );
    }
    assert_eq!(table.columns[1].samples[0], "1.2345678");
}

#[test]
fn integer_samples_preserve_large_values_exactly() {
    let frame = df!(
        "signed" => [i64::MIN, 9_007_199_254_740_993, i64::MAX],
        "unsigned" => [u64::MAX, 9_007_199_254_740_993, 0],
        "signed32" => [i32::MIN, 0, i32::MAX],
        "unsigned32" => [u32::MAX, 0, 1]
    )
    .unwrap();
    let table = Table::from_polars("integers", &frame).unwrap();
    assert_eq!(
        table.columns[0].samples,
        [
            "-9223372036854775808",
            "9007199254740993",
            "9223372036854775807"
        ]
    );
    assert_eq!(
        table.columns[1].samples,
        ["18446744073709551615", "9007199254740993", "0"]
    );
    assert_eq!(table.columns[2].samples, ["-2147483648", "0", "2147483647"]);
    assert_eq!(table.columns[3].samples, ["4294967295", "0", "1"]);
}

#[test]
fn display_settings_do_not_change_matching_samples() {
    // Polars formatting settings are process-wide. Run this test alone in a
    // child process so changing them cannot race with other conversion tests.
    const CHILD: &str = "FIELDKIN_POLARS_DISPLAY_TEST_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "display_settings_do_not_change_matching_samples",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    use polars_core::fmt::{
        FloatFmt, set_decimal_separator, set_float_fmt, set_float_precision,
        set_thousands_separator,
    };
    let mut frame = df!(
        "float" => [12.25f64, 42.0],
        "single" => [12.25f32, 42.0],
        "integer" => [i64::MAX, i64::MIN],
        "unsigned" => [u64::MAX, 9_007_199_254_740_993],
        "text" => ["12,25", " 42 "],
        "boolean" => [true, false]
    )
    .unwrap();
    frame
        .with_column(
            Series::new("date".into(), [-1i32, 18321])
                .cast(&polars_core::prelude::DataType::Date)
                .unwrap()
                .into(),
        )
        .unwrap();
    frame
        .with_column(
            Series::new("timestamp".into(), [-1i64, 123456789])
                .cast(&polars_core::prelude::DataType::Datetime(
                    TimeUnit::Nanoseconds,
                    None,
                ))
                .unwrap()
                .into(),
        )
        .unwrap();
    let baseline = Table::from_polars("numbers", &frame).unwrap();
    let mut converted = Vec::new();
    for (format, precision, decimal, thousands) in [
        (FloatFmt::Full, None, None, None),
        (FloatFmt::Mixed, Some(1), None, Some(',')),
        (FloatFmt::Full, Some(2), Some(','), Some('_')),
    ] {
        set_float_fmt(format);
        set_float_precision(precision);
        set_decimal_separator(decimal);
        set_thousands_separator(thousands);
        converted.push(Table::from_polars("numbers", &frame).unwrap());
    }
    assert_eq!(converted, vec![baseline; 3]);
}

#[test]
fn strings_missing_values_and_infinities_keep_their_existing_semantics() {
    let strings = [
        Some("  padded  "),
        Some("\"quoted\""),
        Some(""),
        Some("東京🙂"),
        None,
    ];
    let frame = df!(
        "text" => strings,
        "double" => [Some(f64::NAN), None, Some(f64::INFINITY), Some(f64::NEG_INFINITY), Some(-0.0)],
        "single" => [Some(f32::NAN), None, Some(f32::INFINITY), Some(f32::NEG_INFINITY), Some(-0.0)]
    )
    .unwrap();
    let table = Table::from_polars("values", &frame).unwrap();
    assert_eq!(
        table.columns[0].samples,
        ["  padded  ", "\"quoted\"", "", "東京🙂"]
    );
    assert_eq!(table.columns[1].samples, ["inf", "-inf", "-0.0"]);
    assert_eq!(table.columns[2].samples, ["inf", "-inf", "-0.0"]);

    let nulls = DataFrame::new(
        3,
        vec![Column::full_null(
            "nulls".into(),
            3,
            &polars_core::prelude::DataType::Null,
        )],
    )
    .unwrap();
    let table = Table::from_polars("nulls", &nulls).unwrap();
    assert_eq!(table.columns[0].data_type, DataType::Unknown);
    assert!(table.columns[0].samples.is_empty());
}

#[test]
fn dates_and_timestamps_preserve_calendar_values_and_subseconds() {
    let dates = Series::new("date".into(), [Some(-1i32), Some(0), Some(18321), None])
        .cast(&polars_core::prelude::DataType::Date)
        .unwrap();
    let dates = DataFrame::new(4, vec![dates.into()]).unwrap();
    let table = Table::from_polars("dates", &dates).unwrap();
    assert_eq!(table.columns[0].data_type, DataType::Date);
    assert_eq!(
        table.columns[0].samples,
        ["1969-12-31", "1970-01-01", "2020-02-29"]
    );

    for (unit, positive, expected) in [
        (
            TimeUnit::Milliseconds,
            123i64,
            ["1969-12-31 23:59:59.999", "1970-01-01 00:00:00.123"],
        ),
        (
            TimeUnit::Microseconds,
            123456,
            ["1969-12-31 23:59:59.999999", "1970-01-01 00:00:00.123456"],
        ),
        (
            TimeUnit::Nanoseconds,
            123456789,
            [
                "1969-12-31 23:59:59.999999999",
                "1970-01-01 00:00:00.123456789",
            ],
        ),
    ] {
        let timestamps = Series::new("timestamp".into(), [Some(-1i64), Some(positive), None])
            .cast(&polars_core::prelude::DataType::Datetime(unit, None))
            .unwrap();
        let frame = DataFrame::new(3, vec![timestamps.into()]).unwrap();
        let table = Table::from_polars("timestamps", &frame).unwrap();
        assert_eq!(table.columns[0].data_type, DataType::Timestamp);
        assert_eq!(table.columns[0].samples, expected);
    }
}

#[test]
fn timezone_aware_timestamps_preserve_instants_and_dst_offsets() {
    for (timezone, expected) in [
        (
            "UTC",
            [
                "2020-03-08 06:59:59.123456789 UTC",
                "2020-03-08 07:00:00 UTC",
            ],
        ),
        (
            "America/New_York",
            [
                "2020-03-08 01:59:59.123456789 EST",
                "2020-03-08 03:00:00 EDT",
            ],
        ),
    ] {
        let timezone = TimeZone::opt_try_new(Some(timezone)).unwrap();
        let timestamps = Series::new(
            "timestamp".into(),
            [
                Some(1_583_650_799_123_456_789i64),
                Some(1_583_650_800_000_000_000),
                None,
            ],
        )
        .i64()
        .unwrap()
        .clone()
        .into_datetime(TimeUnit::Nanoseconds, timezone)
        .into_series();
        let frame = DataFrame::new(3, vec![timestamps.into()]).unwrap();
        let table = Table::from_polars("timestamps", &frame).unwrap();
        assert_eq!(table.columns[0].data_type, DataType::Timestamp);
        assert_eq!(table.columns[0].samples, expected);
    }
}
