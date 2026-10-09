use fieldkin::algorithms::{
    Coma, ComaConfig, Cupid, DistributionBased, JaccardConfig, JaccardDistanceMatcher,
    PrecomputedEmbeddings, SimilarityFlooding, StringDistanceFunction, StringMatcher,
};
use fieldkin::{ColumnPair, DataType, Field, Matcher, Table, valentine_match};
use std::sync::Arc;

fn table(name: &str, columns: &[(&str, &[&str])]) -> Table {
    Table::new(
        name,
        columns
            .iter()
            .map(|(name, samples)| Field {
                name: (*name).into(),
                data_type: DataType::Text,
                samples: samples.iter().map(|value| (*value).into()).collect(),
            })
            .collect(),
    )
    .unwrap()
}

fn matchers() -> Vec<(&'static str, Box<dyn Matcher>)> {
    vec![
        ("Jaccard", Box::new(JaccardDistanceMatcher::default())),
        (
            "exact Jaccard",
            Box::new(
                JaccardDistanceMatcher::new(JaccardConfig {
                    distance_fun: StringDistanceFunction::Exact,
                    process_num: 2,
                    ..Default::default()
                })
                .unwrap(),
            ),
        ),
        ("Distribution", Box::new(DistributionBased::default())),
        (
            "Bloom Distribution",
            Box::new(DistributionBased {
                use_bloom_filters: true,
                process_num: 2,
                ..Default::default()
            }),
        ),
        (
            "instance COMA",
            Box::new(
                Coma::new(ComaConfig {
                    use_instances: true,
                    use_schema: false,
                    ..Default::default()
                })
                .unwrap(),
            ),
        ),
        (
            "combined COMA",
            Box::new(
                Coma::new(ComaConfig {
                    use_instances: true,
                    ..Default::default()
                })
                .unwrap(),
            ),
        ),
        ("Cupid", Box::new(Cupid::default())),
        ("Flooding", Box::new(SimilarityFlooding::default())),
        (
            "TF-IDF Flooding",
            Box::new(SimilarityFlooding {
                string_matcher: StringMatcher::PrefixSuffixTfidf,
                ..Default::default()
            }),
        ),
    ]
}

fn assert_empty_columns_do_not_match(matcher: &dyn Matcher) {
    let cases: &[(&[&str], &[&str])] = &[
        (&[""], &[""]),
        (&["", "", ""], &["", ""]),
        (&[], &[]),
        (&[], &["apple"]),
        (&[""], &["apple"]),
        (&["apple"], &[""]),
        (&[""], &["", "apple"]),
    ];
    for (source, target) in cases {
        let tables = [
            table("source", &[("value", source)]),
            table("target", &[("value", target)]),
        ];
        let expected = valentine_match(&tables, matcher).unwrap();
        assert!(expected.is_empty());
        assert_eq!(
            (
                matcher.get_matches(&tables[0], &tables[1]).unwrap(),
                matcher.get_matches_batch(&tables).unwrap(),
            ),
            (expected.clone(), expected),
            "source={source:?}, target={target:?}"
        );
    }
}

#[test]
fn jaccard_ignores_empty_string_columns_in_direct_and_batch_matching() {
    assert_empty_columns_do_not_match(&JaccardDistanceMatcher::default());
}

#[test]
fn distribution_ignores_empty_string_columns_in_direct_and_batch_matching() {
    assert_empty_columns_do_not_match(&DistributionBased::default());
}

#[test]
fn all_matchers_treat_empty_samples_like_missing_samples_for_pairs() {
    let cases: &[(&[&str], &[&str])] = &[
        (&[""], &[""]),
        (&["", "", ""], &["", ""]),
        (&[], &[]),
        (&[], &["apple"]),
        (&[""], &["apple"]),
        (&["apple"], &[""]),
        (&["", "apple"], &["apple", ""]),
        (&["", "apple", "apple", "", "pear"], &["apple", "plum"]),
        (&["", "1", "1", "2"], &["1", "", "1", "2", ""]),
        (&["", " "], &[" "]),
        (&["apple", "pear"], &["apple", "plum"]),
    ];
    for (name, matcher) in matchers() {
        for (source, target) in cases {
            let tables = [
                table("source", &[("value", source)]),
                table("target", &[("value", target)]),
            ];
            let original = tables.clone();
            let source: Vec<_> = source.iter().copied().filter(|s| !s.is_empty()).collect();
            let target: Vec<_> = target.iter().copied().filter(|s| !s.is_empty()).collect();
            let clean = [
                table("source", &[("value", &source)]),
                table("target", &[("value", &target)]),
            ];
            let expected = matcher.get_matches(&clean[0], &clean[1]).unwrap();
            assert_eq!(
                valentine_match(&tables, matcher.as_ref()).unwrap(),
                expected
            );
            assert_eq!(
                matcher.get_matches(&tables[0], &tables[1]).unwrap(),
                expected,
                "{name}: {tables:?}"
            );
            assert_eq!(matcher.get_matches_batch(&tables).unwrap(), expected);
            assert_eq!(tables, original, "{name}: caller samples changed");
        }
    }
}

#[test]
fn all_matchers_ignore_empty_samples_in_global_batch_context() {
    let tables = [
        table(
            "source",
            &[
                ("empty", &["", ""]),
                ("value", &["", "apple", "apple", "pear"]),
            ],
        ),
        table(
            "target",
            &[("empty", &[]), ("value", &["apple", "", "plum"])],
        ),
        table(
            "third",
            &[("empty", &[""]), ("value", &["", "pear", "plum", ""])],
        ),
    ];
    let original = tables.clone();
    let mut clean = tables.clone();
    for table in &mut clean {
        for column in &mut table.columns {
            column.samples.retain(|value| !value.is_empty());
        }
    }
    for (name, matcher) in matchers() {
        let expected = matcher.get_matches_batch(&clean).unwrap();
        assert_eq!(
            valentine_match(&tables, matcher.as_ref()).unwrap(),
            expected
        );
        assert_eq!(
            matcher.get_matches_batch(&tables).unwrap(),
            expected,
            "{name}"
        );
        assert_eq!(tables, original, "{name}: caller samples changed");
    }
}

#[test]
fn jaccard_table_embeddings_do_not_require_empty_string_vectors() {
    let provider = PrecomputedEmbeddings::new([
        ("apple".into(), vec![1.0, 0.0]),
        ("pear".into(), vec![0.0, 1.0]),
    ])
    .unwrap();
    let matcher = JaccardDistanceMatcher::default().with_embedding_provider(Arc::new(provider));
    let tables = [
        table("source", &[("empty", &["", ""]), ("value", &["", "apple"])]),
        table("target", &[("empty", &[""]), ("value", &["apple", ""])]),
        table("third", &[("empty", &[""]), ("value", &["", "pear"])]),
    ];
    let expected = valentine_match(&tables, &matcher).unwrap();
    assert_eq!(expected.len(), 1);
    assert_eq!(
        expected.get(&ColumnPair::new("source", "value", "target", "value")),
        Some(1.0)
    );
    assert_eq!(
        matcher.get_matches(&tables[0], &tables[1]).unwrap(),
        expected
    );
    assert_eq!(matcher.get_matches_batch(&tables).unwrap(), expected);
    assert_empty_columns_do_not_match(&matcher);
}
