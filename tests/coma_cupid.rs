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
