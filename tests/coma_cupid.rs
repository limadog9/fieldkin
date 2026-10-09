use fieldkin::algorithms::Matcher;
use fieldkin::algorithms::coma::{Coma, ComaConfig};
use fieldkin::algorithms::cupid::{Cupid, CupidConfig, word_similarity};
use fieldkin::{ColumnPair, DataType, Field, MatcherResults, Table};
use serde::Deserialize;
use std::collections::BTreeMap;

#[derive(Deserialize)]
struct Fixture {
    cases: Vec<Case>,
    word_pairs: Vec<WordPair>,
}
#[derive(Deserialize)]
struct Case {
    algorithm: String,
    name: String,
    tables: Vec<Table>,
    config: serde_json::Value,
    expected: Vec<Expected>,
}
#[derive(Deserialize)]
struct Expected {
    pair: ColumnPair,
    score: f64,
    details: BTreeMap<String, f64>,
}
#[derive(Deserialize)]
struct WordPair {
    a: String,
    b: String,
    score: f64,
}

fn fixture() -> Fixture {
    serde_json::from_str(include_str!("fixtures/coma_cupid.json")).unwrap()
}

fn assert_reference(case: &Case, actual: &MatcherResults, tolerance: f64) {
    assert_eq!(
        actual.len(),
        case.expected.len(),
        "{}: matched pairs differ",
        case.name
    );
    for expected in &case.expected {
        let score = actual
            .get(&expected.pair)
            .unwrap_or_else(|| panic!("{}: missing {:?}", case.name, expected.pair));
        assert!(
            (score - expected.score).abs() <= tolerance,
            "{}: {:?}: expected {}, actual {}",
            case.name,
            expected.pair,
            expected.score,
            score
        );
        let details = actual
            .get_details(&expected.pair)
            .expect("submatcher details");
        for (name, expected) in &expected.details {
            let actual = details[name];
            assert!(
                (actual - expected).abs() <= tolerance,
                "{}: {:?} {name}: expected {expected}, actual {actual}",
                case.name,
                expected
            );
        }
    }
}

#[test]
fn coma_matches_pinned_upstream_scores_details_selection_and_batch_idf() {
    for case in fixture()
        .cases
        .into_iter()
        .filter(|case| case.algorithm == "coma")
    {
        let config: ComaConfig = serde_json::from_value(case.config.clone()).unwrap();
        let actual = Coma::new(config)
            .unwrap()
            .get_matches_batch(&case.tables)
            .unwrap();
        // Upstream stores TF-IDF weights and dot products in float32. Native
        // f64 normalization preserves its algorithm with small rounding drift.
        assert_reference(&case, &actual, 2e-6);
    }
}

#[test]
fn cupid_matches_pinned_upstream_semantics_weights_and_postorder_propagation() {
    for case in fixture()
        .cases
        .into_iter()
        .filter(|case| case.algorithm == "cupid")
    {
        let config: CupidConfig = serde_json::from_value(case.config.clone()).unwrap();
        let actual = Cupid::new(config)
            .unwrap()
            .get_matches(&case.tables[0], &case.tables[1])
            .unwrap();
        assert_reference(&case, &actual, 1e-12);
    }
}

#[test]
fn complete_wordnet_semantics_match_nltk_reference() {
    for pair in fixture().word_pairs {
        let actual = word_similarity(&pair.a, &pair.b);
        assert!(
            (actual - pair.score).abs() < 1e-12,
            "{} / {}: expected {}, actual {}",
            pair.a,
            pair.b,
            pair.score,
            actual
        );
        assert_eq!(actual, word_similarity(&pair.b, &pair.a), "word symmetry");
    }
}

fn table(name: &str, names: &[&str]) -> Table {
    Table::new(
        name,
        names
            .iter()
            .map(|name| Field {
                name: (*name).into(),
                data_type: DataType::Text,
                samples: Vec::new(),
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn empty_tables_and_invalid_configuration_are_handled_without_panics() {
    let empty = table("empty", &[]);
    let table = table("columns", &["name"]);
    assert!(
        Coma::default()
            .get_matches(&empty, &table)
            .unwrap()
            .is_empty()
    );
    assert!(
        Cupid::default()
            .get_matches(&table, &empty)
            .unwrap()
            .is_empty()
    );
    assert!(
        Coma::new(ComaConfig {
            use_schema: false,
            use_instances: false,
            ..Default::default()
        })
        .is_err()
    );
    assert!(
        Coma::new(ComaConfig {
            instance_weight: -1.0,
            ..Default::default()
        })
        .is_err()
    );
    assert!(
        Coma::new(ComaConfig {
            delta: f64::NAN,
            ..Default::default()
        })
        .is_err()
    );
    assert!(
        Cupid::new(CupidConfig {
            process_num: 0,
            ..Default::default()
        })
        .is_err()
    );
    assert!(
        Cupid::new(CupidConfig {
            c_inc: f64::INFINITY,
            ..Default::default()
        })
        .is_err()
    );
    assert!(
        Cupid::new(CupidConfig {
            th_ns: 1.1,
            ..Default::default()
        })
        .is_err()
    );
    let mut matcher = Cupid::default();
    matcher.config.leaf_w_struct = -1.0;
    assert!(matcher.get_matches(&table, &table).is_err());
}

#[test]
fn cupid_worker_count_changes_execution_without_changing_results() {
    let source = table(
        "source",
        &["car", "doctor", "running", "cat", "name", "quick"],
    );
    let target = table(
        "target",
        &["automobile", "physician", "walking", "dog", "name", "fast"],
    );
    let single = Cupid::default().get_matches(&source, &target).unwrap();
    let parallel = Cupid::new(CupidConfig {
        process_num: 4,
        ..Default::default()
    })
    .unwrap()
    .get_matches(&source, &target)
    .unwrap();
    assert_eq!(single, parallel);
}

#[test]
fn flat_schema_parent_propagation_occurs_four_times() {
    let source = table("source", &["name"]);
    let target = table("target", &["name"]);
    let matches = Cupid::default().get_matches(&source, &target).unwrap();
    let score = matches
        .get(&ColumnPair::new("source", "name", "target", "name"))
        .unwrap();
    let expected = 0.8 + 0.2 * 0.5 * 0.9_f64.powi(4);
    assert!((score - expected).abs() < 1e-12);
    let too_wide = table("wide", &["name", "a", "b"]);
    let matches = Cupid::default().get_matches(&source, &too_wide).unwrap();
    assert_eq!(
        matches.get(&ColumnPair::new("source", "name", "wide", "name")),
        Some(0.9)
    );
}

#[test]
fn cupid_asymmetric_wordnet_root_ties_use_stable_canonical_order() {
    // NLTK account.v.02 -> balance.v.02 is 2/3, but the reverse is 2/5.
    // Evaluating the pair by canonical names keeps Cupid symmetric, even after
    // unrelated corpus lookups or requests with the arguments reversed.
    for (a, b) in [("account", "balance"), ("balance", "account")] {
        assert_eq!(word_similarity(a, b), 2.0 / 3.0);
        let _ = word_similarity("department", "division");
        let _ = word_similarity("cats", "dogs");
        assert_eq!(word_similarity(a, b), 2.0 / 3.0);
    }
}

#[test]
fn cupid_customers_scores_follow_canonical_reference_with_different_weights_and_workers() {
    #[derive(Deserialize)]
    struct Dataset {
        source: fieldkin::Schema,
        target: fieldkin::Schema,
    }
    let dataset: Dataset =
        serde_json::from_str(include_str!("fixtures/cupid_customers.json")).unwrap();
    let source = Table::from_schema("source", &dataset.source);
    let target = Table::from_schema("target", &dataset.target);
    let pair = ColumnPair::new("source", "balance", "target", "account_balance");
    // Upstream with canonical synset ordering yields this independently
    // generated value; the object-ID baseline sometimes yielded .7630458974.
    for (config, expected) in [
        (CupidConfig::default(), 0.7767211111111111),
        (
            CupidConfig {
                leaf_w_struct: 0.4,
                th_accept: 0.5,
                process_num: 4,
                ..Default::default()
            },
            0.6645533333333333,
        ),
    ] {
        let matcher = Cupid::new(config).unwrap();
        let results = matcher.get_matches(&source, &target).unwrap();
        assert!((results.get(&pair).unwrap() - expected).abs() < 1e-12);
        let reverse = matcher.get_matches(&target, &source).unwrap();
        let reverse_pair = ColumnPair::new("target", "account_balance", "source", "balance");
        assert_eq!(results.get(&pair), reverse.get(&reverse_pair));
    }
}
