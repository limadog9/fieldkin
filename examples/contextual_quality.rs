//! Run the explicitly named policy on independently sampled identifier values.

use fieldkin::{Config, DataType, Field, MatchEngine, SampleValue, Schema};

fn identifier(id: &str, name: &str, values: &[i128]) -> Field {
    Field::new(id, name, DataType::Integer)
        .with_samples(values.iter().copied().map(SampleValue::Integer).collect())
}

fn main() -> Result<(), fieldkin::MatchError> {
    let source = Schema::new(vec![identifier("s", "customer_id", &[10, 20, 30])]);
    let target = Schema::new(vec![
        identifier("customer", "customer_key", &[40, 50, 60]),
        identifier("supplier", "supplier_id", &[10, 20, 30]),
    ]);
    for one_to_one in [false, true] {
        let config = Config {
            one_to_one,
            ..Config::contextual_quality()
        };
        let report = MatchEngine::new(config)?.match_schemas(&source, &target)?;
        let selected = report.fields[0]
            .selected
            .as_ref()
            .expect("informative compatible role");
        assert_eq!(selected.target.0, "customer");
        println!(
            "one_to_one={one_to_one}: {} -> {}; heuristic score {:.3}",
            report.fields[0].source.0, selected.target.0, selected.score
        );
    }
    Ok(())
}
