//! Labelled-schema graph construction and fixpoint similarity propagation.

use std::collections::{BTreeMap, BTreeSet};

use super::Matcher;
use super::strings::{StringDistanceFunction, similarity};
use crate::{ColumnPair, DataType, Error, MatcherResults, Table};

/// How equally labelled graph edges share propagation weight.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Policy {
    /// Inverse average of the corresponding schema degrees.
    #[default]
    InverseAverage,
    /// Inverse degree in the graph product.
    InverseProduct,
}

/// The four fixpoint updates from Similarity Flooding.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Formula {
    Basic,
    FormulaA,
    FormulaB,
    #[default]
    FormulaC,
}

/// Initial similarities between literal graph labels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StringMatcher {
    #[default]
    PrefixSuffix,
    PrefixSuffixTfidf,
    Levenshtein,
}

/// Melnik, Garcia-Molina and Rahm's Similarity Flooding matcher.
///
/// Defaults reproduce Valentine's inverse-average, formula-C algorithm:
/// at most 100 iterations, stopping at a Euclidean residual of `1e-4`.
#[derive(Clone, Debug)]
pub struct SimilarityFlooding {
    pub coeff_policy: Policy,
    pub formula: Formula,
    pub string_matcher: StringMatcher,
    /// Extra tables contributing IDF documents to pairwise matching.
    pub tfidf_corpus: Vec<Table>,
    pub max_iterations: usize,
    pub residual_threshold: f64,
}

impl Default for SimilarityFlooding {
    fn default() -> Self {
        Self {
            coeff_policy: Policy::default(),
            formula: Formula::default(),
            string_matcher: StringMatcher::default(),
            tfidf_corpus: Vec::new(),
            max_iterations: 100,
            residual_threshold: 1e-4,
        }
    }
}

impl SimilarityFlooding {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&self) -> Result<(), Error> {
        if self.max_iterations == 0 {
            return Err(Error::InvalidConfig("max_iterations must be >= 1".into()));
        }
        if !self.residual_threshold.is_finite() || self.residual_threshold < 0.0 {
            return Err(Error::InvalidConfig(
                "residual_threshold must be finite and >= 0".into(),
            ));
        }
        for table in &self.tfidf_corpus {
            table.validate()?;
        }
        Ok(())
    }

    fn match_graphs(
        &self,
        source: &Table,
        target: &Table,
        g1: &Graph,
        g2: &Graph,
        idf: Option<&BTreeMap<String, f64>>,
    ) -> Vec<(ColumnPair, f64)> {
        let width = g2.names.len();
        let initial: Vec<f64> = g1
            .names
            .iter()
            .flat_map(|a| {
                g2.names.iter().map(move |b| {
                    if a.starts_with('\0') || b.starts_with('\0') {
                        return 0.0;
                    }
                    match self.string_matcher {
                        StringMatcher::PrefixSuffix => prefix_suffix(a, b, None),
                        StringMatcher::PrefixSuffixTfidf => prefix_suffix(a, b, idf),
                        StringMatcher::Levenshtein => {
                            similarity(a, b, StringDistanceFunction::Levenshtein)
                        }
                    }
                })
            })
            .collect();
        let propagation = Propagation::new(g1, g2, self.coeff_policy);
        let mut previous = initial.clone();
        for _ in 0..self.max_iterations {
            let mut next = vec![0.0; initial.len()];
            for &node in &propagation.nodes {
                next[node] = match self.formula {
                    Formula::Basic => previous[node],
                    Formula::FormulaA => initial[node],
                    Formula::FormulaB => 0.0,
                    Formula::FormulaC => initial[node] + previous[node],
                };
            }
            for &(from, to, weight) in &propagation.edges {
                next[to] += weight
                    * match self.formula {
                        Formula::Basic | Formula::FormulaA => previous[from],
                        Formula::FormulaB | Formula::FormulaC => initial[from] + previous[from],
                    };
            }
            let maximum = next.iter().copied().fold(0.0, f64::max);
            if maximum > 0.0 {
                for value in &mut next {
                    *value /= maximum;
                }
            }
            let residual = previous
                .iter()
                .zip(&next)
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f64>()
                .sqrt();
            previous = next;
            if residual <= self.residual_threshold {
                break;
            }
        }
        let mut output = Vec::with_capacity(source.columns.len() * target.columns.len());
        for &(s_node, s_column) in &g1.columns {
            for &(t_node, t_column) in &g2.columns {
                let index = s_node * width + t_node;
                if propagation.nodes.contains(&index) {
                    output.push((
                        ColumnPair::new(
                            &source.name,
                            &source.columns[s_column].name,
                            &target.name,
                            &target.columns[t_column].name,
                        ),
                        previous[index],
                    ));
                }
            }
        }
        output
    }
}

impl Matcher for SimilarityFlooding {
    fn get_matches(&self, source: &Table, target: &Table) -> Result<MatcherResults, Error> {
        self.validate()?;
        super::validate_tables([source, target])?;
        let g1 = Graph::new(source);
        let g2 = Graph::new(target);
        let idf = if self.string_matcher == StringMatcher::PrefixSuffixTfidf {
            let extra: Vec<Graph> = self.tfidf_corpus.iter().map(Graph::new).collect();
            Some(compute_idf(
                std::iter::once(&g1)
                    .chain(std::iter::once(&g2))
                    .chain(extra.iter()),
            ))
        } else {
            None
        };
        MatcherResults::new(self.match_graphs(source, target, &g1, &g2, idf.as_ref()))
    }

    fn get_matches_batch(&self, tables: &[Table]) -> Result<MatcherResults, Error> {
        self.validate()?;
        super::validate_tables(tables)?;
        let graphs: Vec<Graph> = tables.iter().map(Graph::new).collect();
        // Upstream batch computes IDF over the batch itself, ignoring the extra pairwise corpus.
        let idf = (self.string_matcher == StringMatcher::PrefixSuffixTfidf)
            .then(|| compute_idf(graphs.iter()));
        let mut matches = Vec::new();
        for i in 0..tables.len() {
            for j in i + 1..tables.len() {
                matches.extend(self.match_graphs(
                    &tables[i],
                    &tables[j],
                    &graphs[i],
                    &graphs[j],
                    idf.as_ref(),
                ));
            }
        }
        MatcherResults::new(matches)
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Label {
    Name,
    Type,
    Column,
    SqlType,
}

#[derive(Clone, Copy)]
struct Edge {
    from: usize,
    to: usize,
    label: Label,
}

struct Graph {
    names: Vec<String>,
    edges: Vec<Edge>,
    columns: Vec<(usize, usize)>,
}

impl Graph {
    fn node(&mut self, name: String) -> usize {
        if let Some(index) = self.names.iter().position(|old| old == &name) {
            return index;
        }
        self.names.push(name);
        self.names.len() - 1
    }

    fn edge(&mut self, from: usize, to: usize, label: Label) {
        if let Some(edge) = self.edges.iter_mut().find(|e| e.from == from && e.to == to) {
            edge.label = label;
        } else {
            self.edges.push(Edge { from, to, label });
        }
    }

    fn new(table: &Table) -> Self {
        let mut graph = Self {
            names: Vec::new(),
            edges: Vec::new(),
            columns: Vec::new(),
        };
        let table_class = graph.node("Table".into());
        let column_class = graph.node("Column".into());
        let type_class = graph.node("ColumnType".into());
        let mut unique_id = 1;
        let table_node = graph.node(format!("\0NID{unique_id}"));
        let name = graph.node(table.name.clone());
        graph.edge(table_node, name, Label::Name);
        graph.edge(table_node, table_class, Label::Type);
        for (column_index, column) in table.columns.iter().enumerate() {
            unique_id += 1;
            let column_node = graph.node(format!("\0NID{unique_id}"));
            let attribute = graph.node(column.name.clone());
            graph.columns.push((column_node, column_index));
            graph.edge(column_node, column_class, Label::Type);
            graph.edge(table_node, column_node, Label::Column);
            graph.edge(column_node, attribute, Label::Name);
            let type_name = match column.data_type {
                DataType::Integer => "int",
                DataType::Float | DataType::Decimal => "float",
                DataType::Date | DataType::Timestamp => "date",
                DataType::Unknown | DataType::Boolean | DataType::Text => "varchar",
            };
            // Literal nodes are interned by name, as in Valentine's networkx graph.
            let existing_type = graph.names.iter().position(|n| n == type_name);
            if let Some(type_literal) = existing_type
                && let Some(type_node) = graph
                    .edges
                    .iter()
                    .find(|e| e.to == type_literal)
                    .map(|e| e.from)
            {
                graph.edge(column_node, type_node, Label::SqlType);
                continue;
            }
            unique_id += 1;
            let type_node = graph.node(format!("\0NID{unique_id}"));
            let type_literal = graph.node(type_name.into());
            graph.edge(type_node, type_class, Label::Type);
            graph.edge(type_node, type_literal, Label::Name);
            graph.edge(column_node, type_node, Label::SqlType);
        }
        graph
    }

    #[cfg(test)]
    fn degree(&self, node: usize, label: Label, incoming: bool) -> usize {
        self.edges
            .iter()
            .filter(|e| {
                e.label == label
                    && if incoming {
                        e.to == node
                    } else {
                        e.from == node
                    }
            })
            .count()
    }
}

struct Propagation {
    nodes: BTreeSet<usize>,
    edges: Vec<(usize, usize, f64)>,
}

impl Propagation {
    fn new(g1: &Graph, g2: &Graph, policy: Policy) -> Self {
        let width = g2.names.len();
        let [source_edges, target_edges] = [g1, g2].map(|graph| {
            let mut adjacent = vec![[Vec::new(), Vec::new()]; graph.names.len()];
            for &edge in &graph.edges {
                adjacent[edge.to][0].push(edge);
                adjacent[edge.from][1].push(edge);
            }
            adjacent
        });
        let mut nodes = BTreeSet::new();
        let mut insertion_order = Vec::new();
        for e1 in &g1.edges {
            for e2 in &g2.edges {
                if e1.label != e2.label {
                    continue;
                }
                for node in [e1.from * width + e2.from, e1.to * width + e2.to] {
                    if nodes.insert(node) {
                        insertion_order.push(node);
                    }
                }
            }
        }
        let mut weights = BTreeMap::new();
        for node in insertion_order {
            for (direction, incoming) in [true, false].into_iter().enumerate() {
                let source = &source_edges[node / width][direction];
                let target = &target_edges[node % width][direction];
                let [source_degree, target_degree] = [source, target].map(|edges| {
                    let mut degrees = [0usize; 4];
                    for edge in edges {
                        degrees[edge.label as usize] += 1;
                    }
                    degrees
                });
                // Original graphs have one edge per (from, to), so their
                // product has no duplicates. These adjacency products retain
                // the same order as filtering the complete product edge list.
                for e1 in source {
                    for e2 in target {
                        if e1.label != e2.label {
                            continue;
                        }
                        let label = e1.label as usize;
                        let coefficient = match policy {
                            Policy::InverseAverage => {
                                2.0 / (source_degree[label] + target_degree[label]) as f64
                            }
                            Policy::InverseProduct => {
                                1.0 / (source_degree[label] * target_degree[label]) as f64
                            }
                        };
                        let other = if incoming {
                            e1.from * width + e2.from
                        } else {
                            e1.to * width + e2.to
                        };
                        weights.insert((node, other), coefficient);
                    }
                }
            }
        }
        Self {
            nodes,
            edges: weights
                .into_iter()
                .map(|((from, to), weight)| (from, to, weight))
                .collect(),
        }
    }
}

// Equivalent to upstream's ASCII regex, preserving case in token comparison.
fn split_words(name: &str) -> Vec<String> {
    let chars: Vec<char> = name.chars().collect();
    let mut output = Vec::new();
    let mut start = 0;
    while start < chars.len() {
        let c = chars[start];
        let mut end = start + 1;
        if c.is_ascii_digit() {
            while end < chars.len() && chars[end].is_ascii_digit() {
                end += 1;
            }
        } else if c.is_ascii_lowercase() {
            while end < chars.len() && chars[end].is_ascii_lowercase() {
                end += 1;
            }
        } else if c.is_ascii_uppercase() {
            if end < chars.len() && chars[end].is_ascii_lowercase() {
                while end < chars.len() && chars[end].is_ascii_lowercase() {
                    end += 1;
                }
            } else {
                while end < chars.len() && chars[end].is_ascii_uppercase() {
                    end += 1;
                }
                if end > start + 1 && end < chars.len() && chars[end].is_ascii_lowercase() {
                    end -= 1;
                }
            }
        } else {
            start += 1;
            continue;
        }
        output.push(chars[start..end].iter().collect());
        start = end;
    }
    if output.is_empty() && !name.is_empty() {
        output.push(name.into());
    }
    output
}

fn word_similarity(a: &str, b: &str) -> f64 {
    if a == b {
        return 1.0;
    }
    let a: Vec<char> = a.chars().collect();
    let b: Vec<char> = b.chars().collect();
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let prefix = a.iter().zip(&b).take_while(|(x, y)| x == y).count();
    let suffix = a[prefix..]
        .iter()
        .rev()
        .zip(b[prefix..].iter().rev())
        .take_while(|(x, y)| x == y)
        .count();
    (prefix + suffix) as f64 * a.len().min(b.len()) as f64 / (a.len().max(b.len()) as f64).powi(2)
}

fn prefix_suffix(a: &str, b: &str, idf: Option<&BTreeMap<String, f64>>) -> f64 {
    let a = split_words(a);
    let b = split_words(b);
    if a.is_empty() || b.is_empty() {
        return 0.0;
    }
    let weight = |token: &str| {
        idf.and_then(|weights| weights.get(&token.to_lowercase()))
            .copied()
            .unwrap_or(1.0)
    };
    let mut numerator = 0.0;
    let mut denominator = 0.0;
    for (left, right) in [(&a, &b), (&b, &a)] {
        for token in left {
            let w = weight(token);
            numerator += w * right
                .iter()
                .map(|other| word_similarity(token, other))
                .fold(0.0, f64::max);
            denominator += w;
        }
    }
    if denominator > 0.0 {
        numerator / denominator
    } else {
        0.0
    }
}

fn compute_idf<'a>(graphs: impl Iterator<Item = &'a Graph>) -> BTreeMap<String, f64> {
    let mut count = 0usize;
    let mut frequency = BTreeMap::<String, usize>::new();
    for graph in graphs {
        for name in graph.names.iter().filter(|name| !name.starts_with('\0')) {
            count += 1;
            let document: BTreeSet<String> = split_words(name)
                .into_iter()
                .map(|s| s.to_lowercase())
                .collect();
            for token in document {
                *frequency.entry(token).or_default() += 1;
            }
        }
    }
    frequency
        .into_iter()
        .map(|(token, df)| (token, (count as f64 / df as f64).ln()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paper_prefix_suffix_examples() {
        assert!((word_similarity("Dept", "Department") - 0.16).abs() < 1e-12);
        assert!((word_similarity("date", "Birthdate") - 16.0 / 81.0).abs() < 1e-12);
        assert!((prefix_suffix("ColumnType", "Column", None) - 2.0 / 3.0).abs() < 1e-12);
        assert_eq!(
            split_words("XMLParser_order123_EMPLOYEE_ID"),
            ["XML", "Parser", "order", "123", "EMPLOYEE", "ID"]
        );
        assert_eq!(prefix_suffix("", "Column", None), 0.0);
        assert_eq!(
            prefix_suffix("dept_name", "DeptName", None),
            prefix_suffix("DeptName", "dept_name", None)
        );
    }

    #[test]
    fn sql_type_branches_are_shared_and_coefficients_follow_both_policies() {
        let table = |name: &str| Table {
            name: name.into(),
            columns: ["a", "b"]
                .into_iter()
                .map(|name| crate::Field {
                    name: name.into(),
                    data_type: DataType::Integer,
                    samples: Vec::new(),
                })
                .collect(),
        };
        let source = Graph::new(&table("source"));
        let target = Graph::new(&table("target"));
        assert_eq!(
            source
                .edges
                .iter()
                .filter(|edge| edge.label == Label::SqlType)
                .count(),
            2
        );
        let type_targets: BTreeSet<_> = source
            .edges
            .iter()
            .filter(|edge| edge.label == Label::SqlType)
            .map(|edge| edge.to)
            .collect();
        assert_eq!(type_targets.len(), 1);
        let width = target.names.len();
        let s_table = source
            .names
            .iter()
            .position(|name| name == "\0NID1")
            .unwrap();
        let t_table = target
            .names
            .iter()
            .position(|name| name == "\0NID1")
            .unwrap();
        let from = s_table * width + t_table;
        let to = source.columns[0].0 * width + target.columns[0].0;
        for (policy, forward) in [
            (Policy::InverseAverage, 0.5),
            (Policy::InverseProduct, 0.25),
        ] {
            let graph = Propagation::new(&source, &target, policy);
            assert_eq!(
                graph
                    .edges
                    .iter()
                    .find(|edge| edge.0 == from && edge.1 == to)
                    .unwrap()
                    .2,
                forward
            );
            assert_eq!(
                graph
                    .edges
                    .iter()
                    .find(|edge| edge.0 == to && edge.1 == from)
                    .unwrap()
                    .2,
                1.0
            );
        }
    }
    #[test]
    fn indexed_propagation_preserves_edges_and_scores_exactly() {
        let table = |name: &str, fields: &[(&str, DataType)]| Table {
            name: name.into(),
            columns: fields
                .iter()
                .map(|(name, data_type)| crate::Field {
                    name: (*name).into(),
                    data_type: data_type.clone(),
                    samples: Vec::new(),
                })
                .collect(),
        };
        let tables = [
            table("empty", &[]),
            table("one", &[("amount", DataType::Float)]),
            table(
                "orders",
                &[
                    ("order_id", DataType::Integer),
                    ("orderDate", DataType::Date),
                    ("name", DataType::Text),
                ],
            ),
            table(
                "ColumnType",
                &[
                    ("Table", DataType::Integer),
                    ("Column", DataType::Integer),
                    ("ColumnType", DataType::Text),
                    ("varchar", DataType::Float),
                    ("int", DataType::Date),
                    ("float", DataType::Boolean),
                    ("date", DataType::Unknown),
                ],
            ),
            // Literal/structural identifier collisions can create self-loops
            // and reversed edges; their coefficient overwrite order matters.
            table(
                "\0NID1",
                &[
                    ("\0NID2", DataType::Integer),
                    ("\0NID3", DataType::Timestamp),
                    ("varchar", DataType::Text),
                    ("int", DataType::Decimal),
                ],
            ),
            table(
                "注文🙂",
                &[
                    ("生年月日", DataType::Date),
                    ("café", DataType::Text),
                    ("cafe\u{301}", DataType::Text),
                    ("x", DataType::Integer),
                    ("unrelated_customer_identifier", DataType::Text),
                ],
            ),
        ];
        for source in &tables {
            for target in &tables {
                let g1 = Graph::new(source);
                let g2 = Graph::new(target);
                let idf = compute_idf([&g1, &g2].into_iter());
                for policy in [Policy::InverseAverage, Policy::InverseProduct] {
                    let previous = previous_propagation(&g1, &g2, policy);
                    let current = Propagation::new(&g1, &g2, policy);
                    assert_eq!(current.nodes, previous.nodes);
                    let edge_bits = |graph: &Propagation| {
                        graph
                            .edges
                            .iter()
                            .map(|&(from, to, weight)| (from, to, weight.to_bits()))
                            .collect::<Vec<_>>()
                    };
                    assert_eq!(edge_bits(&current), edge_bits(&previous));
                    for formula in [
                        Formula::Basic,
                        Formula::FormulaA,
                        Formula::FormulaB,
                        Formula::FormulaC,
                    ] {
                        for string_matcher in [
                            StringMatcher::PrefixSuffix,
                            StringMatcher::PrefixSuffixTfidf,
                            StringMatcher::Levenshtein,
                        ] {
                            let matcher = SimilarityFlooding {
                                coeff_policy: policy,
                                formula,
                                string_matcher,
                                ..Default::default()
                            };
                            let previous =
                                previous_matches(&matcher, source, target, &g1, &g2, Some(&idf));
                            let current =
                                matcher.match_graphs(source, target, &g1, &g2, Some(&idf));
                            let score_bits = |scores: Vec<(ColumnPair, f64)>| {
                                scores
                                    .into_iter()
                                    .map(|(pair, score)| (pair, score.to_bits()))
                                    .collect::<Vec<_>>()
                            };
                            assert_eq!(
                                score_bits(current),
                                score_bits(previous),
                                "{} / {}: {policy:?}, {formula:?}, {string_matcher:?}",
                                source.name,
                                target.name,
                            );
                        }
                    }
                }
            }
        }
    }

    // The pre-optimization implementation is an oracle for graph construction,
    // coefficient overwrite order, and bit-for-bit final score compatibility.
    fn previous_propagation(g1: &Graph, g2: &Graph, policy: Policy) -> Propagation {
        let width = g2.names.len();
        let mut connectivity: Vec<Edge> = Vec::new();
        let mut nodes = BTreeSet::new();
        let mut insertion_order = Vec::new();
        for e1 in &g1.edges {
            for e2 in &g2.edges {
                if e1.label != e2.label {
                    continue;
                }
                let from = e1.from * width + e2.from;
                let to = e1.to * width + e2.to;
                for node in [from, to] {
                    if nodes.insert(node) {
                        insertion_order.push(node);
                    }
                }
                if let Some(edge) = connectivity
                    .iter_mut()
                    .find(|e| e.from == from && e.to == to)
                {
                    edge.label = e1.label;
                } else {
                    connectivity.push(Edge {
                        from,
                        to,
                        label: e1.label,
                    });
                }
            }
        }
        let mut weights = BTreeMap::new();
        for node in insertion_order {
            for incoming in [true, false] {
                for edge in connectivity.iter().filter(|e| {
                    if incoming {
                        e.to == node
                    } else {
                        e.from == node
                    }
                }) {
                    let coefficient = match policy {
                        Policy::InverseAverage => {
                            2.0 / (g1.degree(node / width, edge.label, incoming)
                                + g2.degree(node % width, edge.label, incoming))
                                as f64
                        }
                        Policy::InverseProduct => {
                            1.0 / connectivity
                                .iter()
                                .filter(|e| {
                                    e.label == edge.label
                                        && if incoming {
                                            e.to == node
                                        } else {
                                            e.from == node
                                        }
                                })
                                .count() as f64
                        }
                    };
                    weights.insert(
                        if incoming {
                            (edge.to, edge.from)
                        } else {
                            (edge.from, edge.to)
                        },
                        coefficient,
                    );
                }
            }
        }
        Propagation {
            nodes,
            edges: weights
                .into_iter()
                .map(|((from, to), weight)| (from, to, weight))
                .collect(),
        }
    }

    // Keep the former Unicode edit-distance calculation as an independent
    // oracle for the shared scorer and the complete propagation output.
    fn levenshtein(a: &str, b: &str) -> f64 {
        let a: Vec<char> = a.chars().collect();
        let b: Vec<char> = b.chars().collect();
        let length = a.len().max(b.len());
        if length == 0 {
            return 1.0;
        }
        let mut previous: Vec<usize> = (0..=b.len()).collect();
        for (i, x) in a.iter().enumerate() {
            let mut next = Vec::with_capacity(b.len() + 1);
            next.push(i + 1);
            for (j, y) in b.iter().enumerate() {
                next.push(
                    (next[j] + 1)
                        .min(previous[j + 1] + 1)
                        .min(previous[j] + usize::from(x != y)),
                );
            }
            previous = next;
        }
        1.0 - previous[b.len()] as f64 / length as f64
    }

    fn previous_matches(
        matcher: &SimilarityFlooding,
        source: &Table,
        target: &Table,
        g1: &Graph,
        g2: &Graph,
        idf: Option<&BTreeMap<String, f64>>,
    ) -> Vec<(ColumnPair, f64)> {
        let width = g2.names.len();
        let initial: Vec<f64> = g1
            .names
            .iter()
            .flat_map(|a| {
                g2.names.iter().map(move |b| {
                    if a.starts_with('\0') || b.starts_with('\0') {
                        return 0.0;
                    }
                    match matcher.string_matcher {
                        StringMatcher::PrefixSuffix => prefix_suffix(a, b, None),
                        StringMatcher::PrefixSuffixTfidf => prefix_suffix(a, b, idf),
                        StringMatcher::Levenshtein => levenshtein(a, b),
                    }
                })
            })
            .collect();
        let propagation = previous_propagation(g1, g2, matcher.coeff_policy);
        let mut previous = initial.clone();
        for _ in 0..matcher.max_iterations {
            let mut next = vec![0.0; initial.len()];
            for &node in &propagation.nodes {
                next[node] = match matcher.formula {
                    Formula::Basic => previous[node],
                    Formula::FormulaA => initial[node],
                    Formula::FormulaB => 0.0,
                    Formula::FormulaC => initial[node] + previous[node],
                };
            }
            for &(from, to, weight) in &propagation.edges {
                next[to] += weight
                    * match matcher.formula {
                        Formula::Basic | Formula::FormulaA => previous[from],
                        Formula::FormulaB | Formula::FormulaC => initial[from] + previous[from],
                    };
            }
            let maximum = next.iter().copied().fold(0.0, f64::max);
            if maximum > 0.0 {
                for value in &mut next {
                    *value /= maximum;
                }
            }
            let residual = previous
                .iter()
                .zip(&next)
                .map(|(a, b)| (a - b).powi(2))
                .sum::<f64>()
                .sqrt();
            previous = next;
            if residual <= matcher.residual_threshold {
                break;
            }
        }
        let mut output = Vec::with_capacity(source.columns.len() * target.columns.len());
        for &(s_node, s_column) in &g1.columns {
            for &(t_node, t_column) in &g2.columns {
                let index = s_node * width + t_node;
                if propagation.nodes.contains(&index) {
                    output.push((
                        ColumnPair::new(
                            &source.name,
                            &source.columns[s_column].name,
                            &target.name,
                            &target.columns[t_column].name,
                        ),
                        previous[index],
                    ));
                }
            }
        }
        output
    }
}
