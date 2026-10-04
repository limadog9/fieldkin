# Requiring corroborating evidence

The optional `Config::corroboration` gate lets an application require both name
support and distinct-aware sampled-value support before a pair is eligible.
It does not raise scores or turn scores into probabilities. The ordinary weighted
policy remains the default, including when samples are unavailable.

```rust
use fieldkin::{Config, Corroboration, MatchEngine};

let engine = MatchEngine::new(Config {
    corroboration: Some(Corroboration::default()),
    ..Config::default()
})?;
# Ok::<(), fieldkin::MatchError>(())
```

`Corroboration::default()` requires a **positive** raw name score and a raw sample
score of at least **0.50**. The name floor defaults to zero; positive evidence is
still required. These are inclusive heuristic floors, applied before weighting.
Both scores must be positive. The name floor may be
configured in `[0, 1]`; the sample floor must be in `(0, 1]`. Non-finite floors
are invalid configuration. The overall weighted threshold, type veto, semantic
conflicts, local ambiguity and partial assignment rules still apply.

Only active concrete built-in `NameMatcher` and distinct-aware `SampleMatcher`
signals can meet the gate. A custom matcher named `name` or `samples`, a zero
weight, the historical `Legacy` sample policy, or profile similarity cannot
substitute. Missing signals fail the corresponding requirement. Custom signals
still contribute their configured scores and retain their existing callback
contract. The gate reuses already prepared evidence; it performs no extra matcher
callbacks or sample scans.

Candidates retain their scores and ranking even if the gate excludes them.
`CandidateIssue::InsufficientNameSupport` and `InsufficientSampleSupport` explain
exclusions without exposing values. Inspect eligibility and selected proposals,
not just the largest score. Applying an eligibility gate can remove a competing
alternative and resolve a local tie, or change the global assignment; new
selections are therefore not necessarily a subset of old selections. Top-k
presentation limits do not affect this analysis.

This policy fits applications where representative overlapping samples are
available and unsupported proposals are more costly than missed matches. It
loses coverage when matching fields have disjoint samples, many nulls, constant
values, few observations or different sample representations. Reused identifiers,
coincidental overlap and semantically distinct fields with identical names and
values can still pass. Agreeing currency or unit hints do not satisfy the gate:
gross and net amounts can use the same currency. Conflicting hints still exclude
a pair, and verified hints remain caller-owned evidence.

The corrective experiment compares the existing weighted policy, this sample
gate, a stricter ablation with a **0.80** name floor, and true name-only matching.
Experimental settings and new synthetic family partitions are committed before
policy implementation. Fixture labels are authored separately without matching
outputs and committed before evaluation; the fresh holdout is scored only after
a feature freeze.
Previously examined families are explicitly regression data. The existing
95% precision and 60% unique-coverage targets are retained, including any misses.
See [the protocol](../evaluation/corrective-protocol.json). No default is promoted
based solely on these synthetic results. Published regression analysis informed
the opt-in preset before the protocol freeze: a high name floor can
discard true partial renames while retaining false exact-name matches. Stricter
lexical requirements do not necessarily improve precision.
