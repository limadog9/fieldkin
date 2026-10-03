//! Exact values and caller-verified hints remain separate kinds of evidence.
use fieldkin::{
    Config, DataType, ExactDecimal, Field, MatchEngine, SampleValue, Schema, SemanticHints,
};

fn main() -> Result<(), fieldkin::MatchError> {
    let prices = [1250, 1800, 2175]
        .into_iter()
        .map(|coefficient| ExactDecimal::new(coefficient, 2).map(SampleValue::Decimal))
        .collect::<Result<Vec<_>, _>>()?;
    let source = Schema::new(vec![Field::new("source-price", "price", DataType::Decimal)
        .with_samples(prices.clone())
        .with_hints(SemanticHints {
            currency: Some("USD".into()),
            ..SemanticHints::default()
        })]);
    let target = Schema::new(vec![
        Field::new("euro-price", "price", DataType::Decimal)
            .with_samples(prices.clone())
            .with_hints(SemanticHints {
                currency: Some("EUR".into()),
                ..SemanticHints::default()
            }),
        Field::new("dollar-price", "price", DataType::Decimal)
            .with_samples(prices)
            .with_hints(SemanticHints {
                currency: Some("USD".into()),
                ..SemanticHints::default()
            }),
    ]);
    let report = MatchEngine::new(Config::default())?.match_schemas(&source, &target)?;
    assert_eq!(
        report.fields[0]
            .selected
            .as_ref()
            .map(|c| c.target.0.as_str()),
        Some("dollar-price")
    );
    let conflict = report.fields[0]
        .candidates
        .iter()
        .find(|c| c.target.0 == "euro-price")
        .unwrap();
    assert!(!conflict.eligible);
    // The report explains the conflict without printing prices or hint payloads.
    println!("{report:#?}");
    Ok(())
}
