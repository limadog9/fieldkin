use fieldkin::{CandidateIssue, Config, ContextualReason, DataType, Field, MatchEngine, MatchReport, SampleValue, Schema};
fn observed(id: &str, name: &str, values: &[&str]) -> Field {
    Field::new(id, name, DataType::Text).with_samples(values.iter().map(|value| SampleValue::Text((*value).to_owned())).collect())
}
fn public_quality(one_to_one: bool) -> Result<MatchReport, Box<dyn std::error::Error>> {
    let source = Schema::new(vec![observed("source", "customer_id", &["PACKAGE_PRIVATE_10", "PACKAGE_PRIVATE_20", "PACKAGE_PRIVATE_30"])]);
    let target = Schema::new(vec![
        observed("customer", "customer_key", &["PACKAGE_OTHER_40", "PACKAGE_OTHER_50", "PACKAGE_OTHER_60"]),
        observed("supplier", "supplier_id", &["PACKAGE_PRIVATE_10", "PACKAGE_PRIVATE_20", "PACKAGE_PRIVATE_30"]),
    ]);
    let mut config = Config::contextual_quality(); config.one_to_one = one_to_one;
    let engine = MatchEngine::new(config)?;
    let report = engine.match_schemas(&source, &target)?;
    let selected = report.fields[0].selected.as_ref().expect("informative independent populations");
    assert_eq!(selected.target.0, "customer");
    assert!(selected.issues.contains(&CandidateIssue::ContextualReason(ContextualReason::SupportedEquivalence)));
    let supplier = report.fields[0].candidates.iter().find(|candidate| candidate.target.0 == "supplier").unwrap();
    assert!(!supplier.eligible);
    assert!(supplier.issues.contains(&CandidateIssue::ContextualReason(ContextualReason::Contradiction)));
    let no_samples = engine.match_schemas(
        &Schema::new(vec![Field::new("s", "customer_id", DataType::Unknown)]),
        &Schema::new(vec![Field::new("t", "customer_key", DataType::Unknown)]),
    )?;
    assert_eq!(no_samples.fields[0].selected.as_ref().unwrap().target.0, "t");
    let opaque = engine.match_schemas(
        &source,
        &Schema::new(vec![
            observed("customer", "customer_key", &["PACKAGE_PRIVATE_10", "PACKAGE_PRIVATE_20", "PACKAGE_PRIVATE_30"]),
            observed("opaque", "opaque_code", &["PACKAGE_PRIVATE_10", "PACKAGE_PRIVATE_20", "PACKAGE_PRIVATE_30"]),
        ]),
    )?;
    assert!(opaque.fields[0].selected.is_none(), "opaque competitors must survive support failure");
    Ok(report)
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let report = MatchEngine::new(Config::default())?.match_schemas(
        &Schema::new(vec![Field::new("s", "TransDate", DataType::Date)]),
        &Schema::new(vec![Field::new("t", "transaction_date", DataType::Date)]),
    )?;
    let selected = report.fields[0].selected.as_ref().unwrap();
    assert_eq!(selected.target.0, "t");
    assert!((selected.score - 0.85).abs() < 1e-12);
    for one_to_one in [false, true] {
        let quality_report = public_quality(one_to_one)?;
        
        let _ = quality_report;
    }
    println!("PASS default compatibility; public contextual quality independent populations and missing samples; customer/supplier contradiction; opaque alternatives; both assignments");
    Ok(())
}
