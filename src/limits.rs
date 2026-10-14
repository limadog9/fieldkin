use crate::Error;

/// Opt-in work-count limits for expensive matching and assignment operations.
///
/// Defaults impose no numeric cap; guarded arithmetic still rejects overflow.
/// These limits are neither process-memory limits nor cancellable deadlines.
/// Use an isolated process when hard memory or time containment is required.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkLimits {
    /// Maximum unique source/target dimension of a Hungarian assignment.
    pub max_assignment_dimension: usize,
    /// Maximum cells in each assignment/distribution matrix, or total Jaccard
    /// column-pair slots across a batch. This is a count, not a byte limit.
    pub max_matrix_cells: usize,
    /// Maximum directed binary variables in each conflicting distribution
    /// clustering model. Zero-cost components need no integer model.
    pub max_clustering_variables: usize,
    /// Maximum triangle constraints in each distribution clustering model.
    pub max_clustering_constraints: usize,
    /// Maximum conservative distinct-value Cartesian comparisons across a
    /// Jaccard batch, before lexical/embedding shortcuts and rejection filters.
    pub max_jaccard_comparisons: usize,
}

impl Default for WorkLimits {
    fn default() -> Self {
        Self {
            max_assignment_dimension: usize::MAX,
            max_matrix_cells: usize::MAX,
            max_clustering_variables: usize::MAX,
            max_clustering_constraints: usize::MAX,
            max_jaccard_comparisons: usize::MAX,
        }
    }
}

pub(crate) fn check_work(operation: &str, count: Option<usize>, limit: usize) -> Result<(), Error> {
    let count = count.ok_or_else(|| {
        Error::InvalidConfig(format!(
            "{operation} workload overflows usize; configured limit is {limit}"
        ))
    })?;
    if count > limit {
        return Err(Error::InvalidConfig(format!(
            "{operation} workload {count} exceeds configured limit {limit}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_limits_distinguish_boundary_excess_and_overflow() {
        assert!(check_work("example", Some(0), 0).is_ok());
        assert!(check_work("example", Some(7), 7).is_ok());
        assert!(check_work("example", Some(usize::MAX), usize::MAX).is_ok());
        for (count, expected) in [(Some(8), "8"), (None, "overflows")] {
            let message = check_work("example", count, 7).unwrap_err().to_string();
            for word in ["example", "7", expected] {
                assert!(message.contains(word), "{message}");
            }
        }
    }
}
