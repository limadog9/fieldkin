//! An application resumes explicit review after restarting; no files or values are rewritten.
use fieldkin::json::{
    report_to_json, review_from_json, review_to_json, JsonLimits, ReportJsonOptions, ReviewContext,
};
use fieldkin::{
    Config, DataType, Decision, Field, FieldPair, MatchConstraints, MatchEngine, Schema,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut source = Schema::new(vec![Field::new("total", "amount", DataType::Decimal)]);
    let mut target = Schema::new(vec![
        Field::new("gross", "amount", DataType::Decimal),
        Field::new("net", "amount", DataType::Decimal),
    ]);
    let engine = MatchEngine::new(Config {
        one_to_one: true,
        ..Config::default()
    })?;
    let initial = engine.match_schemas(&source, &target)?;
    assert_eq!(initial.fields[0].decision, Decision::Ambiguous);
    let display_bytes = report_to_json(&initial, &ReportJsonOptions::default())?;
    // The application supplies these revisions from trusted state, not from a
    // submitted document. It must advance them when prior review becomes stale.
    let context = ReviewContext {
        source_revision: "import-v4",
        target_revision: "catalog-v9",
    };
    let limits = JsonLimits::default();
    // A simulated domain reviewer explicitly confirms gross, outside the matcher.
    let review = MatchConstraints {
        confirmed: vec![FieldPair::new("total", "gross")],
        ..Default::default()
    };
    let saved_bytes = review_to_json(&review, context, &limits)?;
    // Simulate loading application-owned bytes after restarting and reordering.
    source.fields.reverse();
    target.fields.reverse();
    let resumed = review_from_json(&saved_bytes, context, &limits)?;
    let report = engine.match_schemas_with_constraints(&source, &target, &resumed)?;
    assert_eq!(report.fields[0].decision, Decision::Confirmed);
    let changed = ReviewContext {
        source_revision: "import-v5",
        ..context
    };
    assert!(review_from_json(&saved_bytes, changed, &limits).is_err());
    assert!(review_from_json(&display_bytes, context, &limits).is_err());
    println!("Exported {} display bytes and {} review bytes; resumed explicit confirmation; stale context rejected.", display_bytes.len(), saved_bytes.len());
    Ok(())
}
