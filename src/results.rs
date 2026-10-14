use crate::{Error, WorkLimits, limits::check_work};
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
        self.hungarian_selection(threshold, false, None)
    }

    /// Legacy assignment with opt-in dimension and real cost-cell limits.
    /// Limits include all candidate identities, even when a shortcut could apply.
    /// They bound work counts, not solver time or process memory.
    pub fn one_to_one_hungarian_with_limits(
        &self,
        threshold: Option<f64>,
        limits: &WorkLimits,
    ) -> Result<Self, Error> {
        let threshold = self.selection_threshold(threshold)?;
        self.hungarian_selection(threshold, false, Some(limits))
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
        self.hungarian_selection(threshold, true, None)
    }

    /// Threshold-aware assignment with the same work limits as
    /// [`Self::one_to_one_hungarian_with_limits`]. The cardinality-first objective
    /// and existing tie behavior are preserved for accepted calls.
    pub fn one_to_one_hungarian_threshold_aware_with_limits(
        &self,
        threshold: Option<f64>,
        limits: &WorkLimits,
    ) -> Result<Self, Error> {
        let threshold = self.selection_threshold(threshold)?;
        self.hungarian_selection(threshold, true, Some(limits))
    }

    /// Maximum-total-score rectangular assignment, followed by threshold filtering.
    /// `None` uses the collection-wide distinct-score cutoff documented by
    /// [`Self::one_to_one_hungarian`]. Finite thresholds outside [0, 1] are allowed.
    ///
    /// This opt-in solver augments the smaller identity set. Full (table, column)
    /// identities are sorted, with sources augmented on square problems; equal
    /// reduced costs retain the first predecessor and first larger-side identity.
    /// Results are deterministic, including for permuted input. Equally optimal
    /// assignments can differ from the legacy Hungarian selector, and threshold
    /// filtering can consequently retain different pairs or a different count.
    ///
    /// For `r` source and `c` target identities, the solver uses O(min(r,c)^2 *
    /// max(r,c)) work, O(r*c) cost storage, and O(r+c) workspace. Preparing sorted
    /// identities and locating candidates additionally costs O(k log k + r*c)
    /// for `k` input correspondences. No dummy rows are augmented.
    pub fn one_to_one_rectangular(&self, threshold: Option<f64>) -> Result<Self, Error> {
        let threshold = self.selection_threshold(threshold)?;
        self.rectangular_selection(threshold, false, None)
    }

    /// Rectangular maximum-score assignment with opt-in dimension and real
    /// cost-cell limits. Every input candidate identity counts, including those
    /// below the threshold, before cost allocation. Limits are per assignment,
    /// not cumulative budgets, process-memory limits, or solver deadlines.
    pub fn one_to_one_rectangular_with_limits(
        &self,
        threshold: Option<f64>,
        limits: &WorkLimits,
    ) -> Result<Self, Error> {
        let threshold = self.selection_threshold(threshold)?;
        self.rectangular_selection(threshold, false, Some(limits))
    }

    /// Rectangular assignment maximizing the number of real pairs at or above
    /// the threshold, then their total similarity. Uses the same threshold
    /// resolution, complexity, and deterministic tie policy as
    /// [`Self::one_to_one_rectangular`]. Existing Hungarian selectors are unchanged.
    pub fn one_to_one_rectangular_threshold_aware(
        &self,
        threshold: Option<f64>,
    ) -> Result<Self, Error> {
        let threshold = self.selection_threshold(threshold)?;
        self.rectangular_selection(threshold, true, None)
    }

    /// Cardinality-first rectangular assignment with the per-assignment limits
    /// documented by [`Self::one_to_one_rectangular_with_limits`].
    pub fn one_to_one_rectangular_threshold_aware_with_limits(
        &self,
        threshold: Option<f64>,
        limits: &WorkLimits,
    ) -> Result<Self, Error> {
        let threshold = self.selection_threshold(threshold)?;
        self.rectangular_selection(threshold, true, Some(limits))
    }

    fn rectangular_selection(
        &self,
        threshold: f64,
        cardinality_first: bool,
        limits: Option<&WorkLimits>,
    ) -> Result<Self, Error> {
        let [sources, targets] = self.assignment_identities();
        // Guard the original dimensions, even when every candidate is filtered
        // out, and check storage arithmetic for unguarded calls as well.
        check_rectangular_work(
            sources.len(),
            targets.len(),
            limits.unwrap_or(&WorkLimits::default()),
        )?;
        if self.is_empty() || self.entries[0].1 < threshold {
            return Ok(Self::default());
        }
        let transposed = sources.len() > targets.len();
        let (rows, cols) = if transposed {
            (targets.len(), sources.len())
        } else {
            (sources.len(), targets.len())
        };
        let mut costs = assignment_costs(rows, cols);
        for (pair, score) in &self.entries {
            if cardinality_first && *score < threshold {
                continue;
            }
            let source = sources
                .binary_search(&pair.source())
                .expect("source indexed");
            let target = targets
                .binary_search(&pair.target())
                .expect("target indexed");
            let (row, col) = if transposed {
                (target, source)
            } else {
                (source, target)
            };
            costs[row][col] = (if cardinality_first { -1 } else { 0 }, -*score);
        }
        let assignment = rectangular_assignment(&costs);
        // Select original entries rather than materializing neutral completion
        // slots. This also retains score/detail bits and the ranked order.
        Ok(self.select(|(pair, score)| {
            if *score < threshold {
                return false;
            }
            let source = sources
                .binary_search(&pair.source())
                .expect("source indexed");
            let target = targets
                .binary_search(&pair.target())
                .expect("target indexed");
            let (row, col) = if transposed {
                (target, source)
            } else {
                (source, target)
            };
            assignment[row] == col
        }))
    }

    fn hungarian_selection(
        &self,
        threshold: f64,
        cardinality_first: bool,
        limits: Option<&WorkLimits>,
    ) -> Result<Self, Error> {
        let indexed = if let Some(limits) = limits {
            let [sources, targets] = self.assignment_identities();
            check_assignment_work(sources.len(), targets.len(), limits)?;
            Some([sources, targets])
        } else {
            None
        };
        if self.is_empty() {
            return Ok(self.clone());
        }
        let (first, highest) = &self.entries[0];
        if *highest < threshold {
            return Ok(Self::default());
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
            return Ok(self.subset(std::iter::once(best)));
        }
        if self
            .entries
            .iter()
            .all(|(pair, score)| pair.target() == first.target() && score == highest)
        {
            // With one target and equal numeric costs, the first sorted source
            // keeps that target: later augmentations have equal reduced costs
            // and strict comparisons leave its predecessor unchanged. Varied
            // costs cannot use this shortcut, even when maxima are tied.
            let best = self
                .entries
                .iter()
                .min_by(|(a, _), (b, _)| a.cmp(b))
                .expect("nonempty results");
            return Ok(self.subset(std::iter::once(best)));
        }
        let [sources, targets] = indexed.unwrap_or_else(|| self.assignment_identities());
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
        Ok(self.select(|(p, s)| chosen.contains(p) && *s >= threshold))
    }

    fn assignment_identities(&self) -> [Vec<(&str, &str)>; 2] {
        let sources = self
            .entries
            .iter()
            .map(|(p, _)| p.source())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let targets = self
            .entries
            .iter()
            .map(|(p, _)| p.target())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        [sources, targets]
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
            let row = cost.get(i0 - 1).map_or(&[][..], Vec::as_slice);
            let mut delta = (isize::MAX, f64::INFINITY);
            let mut j1 = 0;
            for j in 1..=n {
                if !used[j] {
                    let (count, score) = row.get(j - 1).copied().unwrap_or((0, 0.0));
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

/// Rectangular shortest augmenting paths: every row is augmented, and there
/// are at least as many columns as rows. Neutral cells complete sparse inputs.
/// The cardinality coordinate is compared independently of the score coordinate.
fn rectangular_assignment(cost: &[Vec<(isize, f64)>]) -> Vec<usize> {
    let rows = cost.len();
    let cols = cost.first().map_or(0, Vec::len);
    debug_assert!(rows <= cols);
    let mut u = vec![(0isize, AssignmentScore::from(0.0)); rows + 1];
    let mut v = vec![(0isize, AssignmentScore::from(0.0)); cols + 1];
    let mut p = vec![0; cols + 1];
    let mut way = vec![0; cols + 1];
    let mut minv = vec![(isize::MAX, AssignmentScore::from(f64::INFINITY)); cols + 1];
    let mut used = vec![false; cols + 1];
    for i in 1..=rows {
        #[cfg(test)]
        assignment_storage_tests::RECTANGULAR_AUGMENTATIONS
            .with(|count| count.set(count.get() + 1));
        p[0] = i;
        let mut j0 = 0;
        minv.fill((isize::MAX, AssignmentScore::from(f64::INFINITY)));
        used.fill(false);
        loop {
            used[j0] = true;
            let i0 = p[j0];
            let mut delta = (isize::MAX, AssignmentScore::from(f64::INFINITY));
            let mut j1 = 0;
            for j in 1..=cols {
                if !used[j] {
                    let (count, score) = cost[i0 - 1][j - 1];
                    let cur = (
                        count - u[i0].0 - v[j].0,
                        AssignmentScore::from(score).sub(u[i0].1).sub(v[j].1),
                    );
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
            for j in 0..=cols {
                if used[j] {
                    u[p[j]].0 += delta.0;
                    u[p[j]].1 = u[p[j]].1.add(delta.1);
                    v[j].0 -= delta.0;
                    v[j].1 = v[j].1.sub(delta.1);
                } else {
                    minv[j].0 -= delta.0;
                    minv[j].1 = minv[j].1.sub(delta.1);
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
    let mut assignment = vec![0; rows];
    for j in 1..=cols {
        if p[j] > 0 {
            assignment[p[j] - 1] = j - 1;
        }
    }
    assignment
}

// Dual-potential subtraction can erase a real score difference even when the
// input and final objective differ by an f64 ULP. Keep its rounding residual in
// the linear workspace; original costs and returned scores remain unchanged.
#[derive(Clone, Copy, PartialEq, PartialOrd)]
struct AssignmentScore {
    high: f64,
    low: f64,
}
impl From<f64> for AssignmentScore {
    fn from(high: f64) -> Self {
        Self { high, low: 0.0 }
    }
}
impl AssignmentScore {
    fn add(self, other: Self) -> Self {
        // Neutral dual updates need no compensation, especially on tied paths.
        if other == Self::from(0.0) {
            return self;
        }
        if self == Self::from(0.0) {
            return other;
        }
        fn two_sum(a: f64, b: f64) -> (f64, f64) {
            let sum = a + b;
            let b_rounded = sum - a;
            let residual = (a - (sum - b_rounded)) + (b - b_rounded);
            (sum, residual)
        }
        let (high, residual) = two_sum(self.high, other.high);
        let (low, tail) = two_sum(self.low, other.low);
        let (high, low) = two_sum(high, residual + low);
        let (high, low) = two_sum(high, low + tail);
        Self { high, low }
    }

    fn sub(self, other: Self) -> Self {
        if self == other {
            return Self::from(0.0);
        }
        self.add(Self {
            high: -other.high,
            low: -other.low,
        })
    }
}

fn assignment_costs(rows: usize, cols: usize) -> Vec<Vec<(isize, f64)>> {
    #[cfg(test)]
    assignment_storage_tests::MATRIX_BUILDS.with(|count| count.set(count.get() + 1));
    vec![vec![(0, 0.0); cols]; rows]
}

fn check_assignment_work(rows: usize, cols: usize, limits: &WorkLimits) -> Result<(), Error> {
    check_work(
        "Hungarian assignment dimension",
        Some(rows.max(cols)),
        limits.max_assignment_dimension,
    )?;
    check_work(
        "Hungarian cost matrix cells",
        rows.checked_mul(cols),
        limits.max_matrix_cells,
    )
}

fn check_rectangular_work(rows: usize, cols: usize, limits: &WorkLimits) -> Result<(), Error> {
    check_assignment_work(rows, cols, limits)?;
    check_work(
        "Rectangular assignment workspace length",
        rows.max(cols).checked_add(1),
        usize::MAX,
    )
}

#[cfg(test)]
mod assignment_storage_tests {
    use super::*;

    thread_local! {
        pub(super) static MATRIX_BUILDS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
        pub(super) static RECTANGULAR_AUGMENTATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    }

    #[test]
    fn guarded_rectangular_selection_checks_all_identities_before_allocation() {
        let results = MatcherResults::new(
            (0..6)
                .map(|i| {
                    (
                        ColumnPair::new("s", (i / 2).to_string(), "t", (i % 2).to_string()),
                        0.5,
                    )
                })
                .collect(),
        )
        .unwrap();
        for limits in [
            WorkLimits {
                max_assignment_dimension: 2,
                max_matrix_cells: 6,
                ..Default::default()
            },
            WorkLimits {
                max_assignment_dimension: 3,
                max_matrix_cells: 5,
                ..Default::default()
            },
        ] {
            MATRIX_BUILDS.with(|count| count.set(0));
            RECTANGULAR_AUGMENTATIONS.with(|count| count.set(0));
            for threshold in [None, Some(0.5), Some(1.1)] {
                assert!(
                    results
                        .one_to_one_rectangular_with_limits(threshold, &limits)
                        .is_err()
                );
                assert!(
                    results
                        .one_to_one_rectangular_threshold_aware_with_limits(threshold, &limits)
                        .is_err()
                );
            }
            assert_eq!(MATRIX_BUILDS.with(|count| count.get()), 0);
            assert_eq!(RECTANGULAR_AUGMENTATIONS.with(|count| count.get()), 0);
        }
        let exact = WorkLimits {
            max_assignment_dimension: 3,
            max_matrix_cells: 6,
            ..Default::default()
        };
        for (checked, plain) in [
            (
                results
                    .one_to_one_rectangular_with_limits(None, &exact)
                    .unwrap(),
                results.one_to_one_rectangular(None).unwrap(),
            ),
            (
                results
                    .one_to_one_rectangular_threshold_aware_with_limits(None, &exact)
                    .unwrap(),
                results
                    .one_to_one_rectangular_threshold_aware(None)
                    .unwrap(),
            ),
        ] {
            assert_eq!(checked, plain);
            assert_eq!(checked.len(), 2);
        }
        MATRIX_BUILDS.with(|count| count.set(0));
        assert!(
            results
                .one_to_one_rectangular_with_limits(Some(1.1), &exact)
                .unwrap()
                .is_empty()
        );
        assert!(
            results
                .one_to_one_rectangular_threshold_aware_with_limits(Some(1.1), &exact)
                .unwrap()
                .is_empty()
        );
        assert_eq!(MATRIX_BUILDS.with(|count| count.get()), 0);
        for (rows, cols) in [(usize::MAX, 2), (2, usize::MAX), (usize::MAX, 0)] {
            assert!(
                check_rectangular_work(rows, cols, &WorkLimits::default())
                    .unwrap_err()
                    .to_string()
                    .contains("overflows")
            );
        }
    }

    #[test]
    fn rectangular_solver_augments_only_the_smaller_dimension() {
        for transposed in [false, true] {
            let results = MatcherResults::new(
                (0..10_000)
                    .map(|i| {
                        (
                            if transposed {
                                ColumnPair::new("s", i.to_string(), "t", "one")
                            } else {
                                ColumnPair::new("s", "one", "t", i.to_string())
                            },
                            i as f64 / 10_000.0,
                        )
                    })
                    .collect(),
            )
            .unwrap();
            let limits = WorkLimits {
                max_assignment_dimension: 10_000,
                max_matrix_cells: 10_000,
                ..Default::default()
            };
            for cardinality_first in [false, true] {
                MATRIX_BUILDS.with(|count| count.set(0));
                RECTANGULAR_AUGMENTATIONS.with(|count| count.set(0));
                let selected = if cardinality_first {
                    results.one_to_one_rectangular_threshold_aware_with_limits(Some(0.0), &limits)
                } else {
                    results.one_to_one_rectangular_with_limits(Some(0.0), &limits)
                }
                .unwrap();
                assert_eq!(selected.len(), 1);
                assert_eq!(
                    selected.iter().next().unwrap().1.to_bits(),
                    0.9999f64.to_bits()
                );
                assert_eq!(MATRIX_BUILDS.with(|count| count.get()), 1);
                assert_eq!(RECTANGULAR_AUGMENTATIONS.with(|count| count.get()), 1);
            }
        }
        let zero = WorkLimits {
            max_assignment_dimension: 0,
            max_matrix_cells: 0,
            ..Default::default()
        };
        assert!(
            MatcherResults::default()
                .one_to_one_rectangular_with_limits(None, &zero)
                .unwrap()
                .is_empty()
        );
        assert!(
            MatcherResults::default()
                .one_to_one_rectangular_threshold_aware_with_limits(None, &zero)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn guarded_assignment_rejects_before_cost_allocation() {
        let results = MatcherResults::new(
            (0..6)
                .map(|i| {
                    (
                        ColumnPair::new("s", (i / 2).to_string(), "t", (i % 2).to_string()),
                        0.5,
                    )
                })
                .collect(),
        )
        .unwrap();
        for limits in [
            WorkLimits {
                max_assignment_dimension: 2,
                ..Default::default()
            },
            WorkLimits {
                max_matrix_cells: 5,
                ..Default::default()
            },
        ] {
            MATRIX_BUILDS.with(|count| count.set(0));
            assert!(
                results
                    .one_to_one_hungarian_with_limits(None, &limits)
                    .is_err()
            );
            assert!(
                results
                    .one_to_one_hungarian_threshold_aware_with_limits(None, &limits)
                    .is_err()
            );
            assert_eq!(MATRIX_BUILDS.with(|count| count.get()), 0);
        }
        let limits = WorkLimits {
            max_assignment_dimension: 3,
            max_matrix_cells: 6,
            ..Default::default()
        };
        MATRIX_BUILDS.with(|count| count.set(0));
        assert_eq!(
            results
                .one_to_one_hungarian_with_limits(None, &limits)
                .unwrap(),
            results.one_to_one_hungarian(None).unwrap()
        );
        assert_eq!(MATRIX_BUILDS.with(|count| count.get()), 2);
        assert!(
            check_assignment_work(usize::MAX, 2, &WorkLimits::default())
                .unwrap_err()
                .to_string()
                .contains("overflows")
        );
    }

    #[test]
    fn guarded_empty_singleton_and_rectangular_shortcuts_validate_limits() {
        let empty = MatcherResults::default();
        let zero = WorkLimits {
            max_assignment_dimension: 0,
            max_matrix_cells: 0,
            ..Default::default()
        };
        assert!(
            empty
                .one_to_one_hungarian_with_limits(None, &zero)
                .unwrap()
                .is_empty()
        );
        let key = ColumnPair::new("s", "one", "t", "one");
        let singleton = MatcherResults::with_details(
            vec![(key.clone(), -0.0)],
            [(key.clone(), [("detail".into(), -0.0)].into())].into(),
        )
        .unwrap();
        let one = WorkLimits {
            max_assignment_dimension: 1,
            max_matrix_cells: 1,
            ..Default::default()
        };
        for selected in [
            singleton
                .one_to_one_hungarian_with_limits(None, &one)
                .unwrap(),
            singleton
                .one_to_one_hungarian_threshold_aware_with_limits(Some(-1.0), &one)
                .unwrap(),
        ] {
            assert_eq!(selected.get(&key).unwrap().to_bits(), (-0.0f64).to_bits());
            assert_eq!(
                selected.get_details(&key).unwrap()["detail"].to_bits(),
                (-0.0f64).to_bits()
            );
        }
        for transposed in [false, true] {
            let results = MatcherResults::new(
                (0..10_000)
                    .map(|i| {
                        (
                            if transposed {
                                ColumnPair::new("s", i.to_string(), "t", "one")
                            } else {
                                ColumnPair::new("s", "one", "t", i.to_string())
                            },
                            0.5,
                        )
                    })
                    .collect(),
            )
            .unwrap();
            MATRIX_BUILDS.with(|count| count.set(0));
            let message = results
                .one_to_one_hungarian_with_limits(None, &one)
                .unwrap_err()
                .to_string();
            for word in ["Hungarian", "10000", "limit 1"] {
                assert!(message.contains(word), "{message}");
            }
            assert_eq!(MATRIX_BUILDS.with(|count| count.get()), 0);
        }
        for threshold in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            assert!(
                singleton
                    .one_to_one_hungarian_with_limits(Some(threshold), &one)
                    .is_err()
            );
            assert!(
                singleton
                    .one_to_one_hungarian_threshold_aware_with_limits(Some(threshold), &one)
                    .is_err()
            );
        }
    }

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

    #[test]
    fn uniform_single_target_shortcut_avoids_matrix_after_guard_checks() {
        let results = MatcherResults::new(
            (0..10_000)
                .map(|row| (ColumnPair::new("s", row.to_string(), "t", "one"), 0.75))
                .collect(),
        )
        .unwrap();
        let limits = WorkLimits {
            max_assignment_dimension: 10_000,
            max_matrix_cells: 10_000,
            ..Default::default()
        };
        MATRIX_BUILDS.with(|count| count.set(0));
        for selected in [
            results.one_to_one_hungarian(None).unwrap(),
            results.one_to_one_hungarian_threshold_aware(None).unwrap(),
            results
                .one_to_one_hungarian_with_limits(None, &limits)
                .unwrap(),
            results
                .one_to_one_hungarian_threshold_aware_with_limits(None, &limits)
                .unwrap(),
        ] {
            assert_eq!(selected.len(), 1);
            assert_eq!(selected.iter().next().unwrap().0.source_column, "0");
        }
        assert_eq!(MATRIX_BUILDS.with(|count| count.get()), 0);
        let too_small = WorkLimits {
            max_assignment_dimension: 9_999,
            ..limits
        };
        assert!(
            results
                .one_to_one_hungarian_with_limits(None, &too_small)
                .is_err()
        );
        assert!(
            results
                .one_to_one_hungarian_threshold_aware_with_limits(None, &too_small)
                .is_err()
        );
        assert_eq!(MATRIX_BUILDS.with(|count| count.get()), 0);
    }
}
