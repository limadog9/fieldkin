use fieldkin::{match_schemas, Config, DataType, Decision, Field, Schema};

#[test]
fn exact_name_and_type_match() {
    let source = Schema {
        fields: vec![Field {
            name: "email".into(),
            data_type: DataType::Text,
            samples: vec![],
        }],
    };

    let target = Schema {
        fields: vec![Field {
            name: "email".into(),
            data_type: DataType::Text,
            samples: vec![],
        }],
    };

    let report = match_schemas(&source, &target, Config::default());

    assert!(matches!(
        &report.fields[0].decision,
        Decision::Match { target, .. } if target == "email"
    ));
}
