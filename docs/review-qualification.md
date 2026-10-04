# External-evidence and caller-review qualification

This cycle delivers the roadmap's follow-up phases 7–8: independently authored
annotation evidence and a caller review API. Ordinary matching remains unchanged.
The original precision/coverage release targets remain unmet; no crate or tag was
published. Caller confirmations are not automatic matching successes.

The external fixture, license, importer, policy and class split were committed at
`bc39722`. Runtime, tests and evaluation/tool implementations were frozen at
`56e01d57662301770bdcd3b61684362eaa5a4e94` before the external development run and
performance measurements. No scoring policy changed after results were inspected.
The external evaluator hashes source files; it does not independently attest its
binary or implement a Git freeze guard. Its current interface cannot score the
reserved holdout at all.

## Delivered behavior and evidence

`MatchEngine::match_schemas_with_constraints` accepts confirmed ID pairs, forbidden
pairs and explicitly unmatched sources. It validates original inputs and retains
all pair evaluations. Confirmations retain actual scores, respect hard semantic
and enabled type vetoes, and are distinguished by `Decision::Confirmed`.
One-to-one mode reserves their targets; assignment diagnostics cover only the
remaining automatic graph with conservative original-dimension work accounting.
See [the API contract](review-constraints.md) and
[compiling importer](../examples/reviewed_import.rs).

The [external development report](external-evaluation.md) covers 549 original T2D
annotated table schemas / 1,451 positive correspondences. Name-only matching
recovers 336 known-positive independent proposals and makes 107 unknown proposals;
the combined default abstains because names alone cannot reach its fixed threshold.
All models retrieve 1,058 positive edges in top five. These induced, positive-only
tasks cannot establish precision or unmatched-field safety. The 218 tables from
19 reserved classes were not scored.

Validation passed **213 Rust tests**, including eight doctests, 23 review tests
and a 256-case independent exhaustive-assignment property, plus **five Python
importer tests**. One manual preparation microbenchmark remains intentionally
ignored. Formatting, strict clippy and warning-free docs pass on Rust 1.85/1.99.
Historical original, Stage 3, release-development and corrective regression
checks preserve default full reports; no historical artifact was rewritten.

The expanded v2 qualification campaign passed **1,200,000 generated cases**, seed
`20261005`, with 200,000 cases in each of six categories. The new reviewed-assignment
category verifies fixed decisions, independent row maxima, exhaustive one-to-one
optimality, unchanged scores, constraints on witnesses, unmatched complements and
reordering. The Rust process took 15.13 seconds; this is a bounded generated-case
campaign, not coverage-guided fuzzing or 1.2 million independent consumer schemas.
See the [exact record](../qualification/results/review-v1/run.json).

## Observed performance

The reference machine is Windows x86-64 with an AMD Ryzen 5 5625U, six cores and
twelve logical processors, using Rust 1.85 release builds. Builds, tests,
qualification and measurements ran in separate phases. No CPU affinity or fixed
frequency was imposed. Brackets below are observed process ranges, not confidence
intervals; host scheduling and wall-clock outliers remain uncontrolled.

The existing 50-workload protocol compares `1519dff` with frozen `56e01d5`, using
five timing and five separate allocation processes per revision. Every measured
default allocation/reallocation/deallocation count and corresponding byte total
is unchanged. Sampled 128-field combined medians are:

| Mode | Before ms [min–max] | After ms [min–max] |
| --- | ---: | ---: |
| Independent | 28.10 [27.00–36.51] | 27.35 [26.99–34.47] |
| One-to-one | 42.95 [42.12–56.72] | 43.63 [41.82–64.47] |

Regressions remain visible: sparse assignment rises 8.99→10.18 ms (+13.25%), and
sparse independent rises 6.38→7.12 ms (+11.68%). A candidate unequal-wide assignment
process has an extreme 7,082.74 ms call mean despite a 4.55 ms median across
processes. This outlier is retained, not discarded or rerun away; its cause was
not established. No general speedup or zero-overhead guarantee follows from this
run. The [complete table](../performance/results/review-v1/default-regression/summary.md)
retains the historical Stage 2 protocol title; its build record identifies the
actual revisions.

A separate 36-workload measurement compares ordinary calls, explicit empty review
and mixed review on the same frozen binary. Mixed review confirms one quarter of
sources, keeps one quarter unmatched and forbids one wrong pair for each remaining
source. It changes report/assignment work while still scoring all pairs. Five
processes use fixed workload order and 30/10/5 timed calls for 16/64/128 fields.
At 128 sampled fields:

| Review input | Independent ms [min–max] | One-to-one ms [min–max] |
| --- | ---: | ---: |
| Ordinary call | 26.20 [25.92–27.17] | 38.78 [37.44–50.39] |
| Explicit empty constraints | 26.19 [25.86–26.46] | 36.43 [35.69–39.57] |
| Mixed review | 27.03 [26.64–27.33] | 42.86 [42.07–43.60] |

Timing includes returned-report destruction and excludes schema/engine/constraint
construction. Empty and ordinary calls use the same code path; timing differences
are observations, not evidence of a faster API. Read
[all 36 workloads and raw runs](../performance/results/review-v1/review-cost/summary.json).

## Reproduce

Use a clean frozen checkout and fresh output directories. Standard tests,
formatting, clippy and documentation commands are in CONTRIBUTING and CI.

```text
cargo +1.85.0 test --locked --all-features
cargo +1.85.0 test --locked --manifest-path qualification/Cargo.toml
cargo +1.85.0 test --locked --manifest-path performance/Cargo.toml --all-features
python -m unittest discover -s evaluation -p test_import_t2d.py
python evaluation/import_t2d.py --check
cargo +1.85.0 run --locked --example reviewed_import
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --external --check --output evaluation/results/t2d-v1
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --corrective --check --output evaluation/results/corrective-v1
python qualification/run.py build --output target/review-qualification
python qualification/run.py run --output target/review-qualification --cases 1200000 --seed 20261005
python performance/review.py build --output target/review-cost
git worktree add --detach .local/review-baseline 1519dff
python performance/run.py build --baseline .local/review-baseline --candidate . --output target/review-default-cost
python performance/review.py run --output target/review-cost
python performance/run.py run --output target/review-default-cost
```

Skip the worktree command if that baseline exists. Finish builds/tests before
measurements. Run Rust checks with `+1.99.0` as well. CI checks all six supported
OS/compiler combinations with read-only credentials and never scores the external
holdout. Source/migration changes and failed accuracy targets remain explicit;
serialization, larger-schema pruning and publication are still deferred.
