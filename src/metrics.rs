//! Table-aware evaluation metrics and an extension trait for custom metrics.
use crate::{ColumnPair, Error, MatcherResults};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug)]
pub enum GroundTruth {
    /// Compare column names, ignoring tables (convenient for a single table pair).
    Names(Vec<(String, String)>),
    /// Compare all four identity fields; recommended for batch evaluation.
    Columns(Vec<ColumnPair>),
}
impl From<Vec<ColumnPair>> for GroundTruth {
    fn from(pairs: Vec<ColumnPair>) -> Self {
        Self::Columns(pairs)
    }
}
impl From<Vec<(String, String)>> for GroundTruth {
    fn from(pairs: Vec<(String, String)>) -> Self {
        Self::Names(pairs)
    }
}
impl GroundTruth {
    fn keys(&self) -> BTreeSet<Vec<String>> {
        match self {
            Self::Names(pairs) => pairs
                .iter()
                .map(|(s, t)| vec![s.clone(), t.clone()])
                .collect(),
            Self::Columns(pairs) => pairs.iter().map(|p| self.key(p)).collect(),
        }
    }
    fn key(&self, p: &ColumnPair) -> Vec<String> {
        match self {
            Self::Names(_) => vec![p.source_column.clone(), p.target_column.clone()],
            Self::Columns(_) => vec![
                p.source_table.clone(),
                p.source_column.clone(),
                p.target_table.clone(),
                p.target_column.clone(),
            ],
        }
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub enum OneToOneMethod {
    #[default]
    Hungarian,
    Greedy,
    MutualTop,
}
impl OneToOneMethod {
    fn select(self, results: &MatcherResults) -> Result<MatcherResults, Error> {
        #[cfg(test)]
        tests::SELECT_CALLS.with(|calls| calls.set(calls.get() + 1));
        match self {
            Self::Hungarian => results.one_to_one_hungarian(None),
            Self::Greedy => results.one_to_one_greedy(None),
            Self::MutualTop => results.one_to_one_mutual_top(1),
        }
    }
}
pub trait Metric {
    fn name(&self) -> String;
    fn apply(
        &self,
        matches: &MatcherResults,
        truth: &GroundTruth,
        method: OneToOneMethod,
    ) -> Result<f64, Error>;
}

/// Give a configured metric a distinct key, avoiding collisions in a metric set.
pub struct NamedMetric<'a> {
    pub name: &'a str,
    pub metric: &'a dyn Metric,
}
impl Metric for NamedMetric<'_> {
    fn name(&self) -> String {
        self.name.into()
    }
    fn apply(
        &self,
        matches: &MatcherResults,
        truth: &GroundTruth,
        method: OneToOneMethod,
    ) -> Result<f64, Error> {
        self.metric.apply(matches, truth, method)
    }
}

pub const METRICS_CORE: [&dyn Metric; 6] = [
    &Precision { one_to_one: true },
    &Recall { one_to_one: true },
    &F1Score { one_to_one: true },
    &PrecisionTopNPercent {
        one_to_one: true,
        n: 10,
    },
    &RecallAtSizeofGroundTruth { one_to_one: false },
    &MeanReciprocalRank { one_to_one: false },
];
pub const METRICS_PRECISION_RECALL: [&dyn Metric; 2] = [
    &Precision { one_to_one: true },
    &Recall { one_to_one: true },
];
/// Both selector configurations are retained using distinct, stable metric names.
pub const METRICS_ALL: [&dyn Metric; 9] = [
    &Precision { one_to_one: true },
    &Recall { one_to_one: true },
    &F1Score { one_to_one: true },
    &NamedMetric {
        name: "PrecisionWithoutOneToOne",
        metric: &Precision { one_to_one: false },
    },
    &NamedMetric {
        name: "RecallWithoutOneToOne",
        metric: &Recall { one_to_one: false },
    },
    &NamedMetric {
        name: "F1ScoreWithoutOneToOne",
        metric: &F1Score { one_to_one: false },
    },
    &PrecisionTopNPercent {
        one_to_one: true,
        n: 10,
    },
    &RecallAtSizeofGroundTruth { one_to_one: false },
    &MeanReciprocalRank { one_to_one: false },
];
pub const METRICS_PRECISION_INCREASING_N: [&dyn Metric; 10] = [
    &PrecisionTopNPercent {
        one_to_one: true,
        n: 10,
    },
    &PrecisionTopNPercent {
        one_to_one: true,
        n: 20,
    },
    &PrecisionTopNPercent {
        one_to_one: true,
        n: 30,
    },
    &PrecisionTopNPercent {
        one_to_one: true,
        n: 40,
    },
    &PrecisionTopNPercent {
        one_to_one: true,
        n: 50,
    },
    &PrecisionTopNPercent {
        one_to_one: true,
        n: 60,
    },
    &PrecisionTopNPercent {
        one_to_one: true,
        n: 70,
    },
    &PrecisionTopNPercent {
        one_to_one: true,
        n: 80,
    },
    &PrecisionTopNPercent {
        one_to_one: true,
        n: 90,
    },
    &PrecisionTopNPercent {
        one_to_one: true,
        n: 100,
    },
];

fn counts(matches: &MatcherResults, truth: &GroundTruth) -> (usize, usize, usize) {
    let gold = truth.keys();
    let predicted: BTreeSet<_> = matches.iter().map(|(p, _)| truth.key(p)).collect();
    (
        gold.intersection(&predicted).count(),
        predicted.difference(&gold).count(),
        gold.difference(&predicted).count(),
    )
}
fn ratio(n: usize, d: usize) -> f64 {
    if d == 0 { 0.0 } else { n as f64 / d as f64 }
}

macro_rules! basic_metric {
    ($name:ident, $compute:expr) => {
        #[derive(Clone, Copy, Debug)]
        pub struct $name {
            pub one_to_one: bool,
        }
        impl Default for $name {
            fn default() -> Self {
                Self { one_to_one: true }
            }
        }
        impl Metric for $name {
            fn name(&self) -> String {
                stringify!($name).into()
            }
            fn apply(
                &self,
                matches: &MatcherResults,
                truth: &GroundTruth,
                method: OneToOneMethod,
            ) -> Result<f64, Error> {
                let selected;
                let matches = if self.one_to_one {
                    selected = method.select(matches)?;
                    &selected
                } else {
                    matches
                };
                let (tp, fp, fn_) = counts(matches, truth);
                Ok(($compute)(tp, fp, fn_))
            }
        }
    };
}
basic_metric!(Precision, |tp, fp, _| ratio(tp, tp + fp));
basic_metric!(Recall, |tp, _, fn_| ratio(tp, tp + fn_));
basic_metric!(F1Score, |tp, fp, fn_| ratio(2 * tp, 2 * tp + fp + fn_));

#[derive(Clone, Copy, Debug)]
pub struct PrecisionTopNPercent {
    pub one_to_one: bool,
    pub n: u8,
}
impl Default for PrecisionTopNPercent {
    fn default() -> Self {
        Self {
            one_to_one: true,
            n: 10,
        }
    }
}
impl Metric for PrecisionTopNPercent {
    fn name(&self) -> String {
        format!("PrecisionTop{}Percent", self.n)
    }
    fn apply(
        &self,
        matches: &MatcherResults,
        truth: &GroundTruth,
        method: OneToOneMethod,
    ) -> Result<f64, Error> {
        let selected;
        let matches = if self.one_to_one {
            selected = method.select(matches)?;
            &selected
        } else {
            matches
        };
        let selected = matches.take_top_percent(f64::from(self.n.min(100)))?;
        Precision { one_to_one: false }.apply(&selected, truth, method)
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct RecallAtSizeofGroundTruth {
    pub one_to_one: bool,
}
impl Metric for RecallAtSizeofGroundTruth {
    fn name(&self) -> String {
        "RecallAtSizeofGroundTruth".into()
    }
    fn apply(
        &self,
        matches: &MatcherResults,
        truth: &GroundTruth,
        method: OneToOneMethod,
    ) -> Result<f64, Error> {
        let selected;
        let matches = if self.one_to_one {
            selected = method.select(matches)?;
            &selected
        } else {
            matches
        };
        Recall { one_to_one: false }.apply(&matches.take_top_n(truth.keys().len()), truth, method)
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct MeanReciprocalRank {
    pub one_to_one: bool,
}
impl Metric for MeanReciprocalRank {
    fn name(&self) -> String {
        "MeanReciprocalRank".into()
    }
    fn apply(
        &self,
        matches: &MatcherResults,
        truth: &GroundTruth,
        method: OneToOneMethod,
    ) -> Result<f64, Error> {
        let selected;
        let matches = if self.one_to_one {
            selected = method.select(matches)?;
            &selected
        } else {
            matches
        };
        let split = if matches!(truth, GroundTruth::Names(_)) {
            1
        } else {
            2
        };
        let mut gold: BTreeMap<Vec<String>, BTreeSet<Vec<String>>> = BTreeMap::new();
        for key in truth.keys() {
            gold.entry(key[..split].to_vec())
                .or_default()
                .insert(key[split..].to_vec());
        }
        if gold.is_empty() {
            return Ok(0.0);
        }
        let mut ranks = BTreeMap::new();
        let mut found = BTreeMap::new();
        for (pair, _) in matches {
            let key = truth.key(pair);
            let source = key[..split].to_vec();
            if found.contains_key(&source) {
                continue;
            }
            if let Some(targets) = gold.get(&source) {
                let rank = ranks.entry(source.clone()).or_insert(0usize);
                *rank += 1;
                if targets.contains(&key[split..]) {
                    found.insert(source, 1.0 / *rank as f64);
                }
            }
        }
        Ok(found.values().sum::<f64>() / gold.len() as f64)
    }
}

/// Valentine's core metric set, using Hungarian selection for metrics with a 1:1 flag.
pub fn get_metrics(
    matches: &MatcherResults,
    truth: &GroundTruth,
) -> Result<BTreeMap<String, f64>, Error> {
    let method = OneToOneMethod::Hungarian;
    let selected = method.select(matches)?;
    let (tp, fp, fn_) = counts(&selected, truth);
    Ok(BTreeMap::from([
        ("Precision".into(), ratio(tp, tp + fp)),
        ("Recall".into(), ratio(tp, tp + fn_)),
        ("F1Score".into(), ratio(2 * tp, 2 * tp + fp + fn_)),
        (
            "PrecisionTop10Percent".into(),
            PrecisionTopNPercent {
                one_to_one: false,
                n: 10,
            }
            .apply(&selected, truth, method)?,
        ),
        (
            "RecallAtSizeofGroundTruth".into(),
            RecallAtSizeofGroundTruth::default().apply(matches, truth, method)?,
        ),
        (
            "MeanReciprocalRank".into(),
            MeanReciprocalRank::default().apply(matches, truth, method)?,
        ),
    ]))
}

pub fn precision_increasing_n(
    matches: &MatcherResults,
    truth: &GroundTruth,
    method: OneToOneMethod,
) -> Result<BTreeMap<String, f64>, Error> {
    let selected = method.select(matches)?;
    (10..=100)
        .step_by(10)
        .map(|n| {
            let metric = PrecisionTopNPercent {
                n,
                one_to_one: false,
            };
            metric
                .apply(&selected, truth, method)
                .map(|s| (metric.name(), s))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    thread_local! {
        pub(super) static SELECT_CALLS: Cell<usize> = const { Cell::new(0) };
    }

    #[test]
    fn fixed_reports_select_once_and_preserve_metric_bits() {
        let matches = MatcherResults::new(vec![
            (ColumnPair::new("source", "a", "target", "wrong"), 0.95),
            (ColumnPair::new("source", "a", "target", "yes"), 0.94),
            (ColumnPair::new("source", "b", "target", "yes"), 0.9),
            (ColumnPair::new("source", "c", "target", "spare"), 0.9),
            (ColumnPair::new("other", "a", "target", "yes"), 0.8),
            (ColumnPair::new("source", "d", "target", "zero"), -0.0),
            (ColumnPair::new("source", "e", "target", "zero"), 0.0),
        ])
        .unwrap();
        let truths = [
            GroundTruth::Names(vec![
                ("a".into(), "yes".into()),
                ("a".into(), "yes".into()),
                ("c".into(), "absent".into()),
            ]),
            GroundTruth::Columns(vec![
                ColumnPair::new("source", "a", "target", "yes"),
                ColumnPair::new("other", "a", "target", "yes"),
                ColumnPair::new("source", "c", "target", "absent"),
            ]),
            GroundTruth::Names(vec![]),
            GroundTruth::Columns(vec![]),
        ];
        for matches in [matches, MatcherResults::default()] {
            let original = matches.clone();
            for truth in &truths {
                let expected = matches
                    .get_metrics_with(truth, &METRICS_CORE, OneToOneMethod::Hungarian)
                    .unwrap();
                SELECT_CALLS.with(|calls| calls.set(0));
                let actual = get_metrics(&matches, truth).unwrap();
                assert_eq!(SELECT_CALLS.with(Cell::get), 1);
                assert_eq!(metric_bits(&actual), metric_bits(&expected));
                for method in [
                    OneToOneMethod::Hungarian,
                    OneToOneMethod::Greedy,
                    OneToOneMethod::MutualTop,
                ] {
                    let expected = matches
                        .get_metrics_with(truth, &METRICS_PRECISION_INCREASING_N, method)
                        .unwrap();
                    SELECT_CALLS.with(|calls| calls.set(0));
                    let actual = precision_increasing_n(&matches, truth, method).unwrap();
                    assert_eq!(SELECT_CALLS.with(Cell::get), 1);
                    assert_eq!(metric_bits(&actual), metric_bits(&expected));
                }
                assert_eq!(matches, original);
            }
        }
    }

    #[test]
    fn core_ranking_metrics_still_use_unselected_candidates() {
        let matches = MatcherResults::new(vec![
            (ColumnPair::new("source", "a", "target", "wrong"), 0.95),
            (ColumnPair::new("source", "a", "target", "yes"), 0.94),
            (ColumnPair::new("source", "b", "target", "yes"), 0.9),
        ])
        .unwrap();
        let truth = GroundTruth::Names(vec![
            ("a".into(), "yes".into()),
            ("c".into(), "absent".into()),
        ]);
        let actual = get_metrics(&matches, &truth).unwrap();
        assert_eq!(actual["MeanReciprocalRank"], 0.25);
        assert_eq!(actual["RecallAtSizeofGroundTruth"], 0.5);
        let selected = OneToOneMethod::Hungarian.select(&matches).unwrap();
        assert_eq!(
            MeanReciprocalRank::default()
                .apply(&selected, &truth, OneToOneMethod::Hungarian)
                .unwrap(),
            0.0
        );
        assert_eq!(
            RecallAtSizeofGroundTruth::default()
                .apply(&selected, &truth, OneToOneMethod::Hungarian)
                .unwrap(),
            0.0
        );
    }

    fn metric_bits(values: &BTreeMap<String, f64>) -> BTreeMap<&str, u64> {
        values
            .iter()
            .map(|(name, value)| (name.as_str(), value.to_bits()))
            .collect()
    }
}
