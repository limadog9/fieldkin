//! A catalog preserves unsupported fields and inspects global completeness.
use fieldkin::{
    AssignmentDiagnosticStatus, Config, DataType, Field, FieldId, GlobalDiagnosticsConfig,
    MatchEngine, Schema,
};
use std::collections::BTreeMap;

#[derive(Debug)]
enum CatalogDecision {
    AwaitingReview(Option<FieldId>),
    Accepted(FieldId),
    KeepUnmatched,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = Schema::new(vec![
        Field::new("billing-email", "email", DataType::Text),
        Field::new("contact-email", "email", DataType::Text),
        Field::new("local-note", "internal_note", DataType::Text),
    ]);
    let target = Schema::new(vec![Field::new("email", "email", DataType::Text)]);
    let report = MatchEngine::new(Config {
        one_to_one: true,
        global_diagnostics: GlobalDiagnosticsConfig {
            max_solves: 8,
            ..Default::default()
        },
        ..Default::default()
    })?
    .match_schemas(&source, &target)?;
    assert_eq!(
        report.assignment_diagnostics.status,
        AssignmentDiagnosticStatus::Complete
    );
    assert!(!report.assignment_diagnostics.alternatives.is_empty());
    // A deterministic selected ID is not evidence of a unique mapping. The
    // alternative witnesses and field reasons go to the application's reviewer.
    for field in &report.fields {
        println!("{}: {:?}", field.source, field.diagnostics);
    }
    let mut decisions: BTreeMap<_, _> = report
        .fields
        .iter()
        .map(|field| {
            (
                field.source.clone(),
                CatalogDecision::AwaitingReview(
                    field
                        .selected
                        .as_ref()
                        .map(|candidate| candidate.target.clone()),
                ),
            )
        })
        .collect();
    // Simulated catalog curator decision: contact-email owns the target, even
    // if stable ID tie-breaking initially selected billing-email.
    decisions.insert(
        "contact-email".into(),
        CatalogDecision::Accepted("email".into()),
    );
    decisions.insert("billing-email".into(), CatalogDecision::KeepUnmatched);
    decisions.insert("local-note".into(), CatalogDecision::KeepUnmatched);
    let accepted: Vec<_> = decisions
        .iter()
        .filter_map(|(source, decision)| match decision {
            CatalogDecision::Accepted(target) => Some((source, target)),
            CatalogDecision::AwaitingReview(proposal) => {
                println!("Pending {source}: {proposal:?}");
                None
            }
            CatalogDecision::KeepUnmatched => None,
        })
        .collect();
    assert_eq!(accepted.len(), 1);
    println!(
        "Confirmed ID pairs: {accepted:?}; all {} source entries retained",
        decisions.len()
    );
    // Decisions remain application-owned, and can be stored with the schemas'
    // versions. This example neither mutates samples nor executes a migration.
    Ok(())
}
