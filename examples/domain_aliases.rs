//! Caller-verified aliases can support domain-specific renames.
use fieldkin::{
    Config, DataType, Field, MatchEngine, NameMatcher, SampleMatcher, Schema, TypeMatcher,
    WeightedMatcher,
};

fn main() -> Result<(), fieldkin::MatchError> {
    // The application owner has verified these synonyms for this particular schema pair.
    // Aliases apply to every occurrence of these tokens, so keep them domain-specific.
    let mut names = NameMatcher::default();
    names.aliases.insert("sku".into(), "product".into());
    names.aliases.insert("code".into(), "id".into());
    let engine = MatchEngine::with_matchers(
        Config::default(),
        vec![
            WeightedMatcher::new(0.65, names),
            WeightedMatcher::new(0.20, TypeMatcher),
            WeightedMatcher::new(0.15, SampleMatcher::default()),
        ],
    )?;
    let source = Schema::new(vec![Field::new("sku", "SKU_Code", DataType::Text)]);
    let target = Schema::new(vec![Field::new("product", "product_id", DataType::Text)]);
    let report = engine.match_schemas(&source, &target)?;
    assert_eq!(
        report.fields[0]
            .selected
            .as_ref()
            .map(|c| c.target.0.as_str()),
        Some("product")
    );
    println!("{report:#?}");
    Ok(())
}
