use fieldkin::{DataType, Error, Field, Table};

fn field(name: &str, data_type: DataType, samples: &[&str]) -> Field {
    Field {
        name: name.into(),
        data_type,
        samples: samples.iter().map(|sample| (*sample).into()).collect(),
    }
}

#[test]
fn csv_preserves_quoted_fields_newlines_empty_values_and_formatting() {
    let input = concat!(
        "\u{feff}\"first,name\",memo,integer,number,flag,empty,space\r\n",
        "\"Smith, Jane\",\"line one\nline two\",00042,1e3,true,, \r\n",
        "\"Fox \"\"Red\"\"\",\"quote \"\"inside\"\"\",18446744073709551615,-0.0, FALSE ,\"\",  \r\n",
        ",東京🙂, -7 ,1.25,,, \t \r\n",
    );
    let actual = Table::from_csv("csv", input.as_bytes()).unwrap();
    let expected = Table::new(
        "csv",
        vec![
            field(
                "first,name",
                DataType::Text,
                &["Smith, Jane", "Fox \"Red\""],
            ),
            field(
                "memo",
                DataType::Text,
                &["line one\nline two", "quote \"inside\"", "東京🙂"],
            ),
            field(
                "integer",
                DataType::Integer,
                &["00042", "18446744073709551615", " -7 "],
            ),
            field("number", DataType::Float, &["1e3", "-0.0", "1.25"]),
            field("flag", DataType::Boolean, &["true", " FALSE "]),
            field("empty", DataType::Unknown, &[]),
            field("space", DataType::Text, &[" ", "  ", " \t "]),
        ],
    )
    .unwrap();
    assert_eq!(actual, expected);
}

#[test]
fn empty_and_header_only_csv_keep_empty_columns_and_unknown_types() {
    assert_eq!(
        Table::from_csv("csv", b"".as_slice()).unwrap(),
        Table::new("csv", vec![]).unwrap()
    );
    let expected = Table::new(
        "csv",
        vec![
            field("id", DataType::Unknown, &[]),
            field("note", DataType::Unknown, &[]),
        ],
    )
    .unwrap();
    for input in ["id,note\n", "id,note\n,\n\"\",\"\"\n"] {
        assert_eq!(Table::from_csv("csv", input.as_bytes()).unwrap(), expected);
    }
}

#[test]
fn csv_infers_types_using_values_beyond_the_default_sample_limit() {
    let mut input = String::from("number,flag\n");
    for i in 0..1001 {
        input.push_str(&format!("{i},true\n"));
    }
    input.push_str("1.25,neither\n");
    let table = Table::from_csv("csv", input.as_bytes()).unwrap();
    assert_eq!(table.columns[0].data_type, DataType::Float);
    assert_eq!(table.columns[1].data_type, DataType::Text);
    for column in &table.columns {
        assert_eq!(column.samples.len(), 1002);
    }
    assert_eq!(table.columns[0].samples[0], "0");
    assert_eq!(table.columns[0].samples[1000], "1000");
    assert_eq!(table.columns[0].samples[1001], "1.25");
    assert_eq!(table.columns[1].samples[1001], "neither");
}

#[test]
fn malformed_csv_preserves_width_utf8_errors_and_record_positions() {
    for (input, width) in [
        ("id,name\n1,Alice\n2\n", 1),
        ("id,name\n1,Alice\n2,Bob,extra\n", 3),
    ] {
        let Error::Csv(error) = Table::from_csv("csv", input.as_bytes()).unwrap_err() else {
            panic!("expected a CSV error");
        };
        assert!(matches!(
            error.kind(),
            csv::ErrorKind::UnequalLengths { expected_len: 2, len, .. } if *len == width
        ));
        assert_eq!(error.position().unwrap().record(), 2);
    }
    for input in [
        b"id,\xff\n1,Alice\n".as_slice(),
        b"id,name\n1,\xff\n".as_slice(),
    ] {
        let Error::Csv(error) = Table::from_csv("csv", input).unwrap_err() else {
            panic!("expected a CSV error");
        };
        assert!(matches!(error.kind(), csv::ErrorKind::Utf8 { .. }));
    }
}

#[test]
fn csv_keeps_table_validation_and_parse_error_precedence() {
    for (name, input, message) in [
        (" ", "id,name\n1,Alice\n", "table name must be nonempty"),
        ("csv", "id, \n1,Alice\n", "column name must be nonempty"),
        ("csv", "id,id\n1,2\n", "duplicate column name: id"),
    ] {
        assert!(matches!(
            Table::from_csv(name, input.as_bytes()),
            Err(Error::InvalidInput(actual)) if actual == message
        ));
    }
    // The previous CSV path parsed every record before validating table names.
    for (name, input) in [
        (" ", "id,name\n1\n"),
        ("csv", "id, \n1\n"),
        ("csv", "id,id\n1\n"),
    ] {
        assert!(matches!(
            Table::from_csv(name, input.as_bytes()),
            Err(Error::Csv(error)) if matches!(error.kind(), csv::ErrorKind::UnequalLengths { .. })
        ));
    }
}
