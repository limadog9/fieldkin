# Global assignment diagnostics

Fieldkin can optionally explain alternatives to the global one-to-one mapping.
Enable `Config::one_to_one` and set `Config::global_diagnostics.max_solves` above
zero. The default remains zero: ordinary matching performs no extra solves.
These diagnostics never change scores, selections, abstention, or candidate
truncation. Local ambiguity remains a constraint on the assignment matrix.

```rust
use fieldkin::{Config, GlobalDiagnosticsConfig, MatchEngine};

let engine = MatchEngine::new(Config {
    one_to_one: true,
    global_diagnostics: GlobalDiagnosticsConfig {
        max_solves: 16,
        max_work: 8_388_608,
        objective_margin: 0.08,
    },
    ..Config::default()
})?;
# Ok::<(), fieldkin::MatchError>(())
```

## Reading the report

`MatchReport::assignment_diagnostics` has an explicit status:

- `Disabled`: no diagnostic solves were requested.
- `NotApplicable`: one-to-one assignment was not enabled.
- `Complete`: every selected real edge was probed. An empty alternative list
  excludes competing mappings within the configured objective margin and float
  tolerance, for this eligibility-constrained matrix.
- `BudgetExhausted`: at least one selected edge could not be probed. The available
  witnesses are still valid; an empty list cannot establish uniqueness.

When analysis is applicable and enabled, `base_objective` is the original total
score. Each witness reports its total objective, the loss from the original, and
stable-ID changes. A target of `None` means unmatched. Unlisted fields retain
their original assignments. Returned witnesses are distinct, sorted by decreasing
objective and then by stable-ID changes. They represent optimal mappings for
individual probes; they do not enumerate all possible competing mappings.

The objective is a sum of heuristic scores. Its gap is not a confidence
probability, a semantic guarantee, or directly comparable across schema sizes.
`Complete` refers to this mathematical analysis under the current eligibility
and local-abstention constraints. It does not establish that the mapping is right
for an application's business meaning. Diagnostic analysis cannot recover fields
that the scoring policy already excluded.

## Method and completeness

Starting with the selected mapping, the analyzer visits each selected real edge
in stable source-ID order, forbids that one edge, and solves the original matrix
again. Only alternatives whose objective loss is at most `objective_margin` are
returned. Comparisons add `64 * f64::EPSILON * max(1, source_count)` to allow for
floating-point summation noise. Reported gaps are clamped to zero for this noise.

Every distinct real mapping omits at least one selected real edge: a mapping that
retained all selected edges and merely added a positive edge would have a greater
objective, contradicting the original mapping's optimality. Therefore the best
alternative appears among these probes, subject to ordinary floating-point
precision. When all probes finish, an empty witness list rules out an alternative
within the stated margin. Internal permutations of zero-weight dummy columns
all mean unmatched and are deliberately ignored.

At most one representative optimum is returned per probe, and duplicate mappings
are removed. Equal-objective alternatives can remain unenumerated even when the
analysis is complete. Reordering schemas does not change results: both the engine
and analyzer operate in stable field-ID order.

## Explicit work and memory bounds

With `n` sources and `m` targets, an additional Hungarian solve is charged
`n * n * (m + n)` work units before it begins. This follows the solver's
algorithmic upper bound; it is not a CPU instruction count or a wall-clock
deadline. Checked arithmetic prevents overflow. A solve never starts if it would
exceed either `max_solves` or `max_work`. The engine rejects `max_solves > 1_024`
and negative or nonfinite objective margins.

One scratch matrix is reused across probes. Its memory is `O(n*m)`. Solver
auxiliary space is `O(n+m)`; stored witnesses and duplicate tracking add
`O(k*n)` for `k` completed probes. Existing field, pair, sample and explanation
budgets still apply to matching. Diagnostic work is additive and separately
reported as `solves_used` and `work_used`.

| Matrix | Work per probe | Probes for dense completion | Full charged work | Probes permitted by default work budget |
| --- | ---: | ---: | ---: | ---: |
| 16 × 16 | 8,192 | 16 | 131,072 | 16 (complete) |
| 64 × 64 | 524,288 | 64 | 33,554,432 | 16 |
| 128 × 128 | 4,194,304 | 128 | 536,870,912 | 2 |
| 128 × 64 | 3,145,728 | 64 | 201,326,592 | 2 |
| 64 × 128 | 786,432 | 64 | 50,331,648 | 10 |

The final column assumes a sufficient nonzero solve budget and the default
8,388,608 work budget. No budget is spent when enabled analysis has no selected
real edges; that analysis is vacuously complete.

## Reproducible cost experiment

Run:

```console
cargo +1.85.0 run --locked --release --example diagnostics_cost
```

The public-API example uses deterministic synthetic dense matrices with equal
0.9 scores and a fixed custom matcher. Names and samples do not confound this
cost fixture. It measures complete API calls, including report construction and
the original assignment, across the five dimensions above. Each dimension runs
disabled diagnostics, an exhausted one-probe budget, the default work budget,
and enough work for complete analysis. Each mode gets one warmup and three
measured calls. Every call asserts identical field reports, selections and
unmatched sets to disabled diagnostics.

CSV includes dimensions, configuration, actual solve/work counts, witness counts,
completeness, iteration, elapsed nanoseconds, debug-build flag, crate version,
OS and architecture. Capture the compiler, revision, machine and command alongside
results. Repeating independent release processes provides a more useful spread
than one noisy timing run. These deliberately contested fixtures show the cost of
enabled analysis; they do not establish a general production workload latency or
a performance comparison against another library.

## Tests and limits

The unit oracle exhaustively checks rectangular matrices up to 3 × 3 over absent,
0.5 and 1.0 edges against independently enumerated real mappings. A fixed-seed
campaign adds 512 fractional-score graphs up to 5 × 5. Dedicated tests exercise
equal/near objectives, unmatched transitions, empty graphs, duplicate witnesses,
exact work boundaries and deterministic nonmutation. Public integration tests
cover reordering, top-k presentation, unchanged decisions, local abstention,
nonapplicable mode and budget exhaustion.

Caller-confirmed or forbidden pairs are deferred. Applications may retain review
decisions separately; this stage adds no data rewriting, incremental solver, or
automatic acceptance of a competing mapping.
