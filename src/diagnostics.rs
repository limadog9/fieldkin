//! Optional, bounded explanations of competing global assignments.

use std::collections::BTreeSet;

use crate::{assignment, FieldId};

/// Budgets for optional alternative global assignment analysis.
///
/// Analysis never changes a selection. It is disabled by default and applies
/// only when [`crate::Config::one_to_one`] is enabled. The objective is the sum
/// of heuristic scores, not a probability or a calibrated confidence measure.
/// When caller review constraints are supplied, analysis covers the remaining
/// automatic graph only: fixed confirmations are absent from the objective and
/// are never changed by a witness. Work uses the original schema dimensions.
#[derive(Clone, Debug, PartialEq)]
pub struct GlobalDiagnosticsConfig {
    /// Maximum additional assignment solves. Zero disables analysis.
    ///
    /// The engine accepts at most 1,024. One solve is needed per selected automatic
    /// real edge to complete the analysis, subject also to `max_work`.
    pub max_solves: usize,
    /// Maximum charged work, excluding the original assignment solve.
    ///
    /// Each probe is charged `n * n * (m + n)` units for `n` sources and `m`
    /// targets before it starts. This is a conservative algorithmic work bound,
    /// not a wall-clock deadline or a count of CPU instructions. Overflow or an
    /// insufficient remaining budget stops analysis before the next solve.
    pub max_work: usize,
    /// Largest total-objective loss to include in returned witnesses.
    ///
    /// Must be finite and nonnegative. Comparisons allow an additional
    /// `64 * f64::EPSILON * max(1, n)` for floating-point summation noise.
    pub objective_margin: f64,
}

impl Default for GlobalDiagnosticsConfig {
    fn default() -> Self {
        Self {
            max_solves: 0,
            max_work: 8_388_608,
            objective_margin: 0.08,
        }
    }
}

/// Whether alternative global assignment analysis completed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum AssignmentDiagnosticStatus {
    /// No diagnostic solves were requested (`max_solves == 0`).
    Disabled,
    /// Global one-to-one assignment was not requested.
    NotApplicable,
    /// Every selected automatic real edge was probed within the configured budgets.
    /// Caller-confirmed mappings are fixed and are not probed.
    ///
    /// An empty witness list rules out alternatives within the configured
    /// objective margin and floating tolerance. It does not establish semantic
    /// correctness. Witnesses are representative optimal alternatives per probe;
    /// the report does not enumerate every possible mapping.
    Complete,
    /// A solve or work budget prevented probing at least one selected real edge.
    ///
    /// An empty or nonempty witness list is incomplete. It cannot establish
    /// that the original mapping is the only near-optimal mapping.
    BudgetExhausted,
}

/// One field whose assignment differs in a competing global mapping.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct AssignmentChange {
    /// Stable source field identifier.
    pub source: FieldId,
    /// Target selected by the original assignment; `None` means unmatched.
    pub selected_target: Option<FieldId>,
    /// Target in this alternative; `None` means unmatched.
    pub alternative_target: Option<FieldId>,
}

/// A representative alternative within the configured objective margin.
#[derive(Clone, Debug, PartialEq)]
pub struct AssignmentAlternative {
    /// Sum of the alternative assignment's automatic selected heuristic scores,
    /// excluding fixed caller confirmations.
    pub objective: f64,
    /// Original objective minus this objective, clamped to zero for float noise.
    pub gap: f64,
    /// Changed fields in stable source-ID order, including unmatched choices.
    ///
    /// All other fields retain their original assignments. Changes to internal
    /// dummy columns are omitted because all dummy columns mean unmatched.
    pub changes: Vec<AssignmentChange>,
}

/// Bounded diagnostic evidence about alternatives to a global assignment.
#[derive(Clone, Debug, PartialEq)]
pub struct AssignmentDiagnostics {
    /// Applicability and completeness of the analysis.
    pub status: AssignmentDiagnosticStatus,
    /// Original automatic assignment objective, excluding caller-confirmed pairs,
    /// present whenever analysis was enabled and
    /// global assignment was applicable, including when its budget was exhausted.
    pub base_objective: Option<f64>,
    /// Number of additional assignment solves actually performed.
    pub solves_used: usize,
    /// Charged work for those additional solves.
    pub work_used: usize,
    /// Distinct representative alternatives, ordered by decreasing objective and
    /// then by stable-ID changes. An empty list only excludes near alternatives
    /// when `status` is [`AssignmentDiagnosticStatus::Complete`].
    pub alternatives: Vec<AssignmentAlternative>,
}

impl Default for AssignmentDiagnostics {
    fn default() -> Self {
        Self {
            status: AssignmentDiagnosticStatus::Disabled,
            base_objective: None,
            solves_used: 0,
            work_used: 0,
            alternatives: Vec::new(),
        }
    }
}

impl AssignmentDiagnostics {
    pub(crate) fn not_applicable() -> Self {
        Self {
            status: AssignmentDiagnosticStatus::NotApplicable,
            ..Self::default()
        }
    }
}

fn objective(matrix: &[Vec<Option<f64>>], selected: &[Option<usize>]) -> f64 {
    matrix
        .iter()
        .zip(selected)
        .filter_map(|(row, column)| column.and_then(|column| row.get(column).copied().flatten()))
        .sum()
}

/// Inputs are the validated, stable-ID-ordered matrix and its optimal assignment.
/// Positive real scores and zero-weight dummy columns are the solver contract.
/// Every distinct real assignment omits at least one selected real edge: if it
/// only added a positive edge, the original assignment would not be optimal.
/// Therefore forbidding each selected edge suffices to detect the best distinct
/// alternative without enumerating dummy permutations or all complete mappings.
pub(crate) fn analyze(
    matrix: &[Vec<Option<f64>>],
    selected: &[Option<usize>],
    source_ids: &[FieldId],
    target_ids: &[FieldId],
    config: &GlobalDiagnosticsConfig,
) -> AssignmentDiagnostics {
    if config.max_solves == 0 {
        return AssignmentDiagnostics::default();
    }
    let base_objective = objective(matrix, selected);
    let mut report = AssignmentDiagnostics {
        status: AssignmentDiagnosticStatus::Complete,
        base_objective: Some(base_objective),
        ..AssignmentDiagnostics::default()
    };
    let rows = matrix.len();
    let columns = target_ids.len();
    let work_per_solve = rows
        .checked_add(columns)
        .and_then(|columns| rows.checked_mul(rows)?.checked_mul(columns));
    let tolerance = 64.0 * f64::EPSILON * rows.max(1) as f64;
    let mut seen = BTreeSet::new();
    // Reuse one bounded matrix allocation; restore each forbidden edge before
    // continuing so that every probe constrains exactly one original edge.
    let mut probe = None;
    for (row, &column) in selected.iter().enumerate() {
        let Some(column) = column else { continue };
        let next_work = work_per_solve.and_then(|work| report.work_used.checked_add(work));
        if report.solves_used >= config.max_solves
            || next_work.is_none_or(|work| work > config.max_work)
        {
            report.status = AssignmentDiagnosticStatus::BudgetExhausted;
            break;
        }
        // The matrix and selections are internal validated data. Use checked
        // access anyway so future callers cannot turn a malformed probe into a
        // panic or a false claim of complete analysis.
        let probe = probe.get_or_insert_with(|| matrix.to_vec());
        let Some(edge) = probe.get_mut(row).and_then(|row| row.get_mut(column)) else {
            report.status = AssignmentDiagnosticStatus::BudgetExhausted;
            break;
        };
        let original = edge.take();
        report.solves_used += 1;
        report.work_used = next_work.unwrap_or(config.max_work);
        let alternative = assignment::solve(probe);
        probe[row][column] = original;
        let alternative_objective = objective(matrix, &alternative);
        let gap = (base_objective - alternative_objective).max(0.0);
        if gap > config.objective_margin + tolerance || !seen.insert(alternative.clone()) {
            continue;
        }
        let changes = source_ids
            .iter()
            .zip(selected.iter().zip(&alternative))
            .filter(|(_, (selected, alternative))| selected != alternative)
            .map(|(source, (selected, alternative))| AssignmentChange {
                source: source.clone(),
                selected_target: selected.and_then(|column| target_ids.get(column)).cloned(),
                alternative_target: alternative
                    .and_then(|column| target_ids.get(column))
                    .cloned(),
            })
            .collect();
        report.alternatives.push(AssignmentAlternative {
            objective: alternative_objective,
            gap,
            changes,
        });
    }
    report.alternatives.sort_by(|a, b| {
        b.objective
            .total_cmp(&a.objective)
            .then_with(|| a.changes.cmp(&b.changes))
    });
    report
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(prefix: &str, count: usize) -> Vec<FieldId> {
        (0..count)
            .map(|index| FieldId(format!("{prefix}{index:03}")))
            .collect()
    }

    fn diagnostics(
        matrix: &[Vec<Option<f64>>],
        config: GlobalDiagnosticsConfig,
    ) -> AssignmentDiagnostics {
        let columns = matrix.iter().map(Vec::len).max().unwrap_or(0);
        analyze(
            matrix,
            &assignment::solve(matrix),
            &ids("s", matrix.len()),
            &ids("t", columns),
            &config,
        )
    }

    fn enabled() -> GlobalDiagnosticsConfig {
        GlobalDiagnosticsConfig {
            max_solves: 1_024,
            objective_margin: 1_024.0,
            ..GlobalDiagnosticsConfig::default()
        }
    }

    fn best_distinct_objective(
        matrix: &[Vec<Option<f64>>],
        selected: &[Option<usize>],
    ) -> Option<f64> {
        fn visit(
            matrix: &[Vec<Option<f64>>],
            selected: &[Option<usize>],
            current: &mut Vec<Option<usize>>,
            used: &mut [bool],
            score: f64,
            best: &mut Option<f64>,
        ) {
            if current.len() == matrix.len() {
                if current != selected && best.is_none_or(|best| score > best) {
                    *best = Some(score);
                }
                return;
            }
            let row = current.len();
            current.push(None);
            visit(matrix, selected, current, used, score, best);
            current.pop();
            for (column, edge) in matrix[row].iter().enumerate() {
                if let Some(edge) = edge.filter(|edge| *edge > 0.0) {
                    if !used[column] {
                        used[column] = true;
                        current.push(Some(column));
                        visit(matrix, selected, current, used, score + edge, best);
                        current.pop();
                        used[column] = false;
                    }
                }
            }
        }
        let columns = matrix.iter().map(Vec::len).max().unwrap_or(0);
        let mut best = None;
        visit(
            matrix,
            selected,
            &mut Vec::new(),
            &mut vec![false; columns],
            0.0,
            &mut best,
        );
        best
    }

    #[test]
    fn exhaustive_rectangular_graphs_find_best_distinct_mapping() {
        let values = [None, Some(0.5), Some(1.0)];
        for rows in 0..=3 {
            for columns in 0..=3 {
                for encoded in 0..3_usize.pow((rows * columns) as u32) {
                    let mut remainder = encoded;
                    let mut matrix = vec![vec![None; columns]; rows];
                    for row in &mut matrix {
                        for edge in row {
                            *edge = values[remainder % 3];
                            remainder /= 3;
                        }
                    }
                    let selected = assignment::solve(&matrix);
                    let report = diagnostics(&matrix, enabled());
                    assert_eq!(report.status, AssignmentDiagnosticStatus::Complete);
                    assert_eq!(report.solves_used, selected.iter().flatten().count());
                    assert_eq!(
                        report.work_used,
                        report.solves_used * rows * rows * (columns + rows)
                    );
                    assert_eq!(report.base_objective, Some(objective(&matrix, &selected)));
                    assert_eq!(
                        report
                            .alternatives
                            .first()
                            .map(|alternative| alternative.objective),
                        best_distinct_objective(&matrix, &selected),
                        "{matrix:?}"
                    );
                    let mut mappings = BTreeSet::new();
                    for alternative in report.alternatives {
                        assert!(!alternative.changes.is_empty());
                        assert!(mappings.insert(alternative.changes));
                    }
                }
            }
        }
    }

    #[test]
    fn fractional_contested_graphs_agree_with_independent_oracle() {
        let mut state = 0x464b_4449_4147_0004_u64;
        let mut next = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            state >> 32
        };
        for _ in 0..512 {
            let rows = (next() % 6) as usize;
            let columns = (next() % 6) as usize;
            let mut matrix = vec![vec![None; columns]; rows];
            for row in &mut matrix {
                for edge in row {
                    let value = next() % 12;
                    if value > 2 {
                        *edge = Some((value - 2) as f64 / 9.0);
                    }
                }
            }
            let selected = assignment::solve(&matrix);
            let report = diagnostics(&matrix, enabled());
            let expected = best_distinct_objective(&matrix, &selected);
            let actual = report
                .alternatives
                .first()
                .map(|alternative| alternative.objective);
            match (actual, expected) {
                (Some(actual), Some(expected)) => assert!((actual - expected).abs() < 1e-12),
                (None, None) => {}
                _ => panic!("alternative presence differs for {matrix:?}"),
            }
            assert_eq!(report.status, AssignmentDiagnosticStatus::Complete);
        }
    }

    #[test]
    fn ties_include_unmatched_changes_without_dummy_permutations() {
        let matrix = vec![vec![Some(1.0)], vec![Some(1.0)], vec![None]];
        let report = diagnostics(
            &matrix,
            GlobalDiagnosticsConfig {
                objective_margin: 0.0,
                ..enabled()
            },
        );
        assert_eq!(report.alternatives.len(), 1);
        let alternative = &report.alternatives[0];
        assert_eq!(alternative.objective, 1.0);
        assert_eq!(alternative.gap, 0.0);
        assert_eq!(
            alternative.changes,
            vec![
                AssignmentChange {
                    source: FieldId("s000".into()),
                    selected_target: Some(FieldId("t000".into())),
                    alternative_target: None
                },
                AssignmentChange {
                    source: FieldId("s001".into()),
                    selected_target: None,
                    alternative_target: Some(FieldId("t000".into()))
                },
            ]
        );
    }

    #[test]
    fn duplicate_probe_witnesses_are_deduplicated() {
        let report = diagnostics(
            &vec![vec![Some(1.0); 2]; 2],
            GlobalDiagnosticsConfig {
                objective_margin: 0.0,
                ..enabled()
            },
        );
        assert_eq!(report.solves_used, 2);
        assert_eq!(report.alternatives.len(), 1);
        assert_eq!(report.alternatives[0].changes.len(), 2);
    }

    #[test]
    fn solve_and_work_budgets_stop_before_extra_solves() {
        let matrix = vec![vec![Some(1.0); 3]; 3];
        let per_solve = 3 * 3 * (3 + 3);
        for (solves, work, expected) in [
            (2, usize::MAX, 2),
            (10, per_solve - 1, 0),
            (10, 2 * per_solve, 2),
            (10, 0, 0),
        ] {
            let report = diagnostics(
                &matrix,
                GlobalDiagnosticsConfig {
                    max_solves: solves,
                    max_work: work,
                    ..enabled()
                },
            );
            assert_eq!(report.status, AssignmentDiagnosticStatus::BudgetExhausted);
            assert_eq!(report.solves_used, expected);
            assert_eq!(report.work_used, per_solve * expected);
            assert_eq!(report.base_objective, Some(3.0));
        }
        let report = diagnostics(
            &matrix,
            GlobalDiagnosticsConfig {
                max_solves: 3,
                max_work: 3 * per_solve,
                ..enabled()
            },
        );
        assert_eq!(report.status, AssignmentDiagnosticStatus::Complete);
        assert_eq!(report.solves_used, 3);
    }

    #[test]
    fn empty_and_forbidden_matrices_are_complete_without_spending_budget() {
        for matrix in [vec![], vec![vec![]; 3], vec![vec![None; 2]; 3]] {
            let report = diagnostics(
                &matrix,
                GlobalDiagnosticsConfig {
                    max_solves: 1,
                    max_work: 0,
                    ..enabled()
                },
            );
            assert_eq!(report.status, AssignmentDiagnosticStatus::Complete);
            assert_eq!(report.base_objective, Some(0.0));
            assert_eq!(report.solves_used, 0);
            assert_eq!(report.work_used, 0);
            assert!(report.alternatives.is_empty());
        }
    }

    #[test]
    fn disabled_diagnostics_do_not_compute_objective() {
        assert_eq!(
            diagnostics(&[vec![Some(1.0)]], GlobalDiagnosticsConfig::default()),
            AssignmentDiagnostics::default()
        );
    }

    #[test]
    fn margin_is_total_objective_loss_with_documented_float_tolerance() {
        let matrix = vec![vec![Some(1.0), Some(0.96)], vec![Some(0.96), Some(1.0)]];
        assert_eq!(
            diagnostics(
                &matrix,
                GlobalDiagnosticsConfig {
                    objective_margin: 0.08,
                    ..enabled()
                }
            )
            .alternatives
            .len(),
            1
        );
        assert!(diagnostics(
            &matrix,
            GlobalDiagnosticsConfig {
                objective_margin: 0.079,
                ..enabled()
            }
        )
        .alternatives
        .is_empty());
    }

    #[test]
    fn probes_do_not_mutate_the_original_matrix() {
        let matrix = vec![vec![Some(0.9), Some(0.8)], vec![Some(0.85), None]];
        let original = matrix.clone();
        let report = diagnostics(&matrix, enabled());
        assert_eq!(matrix, original);
        assert_eq!(report, diagnostics(&matrix, enabled()));
    }
}
