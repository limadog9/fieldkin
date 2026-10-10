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

// Keep the previous row-matrix conversion as an equivalence oracle.
fn json_via_rows(name: &str, input: &str) -> Result<Table, Error> {
    let records: Vec<serde_json::Map<String, serde_json::Value>> =
        serde_json::from_reader(input.as_bytes())?;
    let headers: Vec<_> = records
        .iter()
        .flat_map(|record| record.keys().cloned())
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut rows = Vec::new();
    for record in records {
        let mut row = Vec::new();
        for header in &headers {
            row.push(match record.get(header) {
                None | Some(serde_json::Value::Null) => None,
                Some(serde_json::Value::String(value)) => Some(value.clone()),
                Some(serde_json::Value::Bool(value)) => Some(value.to_string()),
                Some(serde_json::Value::Number(value)) => Some(value.to_string()),
                _ => {
                    return Err(Error::InvalidInput(format!(
                        "nested JSON value in column {header}"
                    )));
                }
            });
        }
        rows.push(row);
    }
    Table::from_rows(name, headers, rows)
}

#[test]
fn json_matches_previous_conversion_including_order_types_and_missing_values() {
    for input in [
        "[]",
        "[{}, {}]",
        r#"[{"z":null,"a":""},{"b":"  ","a":"東京🙂\n\"quoted\""},{"late":"new"}]"#,
        r#"[{"id":18446744073709551615,"signed":-9223372036854775808,"float":1e3,"flag":true},{"id":"00042","signed":" -7 ","float":-0.0,"flag":false}]"#,
        r#"[{"number":1,"number":2},{"number":1.25},{"number":"text"}]"#,
        r#"[{"blank":null},{"blank":""},{"text":"null","boolean":" FALSE "}]"#,
    ] {
        assert_eq!(
            Table::from_json("json", input.as_bytes()).unwrap(),
            json_via_rows("json", input).unwrap(),
            "{input}"
        );
    }
    let mut rows = vec![serde_json::json!({"number": 1}); 1001];
    rows.push(serde_json::json!({"number": 1.25, "late": "last"}));
    let input = serde_json::to_string(&rows).unwrap();
    let actual = Table::from_json("json", input.as_bytes()).unwrap();
    assert_eq!(actual, json_via_rows("json", &input).unwrap());
    assert_eq!(actual.columns[1].data_type, DataType::Float);
    assert_eq!(actual.columns[1].samples.len(), 1002);
}

#[test]
fn json_preserves_errors_and_validation_precedence() {
    for name in ["json", " "] {
        for input in [
            "",
            "{}",
            "[null]",
            "[1]",
            "[{\"a\": 1}] trailing",
            r#"[{"z":{},"a":[]}]"#,
            r#"[{"z":[]},{"a":{}}]"#,
            r#"[{" ":1}]"#,
            r#"[{" ":1,"z":{}}]"#,
        ] {
            let actual = Table::from_json(name, input.as_bytes()).unwrap_err();
            let expected = json_via_rows(name, input).unwrap_err();
            match (actual, expected) {
                (Error::Json(actual), Error::Json(expected)) => {
                    assert_eq!(actual.to_string(), expected.to_string());
                }
                (Error::InvalidInput(actual), Error::InvalidInput(expected)) => {
                    assert_eq!(actual, expected);
                }
                (actual, expected) => panic!("{input}: {actual:?} != {expected:?}"),
            }
        }
    }
}
