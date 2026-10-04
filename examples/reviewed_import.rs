//! Recompute remaining proposals after an application records human review.
use fieldkin::DataType::{Date, Decimal, Integer, Text, Unknown};
use fieldkin::{
    Config, Decision, Field, FieldPair, GlobalDiagnosticsConfig, MatchConstraints, MatchEngine,
    Schema,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = Schema::new(vec![
        Field::new("a-total", "amount", Decimal),
        Field::new("b-net", "amount", Decimal),
        Field::new("c-date", "TransDate", Date),
        Field::new("d-note", "freeform_note", Text),
        Field::new("e-legacy", "", Unknown),
    ]);
    let target = Schema::new(vec![
        Field::new("gross", "amount", Decimal),
        Field::new("net", "amount", Decimal),
        Field::new("date", "transaction_date", Date),
        Field::new("product", "product_id", Integer),
    ]);
    let engine = MatchEngine::new(Config {
        one_to_one: true,
        global_diagnostics: GlobalDiagnosticsConfig {
            max_solves: 8,
            ..Default::default()
        },
        ..Config::default()
    })?;
    let initial = engine.match_schemas(&source, &target)?;
    assert_eq!(initial.fields[0].decision, Decision::Ambiguous);
    assert!(initial.fields[4].selected.is_none());

    // These decisions come from a simulated reviewer who knows the source
    // system. They are not inferred from a matcher score or evaluation labels.
    let review = MatchConstraints {
        confirmed: vec![
            FieldPair::new("a-total", "gross"),
            FieldPair::new("e-legacy", "product"),
        ],
        forbidden: vec![FieldPair::new("a-total", "net")],
        unmatched_sources: vec!["d-note".into()],
    };
    let revised = engine.match_schemas_with_constraints(&source, &target, &review)?;
    assert_eq!(revised.fields[0].decision, Decision::Confirmed);
    assert_eq!(revised.fields[1].decision, Decision::Proposed);
    assert_eq!(revised.fields[1].selected.as_ref().unwrap().target.0, "net");
    assert_eq!(revised.fields[3].decision, Decision::ExcludedByCaller);
    let confirmed = revised.fields[4].selected.as_ref().unwrap();
    assert_eq!(revised.fields[4].decision, Decision::Confirmed);
    assert_eq!(confirmed.score, 0.0);
    assert!(!confirmed.eligible);

    for field in &revised.fields {
        println!(
            "{}: {:?}, target={:?}, score={:?}",
            field.source,
            field.decision,
            field.selected.as_ref().map(|candidate| &candidate.target),
            field.selected.as_ref().map(|candidate| candidate.score),
        );
    }
    // Persist decisions together with application-owned schema versions. Only
    // Proposed decisions remain automatic suggestions; Confirmed is caller input.
    // No values are rewritten, and no review state is retained inside the engine.
    Ok(())
}
