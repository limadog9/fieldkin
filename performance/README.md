# Repeated cost measurements

This unpublished, isolated development package measures the public Fieldkin API.
It is excluded from the library archive and normal workspace dependency graph.
All fixtures are original synthetic data under MIT OR Apache-2.0. No input files,
randomness, network access or sample logging occur in the benchmark process.

All measurement drivers and protocol tests now live in native Rust in
[`tooling/src/performance.rs`](../tooling/src/performance.rs). Commands select
the installed stable Rust channel and record the actual Rust and Cargo releases,
the executing runner binary and source hashes, scoped checkout inputs, Cargo
configuration, environment and measured binary hashes. Compiler wrappers,
compiler replacement and cross-target configuration are rejected. Build and run
remain separate; run checks recorded inputs before and after every process.
Outputs are exclusive and never replace earlier records. A fresh baseline must
accept the common harness; an existing different harness is rejected.

Historical measurements under `performance/results` are archival records from
their original toolchains and runners. Their numbers and metadata have not been
rewritten. Native protocol names distinguish new measurements from that archive;
the native commands refuse to run old build records as fresh measurements.
Protocol tests validate archived raw workload shapes without retiming them.

The protocol has 50 workloads: the original 24 combined/name-only combinations
(16/64/128 fields, samples absent or 16 values, independent/one-to-one), plus both
assignment modes for short names, long names, sparse/null-heavy samples, dense
256-value samples, incompatible types, dense competition, and 16-by-128 and
128-by-16 inputs, a single pair, and either empty input orientation. Four additional
cases exercise input, pair, signal and report
budget rejection, checking the exact expected error. Dense competition explicitly
disables ambiguity abstention to exercise the assignment solver. Other settings
are defaults except the original name-only baseline's disabled type veto.

## Reproduce

Use separate baseline and candidate checkouts and a fresh results directory.
The baseline for Stage 2 is the Stage 1 main revision, `00b12b5`.

```text
git worktree add --detach .local/stage2-baseline 00b12b5
cargo +stable run --locked -p fieldkin-tools -- perf-run build --baseline .local/stage2-baseline --candidate . --output target/native-stage2
cargo +stable run --locked -p fieldkin-tools -- perf-run run --output target/native-stage2
```

The worktree needs to be created only once. Build copies these exact harness files
into the baseline checkout, without editing its library sources. The `build`
command compiles both revisions using `cargo +stable build --release --locked` and
records source/lockfile hashes, compiler output, machine information, Git state,
relevant compiler flags and release-profile environment overrides,
build commands and binary hashes. Line endings are normalized only for source
hashes; binary hashes cover exact bytes. The `run` command verifies recorded
sources and binaries still match before measurement. Build and run are separate
so the machine can be idle during measurements. Avoid other builds, tests or
heavy processes. Neither CPU affinity nor a fixed CPU frequency is imposed.

Every revision gets five independent timing processes in alternating order
(before/after, after/before, ...), with three warm-up calls per workload. Each
process reports the mean of 30 calls for 16-field core cases, 10 for 64-field core,
five for 128-field core and extended cases, and 500 for budget rejection cases.
Input and engine construction are excluded. Input validation, per-call evidence
preparation, every pair's evaluation, report construction, optional assignment,
and report destruction are included. Calls/results pass through `black_box`.

After timing, five separate instrumented processes per revision each warm up once
and measure one call per workload. The optional MIT-licensed
[`stats_alloc` 0.1.10](https://docs.rs/stats_alloc/0.1.10/stats_alloc/) wraps the
system allocator in this executable only. We record allocation, deallocation and
reallocation counts, allocated/deallocated bytes and net reallocation-byte changes.
`allocations` counts allocation/zeroed-allocation calls separately from `reallocations`.
`bytes_allocated` sums newly allocated block sizes plus positive size differences
on reallocation; `bytes_deallocated` includes deallocation sizes plus shrinkage.
`bytes_reallocated` is the signed net change requested through reallocation.
It overlaps the allocated/deallocated byte counters and must not be added to them.
Those are allocator requests, **not peak resident memory or live heap size**;
requested allocation bytes do not include allocator bookkeeping or stack memory.
No code copied from that crate and no unsafe Rust added to Fieldkin. Timings from
these instrumented processes are retained as raw output but never used for latency
comparisons. Normal timing builds do not enable the instrumentation dependency.

Machine-readable raw JSONL includes every process and workload. The summary gives
median and min–max spread across the five process means, median absolute deviation,
and allocation medians/ranges. It computes before-median / after-median speedup.
This is a same-machine observation, not a statistical confidence interval,
cross-machine guarantee or claim about other matchers. Shared CI timing must not
be a hard gate. The original one-run timing table is not used as the baseline.

```text
cargo +stable fmt --manifest-path performance/Cargo.toml -- --check
cargo +stable clippy --locked --manifest-path performance/Cargo.toml --all-features -- -D warnings
cargo +stable test --locked --manifest-path performance/Cargo.toml --all-features
cargo +stable run --release --locked --manifest-path performance/Cargo.toml -- --smoke
cargo +stable run --release --locked --manifest-path performance/Cargo.toml --features allocations -- --smoke
```

`--smoke` runs one warm-up and one measured call, useful for checking workload
success/rejection paths; its timing is not a substitute for the protocol. The
library's normal input limits remain unchanged. This harness offers no persistent
prepared-schema API and does not imply that engine reuse eliminates per-call
preparation. Decisions and explanations are checked separately by the frozen
evaluation and equivalence tests; this tool measures cost only.

## Corroboration costs

The public `matching` benchmark includes 36 workloads: 16/64/128 fields, samples
present/absent, independent/one-to-one, and combined/sample-supported/name-only
engines. Its fixed 16-value samples include nulls. The opt-in gate uses the
public `Corroboration::default()`; supplied samples and displayed scores remain
the same, while eligibility and report construction can change.

```text
cargo +stable run --locked -p fieldkin-tools -- perf-corroboration build --output target/native-corroboration-cost
cargo +stable run --locked -p fieldkin-tools -- perf-corroboration run --output target/native-corroboration-cost
```

Build first, then stop concurrent builds/tests before measuring. The runner
records the Cargo-reported executable, source/binary hashes, compiler and build
flags. It checks them before and after five separate processes and rejects
existing outputs. There is one warmup per workload and 30/10/5 timed calls at
16/64/128 fields. Timing includes returned-report destruction and excludes
engine/input setup. Summaries use the median and min/max of five process means.
Order is fixed, without CPU affinity or confidence intervals; these are
descriptive costs, not an optimization claim or a wall-clock budget.

To assess unchanged-default overhead, the existing 50-workload `perf-run` protocol
can separately compare the pre-correction `3b8d65a` checkout against the frozen
candidate, with instrumentation in separate processes. Its generated table
uses a distinct native protocol name; `build.json` identifies the actual
revisions and compiler. Neither cost harness reads accuracy evaluation fixtures.

## Caller-review costs

`examples/review_cost.rs` supplies 36 fixed workloads: 16/64/128 fields, samples
present/absent, independent/one-to-one, and ordinary calls / explicit empty
constraints / mixed review. Mixed review confirms the first quarter of sources,
keeps the second quarter unmatched, and forbids one wrong pair for each remaining
source. Global alternative diagnostics are disabled. All source-target pairs
still receive evidence evaluations.

```text
cargo +stable run --locked --release --example review_cost -- --smoke
cargo +stable run --locked -p fieldkin-tools -- perf-review build --output target/native-review-cost
cargo +stable run --locked -p fieldkin-tools -- perf-review run --output target/native-review-cost
```

The smoke command checks execution only. Build first; run measurements with no
concurrent builds/tests. Five release processes each use one warmup and 30/10/5
calls by size. Timing includes returned-report destruction and excludes schema,
engine and constraint construction. The script verifies exact workload inventory,
compiler/environment/source/binary hashes and exclusive output files. Order is
fixed, no affinity or CPU-frequency control is imposed, and min/max spreads are
process observations rather than confidence intervals. Mixed review changes
eligibility and assignment work as well as adding validation/explanation costs;
it is not a pure optimization comparison.

## Partial assignment before and after

`examples/assignment_cost.rs` supplies 27 graph workloads: 16/64/128 fields,
dense/sparse graphs, partially or fully excluded sources, caller confirmations,
reserved targets and unequal schema sizes. Complete diagnostic cases have four
automatic edges; bounded cases use two probes. Every probe retains the original
dimension-based work charge. These synthetic shapes exercise execution costs,
not schema-matching accuracy, and all original pairs are scored.

```text
git worktree add --detach .local/assignment-baseline 8674c70
cargo +stable run --locked -p fieldkin-tools -- perf-assignment build --baseline .local/assignment-baseline --candidate . --output target/native-assignment-cost
cargo +stable run --locked -p fieldkin-tools -- perf-assignment run --output target/native-assignment-cost
cargo +stable test --locked -p fieldkin-tools performance::tests
```

Skip the worktree command if the baseline already exists. The builder copies only
the common example into the baseline, keeps runtime sources unchanged,
and records fresh isolated binaries and raw input hashes. Measurements alternate
the two revisions across five processes each. Each workload has one warmup and
10/4/2 timed calls by size; timings include matching and report destruction, and
exclude input/engine/constraint construction and result summarization. Decision
counts, diagnostic status, objective and work are checked across revisions.

Finish builds and tests before measurement. The existing 50-workload timing and
allocation protocol provides a separate default-behavior cost comparison. See
[the assignment report](../docs/assignment-performance.md) for results and the
source-row tie behavior that limits the optimization.

## Diagnostics and scale characterization

The diagnostics runner measures the existing 20 dimension/mode combinations,
with one warmup and three measured calls each, across five release processes.
The scale runner retains 128/512/1000-field diagonal, partial and built-in
workloads under both assignment modes. Its explicit limits apply only to the
benchmark; library defaults remain unchanged.

```text
cargo +stable run --locked -p fieldkin-tools -- perf-diagnostics build --output target/native-diagnostics-cost
cargo +stable run --locked -p fieldkin-tools -- perf-diagnostics run --output target/native-diagnostics-cost
cargo +stable run --locked -p fieldkin-tools -- perf-scale build --output target/native-scale-cost
cargo +stable run --locked -p fieldkin-tools -- perf-scale run --output target/native-scale-cost
```

All six commands accept `--help`. Their protocol tests reject changed decision
counts, budgets, objectives, iterations, duplicate or reordered workloads,
malformed durations and pre-existing output. Timing comparisons report medians
and min/max across five process means. They do not establish accuracy, worst-case
latency, a confidence interval or peak memory.
