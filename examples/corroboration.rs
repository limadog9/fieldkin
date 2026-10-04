//! Require sampled support while retaining unsupported candidates for review.
use fieldkin::{
    CandidateIssue, Config, Corroboration, DataType, Field, MatchEngine, SampleValue, Schema,
};

fn dates() -> Vec<SampleValue> {
    ["2026-08-05", "2026-08-11", "2026-08-23"]
        .into_iter()
        .map(|value| SampleValue::Text(value.into()))
        .collect()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = Schema::new(vec![
        Field::new("date", "TransDate", DataType::Date).with_samples(dates()),
        Field::new("unverified-amount", "amount", DataType::Decimal),
    ]);
    let target = Schema::new(vec![
        Field::new("transaction-date", "transaction_date", DataType::Date).with_samples(dates()),
        Field::new("amount", "amount", DataType::Decimal),
    ]);
    let weighted = MatchEngine::new(Config::default())?.match_schemas(&source, &target)?;
    let corroborated = MatchEngine::new(Config {
        corroboration: Some(Corroboration::default()),
        ..Config::default()
    })?
    .match_schemas(&source, &target)?;
    let date = &corroborated.fields[0];
    assert_eq!(
        date.selected
            .as_ref()
            .map(|candidate| candidate.target.0.as_str()),
        Some("transaction-date")
    );
    let amount = &corroborated.fields[1];
    assert!(weighted.fields[1].selected.is_some());
    assert!(amount.selected.is_none());
    assert_eq!(amount.candidates[0].target.0, "amount");
    assert_eq!(
        amount.candidates[0].score,
        weighted.fields[1].candidates[0].score
    );
    assert!(amount.candidates[0]
        .issues
        .contains(&CandidateIssue::InsufficientSampleSupport));
    for field in &corroborated.fields {
        println!("{}: {:?}", field.source, field.decision);
    }
    println!("Unsupported amount retained for review; no data rewritten.");
    // Matching names and representative values still cannot prove shared meaning.
    // A real consumer must verify what the date and amount represent.
    Ok(())
}
