use fieldkin::algorithms::flooding::{Formula, Policy, StringMatcher};
use fieldkin::algorithms::{DistributionBased, Matcher, SimilarityFlooding};
use fieldkin::{ColumnPair, DataType, Field, MatcherResults, Table};
use serde::Deserialize;
use serde_json::Value;

fn field(name: &str, data_type: DataType, samples: &[&str]) -> Field {
    Field {
        name: name.into(),
        data_type,
        samples: samples.iter().map(|value| (*value).into()).collect(),
    }
}

fn table(name: &str, columns: Vec<Field>) -> Table {
    Table::new(name, columns).unwrap()
}

#[derive(Deserialize)]
struct Expected {
    pair: ColumnPair,
    score: f64,
}

#[derive(Deserialize)]
struct Fixture {
    name: String,
    algorithm: String,
    tables: Vec<Table>,
    #[serde(default)]
    corpus: Vec<Table>,
    config: Value,
    batch: bool,
    expected: Vec<Expected>,
}

fn assert_reference(fixture: &Fixture, actual: &MatcherResults) {
    assert_eq!(
        actual.len(),
        fixture.expected.len(),
        "{}: result count; actual={actual:?}",
        fixture.name
    );
    for expected in &fixture.expected {
        let score = actual.get(&expected.pair).unwrap_or_else(|| {
            panic!(
                "{}: missing {:?}; actual={actual:?}",
                fixture.name, expected.pair
            )
        });
        assert!(
            (score - expected.score).abs() <= 1e-10,
            "{}: {:?}: Rust={score}, Python={}",
            fixture.name,
            expected.pair,
            expected.score
        );
    }
}

#[test]
fn all_policies_formulas_string_matchers_and_distribution_phases_match_python() {
    // Generated directly by the pinned upstream implementation. The fixtures
    // exercise all 24 Flooding configurations, global/corpus IDF, both attribute
    // intersection methods, numeric aliases, cutoffs and directional histograms.
    let fixtures: Vec<Fixture> =
        serde_json::from_str(include_str!("fixtures/distribution_flooding.json")).unwrap();
    assert_eq!(fixtures.len(), 42);
    for fixture in fixtures {
        let matcher: Box<dyn Matcher> = if fixture.algorithm == "distribution" {
            let mut matcher = DistributionBased::default();
            if let Some(value) = fixture.config["threshold1"].as_f64() {
                matcher.threshold1 = value;
            }
            if let Some(value) = fixture.config["threshold2"].as_f64() {
                matcher.threshold2 = value;
            }
            if let Some(value) = fixture.config["quantiles"].as_u64() {
                matcher.quantiles = value as usize;
            }
            matcher.use_bloom_filters = fixture.config["use_bloom_filters"]
                .as_bool()
                .unwrap_or(false);
            Box::new(matcher)
        } else {
            let policy = match fixture.config["policy"]
                .as_str()
                .unwrap_or("INVERSE_AVERAGE")
            {
                "INVERSE_AVERAGE" => Policy::InverseAverage,
                "INVERSE_PRODUCT" => Policy::InverseProduct,
                value => panic!("unexpected policy {value}"),
            };
            let formula = match fixture.config["formula"].as_str().unwrap_or("FORMULA_C") {
                "BASIC" => Formula::Basic,
                "FORMULA_A" => Formula::FormulaA,
                "FORMULA_B" => Formula::FormulaB,
                "FORMULA_C" => Formula::FormulaC,
                value => panic!("unexpected formula {value}"),
            };
            let string_matcher = match fixture.config["string_matcher"]
                .as_str()
                .unwrap_or("PREFIX_SUFFIX")
            {
                "PREFIX_SUFFIX" => StringMatcher::PrefixSuffix,
                "PREFIX_SUFFIX_TFIDF" => StringMatcher::PrefixSuffixTfidf,
                "LEVENSHTEIN" => StringMatcher::Levenshtein,
                value => panic!("unexpected string matcher {value}"),
            };
            Box::new(SimilarityFlooding {
                coeff_policy: policy,
                formula,
                string_matcher,
                tfidf_corpus: fixture.corpus.clone(),
                ..SimilarityFlooding::default()
            })
        };
        let actual = if fixture.batch {
            matcher.get_matches_batch(&fixture.tables)
        } else {
            matcher.get_matches(&fixture.tables[0], &fixture.tables[1])
        }
        .unwrap();
        assert_reference(&fixture, &actual);
    }
}

#[test]
fn empty_samples_and_nonfinite_numeric_samples_have_no_distribution_matches() {
    let source = table(
        "source",
        vec![
            field("empty", DataType::Text, &[]),
            field("nan", DataType::Float, &["NaN", "nan"]),
        ],
    );
    let target = table(
        "target",
        vec![field("number", DataType::Integer, &["1", "2", "3"])],
    );
    assert!(
        DistributionBased::default()
            .get_matches(&source, &target)
            .unwrap()
            .is_empty()
    );
    assert!(
        DistributionBased::default()
            .get_matches_batch(&[])
            .unwrap()
            .is_empty()
    );
    assert!(
        SimilarityFlooding::default()
            .get_matches(&table("empty", vec![]), &target)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn constant_distribution_and_parallel_processing_are_supported() {
    let source = table(
        "source",
        vec![field("repeated", DataType::Text, &["same", "same", "same"])],
    );
    let target = table(
        "target",
        vec![
            field("singleton", DataType::Text, &["same"]),
            field("different", DataType::Text, &["else"]),
        ],
    );
    let sequential = DistributionBased::default()
        .get_matches(&source, &target)
        .unwrap();
    let parallel = DistributionBased {
        process_num: 3,
        ..DistributionBased::default()
    }
    .get_matches(&source, &target)
    .unwrap();
    assert_eq!(sequential, parallel);
    assert_eq!(sequential.len(), 1);
    assert_eq!(
        sequential.get(&ColumnPair::new(
            "source",
            "repeated",
            "target",
            "singleton"
        )),
        Some(1.0)
    );
}

#[test]
fn flooding_is_transpose_equivariant_and_reaches_a_fixpoint() {
    let source = table(
        "Employee",
        vec![
            field("EmployeeId", DataType::Integer, &[]),
            field("FullName", DataType::Text, &[]),
        ],
    );
    let target = table(
        "People",
        vec![
            field("PersonId", DataType::Integer, &[]),
            field("Name", DataType::Text, &[]),
        ],
    );
    for policy in [Policy::InverseAverage, Policy::InverseProduct] {
        let matcher = SimilarityFlooding {
            coeff_policy: policy,
            ..SimilarityFlooding::default()
        };
        let forward = matcher.get_matches(&source, &target).unwrap();
        let reverse = matcher.get_matches(&target, &source).unwrap();
        for (pair, score) in &forward {
            let opposite = ColumnPair::new(
                &pair.target_table,
                &pair.target_column,
                &pair.source_table,
                &pair.source_column,
            );
            assert!((score - reverse.get(&opposite).unwrap()).abs() < 1e-12);
        }
        let early = SimilarityFlooding {
            max_iterations: 1,
            ..matcher.clone()
        }
        .get_matches(&source, &target)
        .unwrap();
        let precise = SimilarityFlooding {
            max_iterations: 1000,
            residual_threshold: 1e-9,
            ..matcher
        }
        .get_matches(&source, &target)
        .unwrap();
        assert!(
            forward
                .iter()
                .any(|(pair, score)| (score - early.get(pair).unwrap()).abs() > 1e-4)
        );
        for (pair, score) in &forward {
            assert!((score - precise.get(pair).unwrap()).abs() < 1e-3);
        }
    }
}

#[test]
fn invalid_algorithm_configurations_return_errors() {
    let source = table("source", vec![field("a", DataType::Text, &["v"])]);
    let target = table("target", vec![field("b", DataType::Text, &["v"])]);
    for matcher in [
        DistributionBased {
            quantiles: 0,
            ..DistributionBased::default()
        },
        DistributionBased {
            process_num: 0,
            ..DistributionBased::default()
        },
        DistributionBased {
            threshold1: f64::NAN,
            ..DistributionBased::default()
        },
        DistributionBased {
            threshold2: -0.01,
            ..DistributionBased::default()
        },
    ] {
        assert!(matcher.get_matches(&source, &target).is_err());
    }
    for matcher in [
        SimilarityFlooding {
            max_iterations: 0,
            ..SimilarityFlooding::default()
        },
        SimilarityFlooding {
            residual_threshold: f64::NAN,
            ..SimilarityFlooding::default()
        },
        SimilarityFlooding {
            residual_threshold: -0.01,
            ..SimilarityFlooding::default()
        },
    ] {
        assert!(matcher.get_matches(&source, &target).is_err());
    }
}

#[test]
fn impossible_quantile_allocations_return_configuration_errors() {
    let source = table("source", vec![field("a", DataType::Integer, &["1", "2"])]);
    let target = table("target", vec![field("b", DataType::Integer, &["1", "2"])]);
    let maximum = isize::MAX as usize / std::mem::size_of::<f64>();
    for quantiles in [maximum + 1, usize::MAX] {
        let matcher = DistributionBased {
            quantiles,
            ..DistributionBased::default()
        };
        let error = matcher.get_matches(&source, &target).unwrap_err();
        assert!(matches!(error, fieldkin::Error::InvalidConfig(_)));
        assert!(error.to_string().contains("quantiles"));
        assert!(matches!(
            matcher.validate(),
            Err(fieldkin::Error::InvalidConfig(_))
        ));
    }
    // This is an allocation representability check, not a workload-size cap.
    // Validate the inclusive limit without attempting to allocate that much memory.
    assert!(
        DistributionBased {
            quantiles: maximum,
            ..DistributionBased::default()
        }
        .validate()
        .is_ok()
    );
}

#[test]
fn distribution_preserves_solver_sensitive_and_colliding_column_names() {
    let source_names = [
        "axis[0]",
        "axis_0_",
        "axis-0",
        "axis+0",
        "literal__WHITESPACE__tag",
    ];
    let target_names = [
        "geometry.coordinates[0]",
        "geometry.coordinates_0_",
        "geometry.coordinates-0",
        "geometry.coordinates+0",
        "literal tag",
    ];
    let values = ["north", "east", "south", "west", "middle"];
    let source = table(
        "source[set]-a",
        source_names
            .iter()
            .zip(values)
            .map(|(name, value)| field(name, DataType::Text, &[value, value]))
            .collect(),
    );
    let target = table(
        "target+set",
        target_names
            .iter()
            .zip(values)
            .map(|(name, value)| field(name, DataType::Text, &[value]))
            .collect(),
    );
    let results = DistributionBased::default()
        .get_matches(&source, &target)
        .unwrap();
    assert_eq!(results.len(), source_names.len());
    for (source_name, target_name) in source_names.iter().zip(target_names) {
        assert_eq!(
            results.get(&ColumnPair::new(
                &source.name,
                *source_name,
                &target.name,
                target_name
            )),
            Some(1.0)
        );
    }
    for (pair, _) in &results {
        assert!(
            source
                .columns
                .iter()
                .any(|field| field.name == pair.source_column)
        );
        assert!(
            target
                .columns
                .iter()
                .any(|field| field.name == pair.target_column)
        );
    }
}
