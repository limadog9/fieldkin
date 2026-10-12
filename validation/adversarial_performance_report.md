# Difficult-input scalability follow-up

These are exact-behavior performance changes, independent of matching accuracy.
The baseline is `37375cec20555d429bee29f3b8f42cd5ddfb10d4`; the same benchmark
harness was built against both implementations. Frozen accuracy labels,
thresholds, and reference fixtures are unchanged.

## Changes and guarantees

- **Jaccard:** return similarity 1 when the sorted, nonempty lexical value sets
  are identical: every value already has an exact partner. Cosine still computes
  its actual dot products. Unequal sets retain the original comparison path;
  there is no new hit vector, cache, score rule, or cutoff change.
- **DistributionBased:** recognize connected components that are complete
  directed positive cliques. This is the unique zero-cost binary separation
  matrix, satisfying every triangle constraint, so allocating and solving the
  integer model is unnecessary. This replaces the old all-positive/all-negative
  special cases. Conflicting or asymmetric components use the unchanged solver.
- **Stress testing:** extend the existing generator/runner with high-entropy
  text, interleaved distributions, circular overlapping value sets, and an
  optional Unix child address-space limit. No new library API or dependencies.

Jaccard can still require every distinct-value pair, especially disjoint strings
of comparable length. Distribution still needs quadratic distance matrices and,
for a conflicting component of `n` columns, `n(n−1)` binary variables and
`n(n−1)(n−2)` triangle constraints. Integer solving has no practical worst-case
time guarantee. These changes do **not** make arbitrary inputs resource-bounded.

For untrusted or potentially huge work, enforce deadlines and memory limits in
an isolated calling process. Allocation exhaustion can abort that process;
Fieldkin does not promise a recoverable allocation error or partial approximate
result. Existing explicit row sampling limits values and changes the input;
it does not limit column/component counts or guarantee solver deadlines.

## Method and complete results

Measured 2026-10-12: Linux 6.18.44 x86-64, glibc 2.41, Intel Xeon Platinum 8573C,
five visible CPUs/four-CPU cgroup quota, 32 GiB container limit, Rust 1.99.0
(`b940084d7`), Python 3.12.14, normal release profile, no optional features.
The shared host was not isolated or CPU-pinned. Other repository builds/tests
were paused during the final measurements; host scheduling still varied markedly.

Both versions use algorithm defaults, full data, and one worker. Inputs are
created before timing; timed calls include table preparation and result creation.
Each process performs one unmeasured warmup and three timed matches. Versions
alternate across three fresh processes. Table entries are the median of process
medians with their minimum–maximum range. Linux `VmHWM` is process-lifetime peak
RSS, including generated inputs, warmup and allocator retention, read before
hashing. It is not incremental matcher memory. Small RSS differences are noise.

Each child has a 15-second wall deadline and a **512 MiB virtual address-space**
limit; that limit is not an RSS limit. Raw measurements, parameters, binary
SHA-256 values, environment, and errors are in
[`adversarial_performance_results.json`](adversarial_performance_results.json).
All 14 fully comparable workloads have identical fingerprints across versions
and processes, covering ordered pairs, exact score/component bits and existing
result selections. Three baseline 64-column interleaved processes aborted on
allocation; all optimized processes completed. This is **87/90 complete
processes**, not a fully passing performance run; no baseline time, memory, or
speedup is invented for the failed workload.

| Workload | Before ms [range] | After ms [range] | Peak RSS MiB before → after |
|---|---:|---:|---:|
| distribution-base | 5.363 [2.254–6.820] | 2.032 [1.860–7.104] | 3.59 → 3.08 |
| distribution-interleaved8 | 9.703 [5.444–9.805] | 3.696 [3.671–22.923] | 5.62 → 3.09 |
| distribution-interleaved16 | 36.249 [31.840–47.476] | 37.876 [13.910–59.486] | 24.06 → 3.50 |
| distribution-interleaved32 | 399.681 [354.957–469.443] | 60.906 [53.191–137.068] | 172.20 → 4.16 |
| distribution-interleaved64 | allocation failure | 340.876 [193.044–442.379] | unavailable → 5.81 |
| distribution-ring8 | 309.118 [294.825–411.250] | 361.859 [253.536–458.575] | 7.37 → 7.42 |
| distribution-ring12 | 611.178 [391.009–624.469] | 608.991 [357.009–671.423] | 17.11 → 17.64 |
| distribution-ring16 | 2,726.379 [1,846.586–3,945.625] | 2,572.839 [2,533.846–2,708.153] | 34.64 → 35.15 |
| jaccard-base | 2.823 [2.801–2.849] | 2.902 [2.800–3.063] | 2.84 → 2.82 |
| jaccard-overlap0 | 2.779 [2.773–3.760] | 2.993 [2.729–7.926] | 2.84 → 2.80 |
| jaccard-distinct512 | 870.408 [781.538–873.250] | 482.526 [475.505–586.725] | 3.34 → 3.30 |
| jaccard-codes512 | 10.476 [10.190–10.940] | 9.141 [9.038–10.068] | 3.51 → 3.45 |
| jaccard-single2048-overlap0 | 577.623 [561.482–685.639] | 684.802 [568.761–688.531] | 3.00 → 3.07 |
| jaccard-single2048-overlap50 | 687.811 [558.931–796.897] | 676.958 [574.464–758.223] | 3.15 → 3.07 |
| jaccard-single2048-overlap100 | 560.979 [558.247–757.584] | 0.231 [0.228–0.234] | 3.11 → 2.98 |

Interleaved workloads have two tables, the named width per table, 128 rows and
128 distinct numeric values. Columns span similar distributions but have
separate interleaved value sets; corresponding columns are identical. They
expose wasted model construction for a solved clique partition. Ring workloads
share half their circular vocabulary with adjacent columns, scattering those
values across ranks with a fixed hash. They exercise conflicting clustering;
no speed improvement is claimed. Hash workloads have one column per table,
2,048 distinct 32-character values, and the named exact overlap. Mixed and dense
code controls retain the previous generator unchanged.

The robust improvements are elimination of the identical-set Jaccard search
(**560.979 → 0.231 ms**, 99.96% reduction on this workload) and Distribution's
model allocation for complete partitions: width 32 **399.681 → 60.906 ms**
(84.76% reduction), **172.20 → 4.16 MiB** peak RSS (97.58% reduction). Width 64
now completes within the imposed limit. Width 16's measured median is slightly
slower despite its memory reduction; that unfavorable result is retained.

Unequal Jaccard sets do not benefit from the shortcut. Disjoint large-set timing
increased **577.623 → 684.802 ms**; partial-overlap timing was **687.811 →
676.958 ms**. Neither establishes an improvement on those workloads. The main
small-disjoint control was also slower. A follow-up with **21 timed matches and
five fresh processes per version**, otherwise identical, measured **2.827 →
2.760 ms** (effectively unchanged). Dense-code timing changed direction: the main
batch measured **10.476 → 9.141 ms**, while the follow-up measured **10.534 →
11.931 ms** (13.26% slower). The negative observations are retained, not discarded
as inconvenient noise. Large overlapping per-process ranges preclude a universal
speed claim; the convincing gain is the eliminated identical-set search.

An exact-partner seeding prototype was discarded: its extra scan/allocation
and unstable dense-control regressions did not justify preserving partial-overlap
speedups. The retained shortcut is five lines including comments. Raw final
Jaccard and control results have their own binary fingerprints. Distribution was
measured before that simplification, with the **same retained Distribution code**;
its record explicitly retains only Distribution runs from that batch. No rejected
Jaccard prototype measurement is presented as a result of the final implementation.

## Reproduce

Build the new harness against the original library in a separate worktree:

```sh
git worktree add --detach /tmp/fieldkin-scaling-before 37375cec20555d429bee29f3b8f42cd5ddfb10d4
cp examples/benchmark_performance.rs /tmp/fieldkin-scaling-before/examples/
(cd /tmp/fieldkin-scaling-before && cargo build --locked --release --example benchmark_performance --jobs 1 --target-dir /tmp/fieldkin-scaling-target)
cargo build --locked --release --example benchmark_performance --jobs 1
python3 validation/benchmark_performance.py \
  --baseline-binary /tmp/fieldkin-scaling-target/release/examples/benchmark_performance \
  --case distribution-base --case distribution-interleaved8 \
  --case distribution-interleaved16 --case distribution-interleaved32 \
  --case distribution-interleaved64 --case distribution-ring8 \
  --case distribution-ring12 --case distribution-ring16 \
  --case jaccard-base --case jaccard-overlap0 --case jaccard-codes512 \
  --case jaccard-distinct512 --case jaccard-single2048-overlap0 \
  --case jaccard-single2048-overlap50 --case jaccard-single2048-overlap100 \
  --repetitions 3 --rounds 3 --timeout 15 --memory-limit-mib 512 \
  --output target/adversarial-performance.json
```

The comparison exits nonzero when a child fails, times out, or changes outputs;
the recorded baseline allocation failures therefore intentionally produce exit 1.
Do not suppress that exit and call the run successful. `--memory-limit-mib`
requires Unix `RLIMIT_AS`; omitting it leaves only the process deadline. The
runner applies limits only in children, never to its own process. Reproduce the
control follow-up with just `--case jaccard-overlap0 --case jaccard-codes512`,
`--repetitions 21 --rounds 5`, and the same binaries, limits, and a fresh output.

Correctness regression coverage includes full-Cartesian lexical/cosine oracles
with bitwise scores, Unicode/cutoff boundaries, noncontiguous clique partitions
with isolated nodes, deterministic stress-input intersection checks, and the
existing pinned Python fixtures. Broader validation is recorded below.

## Validation and CI

Final validation passed:

- `cargo fmt --all -- --check`
- `cargo test --locked --all-targets --jobs 1`: 173 tests.
- `cargo test --locked --all-targets --features polars --jobs 1`: 185 tests.
- `cargo test --locked --doc`: four doctests, including README examples.
- Strict all-targets Polars Clippy and warning-free Polars rustdoc.
- `evaluate_all --check`, `--check-independent`, and `--check-reliability`;
  both development and independent Valentine accuracy runs, including the
  independent frozen gate. Corpus checksums and offline extraction passed.
- All 90 parity cases against the saved corrected pinned Python reference.
  All 9,199 correspondence pairs and score bits also equal the pre-task Rust
  output; timing fields are excluded from this comparison.
- Python parity/benchmark helper tests and
  `cargo package --locked --allow-dirty`, including package compilation.

Normal all-feature tests and Clippy failed during the `ort-sys` build because
the ONNX host `cdn.pyke.io` returned HTTP CONNECT 403. Static all-feature Clippy
passed with `ORT_SKIP_DOWNLOAD=1`; this does **not** validate ONNX execution.
No Windows execution was performed locally; the existing Windows CI job remains.

The workflow now reports real `format`, `clippy`, and `docs` checks plus a `test`
check that requires both Linux and Windows tests to succeed. Its aggregation
rejects failed, cancelled, skipped, and empty results. Actionlint and these
status conditions passed validation. GitHub currently reports only
`test (ubuntu-latest)` and `test (windows-latest)` for the previous workflow.
Branch-protection API reads were denied, so the replacement names follow the
user-reported requirements; repository settings were not changed.

Semantic scoring remains unchanged. The separate
[scoring review](reliability_followup_report.md#subsequent-scoring-review-2026-10-12)
records the rejected accuracy experiments and unresolved annual/monthly false
match. Neither parity nor these performance improvements establish better
semantic matching accuracy, and no new held-out evaluation is claimed.
