use std::{
    cell::RefCell,
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
    sync::atomic::{AtomicUsize, Ordering},
    time::{SystemTime, UNIX_EPOCH},
};

use fieldkin::algorithms::{JaccardDistanceMatcher, SimilarityFlooding};
use fieldkin::{
    ColumnPair, DataType, Error, Field, MatchOptions, Matcher, MatcherResults, Table, match_tables,
    valentine_match,
};

fn table(name: &str, values: &[&str]) -> Table {
    Table::new(
        name,
        vec![Field {
            name: "value".into(),
            data_type: DataType::Text,
            samples: values.iter().map(|value| (*value).into()).collect(),
        }],
    )
    .unwrap()
}

#[derive(Default)]
struct RecordingMatcher {
    calls: RefCell<Vec<(Table, Table)>>,
}

impl Matcher for RecordingMatcher {
    fn get_matches(&self, source: &Table, target: &Table) -> Result<MatcherResults, Error> {
        self.calls
            .borrow_mut()
            .push((source.clone(), target.clone()));
        let pair = ColumnPair::new(
            &source.name,
            &source.columns[0].name,
            &target.name,
            &target.columns[0].name,
        );
        MatcherResults::with_details(
            vec![(pair.clone(), 0.75)],
            BTreeMap::from([(pair, BTreeMap::from([("custom_evidence".into(), 0.375)]))]),
        )
    }
}

#[test]
fn batch_input_errors_are_detected_before_the_matcher_runs() {
    let matcher = RecordingMatcher::default();
    let first = table("first", &["one"]);
    for tables in [vec![], vec![first.clone()], vec![first.clone(), first]] {
        assert!(matches!(
            valentine_match(&tables, &matcher),
            Err(Error::InvalidInput(_))
        ));
    }
    assert!(matcher.calls.borrow().is_empty());
}

#[test]
fn three_tables_emit_each_unique_table_pair_with_forward_identity() {
    let tables = [
        table("third", &["shared"]),
        table("first", &["shared"]),
        table("second", &["shared"]),
    ];
    let matches = valentine_match(&tables, &JaccardDistanceMatcher::default()).unwrap();
    assert_eq!(matches.len(), 3);
    let pairs: BTreeSet<_> = matches
        .iter()
        .map(|(pair, _)| (pair.source_table.as_str(), pair.target_table.as_str()))
        .collect();
    assert_eq!(
        pairs,
        BTreeSet::from([("third", "first"), ("third", "second"), ("first", "second")])
    );
    assert!(matches.iter().all(|(_, score)| *score == 1.0));
}

#[test]
fn custom_matcher_default_batch_combines_scores_and_details() {
    let tables = [table("a", &["a"]), table("b", &["b"]), table("c", &["c"])];
    let matcher = RecordingMatcher::default();
    let matches = valentine_match(&tables, &matcher).unwrap();
    assert_eq!(matches.len(), 3);
    assert_eq!(matches.details().len(), 3);
    for (pair, score) in &matches {
        assert_eq!(*score, 0.75);
        assert_eq!(matches.get_details(pair).unwrap()["custom_evidence"], 0.375);
    }
    let calls = matcher.calls.borrow();
    assert_eq!(calls.len(), 3);
    assert_eq!((&calls[0].0, &calls[0].1), (&tables[0], &tables[1]));
    assert_eq!((&calls[1].0, &calls[1].1), (&tables[0], &tables[2]));
    assert_eq!((&calls[2].0, &calls[2].1), (&tables[1], &tables[2]));
}

#[test]
fn explicit_sampling_is_evenly_spaced_deterministic_and_preserves_inputs() {
    let tables = [
        table(
            "source",
            &["", "zero", "one", "two", "three", "four", "five", "", "six"],
        ),
        table("target", &["a", "b", "c", "d", "e"]),
    ];
    let original = tables.clone();
    let matcher = RecordingMatcher::default();
    let options = MatchOptions {
        instance_sample_size: Some(3),
    };
    let first = match_tables(&tables, &matcher, options).unwrap();
    let second = match_tables(&tables, &matcher, options).unwrap();
    assert_eq!(first, second);
    assert_eq!(tables, original);
    let calls = matcher.calls.borrow();
    assert_eq!(calls[0], calls[1]);
    assert_eq!(calls[0].0.columns[0].samples, ["zero", "two", "four"]);
    assert_eq!(calls[0].1.columns[0].samples, ["a", "b", "d"]);
    assert_eq!(calls[0].0.columns[0].data_type, DataType::Text);
    drop(calls);

    valentine_match(&tables, &matcher).unwrap();
    let calls = matcher.calls.borrow();
    assert_eq!(
        calls[2].0.columns[0].samples,
        ["zero", "one", "two", "three", "four", "five", "six"]
    );
    assert_eq!(
        calls[2].1.columns[0].samples,
        original[1].columns[0].samples
    );
}

#[test]
fn zero_sampling_clears_instances_and_keeps_schema_matching_available() {
    let tables = [
        table("source", &["one", "two"]),
        table("target", &["one", "two"]),
    ];
    let options = MatchOptions {
        instance_sample_size: Some(0),
    };
    let matcher = RecordingMatcher::default();
    assert_eq!(match_tables(&tables, &matcher, options).unwrap().len(), 1);
    let calls = matcher.calls.borrow();
    for table in [&calls[0].0, &calls[0].1] {
        assert_eq!(table.columns.len(), 1);
        assert_eq!(table.columns[0].name, "value");
        assert!(table.columns[0].samples.is_empty());
    }
    drop(calls);
    assert!(
        match_tables(&tables, &JaccardDistanceMatcher::default(), options)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        match_tables(&tables, &SimilarityFlooding::default(), options)
            .unwrap()
            .len(),
        1
    );
    assert_eq!(tables[0].columns[0].samples, ["one", "two"]);
}

struct Fixtures(PathBuf);

impl Fixtures {
    fn new(label: &str) -> Self {
        static SEQUENCE: AtomicUsize = AtomicUsize::new(0);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let folder = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join("test-fixtures")
            .join(format!(
                "batch-cli-{}-{label}-{timestamp}-{}",
                std::process::id(),
                SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&folder).unwrap();
        Self(folder)
    }

    fn write(&self, name: &str, content: &str) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, content).unwrap();
        path
    }
}

impl Drop for Fixtures {
    fn drop(&mut self) {
        // This uniquely owned directory is always inside target/test-fixtures.
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn cli(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fieldkin"))
        .args(arguments)
        .output()
        .expect("fieldkin binary should run")
}

#[test]
fn cli_help_lists_the_available_algorithms() {
    let output = cli(&["--help"]);
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let help = String::from_utf8(output.stdout).unwrap();
    assert!(help.contains("Usage: fieldkin"));
    for algorithm in ["coma", "cupid", "distribution", "jaccard", "flooding"] {
        assert!(help.contains(algorithm), "missing algorithm {algorithm}");
    }
}

#[test]
fn cli_matches_quoted_csv_against_json_and_emits_table_aware_json() {
    let fixtures = Fixtures::new("quoted-inputs");
    let source = fixtures.write(
        "source with spaces.csv",
        "\"first,name\",memo\r\n\"Smith, Jane\",\"line one\nline two\"\r\n\"Fox \"\"Red\"\"\",\"quote \"\"inside\"\"\"\r\n",
    );
    let target = fixtures.write(
        "target with spaces.JSON",
        r#"[{"person":"Smith, Jane","notes":"line one\nline two"},{"person":"Fox \"Red\"","notes":"quote \"inside\""}]"#,
    );
    let output = cli(&[
        "jaccard",
        source.to_str().unwrap(),
        target.to_str().unwrap(),
        "People",
        "Contacts",
    ]);
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let matches = json["matches"].as_array().unwrap();
    let found: BTreeMap<_, _> = matches
        .iter()
        .map(|entry| {
            let pair = serde_json::from_value::<ColumnPair>(entry["pair"].clone()).unwrap();
            (pair, entry["score"].as_f64().unwrap())
        })
        .collect();
    assert_eq!(
        found.get(&ColumnPair::new(
            "People",
            "first,name",
            "Contacts",
            "person"
        )),
        Some(&1.0)
    );
    assert_eq!(
        found.get(&ColumnPair::new("People", "memo", "Contacts", "notes")),
        Some(&1.0)
    );
    assert_eq!(found.len(), 2);
    assert!(
        matches
            .windows(2)
            .all(|pair| pair[0]["score"].as_f64().unwrap() >= pair[1]["score"].as_f64().unwrap())
    );
}

#[test]
fn cli_rejects_malformed_inputs_and_unknown_algorithms_with_nonzero_exit() {
    let fixtures = Fixtures::new("invalid-inputs");
    let good = fixtures.write("good.csv", "value\none\n");
    let bad_csv = fixtures.write("bad.csv", "id,name\n1\n");
    let bad_json = fixtures.write("bad.json", r#"[{"nested":{"key":"value"}}]"#);
    for arguments in [
        vec!["jaccard", bad_csv.to_str().unwrap(), good.to_str().unwrap()],
        vec![
            "jaccard",
            bad_json.to_str().unwrap(),
            good.to_str().unwrap(),
        ],
        vec!["unknown", good.to_str().unwrap(), good.to_str().unwrap()],
        vec!["jaccard"],
    ] {
        let output = cli(&arguments);
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(String::from_utf8_lossy(&output.stderr).starts_with("fieldkin: "));
    }
}
