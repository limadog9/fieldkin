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
        use fieldkin::json::{report_to_json, review_from_json, review_to_json, JsonLimits, ReportJsonOptions, ReviewContext};
        use fieldkin::{Decision, FieldPair, MatchConstraints};
        let display_bytes = report_to_json(&quality_report, &ReportJsonOptions::default())?;
        let wire = String::from_utf8(display_bytes.clone())?;
        assert!(wire.contains("fieldkin.report"));
        assert!(wire.contains("\"code\":\"contextual_supported_equivalence\""));
        assert!(wire.contains("\"code\":\"contextual_contradiction\""));
        for private in ["PACKAGE_PRIVATE_10", "PACKAGE_PRIVATE_20", "PACKAGE_PRIVATE_30", "PACKAGE_OTHER_40", "PACKAGE_OTHER_50", "PACKAGE_OTHER_60"] { assert!(!wire.contains(private)); }
        assert!(!wire.contains("\"warnings\":"));
        assert!(!wire.contains("\"explanation\":"));
        assert!(!wire.contains("\"samples\":"));
        let context = ReviewContext { source_revision: "package-source-v1", target_revision: "package-target-v1" };
        let review = MatchConstraints { confirmed: vec![FieldPair::new("source", "customer")], forbidden: vec![FieldPair::new("source", "supplier")], ..Default::default() };
        let limits = JsonLimits::default();
        let bytes = review_to_json(&review, context, &limits)?;
        let resumed = review_from_json(&bytes, context, &limits)?;
        assert_eq!(resumed.confirmed, review.confirmed); assert_eq!(resumed.forbidden, review.forbidden);
        assert!(review_from_json(&bytes, ReviewContext { source_revision: "package-source-v2", ..context }, &limits).is_err());
        assert!(review_from_json(&display_bytes, context, &limits).is_err());
        let explicit = MatchEngine::new(Config::contextual_quality())?.match_schemas_with_constraints(
            &Schema::new(vec![Field::new("source", "customer_id", DataType::Unknown)]),
            &Schema::new(vec![Field::new("customer", "customer_key", DataType::Unknown), Field::new("supplier", "supplier_id", DataType::Unknown)]),
            &resumed,
        )?;
        assert_eq!(explicit.fields[0].decision, Decision::Confirmed);
        println!("PASS JSON contextual reason fidelity, omitted private values/text, explicit review round-trip, stale revision and display-as-review rejection");

        let _ = quality_report;
    }
    println!("PASS default compatibility; public contextual quality independent populations and missing samples; customer/supplier contradiction; opaque alternatives; both assignments");
    Ok(())
}
