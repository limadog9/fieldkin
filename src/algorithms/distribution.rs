//! Zhang et al.'s distribution and attribute clustering with quantile EMD.

use good_lp::{Expression, ProblemVariables, Solution, SolverModel, microlp, variable};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};

use super::Matcher;
use crate::{ColumnPair, Error, MatcherResults, Table};

/// Distribution-based matching from *Automatic Discovery of Attributes in
/// Relational Databases* (Zhang et al., SIGMOD 2011).
///
/// Global type-aware value ranks feed equi-depth histograms. A first graph
/// clusters distributions; intersection EMD and integer correlation clustering
/// then identify attributes. Output scores are `1 / (1 + EMD)`.
#[derive(Clone, Debug)]
pub struct DistributionBased {
    pub threshold1: f64,
    pub threshold2: f64,
    pub quantiles: usize,
    /// Parallel workers. Rust uses scoped threads rather than Python processes.
    pub process_num: usize,
    pub use_bloom_filters: bool,
}

impl Default for DistributionBased {
    fn default() -> Self {
        Self {
            threshold1: 0.15,
            threshold2: 0.15,
            quantiles: 256,
            process_num: 1,
            use_bloom_filters: false,
        }
    }
}

impl DistributionBased {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn validate(&self) -> Result<(), Error> {
        for (name, value) in [
            ("threshold1", self.threshold1),
            ("threshold2", self.threshold2),
        ] {
            if !(0.0..=1.0).contains(&value) {
                return Err(Error::InvalidConfig(format!(
                    "{name} must be between 0 and 1"
                )));
            }
        }
        if self.quantiles == 0 {
            return Err(Error::InvalidConfig("quantiles must be >= 1".into()));
        }
        if self.process_num == 0 {
            return Err(Error::InvalidConfig("process_num must be >= 1".into()));
        }
        Ok(())
    }

    fn match_tables(&self, tables: &[&Table]) -> Result<MatcherResults, Error> {
        self.validate()?;
        super::validate_tables(tables.iter().copied())?;
        // Empty strings are missing samples and receive no value rank.
        let raw_corpus: BTreeSet<&str> = tables
            .iter()
            .flat_map(|table| &table.columns)
            .flat_map(|column| column.samples.iter().map(String::as_str))
            .filter(|value| !value.is_empty())
            .collect();
        // Keep numeric aliases until ranks are assigned: upstream first deduplicates
        // raw values, then converts them and overwrites aliases at their final rank.
        let mut values: Vec<Value> = raw_corpus
            .iter()
            .filter_map(|value| Value::parse(value))
            .collect();
        values.sort();
        let ranks: BTreeMap<Value, usize> = values
            .into_iter()
            .enumerate()
            .map(|(i, value)| (value, i + 1))
            .collect();
        let mut columns = Vec::new();
        for (table_index, table) in tables.iter().enumerate() {
            for (column_index, column) in table.columns.iter().enumerate() {
                let data: Vec<(&str, usize)> = column
                    .samples
                    .iter()
                    .filter_map(|value| {
                        Value::parse(value)
                            .and_then(|key| ranks.get(&key).map(|&rank| (value.as_str(), rank)))
                    })
                    .collect();
                if data.is_empty() {
                    continue;
                }
                columns.push(Column::new(
                    table_index,
                    column_index,
                    data,
                    self.quantiles,
                    self.use_bloom_filters,
                ));
            }
        }
        if columns.len() < 2 {
            return MatcherResults::new(Vec::new());
        }
        let all: Vec<usize> = (0..columns.len()).collect();
        let distribution_distances = self.distance_matrix(&columns, &all, false)?;
        let distribution_edges = neighbours(&distribution_distances, self.threshold1);
        let components = connected_components(&distribution_edges);
        let mut matches = Vec::new();
        for mut component in components {
            if component.len() < 2 {
                continue;
            }
            component.sort_by(|&a, &b| {
                let ca = &columns[a];
                let cb = &columns[b];
                (
                    tables[ca.table].name.as_str(),
                    tables[ca.table].columns[ca.column].name.as_str(),
                )
                    .cmp(&(
                        tables[cb.table].name.as_str(),
                        tables[cb.table].columns[cb.column].name.as_str(),
                    ))
            });
            let intersection_distances = self.distance_matrix(&columns, &component, true)?;
            let adjacent = neighbours(&intersection_distances, self.threshold2);
            let n = component.len();
            // Attribute graph is sign(E + E^2), including both direct and
            // two-hop neighbours. Its signs are deliberately directed upstream.
            let mut positive = vec![vec![false; n]; n];
            for i in 0..n {
                for j in 0..n {
                    positive[i][j] = i != j
                        && (adjacent[i][j] || (0..n).any(|k| adjacent[i][k] && adjacent[k][j]));
                }
            }
            for cluster in correlation_clusters(&positive)? {
                for (position, &i) in cluster.iter().enumerate() {
                    for &j in &cluster[position + 1..] {
                        let c1 = &columns[component[i]];
                        let c2 = &columns[component[j]];
                        if c1.table == c2.table {
                            continue;
                        }
                        // Ranking recomputes directional EMD in sorted cluster order.
                        let distance = quantile_emd(&c1.histogram, &c2.ranks);
                        let distance = if distance.is_finite() {
                            format!("{distance:.12}").parse::<f64>().unwrap()
                        } else {
                            distance
                        };
                        let score = 1.0 / (1.0 + distance);
                        let (source, target) = if c1.table < c2.table {
                            (c1, c2)
                        } else {
                            (c2, c1)
                        };
                        matches.push((
                            ColumnPair::new(
                                &tables[source.table].name,
                                &tables[source.table].columns[source.column].name,
                                &tables[target.table].name,
                                &tables[target.table].columns[target.column].name,
                            ),
                            score,
                        ));
                    }
                }
            }
        }
        MatcherResults::new(matches)
    }

    fn distance_matrix(
        &self,
        columns: &[Column<'_>],
        indices: &[usize],
        intersection: bool,
    ) -> Result<Vec<Vec<f64>>, Error> {
        let n = indices.len();
        let distance = |i: usize, j: usize| {
            let a = &columns[indices[i]];
            let b = &columns[indices[j]];
            if intersection {
                intersection_emd(a, b, self.use_bloom_filters)
            } else {
                quantile_emd(&a.histogram, &b.ranks)
            }
        };
        if self.process_num == 1 {
            let mut matrix = vec![vec![f64::INFINITY; n]; n];
            for i in 0..n {
                let (head, tail) = matrix.split_at_mut(i + 1);
                for (offset, other) in tail.iter_mut().enumerate() {
                    let j = i + 1 + offset;
                    let value = distance(i, j);
                    head[i][j] = value;
                    other[i] = value;
                }
            }
            return Ok(matrix);
        }
        let pairs: Vec<(usize, usize)> = (0..n)
            .flat_map(|i| (i + 1..n).map(move |j| (i, j)))
            .collect();
        let workers = self.process_num.min(pairs.len().max(1));
        let outputs = if workers == 1 {
            vec![
                pairs
                    .iter()
                    .map(|&(i, j)| (i, j, distance(i, j)))
                    .collect::<Vec<_>>(),
            ]
        } else {
            std::thread::scope(|scope| {
                let handles: Vec<_> = (0..workers)
                    .map(|worker| {
                        let pairs = &pairs;
                        let distance = &distance;
                        std::thread::Builder::new().spawn_scoped(scope, move || {
                            pairs
                                .iter()
                                .skip(worker)
                                .step_by(workers)
                                .map(|&(i, j)| (i, j, distance(i, j)))
                                .collect::<Vec<_>>()
                        })
                    })
                    .collect::<Result<_, _>>()?;
                handles
                    .into_iter()
                    .map(|handle| {
                        handle
                            .join()
                            .map_err(|_| Error::Algorithm("distribution worker panicked".into()))
                    })
                    .collect::<Result<Vec<_>, _>>()
            })?
        };
        let mut matrix = vec![vec![f64::INFINITY; n]; n];
        for output in outputs {
            for (i, j, value) in output {
                matrix[i][j] = value;
                matrix[j][i] = value;
            }
        }
        Ok(matrix)
    }
}

impl Matcher for DistributionBased {
    fn get_matches(&self, source: &Table, target: &Table) -> Result<MatcherResults, Error> {
        self.match_tables(&[source, target])
    }

    fn get_matches_batch(&self, tables: &[Table]) -> Result<MatcherResults, Error> {
        self.match_tables(&tables.iter().collect::<Vec<_>>())
    }
}

#[derive(Clone, Debug)]
enum Value {
    Number(f64),
    Text(String),
}

impl Value {
    fn parse(raw: &str) -> Option<Self> {
        let trimmed = raw.trim();
        // Python float accepts digit separators, provided they occur between digits.
        let numeric = if trimmed.contains('_')
            && trimmed.as_bytes().iter().enumerate().all(|(i, &c)| {
                c != b'_'
                    || (i > 0
                        && i + 1 < trimmed.len()
                        && trimmed.as_bytes()[i - 1].is_ascii_digit()
                        && trimmed.as_bytes()[i + 1].is_ascii_digit())
            }) {
            trimmed.replace('_', "")
        } else {
            trimmed.into()
        };
        match numeric.parse::<f64>() {
            Ok(number) if number.is_nan() => None,
            Ok(number) => Some(Self::Number(if number == 0.0 { 0.0 } else { number })),
            Err(_) => Some(Self::Text(raw.into())),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}
impl Eq for Value {}
impl PartialOrd for Value {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Value {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (Self::Number(a), Self::Number(b)) => a.total_cmp(b),
            (Self::Text(a), Self::Text(b)) => a.cmp(b),
            (Self::Number(_), Self::Text(_)) => Ordering::Less,
            (Self::Text(_), Self::Number(_)) => Ordering::Greater,
        }
    }
}

struct Column<'a> {
    table: usize,
    column: usize,
    data: Vec<(&'a str, usize)>,
    ranks: Vec<usize>,
    histogram: Histogram,
    bloom: Option<BloomFilter>,
}

impl<'a> Column<'a> {
    fn new(
        table: usize,
        column: usize,
        data: Vec<(&'a str, usize)>,
        quantiles: usize,
        use_bloom: bool,
    ) -> Self {
        let mut ranks: Vec<usize> = data.iter().map(|(_, rank)| *rank).collect();
        ranks.sort_unstable();
        let histogram = Histogram::new(&ranks, quantiles);
        let bloom = use_bloom.then(|| BloomFilter::new(&data));
        Self {
            table,
            column,
            data,
            ranks,
            histogram,
            bloom,
        }
    }
}

struct Histogram {
    minimum: f64,
    upper_bounds: Vec<f64>,
    values: Vec<f64>,
}

impl Histogram {
    fn new(ranks: &[usize], quantiles: usize) -> Self {
        debug_assert!(!ranks.is_empty());
        let mut upper_bounds = if ranks.len() == 1 {
            vec![ranks[0] as f64]
        } else {
            (1..=quantiles)
                .map(|i| {
                    let denominator = quantiles as u128 + 1;
                    let position = (ranks.len() - 1) as u128 * i as u128;
                    let lower = (position / denominator) as usize;
                    let remainder = position % denominator;
                    let numerator = ranks[lower] as u128 * (denominator - remainder)
                        + ranks[(lower + 1).min(ranks.len() - 1)] as u128 * remainder;
                    let boundary = numerator as f64 / denominator as f64;
                    // Formatting rounds the actual binary float to decimal, as
                    // Python round(x, 3) does (multiplying by 1000 can change ties).
                    format!("{boundary:.3}").parse::<f64>().unwrap()
                })
                .collect()
        };
        upper_bounds.dedup();
        let mut histogram = Self {
            minimum: ranks[0] as f64,
            upper_bounds,
            values: Vec::new(),
        };
        histogram.values = histogram.bin(ranks);
        histogram
    }

    fn bin(&self, ranks: &[usize]) -> Vec<f64> {
        let mut values = vec![0.0; self.upper_bounds.len()];
        for &rank in ranks {
            let x = rank as f64;
            if x < self.minimum {
                continue;
            }
            let index = self.upper_bounds.partition_point(|&boundary| boundary < x);
            if index < values.len() {
                values[index] += 1.0;
            }
        }
        let total: f64 = values.iter().sum();
        if total > 0.0 {
            for value in &mut values {
                *value /= total;
            }
        }
        values
    }
}

fn quantile_emd(reference: &Histogram, ranks: &[usize]) -> f64 {
    if ranks.is_empty() {
        return f64::INFINITY;
    }
    let other = reference.bin(ranks);
    if other.iter().sum::<f64>() == 0.0 {
        return f64::INFINITY;
    }
    // The optimal transport cost on an evenly spaced 1-D support equals the
    // integral of the absolute difference of cumulative distributions.
    let mut cumulative = 0.0;
    let mut cost = 0.0;
    for (a, b) in reference.values.iter().zip(other) {
        cumulative += a - b;
        cost += cumulative.abs();
    }
    cost / reference.upper_bounds.len() as f64
}

fn intersection_emd(a: &Column<'_>, b: &Column<'_>, bloom: bool) -> f64 {
    let mut intersection_a;
    let mut intersection_b;
    if bloom {
        intersection_a = a
            .data
            .iter()
            .filter(|(value, _)| b.bloom.as_ref().unwrap().contains(value))
            .map(|(_, rank)| *rank)
            .collect::<Vec<_>>();
        intersection_b = b
            .data
            .iter()
            .filter(|(value, _)| a.bloom.as_ref().unwrap().contains(value))
            .map(|(_, rank)| *rank)
            .collect::<Vec<_>>();
        if intersection_a.is_empty() {
            intersection_a = intersection_b.clone();
        }
        if intersection_b.is_empty() {
            intersection_b = intersection_a.clone();
        }
    } else {
        let a_values: BTreeSet<&str> = a.data.iter().map(|(value, _)| *value).collect();
        let b_values: BTreeSet<&str> = b.data.iter().map(|(value, _)| *value).collect();
        intersection_a = a
            .data
            .iter()
            .filter(|(value, _)| b_values.contains(value))
            .map(|(_, rank)| *rank)
            .collect::<Vec<_>>();
        intersection_b = b
            .data
            .iter()
            .filter(|(value, _)| a_values.contains(value))
            .map(|(_, rank)| *rank)
            .collect::<Vec<_>>();
    }
    (quantile_emd(&a.histogram, &intersection_a) + quantile_emd(&b.histogram, &intersection_b))
        / 2.0
}

fn cutoff(distances: &[f64], threshold: f64) -> f64 {
    let mut values: Vec<f64> = distances.to_vec();
    values.push(threshold);
    values.sort_by(f64::total_cmp);
    let mut cutoff = 0.0;
    let mut largest_gap = 0.0;
    for adjacent in values
        .windows(2)
        .take_while(|window| window[1] <= threshold)
    {
        let gap = adjacent[1] - adjacent[0];
        if gap > largest_gap {
            largest_gap = gap;
            cutoff = adjacent[0];
        }
    }
    cutoff
}

fn neighbours(distances: &[Vec<f64>], threshold: f64) -> Vec<Vec<bool>> {
    distances
        .iter()
        .enumerate()
        .map(|(i, row)| {
            let eligible: Vec<f64> = row
                .iter()
                .enumerate()
                .filter_map(|(j, &value)| (i != j).then_some(value))
                .collect();
            let limit = cutoff(&eligible, threshold);
            row.iter()
                .enumerate()
                .map(|(j, &value)| i != j && value <= limit)
                .collect()
        })
        .collect()
}

fn connected_components(edges: &[Vec<bool>]) -> Vec<Vec<usize>> {
    let mut seen = vec![false; edges.len()];
    let mut result = Vec::new();
    for start in 0..edges.len() {
        if seen[start] {
            continue;
        }
        let mut stack = vec![start];
        seen[start] = true;
        let mut cluster = Vec::new();
        while let Some(node) = stack.pop() {
            cluster.push(node);
            for other in 0..edges.len() {
                if !seen[other] && (edges[node][other] || edges[other][node]) {
                    seen[other] = true;
                    stack.push(other);
                }
            }
        }
        cluster.sort_unstable();
        result.push(cluster);
    }
    result
}

fn correlation_clusters(positive: &[Vec<bool>]) -> Result<Vec<Vec<usize>>, Error> {
    let n = positive.len();
    if (0..n).all(|i| (0..n).all(|j| i == j || positive[i][j])) {
        return Ok(vec![(0..n).collect()]);
    }
    if (0..n).all(|i| (0..n).all(|j| i == j || !positive[i][j])) {
        return Ok((0..n).map(|i| vec![i]).collect());
    }
    let mut variables = ProblemVariables::new();
    let mut x = vec![vec![None; n]; n];
    let mut objective = Expression::from(0.0);
    for i in 0..n {
        for j in 0..n {
            if i == j {
                continue;
            }
            let separation = variables.add(variable().binary());
            x[i][j] = Some(separation);
            if positive[i][j] {
                objective += separation;
            } else {
                objective += 1.0 - separation;
            }
        }
    }
    let mut model = variables.minimise(objective).using(microlp);
    // Preserve the upstream directed binary formulation, including the absence
    // of symmetry constraints. Zero-valued edges are merged as undirected edges.
    for u in 0..n {
        for v in 0..n {
            if u == v {
                continue;
            }
            for w in 0..n {
                if w != u && w != v {
                    model.add_constraint(
                        (x[u][w].unwrap() - x[u][v].unwrap() - x[v][w].unwrap()).leq(0.0),
                    );
                }
            }
        }
    }
    let solution = model
        .solve()
        .map_err(|error| Error::Algorithm(format!("correlation clustering failed: {error}")))?;
    let mut joined = vec![vec![false; n]; n];
    for i in 0..n {
        for j in 0..n {
            if let Some(value) = x[i][j] {
                joined[i][j] = solution.value(value) < 0.5;
            }
        }
    }
    Ok(connected_components(&joined))
}

struct BloomFilter {
    bits: Vec<bool>,
    hashes: usize,
}

impl BloomFilter {
    fn new(data: &[(&str, usize)]) -> Self {
        let n = data.len().max(1) as f64;
        let size = (-n * 0.01f64.ln() / std::f64::consts::LN_2.powi(2)) as usize;
        let mut filter = Self {
            bits: vec![false; size.max(1)],
            hashes: ((size as f64 / n) * std::f64::consts::LN_2)
                .floor()
                .max(1.0) as usize,
        };
        for (value, _) in data {
            for seed in 0..filter.hashes {
                let index = filter.index(value, seed);
                filter.bits[index] = true;
            }
        }
        filter
    }

    fn index(&self, value: &str, seed: usize) -> usize {
        let digest = Sha256::digest(format!("{seed}:{value}").as_bytes());
        (u64::from_be_bytes(digest[..8].try_into().unwrap()) % self.bits.len() as u64) as usize
    }

    fn contains(&self, value: &str) -> bool {
        (0..self.hashes).all(|seed| self.bits[self.index(value, seed)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inclusive_quantile_histogram_and_transport() {
        let histogram = Histogram::new(&[1, 2, 3, 4, 5], 4);
        assert_eq!(histogram.upper_bounds, [1.8, 2.6, 3.4, 4.2]);
        assert_eq!(histogram.values, [0.25; 4]);
        assert_eq!(quantile_emd(&histogram, &[1, 2, 3, 4, 5]), 0.0);
        assert_eq!(quantile_emd(&histogram, &[1, 1]), 0.375);
        assert!(quantile_emd(&histogram, &[20]).is_infinite());
        let constant = Histogram::new(&[7, 7, 7], 256);
        assert_eq!(constant.upper_bounds, [7.0]);
        assert_eq!(quantile_emd(&constant, &[7]), 0.0);
    }

    #[test]
    fn conservative_cutoff_selects_largest_gap() {
        assert_eq!(cutoff(&[0.01, 0.02, 0.14, 0.8], 0.15), 0.02);
        assert_eq!(cutoff(&[0.0, 0.0], 0.15), 0.0);
        assert_eq!(cutoff(&[0.6, f64::INFINITY], 0.15), 0.0);
    }

    #[test]
    fn bloom_hashes_match_upstream_and_have_no_false_negatives() {
        let filter = BloomFilter::new(&[("alpha", 1), ("beta", 2), ("gamma", 3)]);
        assert_eq!(filter.bits.len(), 28);
        assert_eq!(filter.hashes, 6);
        for value in ["alpha", "beta", "gamma"] {
            assert!(filter.contains(value));
        }
    }

    #[test]
    fn directed_integer_clustering_recovers_unique_zero_cost_partition() {
        let graph = vec![
            vec![false, true, false, false],
            vec![true, false, false, false],
            vec![false, false, false, true],
            vec![false, false, true, false],
        ];
        assert_eq!(
            correlation_clusters(&graph).unwrap(),
            [vec![0, 1], vec![2, 3]]
        );
        assert_eq!(
            correlation_clusters(&[vec![false, true], vec![true, false]]).unwrap(),
            [vec![0, 1]]
        );
    }

    #[test]
    fn sequential_and_parallel_distances_preserve_collected_pair_bits() {
        for bloom in [false, true] {
            let data = [
                vec![("1", 2), ("1.0", 2), ("2", 3), ("2", 3), ("10", 4)],
                vec![("1.0", 2), ("2", 3), ("10", 4), ("10", 4)],
                vec![("other", 5), ("text", 6)],
                vec![("same", 7), ("same", 7)],
                vec![("same", 7)],
            ];
            let columns: Vec<_> = data
                .into_iter()
                .enumerate()
                .map(|(i, data)| Column::new(i / 3, i, data, 4, bloom))
                .collect();
            for indices in [vec![], vec![0], vec![2, 0, 4, 3, 1]] {
                for intersection in [false, true] {
                    // Keep the former pair/output collection path as an exact
                    // comparison, including asymmetric EMD and infinite entries.
                    let pairs: Vec<_> = (0..indices.len())
                        .flat_map(|i| (i + 1..indices.len()).map(move |j| (i, j)))
                        .collect();
                    let outputs: Vec<_> = pairs
                        .iter()
                        .map(|&(i, j)| {
                            let a = &columns[indices[i]];
                            let b = &columns[indices[j]];
                            let value = if intersection {
                                intersection_emd(a, b, bloom)
                            } else {
                                quantile_emd(&a.histogram, &b.ranks)
                            };
                            (i, j, value.to_bits())
                        })
                        .collect();
                    let mut expected =
                        vec![vec![f64::INFINITY.to_bits(); indices.len()]; indices.len()];
                    for (i, j, value) in outputs {
                        expected[i][j] = value;
                        expected[j][i] = value;
                    }
                    for process_num in [1, 3] {
                        let matcher = DistributionBased {
                            process_num,
                            use_bloom_filters: bloom,
                            ..DistributionBased::default()
                        };
                        let actual = matcher
                            .distance_matrix(&columns, &indices, intersection)
                            .unwrap();
                        let actual: Vec<Vec<_>> = actual
                            .into_iter()
                            .map(|row| row.into_iter().map(f64::to_bits).collect())
                            .collect();
                        assert_eq!(actual, expected);
                    }
                }
            }
        }
    }

    #[test]
    fn sequential_and_parallel_matching_preserve_score_bits_and_details() {
        let tables = [
            Table::from_csv(
                "source",
                "number,constant,empty\n1,same,\n1.0,same,\n2,same,\n10,same,\nNaN,same,\n".as_bytes(),
            )
            .unwrap(),
            Table::from_csv(
                "target",
                "number_alias,constant,disjoint\n1.0,same,other\n2,same,text\n10,same,other\n10,same,text\n".as_bytes(),
            )
            .unwrap(),
        ];
        for bloom in [false, true] {
            let matcher = DistributionBased {
                use_bloom_filters: bloom,
                ..DistributionBased::default()
            };
            let sequential = matcher.get_matches_batch(&tables).unwrap();
            let parallel = DistributionBased {
                process_num: 3,
                ..matcher
            }
            .get_matches_batch(&tables)
            .unwrap();
            let bits = |results: &MatcherResults| {
                results
                    .iter()
                    .map(|(pair, score)| (pair.clone(), score.to_bits()))
                    .collect::<Vec<_>>()
            };
            assert!(!sequential.is_empty());
            assert_eq!(bits(&sequential), bits(&parallel));
            assert_eq!(sequential.details(), parallel.details());
        }
    }
}
