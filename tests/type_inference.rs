use fieldkin::algorithms::cupid::Cupid;
use fieldkin::{ColumnPair, DataType, Field, Matcher, Table};

const SIGNED: [&str; 4] = ["-9223372036854775808", "-1", "0", "9223372036854775807"];
const UNSIGNED: [&str; 4] = [
    "0",
    "9223372036854775807",
    "9223372036854775808",
    "18446744073709551615",
];

fn integer_inputs(name: &str) -> [Table; 2] {
    [
        Table::from_csv(
            name,
            b"signed,unsigned\n-9223372036854775808,0\n-1,9223372036854775807\n0,9223372036854775808\n9223372036854775807,18446744073709551615\n".as_slice(),
        )
        .unwrap(),
        Table::from_json(
            name,
            br#"[
                {"signed":-9223372036854775808,"unsigned":0},
                {"signed":-1,"unsigned":9223372036854775807},
                {"signed":0,"unsigned":9223372036854775808},
                {"signed":9223372036854775807,"unsigned":18446744073709551615}
            ]"#
            .as_slice(),
        )
        .unwrap(),
    ]
}

fn assert_integer_samples(table: &Table) {
    assert_eq!(table.columns[0].samples, SIGNED);
    assert_eq!(table.columns[1].samples, UNSIGNED);
    assert_eq!(table.columns[0].data_type, DataType::Integer);
    assert_eq!(table.columns[1].data_type, DataType::Integer);
}

#[test]
fn csv_recognizes_signed_and_unsigned_64_bit_integer_ranges() {
    assert_integer_samples(&integer_inputs("values")[0]);
}

#[test]
fn json_recognizes_signed_and_unsigned_64_bit_integer_ranges() {
    assert_integer_samples(&integer_inputs("values")[1]);
}

#[cfg(feature = "polars")]
fn polars_integer_input(name: &str) -> Table {
    use polars_core::prelude::*;
    let frame = df!(
        "signed" => [i64::MIN, -1, 0, i64::MAX],
        "unsigned" => [0u64, 9_223_372_036_854_775_807, 9_223_372_036_854_775_808, u64::MAX]
    )
    .unwrap();
    Table::from_polars(name, &frame).unwrap()
}

#[cfg(feature = "polars")]
#[test]
fn polars_csv_and_json_agree_on_integer_types_and_exact_samples() {
    let polars = polars_integer_input("values");
    assert_integer_samples(&polars);
    for inferred in integer_inputs("values") {
        assert_eq!(inferred, polars);
    }
}

#[test]
fn cupid_matches_equivalent_integer_columns_from_every_input_format() {
    let reference = Table::new(
        "source",
        [SIGNED, UNSIGNED]
            .into_iter()
            .zip(["signed", "unsigned"])
            .map(|(samples, name)| Field {
                name: name.into(),
                data_type: DataType::Integer,
                samples: samples.into_iter().map(str::to_owned).collect(),
            })
            .collect(),
    )
    .unwrap();
    let target = Table::new("target", reference.columns.clone()).unwrap();
    let matcher = Cupid::default();
    let expected = matcher.get_matches(&reference, &target).unwrap();
    for column in ["signed", "unsigned"] {
        assert!(
            expected
                .get(&ColumnPair::new("source", column, "target", column))
                .is_some(),
            "the declared integer baseline must match {column}"
        );
    }
    for input in integer_inputs("source") {
        assert_eq!(matcher.get_matches(&input, &target).unwrap(), expected);
    }
    #[cfg(feature = "polars")]
    assert_eq!(
        matcher
            .get_matches(&polars_integer_input("source"), &target)
            .unwrap(),
        expected
    );
}

#[test]
fn inference_preserves_numeric_syntax_overflow_and_original_text() {
    for (samples, expected) in [
        (
            &["-9223372036854775808", "9223372036854775807"][..],
            DataType::Integer,
        ),
        (&["-0", "+0", "0001", " -00042 "][..], DataType::Integer),
        (
            &[
                "-9223372036854775809",
                "18446744073709551616",
                "-18446744073709551615",
            ][..],
            DataType::Float,
        ),
        (
            &["1.0", "-2.5", ".5", "5.", "1e3", "-2E-4"][..],
            DataType::Float,
        ),
        (&["1", "1.5"][..], DataType::Float),
        (&["NaN", "inf", "-inf", "1e400"][..], DataType::Text),
        (
            &["--1", "+", "++1", "1_000", "0x10", "12 widgets"][..],
            DataType::Text,
        ),
        (&["TRUE", " false "][..], DataType::Boolean),
        (&["1", "true"][..], DataType::Text),
        (&[" "][..], DataType::Text),
        (
            &[
                " +00018446744073709551615 ",
                "00018446744073709551615",
                "-0000",
            ][..],
            DataType::Integer,
        ),
        (
            &["-9223372036854775808", "18446744073709551615"][..],
            DataType::Integer,
        ),
        (&["18446744073709551615", "1.5"][..], DataType::Float),
    ] {
        let csv = format!("value\n{}\n", samples.join("\n"));
        // JSON strings preserve lexical forms (such as leading zeros) that JSON
        // number tokens cannot represent. Bare JSON integers are tested above.
        let json = serde_json::to_vec(
            &samples
                .iter()
                .map(|value| serde_json::json!({"value": value}))
                .collect::<Vec<_>>(),
        )
        .unwrap();
        for table in [
            Table::from_csv("values", csv.as_bytes()).unwrap(),
            Table::from_json("values", json.as_slice()).unwrap(),
        ] {
            assert_eq!(table.columns[0].samples, samples);
            assert_eq!(table.columns[0].data_type, expected, "{samples:?}");
        }
    }
}

#[test]
fn bare_json_float_syntax_and_negative_zero_keep_existing_behavior() {
    // serde_json retains a bare -0 as floating-point negative zero. Inference
    // preserves that distinction instead of normalizing its sample to "0".
    for (token, sample) in [
        ("1.0", "1.0"),
        ("1e3", "1000.0"),
        ("1.5", "1.5"),
        ("-0", "-0.0"),
        ("-0.0", "-0.0"),
    ] {
        let json = format!("[{{\"value\":{token}}}]");
        let table = Table::from_json("values", json.as_bytes()).unwrap();
        assert_eq!(table.columns[0].data_type, DataType::Float);
        assert_eq!(table.columns[0].samples, [sample]);
    }
    for token in ["18446744073709551616", "-9223372036854775809"] {
        let json = format!("[{{\"value\":{token}}}]");
        let table = Table::from_json("values", json.as_bytes()).unwrap();
        assert_eq!(table.columns[0].data_type, DataType::Float);
    }
    assert!(Table::from_json("values", br#"[{"value":01}]"#.as_slice()).is_err());
}

#[test]
fn missing_boolean_and_text_values_keep_existing_behavior() {
    let csv = Table::from_csv(
        "values",
        b"empty,flag,integer,text\n, TRUE ,42,hello\n,false,,123abc\n".as_slice(),
    )
    .unwrap();
    let json = Table::from_json(
        "values",
        br#"[{"empty":null,"flag":" TRUE ","integer":42,"text":"hello"},{"flag":false,"text":"123abc"}]"#.as_slice(),
    )
    .unwrap();
    assert_eq!(csv, json);
    assert_eq!(csv.columns[0].data_type, DataType::Unknown);
    assert!(csv.columns[0].samples.is_empty());
    assert_eq!(csv.columns[1].data_type, DataType::Boolean);
    assert_eq!(csv.columns[1].samples, [" TRUE ", "false"]);
    assert_eq!(csv.columns[2].data_type, DataType::Integer);
    assert_eq!(csv.columns[2].samples, ["42"]);
    assert_eq!(csv.columns[3].data_type, DataType::Text);
    assert_eq!(csv.columns[3].samples, ["hello", "123abc"]);
}
