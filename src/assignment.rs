//! Optional maximum-weight bipartite assignment.

/// Maximizes the total score while permitting every source to stay unmatched.
///
/// Rows and columns must arrive in stable field-ID order. Ties are reproducible:
/// rows are processed in order, reduced-cost ties visit the earliest column, and
/// equal predecessor paths retain their first occurrence. Real target columns
/// precede dummy unmatched columns. This is an algorithmic tie convention, not a
/// perturbation of the supplied scores or a promise of lexicographic optimality.
///
/// The caller bounds the dimensions and supplies scores in `(0, 1]`. Missing,
/// nonfinite, and nonpositive entries are forbidden. Short rows are treated as
/// having missing entries. With `n` sources and `m` targets, this rectangular
/// Hungarian kernel uses `O(n²(m + n))` time and `O(m + n)` auxiliary space.
/// The extra `n` zero-weight dummy columns make unmatched choices available even
/// when every real pairing is forbidden. Arithmetic uses the original `f64`
/// scores, so numerical precision is that of floating-point addition.
///
/// Target columns with no usable edge are omitted before solving. Discovery
/// takes at most `O(nm)` time. If `b` columns remain, the projected matrix needs
/// `O(nb)` storage and the kernel takes `O(n²(b + n))` time. All source rows and
/// the original dummy count remain: dropping unmatched rows can change ties.
/// The original dimensions still bound the work.
pub(crate) fn solve(scores: &[Vec<Option<f64>>]) -> Vec<Option<usize>> {
    let row_count = scores.len();
    if row_count == 0 {
        return Vec::new();
    }
    let target_count = scores.iter().map(Vec::len).max().unwrap_or(0);
    if target_count == 0 {
        return vec![None; row_count];
    }

    // A positive cyclic diagonal certifies that every column participates in the
    // eligible graph. Common square inputs take this allocation-free path
    // without inspecting the rest of the matrix. Failure only means that a full
    // scan is needed; it says nothing about which vertices can be omitted.
    if (0..target_count)
        .all(|index| usable_score(scores[index % row_count].get(index).copied().flatten()))
    {
        return solve_dense(scores);
    }

    let columns = active_columns(scores, target_count);
    if columns.is_empty() {
        return vec![None; row_count];
    }
    if columns.len() == target_count {
        return solve_dense(scores);
    }

    // A dead real column has infinite slack on every path, is never visited,
    // and never updates a row potential or predecessor. Removing only those
    // columns preserves every finite-slack comparison in its original order.
    // Keep every row and the full dummy sequence, even for unmatchable rows.
    let projected: Vec<Vec<Option<f64>>> = scores
        .iter()
        .map(|row| {
            columns
                .iter()
                .map(|&column| row.get(column).copied().flatten())
                .collect()
        })
        .collect();
    let mut assignment = solve_dense(&projected);
    for target in &mut assignment {
        *target = target.map(|column| columns[column]);
    }
    assignment
}

fn usable_score(score: Option<f64>) -> bool {
    score.is_some_and(|score| score.is_finite() && score > 0.0)
}

fn active_columns(scores: &[Vec<Option<f64>>], target_count: usize) -> Vec<usize> {
    let mut active_columns = vec![false; target_count];
    for entries in scores {
        for (column, &score) in entries.iter().enumerate() {
            if usable_score(score) {
                active_columns[column] = true;
            }
        }
    }
    active_columns
        .into_iter()
        .enumerate()
        .filter_map(|(column, active)| active.then_some(column))
        .collect()
}

// Keep the original Hungarian implementation shared by both execution paths and
// use it as the exact-selection reference in tests. Both paths preserve its full
// row count and unmatched dummy count.
fn solve_dense(scores: &[Vec<Option<f64>>]) -> Vec<Option<usize>> {
    let row_count = scores.len();
    if row_count == 0 {
        return Vec::new();
    }
    let target_count = scores.iter().map(Vec::len).max().unwrap_or(0);
    let column_count = target_count + row_count;

    // Index zero is the Hungarian algorithm's temporary unmatched root.
    let mut row_potential = vec![0.0; row_count + 1];
    let mut column_potential = vec![0.0; column_count + 1];
    let mut matched_row = vec![0; column_count + 1];
    let mut predecessor = vec![0; column_count + 1];

    for row in 1..=row_count {
        matched_row[0] = row;
        let mut current_column = 0;
        let mut minimum_slack = vec![f64::INFINITY; column_count + 1];
        let mut visited = vec![false; column_count + 1];

        loop {
            visited[current_column] = true;
            let current_row = matched_row[current_column];
            let mut delta = f64::INFINITY;
            let mut next_column = 0;

            for column in 1..=column_count {
                if visited[column] {
                    continue;
                }
                let cost = if column > target_count {
                    0.0
                } else {
                    scores[current_row - 1]
                        .get(column - 1)
                        .copied()
                        .flatten()
                        .filter(|score| score.is_finite() && *score > 0.0)
                        .map_or(f64::INFINITY, |score| -score)
                };
                let slack = cost - row_potential[current_row] - column_potential[column];
                if slack < minimum_slack[column] {
                    minimum_slack[column] = slack;
                    predecessor[column] = current_column;
                }
                // Strict comparison preserves the earliest column on ties.
                if minimum_slack[column] < delta {
                    delta = minimum_slack[column];
                    next_column = column;
                }
            }

            // A free dummy column always supplies a finite augmenting path.
            for column in 0..=column_count {
                if visited[column] {
                    row_potential[matched_row[column]] += delta;
                    column_potential[column] -= delta;
                } else {
                    minimum_slack[column] -= delta;
                }
            }
            current_column = next_column;
            if matched_row[current_column] == 0 {
                break;
            }
        }

        loop {
            let previous_column = predecessor[current_column];
            matched_row[current_column] = matched_row[previous_column];
            current_column = previous_column;
            if current_column == 0 {
                break;
            }
        }
    }

    let mut assignment = vec![None; row_count];
    for (column, &row) in matched_row
        .iter()
        .enumerate()
        .take(target_count + 1)
        .skip(1)
    {
        if row != 0 {
            assignment[row - 1] = Some(column - 1);
        }
    }
    assignment
}

#[cfg(test)]
mod tests {
    use super::{active_columns, solve, solve_dense};

    fn brute_force_score(scores: &[Vec<Option<f64>>]) -> f64 {
        fn search(scores: &[Vec<Option<f64>>], row: usize, used: &mut [bool]) -> f64 {
            if row == scores.len() {
                return 0.0;
            }
            let mut best = search(scores, row + 1, used);
            for (column, score) in scores[row].iter().enumerate() {
                if let Some(score) = score.filter(|score| score.is_finite() && *score > 0.0) {
                    if !used[column] {
                        used[column] = true;
                        best = best.max(score + search(scores, row + 1, used));
                        used[column] = false;
                    }
                }
            }
            best
        }

        let target_count = scores.iter().map(Vec::len).max().unwrap_or(0);
        search(scores, 0, &mut vec![false; target_count])
    }

    fn check_optimal(scores: &[Vec<Option<f64>>]) {
        let assignment = solve(scores);
        assert_eq!(assignment.len(), scores.len());
        assert_eq!(assignment, solve(scores), "assignment must be reproducible");
        assert_eq!(
            assignment,
            solve_dense(scores),
            "projection changed the original tie convention: {scores:?}"
        );
        let target_count = scores.iter().map(Vec::len).max().unwrap_or(0);
        let mut used = vec![false; target_count];
        let mut total = 0.0;
        for (row, target) in assignment.iter().enumerate() {
            if let Some(column) = target {
                assert!(!used[*column], "a target was assigned more than once");
                used[*column] = true;
                let score = scores[row][*column].expect("a forbidden edge was assigned");
                assert!(score.is_finite() && score > 0.0);
                total += score;
            }
        }
        let optimal = brute_force_score(scores);
        assert!(
            (total - optimal).abs() < 1e-12,
            "score {total} differs from optimum {optimal}: {scores:?} -> {assignment:?}"
        );
    }

    #[test]
    fn global_optimum_can_displace_a_rows_top_choice() {
        let scores = vec![vec![Some(0.9), Some(0.8)], vec![Some(0.85), None]];
        assert_eq!(solve(&scores), vec![Some(1), Some(0)]);
        check_optimal(&scores);
    }

    #[test]
    fn equal_scores_follow_stable_input_order() {
        assert_eq!(
            solve(&vec![vec![Some(1.0); 3]; 3]),
            vec![Some(0), Some(1), Some(2)]
        );
        assert_eq!(
            solve(&vec![vec![Some(1.0); 2]; 4]),
            vec![Some(0), Some(1), None, None]
        );
        assert_eq!(solve(&vec![vec![Some(1.0); 4]; 2]), vec![Some(0), Some(1)]);
    }

    #[test]
    fn empty_and_unmatchable_inputs_stay_unmatched() {
        assert!(solve(&[]).is_empty());
        assert_eq!(solve(&[vec![], vec![]]), vec![None, None]);
        assert_eq!(solve(&[vec![None], vec![None]]), vec![None, None]);
        let invalid = vec![vec![
            Some(0.0),
            Some(-1.0),
            Some(f64::NAN),
            Some(f64::INFINITY),
            Some(f64::NEG_INFINITY),
        ]];
        assert_eq!(solve(&invalid), vec![None]);
        check_optimal(&invalid);
    }

    #[test]
    fn missing_entries_in_ragged_rows_are_forbidden() {
        let scores = vec![vec![], vec![Some(0.5)], vec![None, Some(0.75)]];
        assert_eq!(solve(&scores), vec![None, Some(0), Some(1)]);
        check_optimal(&scores);
    }

    #[test]
    fn exhaustive_small_matrices_agree_with_brute_force() {
        // All rectangular matrices up to 3x3 over three possible edge values.
        // This exercises forbidden edges, ties, and every unequal small shape.
        let values = [None, Some(0.5), Some(1.0)];
        for rows in 0..=3 {
            for columns in 0..=3 {
                for encoded in 0..3_usize.pow((rows * columns) as u32) {
                    let mut remainder = encoded;
                    let mut scores = vec![vec![None; columns]; rows];
                    for row in &mut scores {
                        for score in row {
                            *score = values[remainder % 3];
                            remainder /= 3;
                        }
                    }
                    check_optimal(&scores);
                }
            }
        }
    }

    #[test]
    fn seeded_random_matrices_agree_with_brute_force() {
        // Local fixed-seed generator keeps this regression test independent of
        // external RNG implementations and test order.
        let mut state = 0x4649_454c_444b_494e_u64;
        let mut next = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            state >> 32
        };
        for _ in 0..500 {
            let rows = (next() % 6) as usize;
            let columns = (next() % 6) as usize;
            let mut scores = vec![vec![None; columns]; rows];
            for row in &mut scores {
                for score in row {
                    let choice = next() % 12;
                    if choice > 2 {
                        *score = Some((choice - 2) as f64 / 9.0);
                    }
                }
            }
            check_optimal(&scores);
        }
    }

    #[test]
    fn projection_preserves_sparse_target_order() {
        let mut scores = vec![vec![None; 128]; 128];
        let rows = [1, 7, 33, 91, 120];
        let columns = [2, 6, 32, 90, 125];
        for (&row, &column) in rows.iter().zip(&columns) {
            scores[row][column] = Some(0.75);
        }
        assert_eq!(active_columns(&scores, 128), columns.to_vec());
        let mut expected = vec![None; 128];
        for (&row, &column) in rows.iter().zip(&columns) {
            expected[row] = Some(column);
        }
        assert_eq!(solve(&scores), expected);
        assert_eq!(solve(&scores), solve_dense(&scores));
    }

    #[test]
    fn ragged_invalid_entries_do_not_keep_targets_active() {
        let scores = vec![
            vec![],
            vec![Some(f64::NAN), None, Some(0.6)],
            vec![Some(-0.0), Some(f64::NEG_INFINITY)],
            vec![Some(0.0), Some(-1.0), Some(0.6), Some(f64::INFINITY)],
            vec![None; 7],
        ];
        assert_eq!(active_columns(&scores, 7), vec![2]);
        assert_eq!(solve(&scores), vec![None, Some(2), None, None, None]);
        check_optimal(&scores);
    }

    #[test]
    fn inserted_dead_targets_preserve_exact_assignments() {
        let mut state = 0x494e_5345_5254_4544_u64;
        let mut next = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            state >> 32
        };
        let values = [None, Some(0.1), Some(0.3), Some(0.7), Some(1.0)];
        for _ in 0..500 {
            let rows = (next() % 6 + 1) as usize;
            let columns = (next() % 6 + 1) as usize;
            let mut original = vec![vec![None; columns]; rows];
            let mut expanded = vec![vec![Some(0.0); columns * 2 + 1]; rows];
            for (row, entries) in original.iter_mut().enumerate() {
                for (column, entry) in entries.iter_mut().enumerate() {
                    *entry = values[(next() % values.len() as u64) as usize];
                    expanded[row][column * 2 + 1] = *entry;
                }
            }
            let mut expected = vec![None; expanded.len()];
            for (row, target) in solve_dense(&original).into_iter().enumerate() {
                expected[row] = target.map(|column| column * 2 + 1);
            }
            assert_eq!(solve(&expanded), expected, "input: {original:?}");
            assert_eq!(solve_dense(&expanded), expected, "input: {original:?}");
        }
    }

    #[test]
    fn dead_source_rows_can_change_fractional_ties_and_must_be_retained() {
        let mut scores = vec![
            vec![None, None, Some(0.3)],
            vec![Some(1.0), None, None],
            vec![None, Some(0.1), Some(0.3)],
            vec![Some(0.7), None, Some(0.3)],
        ];
        assert_eq!(solve_dense(&scores), vec![Some(2), Some(0), Some(1), None]);

        // An unmatched row can traverse occupied dummy columns and trigger a
        // different real-edge alternating path. Removing it would still give
        // an optimal objective, but would change the existing tie convention.
        scores.push(vec![None; 3]);
        let expected = vec![None, Some(0), Some(1), Some(2), None];
        assert_eq!(solve_dense(&scores), expected);
        assert_eq!(solve(&scores), expected);

        // Also force the column-projection path with the same counterexample.
        for row in &mut scores {
            row.insert(1, None);
        }
        let shifted: Vec<_> = expected
            .into_iter()
            .map(|target| target.map(|column| column + usize::from(column >= 1)))
            .collect();
        assert_eq!(solve(&scores), shifted);
        assert_eq!(solve_dense(&scores), shifted);
    }

    #[test]
    fn fractional_and_near_ties_keep_dense_reference_selections() {
        let mut state = 0x4652_4143_5449_4f4e_u64;
        let mut next = || {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            state >> 32
        };
        let values = [
            None,
            Some(0.0),
            Some(-1.0),
            Some(f64::NAN),
            Some(f64::INFINITY),
            Some(f64::NEG_INFINITY),
            Some(f64::from_bits(1)),
            Some(f64::MIN_POSITIVE),
            Some(0.1),
            Some(0.2),
            Some(0.3),
            Some(0.1 + 0.2),
            Some(0.5),
            Some(0.5 - f64::EPSILON),
            Some(0.5 + f64::EPSILON),
            Some(1.0 - f64::EPSILON),
            Some(1.0),
        ];
        for case in 0..20_000 {
            let rows = (next() % 12 + 1) as usize;
            let columns = (next() % 12 + 1) as usize;
            let live_rows: Vec<bool> = (0..rows).map(|_| next() % 4 != 0).collect();
            let live_columns: Vec<bool> = (0..columns).map(|_| next() % 4 != 0).collect();
            let mut scores = vec![vec![None; columns]; rows];
            for (row, entries) in scores.iter_mut().enumerate() {
                for (column, entry) in entries.iter_mut().enumerate() {
                    if live_rows[row] && live_columns[column] {
                        *entry = values[(next() % values.len() as u64) as usize];
                    }
                }
                // Include missing tails without changing the original ordering.
                if next() % 5 == 0 {
                    entries.truncate((next() % (columns + 1) as u64) as usize);
                }
            }
            assert_eq!(
                solve(&scores),
                solve_dense(&scores),
                "case {case}: {scores:?}"
            );
            if case < 300 && rows <= 5 && columns <= 5 {
                check_optimal(&scores);
            }
        }
    }
}
