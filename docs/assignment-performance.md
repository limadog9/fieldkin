# Partial-assignment cost and compatibility

The private assignment solver can omit target columns with no finite positive
edge, run the existing Hungarian core in their original relative order, and expand
the selections back to original indices. A graph with no valid edge returns all
sources unmatched without running the Hungarian core. This helps when review decisions, type
vetoes, local abstention or missing evidence leave much of the matrix unavailable.
Every source row and the original number/order of dummy unmatched columns remain
in a nonempty solve. The fast path keeps the full matrix when all targets remain active.

This is an internal execution change. No new public API, dependency or scoring
policy is introduced. Every original field and pair is validated and evaluated,
including custom matcher callbacks and explanation budgets. Candidate rankings,
target competition and unmatched lists still use the original schema identities.
Columns with any valid positive edge are retained; forbidding one pair
does not remove its target from other sources.

Optional diagnostic probes also use the optimized solver. Their charged work,
floating-point tolerance, probe order and completeness continue to use the
original dimensions. Faster execution does not purchase additional probes under
the same public budget. Confirmed mappings remain outside the automatic objective.

Projection needs a column index map and a temporary compact score matrix.
Its extra space is bounded by `O(n*m)`; the Hungarian core still uses `O(n+m)`
auxiliary space. For a nearly full matrix, copying may outweigh the saved solve
work. Matrices with every target active avoid the compact copy. Latency and allocation results
must therefore be read by workload, not as a universal speedup guarantee.

The compatibility and measurement records below compare the pre-change main
revision `8674c70a6af4e8f9e7ce0a8a02d3ffa7b9f11656` with the frozen candidate.
No reserved evaluation class is scored. This phase does not improve the original
precision/coverage results or qualify Fieldkin for publication.

## Why source rows remain

Exact-reference testing found that even a trailing row with no valid edge can
change a fractional tie under the existing Hungarian traversal. For example,
rows `[_, _, .3]`, `[1, _, _]`, `[_, .1, .3]`, `[.7, _, .3]` select target
indices `[2, 0, 1, unmatched]`. Appending an all-missing row changes the reference
selection to `[unmatched, 0, 1, 2, unmatched]`, at the same objective.
Removing empty source rows would therefore change the established tie behavior.
The permanent regression keeps this case visible. Only unused real target
columns are omitted: their slack is always infinite, so they cannot participate
in an augmenting path or change row potentials.

Historical evaluation files remain immutable. The strict external snapshot check
uses a new versioned result because implementation hashes change; a separate
comparison must verify unchanged predictions, counts, policy and fixture metadata
against the previous snapshot. No source-hash exception is added to `--check`.
