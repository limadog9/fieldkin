use crate::Error;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet, btree_map::Entry},
    ops::Index,
};

/// Table-aware identity of a correspondence. Every field is part of the key.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ColumnPair {
    pub source_table: String,
    pub source_column: String,
    pub target_table: String,
    pub target_column: String,
}
impl ColumnPair {
    pub fn new(
        st: impl Into<String>,
        sc: impl Into<String>,
        tt: impl Into<String>,
        tc: impl Into<String>,
    ) -> Self {
        Self {
            source_table: st.into(),
            source_column: sc.into(),
            target_table: tt.into(),
            target_column: tc.into(),
        }
    }
    pub fn source(&self) -> (&str, &str) {
        (&self.source_table, &self.source_column)
    }
    pub fn target(&self) -> (&str, &str) {
        (&self.target_table, &self.target_column)
    }
}
pub type MatchDetails = BTreeMap<ColumnPair, BTreeMap<String, f64>>;

/// Immutable scores sorted descending, with deterministic column-key tie breaking.
/// Transformations produce a new collection and retain only relevant details.
/// One-to-one selectors operate globally across this collection, using separate
/// source and target identities of (table, column), rather than per table pair.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct MatcherResults {
    entries: Vec<(ColumnPair, f64)>,
    details: MatchDetails,
}
impl Serialize for MatcherResults {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        #[derive(Serialize)]
        struct Entry<'a> {
            pair: &'a ColumnPair,
            score: f64,
            #[serde(skip_serializing_if = "Option::is_none")]
            details: Option<&'a BTreeMap<String, f64>>,
        }
        let entries: Vec<_> = self
            .iter()
            .map(|(pair, score)| Entry {
                pair,
                score: *score,
                details: self.get_details(pair),
            })
            .collect();
        let mut state = serializer.serialize_struct("MatcherResults", 1)?;
        state.serialize_field("matches", &entries)?;
        state.end()
    }
}
impl MatcherResults {
    /// Build results from unique column pairs with finite scores in [0, 1].
    /// Duplicate pairs are rejected, including when their scores are equal.
    pub fn new(entries: Vec<(ColumnPair, f64)>) -> Result<Self, Error> {
        Self::with_details(entries, MatchDetails::new())
    }
    /// Build validated results, retaining details only for the supplied pairs.
    /// Duplicate pairs, invalid scores, and nonfinite retained details are rejected.
    pub fn with_details(
        entries: Vec<(ColumnPair, f64)>,
        mut details: MatchDetails,
    ) -> Result<Self, Error> {
        let mut map = BTreeMap::new();
        for (pair, score) in entries {
            if !score.is_finite() || !(0.0..=1.0).contains(&score) {
                return Err(Error::InvalidInput(format!(
                    "similarity must be finite and in [0, 1], got {score}"
                )));
            }
            match map.entry(pair) {
                Entry::Vacant(entry) => {
                    entry.insert(score);
                }
                Entry::Occupied(entry) => {
                    return Err(Error::InvalidInput(format!(
                        "duplicate match identifier: {:?}",
                        entry.key()
                    )));
                }
            }
        }
        details.retain(|key, _| map.contains_key(key));
        if details.values().any(|v| v.values().any(|s| !s.is_finite())) {
            return Err(Error::InvalidInput("match details must be finite".into()));
        }
        let mut entries: Vec<_> = map.into_iter().collect();
        entries.sort_by(|(a, x), (b, y)| y.total_cmp(x).then_with(|| a.cmp(b)));
        Ok(Self { entries, details })
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn iter(&self) -> impl ExactSizeIterator<Item = (&ColumnPair, &f64)> {
        self.entries.iter().map(|(p, s)| (p, s))
    }
    pub fn get(&self, pair: &ColumnPair) -> Option<f64> {
        self.entries
            .iter()
            .find(|(p, _)| p == pair)
            .map(|(_, s)| *s)
    }
    pub fn details(&self) -> &MatchDetails {
        &self.details
    }
    pub fn get_details(&self, pair: &ColumnPair) -> Option<&BTreeMap<String, f64>> {
        self.details.get(pair)
    }
    pub fn get_copy(&self) -> Self {
        self.clone()
    }
    // Callers supply a subsequence of the validated, sorted entries.
    fn subset<'a>(&self, entries: impl Iterator<Item = &'a (ColumnPair, f64)>) -> Self {
        let entries: Vec<_> = entries.cloned().collect();
        let details = entries
            .iter()
            .filter_map(|(pair, _)| {
                self.details
                    .get(pair)
                    .map(|details| (pair.clone(), details.clone()))
            })
            .collect();
        Self { entries, details }
    }
    fn select(&self, mut keep: impl FnMut(&(ColumnPair, f64)) -> bool) -> Self {
        self.subset(self.entries.iter().filter(|entry| keep(entry)))
    }
    pub fn filter(&self, min_score: f64) -> Result<Self, Error> {
        finite_threshold(min_score)?;
        let end = self
            .entries
            .partition_point(|(_, score)| *score >= min_score);
        Ok(self.subset(self.entries[..end].iter()))
    }
    pub fn take_top_n(&self, n: usize) -> Self {
        self.subset(self.entries[..n.min(self.len())].iter())
    }
    pub fn take_top_percent(&self, percent: f64) -> Result<Self, Error> {
        if !percent.is_finite() || !(0.0..=100.0).contains(&percent) {
            return Err(Error::InvalidConfig(
                "percent must be finite and in [0, 100]".into(),
            ));
        }
        Ok(self.take_top_n((percent * self.len() as f64 / 100.0).ceil() as usize))
    }
    pub fn take_top_n_per_source(&self, n: usize) -> Self {
        let mut counts = BTreeMap::new();
        self.select(|(p, _)| {
            let count = counts
                .entry((p.source_table.clone(), p.source_column.clone()))
                .or_insert(0usize);
            let keep = *count < n;
            *count += 1;
            keep
        })
    }

    /// Maximum-total-score assignment, followed by threshold filtering.
    /// `None` uses Valentine's distinct-score cutoff: descending unique scores
    /// indexed at ceil(count/2), clamped to the last index. Equal scores still
    /// produce a true one-to-one assignment, fixing the upstream tie shortcut.
    pub fn one_to_one_hungarian(&self, threshold: Option<f64>) -> Result<Self, Error> {
        let threshold = self.selection_threshold(threshold)?;
        Ok(self.hungarian_selection(threshold, false))
    }

    /// Select the most one-to-one matches at or above the threshold, then
    /// maximize total similarity among assignments with that many matches.
    /// `None` uses the same collection-wide distinct-score cutoff as
    /// [`Self::one_to_one_hungarian`]. Finite thresholds outside [0, 1] are allowed.
    /// Sorted table/column identities and the shared solver break ties deterministically.
    pub fn one_to_one_hungarian_threshold_aware(
        &self,
        threshold: Option<f64>,
    ) -> Result<Self, Error> {
        let threshold = self.selection_threshold(threshold)?;
        Ok(self.hungarian_selection(threshold, true))
    }

    fn hungarian_selection(&self, threshold: f64, cardinality_first: bool) -> Self {
        if self.is_empty() {
            return self.clone();
        }
        let (first, highest) = &self.entries[0];
        if *highest < threshold {
            return Self::default();
        }
        if self
            .entries
            .iter()
            .all(|(pair, _)| pair.source() == first.source())
        {
            // With one real row, every dummy-row augmentation has delta zero:
            // strict comparisons preserve the first numeric maximum, including
            // signed-zero ties. Multiple real rows do not share this guarantee.
            let best = self
                .entries
                .iter()
                .take_while(|(_, score)| score == highest)
                .min_by(|(a, _), (b, _)| a.cmp(b))
                .expect("nonempty results");
            return self.subset(std::iter::once(best));
        }
        let sources: Vec<_> = self
            .entries
            .iter()
            .map(|(p, _)| p.source())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let targets: Vec<_> = self
            .entries
            .iter()
            .map(|(p, _)| p.target())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        // Compare cardinality before similarity without rounding away score
        // differences by adding a large floating-point bonus. Padded cells are
        // read as zero by the solver instead of occupying square storage.
        let mut costs = assignment_costs(sources.len(), targets.len());
        let mut lookup = BTreeMap::new();
        for (pair, score) in &self.entries {
            if cardinality_first && *score < threshold {
                continue;
            }
            let row = sources
                .binary_search(&pair.source())
                .expect("source indexed");
            let col = targets
                .binary_search(&pair.target())
                .expect("target indexed");
            costs[row][col] = (if cardinality_first { -1 } else { 0 }, -*score);
            lookup.insert((row, col), pair);
        }
        let assignment = hungarian(&costs);
        let chosen: BTreeSet<_> = assignment
            .into_iter()
            .enumerate()
            .filter_map(|(row, col)| lookup.get(&(row, col)).copied())
            .collect();
        self.select(|(p, s)| chosen.contains(p) && *s >= threshold)
    }
    pub fn one_to_one_greedy(&self, threshold: Option<f64>) -> Result<Self, Error> {
        let threshold = self.selection_threshold(threshold)?;
        let mut sources = BTreeSet::new();
        let mut targets = BTreeSet::new();
        Ok(self.select(|(p, s)| {
            let source = (p.source_table.clone(), p.source_column.clone());
            let target = (p.target_table.clone(), p.target_column.clone());
            if *s < threshold || sources.contains(&source) || targets.contains(&target) {
                return false;
            }
            sources.insert(source);
            targets.insert(target);
            true
        }))
    }
    pub fn one_to_one_mutual_top(&self, n: usize) -> Result<Self, Error> {
        if n == 0 {
            return Err(Error::InvalidConfig(
                "mutual top n must be at least 1".into(),
            ));
        }
        let mut source_counts = BTreeMap::new();
        let mut target_counts = BTreeMap::new();
        Ok(self.select(|(p, _)| {
            let source_count = source_counts
                .entry((p.source_table.clone(), p.source_column.clone()))
                .or_insert(0usize);
            let target_count = target_counts
                .entry((p.target_table.clone(), p.target_column.clone()))
                .or_insert(0usize);
            let keep = *source_count < n && *target_count < n;
            *source_count += 1;
            *target_count += 1;
            keep
        }))
    }
    fn selection_threshold(&self, threshold: Option<f64>) -> Result<f64, Error> {
        if let Some(t) = threshold {
            finite_threshold(t)?;
            return Ok(t);
        }
        let mut scores: Vec<_> = self.entries.iter().map(|(_, s)| *s).collect();
        scores.dedup();
        if scores.is_empty() {
            return Ok(0.0);
        }
        Ok(scores[scores.len().div_ceil(2).min(scores.len() - 1)])
    }
    pub fn get_metrics(
        &self,
        truth: &crate::metrics::GroundTruth,
    ) -> Result<BTreeMap<String, f64>, Error> {
        crate::metrics::get_metrics(self, truth)
    }
    /// Compute metrics with unique names. Use [`crate::metrics::NamedMetric`]
    /// when evaluating multiple configurations of the same metric.
    /// Duplicate names return an error instead of overwriting a measurement.
    pub fn get_metrics_with(
        &self,
        truth: &crate::metrics::GroundTruth,
        metrics: &[&dyn crate::metrics::Metric],
        method: crate::metrics::OneToOneMethod,
    ) -> Result<BTreeMap<String, f64>, Error> {
        let mut values = BTreeMap::new();
        for metric in metrics {
            match values.entry(metric.name()) {
                Entry::Vacant(entry) => {
                    entry.insert(metric.apply(self, truth, method)?);
                }
                Entry::Occupied(entry) => {
                    return Err(Error::InvalidConfig(format!(
                        "duplicate metric name {:?}; use NamedMetric to give each metric a distinct name",
                        entry.key()
                    )));
                }
            }
        }
        Ok(values)
    }
}
impl Index<&ColumnPair> for MatcherResults {
    type Output = f64;
    fn index(&self, key: &ColumnPair) -> &f64 {
        &self
            .entries
            .iter()
            .find(|(p, _)| p == key)
            .expect("column pair not found")
            .1
    }
}
impl<'a> IntoIterator for &'a MatcherResults {
    type Item = (&'a ColumnPair, &'a f64);
    type IntoIter = std::iter::Map<
        std::slice::Iter<'a, (ColumnPair, f64)>,
        fn(&'a (ColumnPair, f64)) -> Self::Item,
    >;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.iter().map(|(p, s)| (p, s))
    }
}
fn finite_threshold(t: f64) -> Result<(), Error> {
    if !t.is_finite() {
        Err(Error::InvalidConfig("threshold must be finite".into()))
    } else {
        Ok(())
    }
}

/// Shortest augmenting path formulation of the Hungarian algorithm, O(n^3).
/// Costs compare lexicographically: negative cardinality, then negative similarity.
/// The original selector uses zero cardinality costs throughout.
fn hungarian(cost: &[Vec<(isize, f64)>]) -> Vec<usize> {
    // Keep dummy-row traversal: omitting it can change floating-point tie choices.
    let n = cost.len().max(cost.first().map_or(0, Vec::len));
    let mut u = vec![(0isize, 0.0); n + 1];
    let mut v = vec![(0isize, 0.0); n + 1];
    let mut p = vec![0; n + 1];
    let mut way = vec![0; n + 1];
    for i in 1..=n {
        p[0] = i;
        let mut j0 = 0;
        let mut minv = vec![(isize::MAX, f64::INFINITY); n + 1];
        let mut used = vec![false; n + 1];
        loop {
            used[j0] = true;
            let i0 = p[j0];
            let mut delta = (isize::MAX, f64::INFINITY);
            let mut j1 = 0;
            for j in 1..=n {
                if !used[j] {
                    let (count, score) = cost
                        .get(i0 - 1)
                        .and_then(|row| row.get(j - 1))
                        .copied()
                        .unwrap_or((0, 0.0));
                    let cur = (count - u[i0].0 - v[j].0, score - u[i0].1 - v[j].1);
                    if cur < minv[j] {
                        minv[j] = cur;
                        way[j] = j0;
                    }
                    if minv[j] < delta {
                        delta = minv[j];
                        j1 = j;
                    }
                }
            }
            for j in 0..=n {
                if used[j] {
                    u[p[j]].0 += delta.0;
                    u[p[j]].1 += delta.1;
                    v[j].0 -= delta.0;
                    v[j].1 -= delta.1;
                } else {
                    minv[j].0 -= delta.0;
                    minv[j].1 -= delta.1;
                }
            }
            j0 = j1;
            if p[j0] == 0 {
                break;
            }
        }
        loop {
            let j1 = way[j0];
            p[j0] = p[j1];
            j0 = j1;
            if j0 == 0 {
                break;
            }
        }
    }
    let mut assignment = vec![0; n];
    for j in 1..=n {
        if p[j] > 0 {
            assignment[p[j] - 1] = j - 1;
        }
    }
    assignment
}

fn assignment_costs(rows: usize, cols: usize) -> Vec<Vec<(isize, f64)>> {
    vec![vec![(0, 0.0); cols]; rows]
}

#[cfg(test)]
mod assignment_storage_tests {
    use super::assignment_costs;

    #[test]
    fn rectangular_cost_storage_contains_only_real_cells() {
        for (rows, cols) in [(1, 10_000), (10_000, 1), (8, 10_000)] {
            let costs = assignment_costs(rows, cols);
            assert_eq!(costs.len(), rows);
            assert!(costs.iter().all(|row| row.len() == cols));
            assert_eq!(costs.iter().map(Vec::len).sum::<usize>(), rows * cols);
            assert_eq!(costs.iter().map(Vec::capacity).sum::<usize>(), rows * cols);
        }
    }
}
