//! A small synthetic correctness comparison, not a representative accuracy claim.
//! Run with `cargo run --example baseline`.

use fieldkin::{
    Config, DataType, Field, MatchEngine, NameMatcher, SampleValue, Schema, WeightedMatcher,
};

struct Case {
    label: &'static str,
    source: Schema,
    target: Schema,
    expected: Option<&'static str>,
}

fn numbers() -> Vec<SampleValue> {
    [1.0, 2.0, 3.0]
        .into_iter()
        .map(SampleValue::Number)
        .collect()
}

fn cases() -> Vec<Case> {
    vec![
        Case {
            label: "case/separator rename",
            source: Schema::new(vec![Field::new("s", "CustomerID", DataType::Integer)]),
            target: Schema::new(vec![Field::new("right", "customer_id", DataType::Integer)]),
            expected: Some("right"),
        },
        Case {
            label: "documented abbreviation",
            source: Schema::new(vec![Field::new("s", "TransDate", DataType::Date)]),
            target: Schema::new(vec![Field::new(
                "right",
                "transaction_date",
                DataType::Date,
            )]),
            expected: Some("right"),
        },
        Case {
            label: "misleading exact name; type plus sample evidence",
            source: Schema::new(vec![
                Field::new("s", "account", DataType::Integer).with_samples(numbers())
            ]),
            target: Schema::new(vec![
                Field::new("wrong", "account", DataType::Boolean),
                Field::new("right", "account_id", DataType::Integer).with_samples(numbers()),
            ]),
            expected: Some("right"),
        },
        Case {
            label: "ambiguous duplicate names",
            source: Schema::new(vec![Field::new("s", "address", DataType::Text)]),
            target: Schema::new(vec![
                Field::new("home", "address", DataType::Text),
                Field::new("work", "address", DataType::Text),
            ]),
            expected: None,
        },
        Case {
            label: "unrelated schemas",
            source: Schema::new(vec![Field::new("s", "humidity", DataType::Float)]),
            target: Schema::new(vec![Field::new("unrelated", "document", DataType::Binary)]),
            expected: None,
        },
        Case {
            label: "opaque rename missed by both",
            source: Schema::new(vec![Field::new("s", "legacy_x7", DataType::Text)
                .with_samples(vec![
                    SampleValue::Text("US".into()),
                    SampleValue::Text("GB".into()),
                    SampleValue::Text("DE".into()),
                ])]),
            target: Schema::new(vec![Field::new(
                "right",
                "shipping_country",
                DataType::Text,
            )
            .with_samples(vec![
                SampleValue::Text("US".into()),
                SampleValue::Text("GB".into()),
                SampleValue::Text("DE".into()),
            ])]),
            expected: Some("right"),
        },
        Case {
            // Ground truth: gross source amount versus net target amount. The
            // supplied names/types omit this distinction; both models are wrong.
            label: "hidden gross/net semantics: false proposal from both",
            source: Schema::new(vec![Field::new("s", "amount", DataType::Decimal)]),
            target: Schema::new(vec![Field::new("wrong", "amount", DataType::Decimal)]),
            expected: None,
        },
    ]
}

#[derive(Default)]
struct Counts {
    correct: usize,
    incorrect: usize,
    missed: usize,
    abstained: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let combined = MatchEngine::new(Config::default())?;
    let name_only = MatchEngine::with_matchers(
        Config {
            reject_incompatible_types: false,
            ..Config::default()
        },
        vec![WeightedMatcher::new(1.0, NameMatcher::default())],
    )?;
    let cases = cases();
    println!("Seven hand-written synthetic cases; thresholds 0.70, ambiguity margin 0.08.");
    println!("Both use the same NameMatcher aliases and abstain on ambiguity.");
    println!("Name-only disables the type veto and all non-name signals.\n");
    for (label, engine) in [("combined", combined), ("name-only", name_only)] {
        let mut counts = Counts::default();
        println!("{label}:");
        for case in &cases {
            let report = engine.match_schemas(&case.source, &case.target)?;
            let field = report
                .fields
                .first()
                .ok_or("synthetic fixture has no source")?;
            let actual = field.selected.as_ref().map(|c| c.target.0.as_str());
            match actual {
                Some(actual) if Some(actual) == case.expected => counts.correct += 1,
                Some(_) => counts.incorrect += 1,
                None => counts.abstained += 1,
            }
            if case.expected.is_some() && actual != case.expected {
                counts.missed += 1;
            }
            println!(
                "  {}: proposed={}, expected={}, decision={:?}",
                case.label,
                actual.unwrap_or("unmatched"),
                case.expected.unwrap_or("unmatched"),
                field.decision
            );
        }
        let precision = counts.correct as f64 / (counts.correct + counts.incorrect) as f64;
        let recall = counts.correct as f64 / (counts.correct + counts.missed) as f64;
        println!(
            "  correct={}, incorrect={}, missed={}, precision={precision:.3}, \
             recall={recall:.3}, abstained={}/{}\n",
            counts.correct,
            counts.incorrect,
            counts.missed,
            counts.abstained,
            cases.len()
        );
    }
    println!("Precision counts correct proposals / all proposals; recall counts correct / labeled matches.");
    println!(
        "Wrong-target proposals count as both an incorrect proposal and a missed labeled match."
    );
    println!("These fixtures illustrate behavior; they are not evidence of real-world accuracy.");
    Ok(())
}
