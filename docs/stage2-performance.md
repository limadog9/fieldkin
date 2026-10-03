# Stage 2: prepared evidence and measured cost

Completed on 2026-10-03. This stage reduces repeated work without changing the
frozen development decisions. It does not improve matching accuracy or resolve
the false proposals documented in the [Stage 1 report](evaluation.md).

## Implementation and behavioral checks

Names and sample sets are prepared once per field within a match call, after
input, pair and signal-work validation. The cache stores normalized tokens,
one-pass aliases, joined names and typed distinct sample sets. Sample text is
borrowed. Nothing is logged or cached across calls. Built-in errors remain
deferred until their original pair/signal evaluation position.

Independent matching now drops undisplayed candidates after each source's
decision and does not build an assignment matrix. Global matching avoids the
initial selection clone that the solver would replace. Sorting uses the same
total score/ID ordering without auxiliary sort allocations. Both modes still
evaluate every pair and charge all explanations against the report budget;
display truncation never changes eligibility, ambiguity or assignment.

The custom `Matcher` interface keeps its pairwise path. Its defaulted, hidden
`as_any` hook lets the engine recognize concrete built-ins safely; custom
implementations need no changes. There is no public prepared-schema API, unsafe
library code, candidate pruning, raised default limit or new runtime dependency.

The development comparison checks all 1,920 model/policy/case prediction records
exactly, including scores, ranks, eligibility, selections, alternatives and
decisions. Corpus, configuration, dependencies and aggregate metrics also match.
Only engine/evaluator source hashes are excluded by the explicit
`--check-behavior` mode. Strict `--check` still checks those hashes. The original
Stage 1 artifacts were not rewritten, and holdout was not rerun or used for tuning.

Eight integration tests additionally compare complete cached and pairwise
reports, including explanations and warnings, and cover aliases, missing/invalid
samples, truncation, both assignment policies, budgets, custom callback order,
disabled signals, empty products and deferred errors. Two signal tests and two
evaluator tests cover preparation and the narrow provenance exception.

## Measurement protocol

Before: Stage 1 main commit
[`00b12b5`](https://github.com/limadog9/fieldkin/commit/00b12b5).
After: implementation and harness frozen before timing at
[`0c8adfd`](https://github.com/limadog9/fieldkin/commit/0c8adfd).
Both used Rust 1.85.0 (`4d91de4e4`), Cargo's release profile and identical locked
runtime dependencies, on an AMD Ryzen 5 5625U, 6 cores/12 logical processors,
Windows build 26200, `x86_64-pc-windows-msvc`.

The isolated [performance package](https://github.com/limadog9/fieldkin/tree/main/performance)
contains 50 deterministic workloads: all 24 original cases, 22 extensions and
four budget failures. It covers short/long names, sparse/dense samples, rejected
pairs, dense competition, unequal sizes, a single pair and empty inputs.

Both revisions were built first. Five independent processes per revision ran
in alternating before/after order, with three warm-up calls per workload.
Latency includes validation, preparation, every pair, reports, optional assignment
and report destruction; it excludes input/engine construction and printing.
The tables summarize medians of five process means, with minimum–maximum spread.
The machine was not isolated and frequency/affinity were not fixed. These are
same-session observations, not confidence intervals or deployment guarantees.

Allocation measurements used another five independent processes per revision,
with one warm-up and one measured call per workload. Only these executables
enabled MIT-licensed `stats_alloc` 0.1.10. Allocation counts exclude reallocation
counts; allocated bytes include initial allocation sizes and positive reallocation
growth. Net reallocation bytes overlap those counters and must not be added to
them. These are allocator requests, not peak heap usage or resident memory.
Instrumented timings were excluded from latency comparisons.

## Results

The 128-by-128 combined workload has 16 samples per field, four nulls and twelve
numbers, with reversed target order and default configuration:

| Assignment | Before median ms [min–max] | After median ms [min–max] | Speedup |
| --- | ---: | ---: | ---: |
| Independent | 96.17 [91.53–105.64] | 25.06 [24.64–29.77] | 3.84× |
| One-to-one | 96.59 [92.52–103.47] | 42.32 [38.56–44.71] | 2.28× |

| Assignment | Allocations per call, before → after | Allocated bytes per call, before → after |
| --- | ---: | ---: |
| Independent | 509,883 → 151,359 | 65,180,112 → 17,626,192 |
| One-to-one | 511,169 → 151,750 | 65,568,496 → 18,196,848 |

Both policies exceed the 2× stretch target for this workload. Allocations fell
70.31%; allocated bytes fell 72.96% and 72.25%, respectively. All 24 core workload
medians improved, with speedups from 1.69× to 3.89×. There was no core median
regression exceeding 10%. The full
[50-workload table](https://github.com/limadog9/fieldkin/blob/main/performance/results/stage2-v1/summary.md)
and adjacent raw JSONL/JSON include every result, not just these two rows.
Build metadata records compiler, machine, revisions, source/lockfile hashes,
commands and binary hashes; the run verifies them before measuring.

There is a measured regression on **report-budget rejection**: a 16-by-16 call
with a one-byte explanation budget took 5.60 µs before and 49.14 µs after, about
8.77× slower. It allocates 30 → 306 times and 5,658 → 45,512 bytes. Eager
preparation now processes all fields before the first explanation exceeds that
budget. Input, pair and signal-budget rejection still happens before preparation;
the report budget remains enforced. We retain the bounded preparation change for
its measured successful-call gains and explicitly accept this failure-path cost.
A tiny report budget is not a substitute for input/work limits. Empty-input
medians increased by at most 2.08%; their absolute times and spread are published.

An additional ignored release test measured just name/sample preparation and
destruction for both sides of the original 128-field sampled fixture. Five batches
of 100 calls after five warm-ups gave 531.875 µs median, range 493.646–725.779 µs,
in one process. This is roughly 2.1% of the optimized independent call's median.
It is an exploratory component measurement, not a benchmark of a cross-call
prepared API; it excludes validation and engine bookkeeping. This evidence does
not justify adding a public cache lifecycle/ownership abstraction now. Revisit
that decision for demonstrated repeated-schema workloads.

## Reproduction and validation

Run from the repository root with a fresh output directory. The worktree needs
to be created only once:

```text
git worktree add --detach .local/stage2-baseline 00b12b5
python performance/run.py build --baseline .local/stage2-baseline --candidate . --output target/stage2-comparison
python performance/run.py run --output target/stage2-comparison
cargo +1.85.0 test --locked --release -p fieldkin --lib signals::tests::prepare_original_128_cost -- --ignored --exact --nocapture
```

The script copies the same harness into the baseline checkout, without changing
its library sources. Keep builds and tests idle during `run`. Later revisions
will naturally have different hashes and may have different performance; use the
frozen after revision above to reproduce this exact implementation.

Validation commands run for this stage:

```text
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo +1.85.0 test --locked --all-features
cargo +1.85.0 doc --locked --no-deps --all-features
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --check-behavior --output evaluation/results/baseline-v1
cargo +1.85.0 fmt --manifest-path performance/Cargo.toml -- --check
cargo +1.85.0 clippy --locked --manifest-path performance/Cargo.toml --all-targets --all-features -- -D warnings
cargo +1.85.0 test --locked --manifest-path performance/Cargo.toml --all-features
cargo +1.85.0 package --locked --allow-dirty -p fieldkin
```

Documentation uses `RUSTDOCFLAGS=-D warnings`. There are 64 passing workspace
tests, including README doctests, plus one performance-harness test covering all
50 successful/rejected calls. The ignored preparation measurement was run
separately. CI preserves the four required check names and read-only permissions;
it checks behavior and harness correctness, with no timing threshold.

The library still has coarse numeric samples, semantic false positives and
limited global ambiguity diagnostics. This stage makes those same decisions
cheaper. Candidate generation for 512–1,000 fields remains deferred; exact
all-pairs behavior and the default 128-field limit are unchanged.
