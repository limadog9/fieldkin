use std::collections::{BTreeMap, BTreeSet};

use fieldkin::MatchReport;
use serde::{Deserialize, Serialize};

use crate::model::{Label, LabelKind};

/// Integer sufficient statistics. Aggregate counts before computing micro averages.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct Counts {
    pub fields: usize,
    pub unique_fields: usize,
    pub no_match_fields: usize,
    pub ambiguous_fields: usize,
    pub proposals: usize,
    pub correct_proposals: usize,
    pub wrong_unique_proposals: usize,
    pub no_match_proposals: usize,
    pub ambiguous_proposals: usize,
    pub abstentions: usize,
    pub unique_proposals: usize,
    pub expected_abstentions: usize,
    pub correct_abstentions: usize,
    pub candidate_relevant: usize,
    pub candidate_hits: usize,
    pub unique_candidate_relevant: usize,
    pub unique_candidate_hits: usize,
    pub ambiguous_candidate_relevant: usize,
    pub ambiguous_candidate_hits: usize,
    pub candidate_any_fields: usize,
    pub candidate_any_hits: usize,
    pub feasible_unique_matches: usize,
}

/// Undefined ratios serialize as null, never as a perfect score or zero.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Metrics {
    pub precision: Option<f64>,
    pub recall: Option<f64>,
    pub feasible_recall: Option<f64>,
    pub unique_coverage: Option<f64>,
    pub proposal_coverage: Option<f64>,
    pub candidate_recall_at_5: Option<f64>,
    pub unique_candidate_recall_at_5: Option<f64>,
    pub ambiguous_candidate_recall_at_5: Option<f64>,
    pub candidate_any_at_5: Option<f64>,
    pub no_match_false_proposal_rate: Option<f64>,
    pub ambiguity_unsafe_proposal_rate: Option<f64>,
    pub expected_abstention_accuracy: Option<f64>,
}

impl Counts {
    pub fn add(&mut self, other: &Self) {
        self.fields += other.fields;
        self.unique_fields += other.unique_fields;
        self.no_match_fields += other.no_match_fields;
        self.ambiguous_fields += other.ambiguous_fields;
        self.proposals += other.proposals;
        self.correct_proposals += other.correct_proposals;
        self.wrong_unique_proposals += other.wrong_unique_proposals;
        self.no_match_proposals += other.no_match_proposals;
        self.ambiguous_proposals += other.ambiguous_proposals;
        self.abstentions += other.abstentions;
        self.unique_proposals += other.unique_proposals;
        self.expected_abstentions += other.expected_abstentions;
        self.correct_abstentions += other.correct_abstentions;
        self.candidate_relevant += other.candidate_relevant;
        self.candidate_hits += other.candidate_hits;
        self.unique_candidate_relevant += other.unique_candidate_relevant;
        self.unique_candidate_hits += other.unique_candidate_hits;
        self.ambiguous_candidate_relevant += other.ambiguous_candidate_relevant;
        self.ambiguous_candidate_hits += other.ambiguous_candidate_hits;
        self.candidate_any_fields += other.candidate_any_fields;
        self.candidate_any_hits += other.candidate_any_hits;
        self.feasible_unique_matches += other.feasible_unique_matches;
    }

    pub fn metrics(&self) -> Metrics {
        fn ratio(numerator: usize, denominator: usize) -> Option<f64> {
            (denominator != 0).then(|| numerator as f64 / denominator as f64)
        }

        Metrics {
            precision: ratio(self.correct_proposals, self.proposals),
            recall: ratio(self.correct_proposals, self.unique_fields),
            feasible_recall: ratio(self.correct_proposals, self.feasible_unique_matches),
            unique_coverage: ratio(self.unique_proposals, self.unique_fields),
            proposal_coverage: ratio(self.proposals, self.fields),
            candidate_recall_at_5: ratio(self.candidate_hits, self.candidate_relevant),
            unique_candidate_recall_at_5: ratio(
                self.unique_candidate_hits,
                self.unique_candidate_relevant,
            ),
            ambiguous_candidate_recall_at_5: ratio(
                self.ambiguous_candidate_hits,
                self.ambiguous_candidate_relevant,
            ),
            candidate_any_at_5: ratio(self.candidate_any_hits, self.candidate_any_fields),
            no_match_false_proposal_rate: ratio(self.no_match_proposals, self.no_match_fields),
            ambiguity_unsafe_proposal_rate: ratio(self.ambiguous_proposals, self.ambiguous_fields),
            expected_abstention_accuracy: ratio(
                self.correct_abstentions,
                self.expected_abstentions,
            ),
        }
    }
}

/// Score each labeled source exactly once. An ambiguous gold label demands
/// abstention: choosing even a plausible target is not a correct proposal.
/// Candidate retrieval is measured independently of eligibility and selection.
pub fn score_case(labels: &[Label], report: &MatchReport) -> Result<Counts, String> {
    let mut by_source = BTreeMap::new();
    let mut unique_targets = BTreeSet::new();
    for label in labels {
        if by_source.insert(label.source.as_str(), label).is_some() {
            return Err(format!("duplicate label for source {}", label.source));
        }
        let target_count = label.targets.len();
        let distinct_targets: BTreeSet<_> = label.targets.iter().collect();
        let valid_cardinality = match label.kind {
            LabelKind::Match => target_count == 1,
            LabelKind::NoMatch => target_count == 0,
            LabelKind::Ambiguous => target_count >= 2,
        };
        if !valid_cardinality || distinct_targets.len() != target_count {
            return Err(format!("invalid target labels for source {}", label.source));
        }
        if label.kind == LabelKind::Match {
            unique_targets.extend(label.targets.iter());
        }
    }
    if report.fields.len() != labels.len() {
        return Err("report must contain exactly one field per labeled source".to_owned());
    }

    let mut counts = Counts::default();
    let mut seen_sources = BTreeSet::new();
    let mut selected_targets = BTreeSet::new();
    for field in &report.fields {
        if !seen_sources.insert(field.source.0.as_str()) {
            return Err(format!("duplicate report source {}", field.source));
        }
        let label = by_source
            .get(field.source.0.as_str())
            .ok_or_else(|| format!("unlabeled report source {}", field.source))?;
        if report.one_to_one {
            if let Some(selected) = &field.selected {
                if !selected_targets.insert(selected.target.0.as_str()) {
                    return Err(format!(
                        "one-to-one report selects target {} more than once",
                        selected.target
                    ));
                }
            }
        }
        counts.fields += 1;
        let proposed = field.selected.is_some();
        counts.proposals += usize::from(proposed);
        counts.abstentions += usize::from(!proposed);
        match label.kind {
            LabelKind::Match => {
                counts.unique_fields += 1;
                counts.unique_proposals += usize::from(proposed);
                if let Some(selected) = &field.selected {
                    if selected.target.0 == label.targets[0] {
                        counts.correct_proposals += 1;
                    } else {
                        counts.wrong_unique_proposals += 1;
                    }
                }
            }
            LabelKind::NoMatch => {
                counts.no_match_fields += 1;
                counts.no_match_proposals += usize::from(proposed);
                counts.expected_abstentions += 1;
                counts.correct_abstentions += usize::from(!proposed);
            }
            LabelKind::Ambiguous => {
                counts.ambiguous_fields += 1;
                counts.ambiguous_proposals += usize::from(proposed);
                counts.expected_abstentions += 1;
                counts.correct_abstentions += usize::from(!proposed);
            }
        }

        // The first five retained ranks count, including candidates the engine
        // would not select. De-duplicate IDs so repeats cannot inflate recall.
        let retrieved: BTreeSet<_> = field
            .candidates
            .iter()
            .take(5)
            .map(|candidate| candidate.target.0.as_str())
            .collect();
        let hits = label
            .targets
            .iter()
            .filter(|target| retrieved.contains(target.as_str()))
            .count();
        counts.candidate_relevant += label.targets.len();
        counts.candidate_hits += hits;
        match label.kind {
            LabelKind::Match => {
                counts.unique_candidate_relevant += label.targets.len();
                counts.unique_candidate_hits += hits;
            }
            LabelKind::Ambiguous => {
                counts.ambiguous_candidate_relevant += label.targets.len();
                counts.ambiguous_candidate_hits += hits;
            }
            LabelKind::NoMatch => {}
        }
        if !label.targets.is_empty() {
            counts.candidate_any_fields += 1;
            counts.candidate_any_hits += usize::from(hits > 0);
        }
    }
    counts.feasible_unique_matches = if report.one_to_one {
        unique_targets.len()
    } else {
        counts.unique_fields
    };
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use fieldkin::{Candidate, Decision, FieldMatch};

    use super::*;

    fn label(source: &str, kind: LabelKind, targets: &[&str]) -> Label {
        Label {
            source: source.to_owned(),
            kind,
            targets: targets.iter().map(|value| (*value).to_owned()).collect(),
            rationale: "metric test".to_owned(),
        }
    }

    fn candidate(target: &str, eligible: bool) -> Candidate {
        Candidate {
            target: target.into(),
            score: 0.8,
            eligible,
            signals: Vec::new(),
            warnings: Vec::new(),
            issues: Vec::new(),
        }
    }

    fn field(source: &str, selected: Option<&str>, ranked: &[&str]) -> FieldMatch {
        FieldMatch {
            source: source.into(),
            diagnostics: Vec::new(),
            candidates: ranked
                .iter()
                .map(|target| candidate(target, true))
                .collect(),
            alternatives: Vec::new(),
            selected: selected.map(|target| candidate(target, true)),
            decision: if selected.is_some() {
                Decision::Proposed
            } else {
                Decision::BelowThreshold
            },
        }
    }

    fn report(fields: Vec<FieldMatch>, one_to_one: bool) -> MatchReport {
        MatchReport {
            unmatched_sources: fields
                .iter()
                .filter(|field| field.selected.is_none())
                .map(|field| field.source.clone())
                .collect(),
            fields,
            unmatched_targets: Vec::new(),
            one_to_one,
            target_competition: Vec::new(),
            assignment_diagnostics: Default::default(),
        }
    }

    #[test]
    fn separates_wrong_targets_unsafe_proposals_and_abstentions() {
        let labels = [
            label("correct", LabelKind::Match, &["t1"]),
            label("wrong", LabelKind::Match, &["t2"]),
            label("missed", LabelKind::Match, &["t3"]),
            label("ambiguous", LabelKind::Ambiguous, &["a", "b"]),
            label("ambiguous-abstain", LabelKind::Ambiguous, &["c", "d"]),
            label("unrelated", LabelKind::NoMatch, &[]),
            label("unrelated-abstain", LabelKind::NoMatch, &[]),
        ];
        let report = report(
            vec![
                field("correct", Some("t1"), &["t1"]),
                field("wrong", Some("other"), &["other", "t2"]),
                field("missed", None, &["t3"]),
                field("ambiguous", Some("a"), &["a", "b"]),
                field("ambiguous-abstain", None, &["c"]),
                field("unrelated", Some("other"), &["other"]),
                field("unrelated-abstain", None, &[]),
            ],
            false,
        );
        let counts = score_case(&labels, &report).unwrap();
        assert_eq!(counts.fields, 7);
        assert_eq!(counts.unique_fields, 3);
        assert_eq!(counts.proposals, 4);
        assert_eq!(counts.correct_proposals, 1);
        assert_eq!(counts.wrong_unique_proposals, 1);
        assert_eq!(counts.ambiguous_proposals, 1);
        assert_eq!(counts.no_match_proposals, 1);
        assert_eq!(counts.unique_proposals, 2);
        assert_eq!(counts.abstentions, 3);
        assert_eq!(counts.expected_abstentions, 4);
        assert_eq!(counts.correct_abstentions, 2);
        assert_eq!(counts.candidate_relevant, 7);
        assert_eq!(counts.candidate_hits, 6);
        assert_eq!(counts.candidate_any_fields, 5);
        assert_eq!(counts.candidate_any_hits, 5);
        let metrics = counts.metrics();
        assert_eq!(metrics.precision, Some(0.25));
        assert_eq!(metrics.recall, Some(1.0 / 3.0));
        assert_eq!(metrics.unique_coverage, Some(2.0 / 3.0));
        assert_eq!(metrics.proposal_coverage, Some(4.0 / 7.0));
        assert_eq!(metrics.ambiguous_candidate_recall_at_5, Some(0.75));
        assert_eq!(metrics.expected_abstention_accuracy, Some(0.5));
        assert_eq!(metrics.no_match_false_proposal_rate, Some(0.5));
        assert_eq!(metrics.ambiguity_unsafe_proposal_rate, Some(0.5));
    }

    #[test]
    fn empty_denominators_are_null_and_never_perfect() {
        let counts = score_case(&[], &report(Vec::new(), false)).unwrap();
        let json = serde_json::to_value(counts.metrics()).unwrap();
        assert!(json
            .as_object()
            .unwrap()
            .values()
            .all(|value| value.is_null()));

        let counts = score_case(
            &[label("s", LabelKind::Match, &["t"])],
            &report(vec![field("s", None, &[])], false),
        )
        .unwrap();
        assert_eq!(counts.metrics().precision, None);
        assert_eq!(counts.metrics().recall, Some(0.0));
    }

    #[test]
    fn candidate_retrieval_uses_first_five_before_eligibility() {
        let labels = [label("s", LabelKind::Ambiguous, &["gold-1", "gold-6"])];
        let mut ranked = field("s", None, &["gold-1", "x", "y", "z", "w", "gold-6"]);
        ranked.candidates[0].eligible = false;
        let counts = score_case(&labels, &report(vec![ranked], false)).unwrap();
        assert_eq!(counts.candidate_hits, 1);
        assert_eq!(counts.candidate_relevant, 2);
        assert_eq!(counts.metrics().candidate_recall_at_5, Some(0.5));
        assert_eq!(counts.metrics().candidate_any_at_5, Some(1.0));
        assert_eq!(counts.correct_abstentions, 1);
    }

    #[test]
    fn duplicate_candidate_ids_cannot_inflate_retrieval() {
        let counts = score_case(
            &[label("s", LabelKind::Ambiguous, &["a", "b"])],
            &report(vec![field("s", None, &["a", "a", "a"])], false),
        )
        .unwrap();
        assert_eq!(counts.candidate_hits, 1);
        assert_eq!(counts.metrics().candidate_recall_at_5, Some(0.5));
    }

    #[test]
    fn one_to_one_reports_semantic_recall_and_feasible_ceiling_separately() {
        let labels = [
            label("s1", LabelKind::Match, &["t"]),
            label("s2", LabelKind::Match, &["t"]),
        ];
        let fields = vec![field("s1", Some("t"), &["t"]), field("s2", None, &["t"])];
        let global = score_case(&labels, &report(fields.clone(), true)).unwrap();
        let independent = score_case(&labels, &report(fields, false)).unwrap();
        assert_eq!(global.feasible_unique_matches, 1);
        assert_eq!(global.metrics().recall, Some(0.5));
        assert_eq!(global.metrics().feasible_recall, Some(1.0));
        assert_eq!(independent.feasible_unique_matches, 2);
        assert_eq!(independent.metrics().feasible_recall, Some(0.5));
    }

    #[test]
    fn duplicate_selected_targets_are_rejected_only_in_one_to_one_reports() {
        let labels = [
            label("s1", LabelKind::Match, &["t"]),
            label("s2", LabelKind::Match, &["t"]),
        ];
        let fields = vec![
            field("s1", Some("t"), &["t"]),
            field("s2", Some("t"), &["t"]),
        ];
        assert!(score_case(&labels, &report(fields.clone(), true)).is_err());
        let independent = score_case(&labels, &report(fields, false)).unwrap();
        assert_eq!(independent.metrics().feasible_recall, Some(1.0));

        // An unsafe ambiguous selection must not be allowed to share a target
        // with a correct unique proposal in a purported one-to-one report.
        let mixed_labels = [
            label("s1", LabelKind::Match, &["t"]),
            label("s2", LabelKind::Ambiguous, &["t", "other"]),
        ];
        let mixed = report(
            vec![field("s1", Some("t"), &[]), field("s2", Some("t"), &[])],
            true,
        );
        assert!(score_case(&mixed_labels, &mixed).is_err());
    }

    #[test]
    fn aggregation_is_micro_averaged_instead_of_averaging_case_ratios() {
        let first = score_case(
            &[label("s1", LabelKind::Match, &["t1"])],
            &report(vec![field("s1", Some("t1"), &["t1"])], false),
        )
        .unwrap();
        let second = score_case(
            &[
                label("s1", LabelKind::Match, &["t1"]),
                label("s2", LabelKind::Match, &["t2"]),
                label("s3", LabelKind::Match, &["t3"]),
            ],
            &report(
                vec![
                    field("s1", Some("t1"), &["t1"]),
                    field("s2", Some("wrong"), &[]),
                    field("s3", Some("wrong"), &[]),
                ],
                false,
            ),
        )
        .unwrap();
        let mut combined = first.clone();
        combined.add(&second);
        assert_eq!(combined.fields, 4);
        assert_eq!(combined.correct_proposals, 2);
        assert_eq!(combined.metrics().precision, Some(0.5));
        let macro_precision =
            (first.metrics().precision.unwrap() + second.metrics().precision.unwrap()) / 2.0;
        assert_ne!(combined.metrics().precision, Some(macro_precision));
    }

    #[test]
    fn rejects_missing_duplicate_and_unknown_report_sources() {
        let labels = [
            label("s1", LabelKind::Match, &["t1"]),
            label("s2", LabelKind::Match, &["t2"]),
        ];
        assert!(score_case(&labels, &report(vec![field("s1", None, &[])], false)).is_err());
        assert!(score_case(
            &labels,
            &report(vec![field("s1", None, &[]), field("s1", None, &[])], false)
        )
        .is_err());
        assert!(score_case(
            &labels,
            &report(
                vec![field("s1", None, &[]), field("unknown", None, &[])],
                false
            )
        )
        .is_err());
    }

    #[test]
    fn rejects_duplicate_sources_and_invalid_label_cardinalities() {
        let duplicate = [
            label("s", LabelKind::Match, &["t"]),
            label("s", LabelKind::NoMatch, &[]),
        ];
        assert!(score_case(&duplicate, &report(Vec::new(), false)).is_err());
        for invalid in [
            label("s", LabelKind::Match, &[]),
            label("s", LabelKind::Match, &["t1", "t2"]),
            label("s", LabelKind::NoMatch, &["t"]),
            label("s", LabelKind::Ambiguous, &["t"]),
            label("s", LabelKind::Ambiguous, &["t", "t"]),
        ] {
            assert!(score_case(&[invalid], &report(vec![field("s", None, &[])], false)).is_err());
        }
    }
}
