//! An importer owns review decisions; Fieldkin only proposes correspondences.
use std::collections::BTreeMap;

use fieldkin::{Config, DataType, Field, FieldId, MatchEngine, MatchReport, Schema};

#[derive(Debug, PartialEq)]
enum ReviewState {
    Pending(Vec<FieldId>),
    Confirmed(FieldId),
    Unmatched,
}

fn review_queue(report: &MatchReport) -> BTreeMap<FieldId, ReviewState> {
    report
        .fields
        .iter()
        .map(|field| {
            // Even the engine's selected proposal still needs application review.
            let eligible = field
                .candidates
                .iter()
                .filter(|c| c.eligible)
                .map(|c| c.target.clone())
                .collect::<Vec<_>>();
            let state = if eligible.is_empty() {
                ReviewState::Unmatched
            } else {
                ReviewState::Pending(eligible)
            };
            (field.source.clone(), state)
        })
        .collect()
}

fn confirm(
    queue: &mut BTreeMap<FieldId, ReviewState>,
    source: &FieldId,
    target: &FieldId,
) -> Result<(), &'static str> {
    match queue.get_mut(source) {
        Some(ReviewState::Pending(candidates)) => {
            if !candidates.contains(target) {
                return Err("target was not offered for review");
            }
            queue.insert(source.clone(), ReviewState::Confirmed(target.clone()));
            Ok(())
        }
        _ => Err("source is not awaiting review"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = Schema::new(vec![
        Field::new("src-date", "TransDate", DataType::Date),
        Field::new("src-amount", "amount", DataType::Decimal),
        Field::new("src-note", "freeform_note", DataType::Text),
    ]);
    let target = Schema::new(vec![
        Field::new("date", "transaction_date", DataType::Date),
        Field::new("gross", "amount", DataType::Decimal),
        Field::new("net", "amount", DataType::Decimal),
    ]);
    let report = MatchEngine::new(Config::default())?.match_schemas(&source, &target)?;
    let mut queue = review_queue(&report);
    assert_eq!(queue[&FieldId::from("src-note")], ReviewState::Unmatched);
    assert!(report
        .fields
        .iter()
        .find(|f| f.source.0 == "src-amount")
        .is_some_and(|f| f.selected.is_none() && f.alternatives.len() == 2));
    // Synthetic user feedback, based on knowledge outside the matcher.
    confirm(&mut queue, &"src-amount".into(), &"gross".into())?;
    confirm(&mut queue, &"src-date".into(), &"date".into())?;
    assert!(confirm(&mut queue, &"src-note".into(), &"net".into()).is_err());
    println!("Application-owned review state: {queue:?}");
    // The importer would consume confirmed IDs; this example rewrites no data.
    Ok(())
}
