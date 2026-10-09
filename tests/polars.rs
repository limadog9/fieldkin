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
