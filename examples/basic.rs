//! Inspect proposals and abstentions for the motivating flat-schema example.
use fieldkin::{Config, DataType, Field, MatchEngine, Schema};

fn main() -> Result<(), fieldkin::MatchError> {
    let source = Schema::new(vec![
        Field::new("sku", "SKU_Code", DataType::Text),
        Field::new("gross", "GrossAmt", DataType::Decimal),
        Field::new("date", "TransDate", DataType::Date),
    ]);
    let target = Schema::new(vec![
        Field::new("product", "product_id", DataType::Text),
        Field::new("amount", "amount", DataType::Decimal),
        Field::new("date", "transaction_date", DataType::Date),
    ]);
    let report = MatchEngine::new(Config::default())?.match_schemas(&source, &target)?;
    println!("{report:#?}");
    Ok(())
}
