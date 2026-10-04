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

## Frozen implementation and compatibility

Runtime code and measurement tooling were committed at
`180f42b7a655fe904feb8358ff64a1394b51441f` before the recorded qualification,
external evaluation and performance runs on October 4, 2026. The baseline is
`8674c70a6af4e8f9e7ce0a8a02d3ffa7b9f11656`; only the common benchmark and
compatibility harnesses were copied into its separate checkout. Later commits
add results and documentation, without changing the measured implementation.

- The [complete-report comparison](../qualification/results/assignment-v1/compatibility.json)
  covers 1,024 deterministic cases, seed `20261004`, including reordered inputs,
  caller review, rectangular/empty/sparse graphs, fractional and near ties,
  diagnostic limits, budget errors and exact custom-matcher callback order.
  Both revisions emitted identical 5,536,669-byte transcripts, SHA-256
  `5376108d4506acb9c9b84a9bd28413aae3cab989f8d3fc5f92787c378c7e10c1`.
  This compares full reports and errors, beyond the benchmark's count checks.
- The assignment module's 11 tests include exhaustive matrices through 3x3,
  independent objective oracles, 20,000 deterministic fractional/ragged cases,
  500 inserted-column cases and the source-row tie regression above. These are
  regression evidence, not an exhaustive proof for all floating-point inputs.
- The [generated campaign](../qualification/results/assignment-v1/run.json)
  passed 1,200,000 cases, seed `20261006`, in 15.948 seconds of recorded campaign
  time. Each of normalization, malformed schemas, configuration/limits, default
  report invariants, assignment oracle and reviewed-assignment oracle received
  200,000 cases. Its [build record](../qualification/results/assignment-v1/build.json)
  binds the inputs and executable used for this run.
- The [external comparison](../qualification/results/assignment-v1/external-comparison.json)
  confirms identical development predictions and Markdown, and identical JSON
  after accounting for the single changed implementation hash, `assignment.rs`.
  The strict [new snapshot](../evaluation/results/assignment-v1/external/artifacts/development.json)
  still contains all current hashes; the
  [verified run](../evaluation/results/assignment-v1/external/run.json) links to the
  archived [build record](../qualification/results/assignment-v1/external-build.json).
  All 549 development tables were evaluated; 218 holdout tables remain reserved.
  The combined model still makes zero proposals in this annotation-only task.
  No accuracy improvement follows from this solver change.
- Original, Stage 3, release and corrective development regression checks passed.
  Rust 1.85 and 1.99 workspace tests, strict clippy, formatting and documentation
  checks passed locally. There are 218 passing Rust tests across the workspace,
  standalone performance crate and qualification crate, including eight library
  doctests; one manual preparation microbenchmark stays ignored. The new Python
  measurement-protocol suite adds five tests. CI runs the compatibility harness,
  package verification and existing importer/evaluation tests on its supported
  platform/compiler matrix.

## Measurement protocol

Both comparisons use Rust 1.85 release builds on the same Windows x86-64 host.
Builds and tests finished before timing; no measurement was discarded or rerun.
Five independent processes per revision alternate baseline/candidate order. The
27 targeted workloads use one warmup and 10/4/2 measured calls at 16/64/128 fields.
The existing 50-workload protocol additionally measures allocations in five
separate instrumented processes per revision. Construction of inputs and engines
is outside the timers; matching and returned-report destruction are inside.

Raw input and executable hashes, commands and machine details are retained in
the [targeted build record](../performance/results/assignment-v1/solver-cost/build.json)
and [default build record](../performance/results/assignment-v1/default-regression/build.json).
The existing default protocol retains its historical `fieldkin-stage2-v1` name;
its actual revisions are the baseline and freeze above. The assignment protocol
hashes raw bytes; its archive disables Git newline conversion to preserve those
recorded bytes. The older default protocol normalizes text newlines when hashing.
These records detect local input drift, not malicious replacement or a hermetic
build. Fixed workload order, a shared desktop host, no affinity/frequency control
and only five process means limit inference. Min/max observations are not
confidence intervals, and allocated bytes are not peak live memory.

## 27 assignment workloads

[Full targeted records](../performance/results/assignment-v1/solver-cost/summary.json).

At 128 fields, sparse-complete decreased 52.10%, reserved-complete 44.59%, and all-excluded-complete 37.69% by median. Complete cases probe at most four automatic mappings; bounded cases probe two and remain explicitly incomplete. Original-dimensional work charges and all recorded decision/status/objective counts match baseline. The largest targeted median regressions are 16-field all-excluded (+8.34%), 16-field mixed (+8.09%), and 64-field built-in dense (+6.18%).

| Size | Workload | Baseline ms [min, max] | Candidate ms [min, max] | Median change |
| ---: | --- | ---: | ---: | ---: |
| 16 | all-excluded-complete | 0.14198 [0.13547, 0.19661] | 0.15382 [0.14801, 0.16145] | +8.34% |
| 16 | dense-bounded | 0.20334 [0.19762, 0.21924] | 0.20343 [0.19730, 0.22703] | +0.04% |
| 16 | dense-disabled | 0.57334 [0.56127, 0.61649] | 0.56436 [0.54354, 0.64756] | -1.57% |
| 16 | half-excluded-disabled | 0.16088 [0.15457, 0.17175] | 0.16536 [0.15092, 0.19748] | +2.78% |
| 16 | mixed-bounded | 0.13589 [0.12950, 0.18883] | 0.14689 [0.13745, 0.15230] | +8.09% |
| 16 | reserved-complete | 0.16955 [0.16441, 0.23560] | 0.16238 [0.15640, 0.16891] | -4.23% |
| 16 | sparse-complete | 0.13901 [0.13438, 0.14192] | 0.13276 [0.12372, 0.14139] | -4.50% |
| 16 | tall-bounded | 0.04729 [0.04484, 0.05521] | 0.04541 [0.04443, 0.04776] | -3.98% |
| 16 | wide-complete | 0.03106 [0.02985, 0.05338] | 0.02757 [0.02652, 0.02849] | -11.24% |
| 64 | all-excluded-complete | 3.60797 [3.51078, 9.11558] | 2.31010 [2.25357, 2.85395] | -35.97% |
| 64 | dense-bounded | 5.78515 [4.61357, 8.73730] | 4.86768 [4.59195, 5.70475] | -15.86% |
| 64 | dense-disabled | 8.50875 [7.84510, 16.14495] | 9.03465 [7.93345, 10.58452] | +6.18% |
| 64 | half-excluded-disabled | 3.42065 [2.83185, 4.84318] | 2.93250 [2.76560, 3.66830] | -14.27% |
| 64 | mixed-bounded | 3.36137 [2.96385, 3.98263] | 2.69882 [2.62700, 3.24025] | -19.71% |
| 64 | reserved-complete | 7.51987 [6.47527, 14.57305] | 4.37907 [4.05790, 4.66518] | -41.77% |
| 64 | sparse-complete | 6.99793 [5.38750, 7.90773] | 3.74445 [3.56652, 4.48802] | -46.49% |
| 64 | tall-bounded | 1.76667 [1.59058, 3.07518] | 1.35492 [1.32725, 1.53992] | -23.31% |
| 64 | wide-complete | 0.60592 [0.53860, 0.96237] | 0.43812 [0.43105, 0.51558] | -27.69% |
| 128 | all-excluded-complete | 17.81025 [16.96385, 26.41320] | 11.09730 [9.64755, 14.68610] | -37.69% |
| 128 | dense-bounded | 28.27930 [27.69195, 44.33325] | 27.32755 [26.93660, 28.79215] | -3.37% |
| 128 | dense-disabled | 46.40640 [44.09230, 52.18405] | 44.48285 [44.03920, 53.29860] | -4.15% |
| 128 | half-excluded-disabled | 13.82780 [13.35050, 14.45970] | 13.63315 [12.57635, 14.78560] | -1.41% |
| 128 | mixed-bounded | 15.82435 [13.84530, 19.94125] | 13.72955 [12.95280, 16.33450] | -13.24% |
| 128 | reserved-complete | 45.78390 [43.46030, 48.40615] | 25.36865 [23.16795, 30.84240] | -44.59% |
| 128 | sparse-complete | 38.73940 [36.82865, 40.19465] | 18.55475 [18.33400, 18.94015] | -52.10% |
| 128 | tall-bounded | 12.60150 [10.96875, 13.81755] | 8.55345 [8.49790, 8.75485] | -32.12% |
| 128 | wide-complete | 2.80645 [2.75115, 3.83320] | 1.73535 [1.72960, 1.91790] | -38.17% |

Targeted spread is material. The 64-field all-excluded baseline ranged 3.51078-9.11558 ms; reserved baseline ranged 6.47527-14.57305 ms; built-in dense baseline ranged 7.84510-16.14495 ms. These measurements are retained without selecting away slow processes. Sparse and reserved 128-field before/after ranges do not overlap in these five processes, but this is not a statistical or general performance guarantee.

## 50 unchanged default workloads

[Full 50-workload table](../performance/results/assignment-v1/default-regression/summary.md) and [machine-readable summary](../performance/results/assignment-v1/default-regression/summary.json).

All 50 summaries were independently recomputed from 500 timing rows and 500 allocation rows. Five timing and five separate instrumented allocation processes per revision used the historical Stage 2 harness protocol; the generated table title is inherited from that protocol. These are current baseline/candidate measurements.

| Selected workload | Baseline ms [min, max] | Candidate ms [min, max] | Median change |
| --- | ---: | ---: | ---: |
| extended-empty-target-assignment | 2.49564 [2.23786, 2.94552] | 0.03494 [0.03318, 0.04302] | -98.60% |
| extended-dense-assignment | 11.44522 [11.32118, 14.08340] | 14.19590 [11.19920, 14.76224] | +24.03% |
| core-128-empty-assignment-combined | 39.10512 [37.45442, 49.64132] | 46.40676 [35.10532, 51.93012] | +18.67% |
| core-128-sampled-assignment-combined | 41.51124 [39.85576, 46.06242] | 47.69168 [40.87162, 58.48772] | +14.89% |
| core-128-sampled-independent-combined | 28.99764 [27.81644, 36.64666] | 28.12594 [27.78450, 31.44124] | -3.01% |
| extended-sparse-assignment | 9.34786 [8.80024, 12.21112] | 9.79892 [8.96572, 32.67362] | +4.83% |
| extended-unequal-wide-assignment | 4.62588 [4.39748, 5.13692] | 4.86112 [4.57982, 6.09022] | +5.09% |

The largest default median regression is dense assignment: 11.44522 to 14.19590 ms (+24.03%). Empty 128-field combined assignment increases 18.67%; sampled 128-field combined assignment increases 14.89%. Independent paths also fluctuate: sampled 16-field name-only independent matching increases 15.70%. That does not justify dropping the measured regressions or claiming a general speedup. Candidate sparse assignment includes a 32.67362 ms process mean (median 9.79892 ms), and candidate competition-independent includes 12.59716 ms (median 5.46222 ms); both remain in the raw results.

All six allocation/deallocation/reallocation count-and-byte measures are identical for 46 of 50 workloads, across all five allocation processes per revision. All allocation measurements are deterministic within each revision. Four assignment cases differ:

| Workload | Allocations before -> after | Allocated bytes before -> after | Other allocation changes |
| --- | ---: | ---: | --- |
| extended-empty-target-assignment | 671 -> 411 | 210,184 -> 57,448 | deallocations: 671 -> 411; bytes_deallocated: 210,184 -> 57,448 |
| extended-long-assignment | 56,421 -> 56,290 | 6,109,248 -> 6,031,392 | deallocations: 56,421 -> 56,290; bytes_deallocated: 6,109,248 -> 6,031,392 |
| extended-rejected-assignment | 50,853 -> 50,722 | 6,090,560 -> 6,012,704 | deallocations: 50,853 -> 50,722; bytes_deallocated: 6,090,560 -> 6,012,704 |
| extended-unequal-wide-assignment | 22,156 -> 22,175 | 2,914,104 -> 2,900,024 | reallocations: 503 -> 505; deallocations: 22,156 -> 22,175; bytes_deallocated: 2,914,104 -> 2,900,024; bytes_reallocated: 240,384 -> 240,480 |

Empty-target assignment falls from 2.49564 to 0.03494 ms (98.60% lower median); this specific all-empty graph avoids the old dummy-only solve. The unequal-wide case allocates 19 additional objects but 14,080 fewer bytes per call. Allocation totals are not peak-memory measurements.

## Reproduction commands

The commands below were run from the repository root. On this Windows host,
`py -3.11` selects the Python version required by the verified evaluator. Use fresh
output directories for new measurements; archived result directories are not
scratch space. Keep build/test work separate from the measurement phase.

```text
cargo +1.85.0 fmt --all -- --check
cargo +1.99.0 fmt --all -- --check
cargo +1.85.0 test --locked --all-features
cargo +1.99.0 test --locked --all-features
cargo +1.85.0 test --locked --manifest-path performance/Cargo.toml --all-features
cargo +1.85.0 test --locked --manifest-path qualification/Cargo.toml
cargo +1.85.0 clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo +1.99.0 clippy --locked --workspace --all-targets --all-features -- -D warnings
# With RUSTDOCFLAGS=-D warnings:
cargo +1.85.0 doc --locked --no-deps --all-features
cargo +1.99.0 doc --locked --no-deps --all-features
py -3.11 -m unittest discover -s performance -p test_assignment.py
cargo +1.85.0 package --locked -p fieldkin

cargo +1.85.0 run --locked --release --example assignment_compatibility -- --check-only
# For full cross-revision comparison, run without --check-only in each checkout
# with the identical example copied in; compare stdout bytes, not just summaries.

py -3.11 qualification/run.py build --output qualification/results/assignment-v1
py -3.11 qualification/run.py run --output qualification/results/assignment-v1 --cases 1200000 --seed 20261006
py -3.11 evaluation/verified.py build --build-dir target/external-assignment-v1 --toolchain 1.85.0
py -3.11 evaluation/verified.py run --build-dir target/external-assignment-v1 --output evaluation/results/assignment-v1/external
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --external --check --output evaluation/results/assignment-v1/external/artifacts
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --check-behavior --output evaluation/results/baseline-v1
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --stage3 --check-behavior --output evaluation/results/rc-v1/stage3-development
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --release --check-behavior --output evaluation/results/rc-v1/release
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --corrective --check --output evaluation/results/corrective-v1

# Create the detached baseline once; retain its runtime sources unchanged.
git worktree add --detach .local/assignment-baseline 8674c70
py -3.11 performance/assignment.py build --baseline .local/assignment-baseline --candidate . --output performance/results/assignment-v1/solver-cost
py -3.11 performance/run.py build --baseline .local/assignment-baseline --candidate . --output performance/results/assignment-v1/default-regression
# After all builds and tests finish, run these sequentially:
py -3.11 performance/assignment.py run --output performance/results/assignment-v1/solver-cost
py -3.11 performance/run.py run --output performance/results/assignment-v1/default-regression
```

The package remains experimental and unpublished. This change reduces selected
assignment costs; it does not reduce all-pairs evidence work, introduce a sparse
public schema API, promise a wall-clock budget or establish production accuracy.
