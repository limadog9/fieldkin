use crate::Error;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
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
    pub fn new(entries: Vec<(ColumnPair, f64)>) -> Result<Self, Error> {
        Self::with_details(entries, MatchDetails::new())
    }
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
            map.insert(pair, score);
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
    fn select(&self, mut keep: impl FnMut(&(ColumnPair, f64)) -> bool) -> Self {
        // Entries and details have already passed validation.
        Self::with_details(
            self.entries
                .iter()
                .filter(|entry| keep(entry))
                .cloned()
                .collect(),
            self.details.clone(),
        )
        .expect("validated result subset")
    }
    pub fn filter(&self, min_score: f64) -> Result<Self, Error> {
        finite_threshold(min_score)?;
        Ok(self.select(|(_, s)| *s >= min_score))
    }
    pub fn take_top_n(&self, n: usize) -> Self {
        let mut left = n;
        self.select(|_| {
            let keep = left > 0;
            left = left.saturating_sub(1);
            keep
        })
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
        if self.is_empty() {
            return Ok(self.clone());
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
        let n = sources.len().max(targets.len());
        let mut costs = vec![vec![0.0; n]; n];
        let mut lookup = BTreeMap::new();
        for (pair, score) in &self.entries {
            let row = sources
                .binary_search(&pair.source())
                .expect("source indexed");
            let col = targets
                .binary_search(&pair.target())
                .expect("target indexed");
            costs[row][col] = -*score;
            lookup.insert((row, col), pair);
        }
        let assignment = hungarian(&costs);
        let chosen: BTreeSet<_> = assignment
            .into_iter()
            .enumerate()
            .filter_map(|(row, col)| lookup.get(&(row, col)).copied())
            .collect();
        Ok(self.select(|(p, s)| chosen.contains(p) && *s >= threshold))
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
    pub fn get_metrics_with(
        &self,
        truth: &crate::metrics::GroundTruth,
        metrics: &[&dyn crate::metrics::Metric],
        method: crate::metrics::OneToOneMethod,
    ) -> Result<BTreeMap<String, f64>, Error> {
        metrics
            .iter()
            .map(|m| m.apply(self, truth, method).map(|value| (m.name(), value)))
            .collect()
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
fn hungarian(cost: &[Vec<f64>]) -> Vec<usize> {
    let n = cost.len();
    let mut u = vec![0.0; n + 1];
    let mut v = vec![0.0; n + 1];
    let mut p = vec![0; n + 1];
    let mut way = vec![0; n + 1];
    for i in 1..=n {
        p[0] = i;
        let mut j0 = 0;
        let mut minv = vec![f64::INFINITY; n + 1];
        let mut used = vec![false; n + 1];
        loop {
            used[j0] = true;
            let i0 = p[j0];
            let mut delta = f64::INFINITY;
            let mut j1 = 0;
            for j in 1..=n {
                if !used[j] {
                    let cur = cost[i0 - 1][j - 1] - u[i0] - v[j];
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
                    u[p[j]] += delta;
                    v[j] -= delta;
                } else {
                    minv[j] -= delta;
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
