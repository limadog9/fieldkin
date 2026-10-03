# Repeated cost measurements

This unpublished, isolated development package measures the public Fieldkin API.
It is excluded from the library archive and normal workspace dependency graph.
All fixtures are original synthetic data under MIT OR Apache-2.0. No input files,
randomness, network access or sample logging occur in the benchmark process.

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
python performance/run.py build --baseline .local/stage2-baseline --candidate . --output performance/results/stage2-v1
python performance/run.py run --output performance/results/stage2-v1
```

The worktree needs to be created only once. Build copies these exact harness files
into the baseline checkout, without editing its library sources. The `build`
command compiles both revisions using `cargo +1.85.0 build --release --locked` and
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
cargo +1.85.0 fmt --manifest-path performance/Cargo.toml -- --check
cargo +1.85.0 clippy --locked --manifest-path performance/Cargo.toml --all-features -- -D warnings
cargo +1.85.0 test --locked --manifest-path performance/Cargo.toml --all-features
cargo +1.85.0 run --release --locked --manifest-path performance/Cargo.toml -- --smoke
cargo +1.85.0 run --release --locked --manifest-path performance/Cargo.toml --features allocations -- --smoke
```

`--smoke` runs one warm-up and one measured call, useful for checking workload
success/rejection paths; its timing is not a substitute for the protocol. The
library's normal input limits remain unchanged. This harness offers no persistent
prepared-schema API and does not imply that engine reuse eliminates per-call
preparation. Decisions and explanations are checked separately by the frozen
evaluation and equivalence tests; this tool measures cost only.
