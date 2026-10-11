# Python Valentine comparison and measured Rust optimizations

Measured 2026-10-11. These are implementation and performance comparisons, not
schema-matching accuracy claims. No scoring formulas, thresholds, public APIs,
reference fixtures, evaluation labels or quality baselines changed.

## Reproduce the comparison

Prepare the pinned reference and NLTK data as described in [PORTING.md](../PORTING.md).
The measured dependency wheels were:

```sh
python -m pip install --target .upstream/python \
  numpy==2.3.5 pandas==2.2.3 scipy==1.17.0 networkx==3.7 nltk==3.10.3 \
  rapidfuzz==3.14.6 pulp==3.3.2 POT==0.9.7.post1 anytree==2.13.0 \
  chardet==7.6.0 python-dateutil==2.9.0.post0 defusedxml==0.7.1 \
  click==8.5.0 joblib==1.6.0 regex==2026.9.29 six==1.17.0 tqdm==4.70.1
PYTHONPATH=.upstream/python python -m nltk.downloader -d .upstream/nltk_data wordnet stopwords punkt_tab
```

Use a fresh dependency directory when reproducing the environment; package
metadata from previous `--target` installs can coexist. NLTK resource hashes are
recorded in the measurements. Then, from the repository root:

```sh
cargo build --locked --release --lib --example valentine_parity --jobs 1
python validation/benchmark_valentine.py --output target/valentine-performance.json
```

The second command generates its synthetic inputs, runs both implementations,
checks correctness, and writes JSON plus a Markdown table beside it. It requires
Linux and Python 3.11+; measurements use the standard library and `/proc`, with
no additional runtime or benchmarking dependencies. Expensive comparisons are
opt-in; the small methodology tests run in regular CI. Use `--case`, `--algorithm`
and `--timeout` to select or bound exploratory runs. `--upstream` compares the
unmodified reference instead of the explicitly recorded source fixes.
The complete measured command exits **1** because large Rust Jaccard exceeds its
180-second worker deadline; it still runs the remaining comparisons and writes
every outcome. This is a measured scaling limitation, not a passing full suite.
The deadline includes startup, one warmup and three timed matches; a timeout is
not a substitute for a completed matching-time measurement.

The committed [machine-readable measurements](python_performance_results.json)
record exact compiler/Python/package versions, Git revision, source and binary
hashes, constructor defaults, overrides, input hashes, raw timings, memory scopes
and unsuccessful comparisons. Raw correspondence reports and diagnostics stay
under `target/valentine-performance/`; they are recreated by the command above.
The initial batch is retained separately in [baseline measurements](python_performance_baseline.json),
including its unsuccessful trials. Its installed-distribution metadata incorrectly
reports PuLP 4.0.0: stale metadata coexists with the actual 3.3.2 package. All 49
hashed package files match the 3.3.2 wheel record. Subsequent runs record imported
module-declared versions, paths and installed distribution metadata separately;
no package was changed during measurement. The [provenance check](python_performance_provenance.json)
records the correction and NLTK resource fingerprints. AnyTree has the reverse
problem: the verified 2.13.0 wheel declares `__version__ = "2.12.1"`; neither
version source is treated as an infallible substitute for file provenance.
The starting commit is `cf8c1bac4f730e06b77d3f38b3e990374f973beb`; source hashes and
patches identify each measured working tree. The initial head-to-head Rust batch
already includes borrowed trigrams and uncapped preparation. Separate measurements
below establish those changes against the preceding implementation; the final
head-to-head batch additionally includes the two kernel optimizations.

## Correctness assessment

The reference is Valentine 1.0.0 at
`d8fa9ee6d312b49ce2c45b62824df1fd31b94a72`. Live comparisons use identical column
names, types mapped to common Valentine categories, and all nonempty samples in
their original order. Python dataframe inference and its adapter's default COMA
sample cap are bypassed. This checks the algorithms on equivalent supplied
inputs, not equivalence of all ingestion APIs.

| Reference | Equivalent cases | Exact numerical cases | Exact rankings | Equivalent rankings |
| --- | ---: | ---: | ---: | ---: |
| Unmodified pinned Python | 89/90 | 59/90 | 88/90 | 89/90 |
| Explicit two-fix source copy | 90/90 | 60/90 | 89/90 | 90/90 |

The 90 cases cover all 15 development schema pairs with five default algorithms
plus COMA instances. Unmodified DistributionBased rewrites three USGS bracketed
identifiers through PuLP variable-name sanitization. The existing reference fixes
preserve those identifiers and make Cupid's WordNet sense-pair order deterministic.
The clean upstream checkout remains untouched. Source and patch hashes are
verified; these are not performance patches.

All **9,199 correspondence pairs** agree against the corrected reference. Maximum
absolute errors are **8.94069671631e-8** for COMA instances and
**8.32667268469e-17** for Flooding; the other configurations are numerically exact.
Original tolerances remain `1e-6` for COMA and `1e-10` otherwise. EPA Flooding has
nine rounding-scale rank reversals across three source fields. Rankings use
descending scores and lexical exact ties; a reversal is reported as a rounding
tie only if both score gaps are at most twice the tolerance, while individual
scores must still satisfy their unchanged tolerance. This does not claim Python
dictionary insertion order or equal-score assignment behavior is identical.

Every post-optimization Rust pair list and score is exactly equal to the starting
commit across these 90 comparisons. Existing fixture/configuration tests and the
documented intentional differences in [PORTING.md](../PORTING.md) remain relevant;
finite test coverage is not proof of equivalence for every possible input.

## Performance method and limits

The host is a shared Linux 6.18.44 x86-64 container, Intel Xeon Platinum 8573C,
with five visible CPUs, a four-CPU quota and 32 GiB memory limit. Rust is
1.99.0 (`b940084d7`), LLVM 23.1.1; Python is 3.12.14. Builds use the normal release
profile, no optional features. No other task builds or matching tests ran during
the timed comparisons. Shared-host scheduling still causes visible variation.

The suite has three fixed synthetic pairs: **4 columns × 32 rows × 16 distinct
values**, **16 × 1,024 × 128**, and **32 × 8,192 × 1,024**, per table. Integer and
text columns alternate; half the target names change and 75% of distinct values
overlap. These are workload controls, not labeled ground truth. Two unmodified
real cases use the frozen penguin and aircraft/flight schemas and their supplied
samples. A large row count with repeated values does not establish performance
on arbitrary high-cardinality or difficult integer-clustering workloads.

Each case/configuration runs in **three fresh processes per language**, alternating
language order. Each process has one recorded warmup and three timed matches.
Tables/adapters and constructors are prepared outside the timer. Both sides time
`get_matches`; parsing, startup, result serialization and correctness checks are
outside it. Python's first import and lazy initialization are visible separately
in process/setup/warmup measurements. In particular, warmed Cupid timings do not
represent its first-call WordNet cost. Loading/preparation fields have different
boundaries in the two runners and are diagnostics, not ingestion speed comparisons.
One lifecycle difference remains: Python's timed assignment releases the previous
returned mapping (at most 1,024 candidate pairs here); Rust drops the previous
result outside the timer. That disposal cost is not isolated, so especially small
timings should not be interpreted as pure algorithm-kernel ratios.
Workers and BLAS threads are fixed to one;
Python hash seed is zero. Full inputs are retained; embeddings are not measured.

Reported matching times are medians of process medians, with their min–max range.
Every warmup/timed output must repeat exactly within its runtime and across fresh
processes. A speedup is published only after every run completes and pair, score
and ranking comparison passes. Failures, timeouts and unfavorable measurements
remain in the results; no score-based filtering or threshold tuning is applied.

Primary memory is each executable's Linux **`VmHWM`**, including its own imports,
inputs, warmups, allocator retention and result reporting. Python reads its high
watermark after writing the report; Rust reads it before final serialization. It is neither
incremental matcher memory nor simultaneous process-tree memory. Python's CBC
solver child is excluded, so Distribution's total memory is not established.
`wait4` RSS is retained only as a diagnostic: a controlled test showed a tiny
child at 8,348 KiB internally but 139,520 KiB through `wait4` after its launcher
allocated 128 MiB. Using that inherited watermark as Rust's memory would be wrong.

## Head-to-head results

The final batch ran **180 fresh processes: 177 completed, three timed out**.
**29/30 case/configuration comparisons** pass all checks. Their 87 paired rounds
have exact candidate rankings; 63 have exact numerical outputs and 24 satisfy
unchanged tolerances. All 174 successful outputs shared with the initial batch
have identical hashes. The complete benchmark exits 1 for incomplete large
Jaccard; its ratio and Rust peak memory are withheld.

Times are milliseconds. A speedup is Python time divided by Rust time: above one
favors Rust; below one favors Python. RSS uses the qualified process scope above.

| Case | Algorithm | Rust ms [range] | Python ms [range] | Rust/Python peak MiB | Speedup | Status |
|---|---|---:|---:|---:|---:|---|
| small | coma | 0.038 [0.038, 0.038] | 0.114 [0.106, 0.126] | 2.8/141.0 | 2.99x | equivalent |
| small | coma_instances | 0.228 [0.197, 0.232] | 3.976 [3.748, 4.298] | 3.0/141.1 | 17.45x | equivalent |
| small | cupid | 0.065 [0.063, 0.067] | 0.680 [0.673, 1.830] | 42.2/321.8 | 10.51x | equivalent |
| small | distribution | 0.548 [0.537, 0.572] | 25.651 [24.958, 33.533] | 3.0/141.3 | 46.80x | equivalent |
| small | jaccard | 0.200 [0.191, 0.205] | 0.212 [0.209, 0.215] | 2.8/141.7 | 1.06x | equivalent |
| small | flooding | 0.124 [0.116, 0.137] | 23.304 [23.136, 49.993] | 3.1/140.9 | 187.52x | equivalent |
| medium | coma | 0.515 [0.505, 0.516] | 2.181 [2.162, 4.173] | 6.6/143.2 | 4.23x | equivalent |
| medium | coma_instances | 194.826 [174.503, 291.865] | 289.816 [273.316, 315.503] | 22.5/152.8 | 1.49x | equivalent |
| medium | cupid | 0.316 [0.314, 0.330] | 3.839 [3.790, 3.903] | 44.4/324.3 | 12.13x | equivalent |
| medium | distribution | 36.087 [36.041, 36.321] | 360.971 [358.218, 402.179] | 8.8/150.5 | 10.00x | equivalent |
| medium | jaccard | 206.825 [202.757, 210.193] | 48.488 [46.975, 86.789] | 6.6/143.8 | 0.23x | equivalent |
| medium | flooding | 1.081 [1.067, 1.089] | 449.058 [421.916, 457.293] | 6.8/145.0 | 415.55x | equivalent |
| large | coma | 1.938 [1.930, 2.103] | 30.505 [26.305, 33.955] | 65.3/187.6 | 15.74x | equivalent |
| large | coma_instances | 30626.357 [29596.560, 36297.713] | 11661.802 [11204.925, 18720.056] | 316.6/476.7 | 0.38x | equivalent |
| large | cupid | 0.826 [0.824, 0.839] | 18.004 [17.708, 27.471] | 80.5/365.6 | 21.80x | equivalent |
| large | distribution | 2438.212 [2132.285, 2564.810] | 5913.653 [5712.138, 6410.124] | 81.7/250.2 | 2.43x | equivalent |
| large | jaccard | unavailable | 9926.965 [9885.030, 9959.279] | n/a/189.7 | withheld | incomplete (execution failure) |
| large | flooding | 4.652 [4.576, 4.659] | 2592.285 [2579.730, 2660.743] | 65.1/192.2 | 557.20x | equivalent |
| 01_penguins | coma | 0.255 [0.253, 0.281] | 0.629 [0.605, 0.649] | 3.0/141.1 | 2.47x | equivalent |
| 01_penguins | coma_instances | 0.819 [0.815, 9.939] | 18.872 [18.717, 33.152] | 3.1/141.3 | 23.05x | equivalent |
| 01_penguins | cupid | 0.542 [0.540, 0.558] | 1.526 [1.470, 1.540] | 42.4/322.1 | 2.82x | equivalent |
| 01_penguins | distribution | 1.393 [1.390, 1.497] | 57.914 [56.687, 61.027] | 3.0/144.0 | 41.57x | equivalent |
| 01_penguins | jaccard | 0.262 [0.248, 0.271] | 1.379 [1.335, 1.483] | 2.7/141.9 | 5.27x | equivalent |
| 01_penguins | flooding | 0.657 [0.649, 0.710] | 157.052 [155.494, 157.514] | 3.8/142.1 | 238.88x | equivalent |
| 02_aircraft_flights | coma | 0.211 [0.206, 0.214] | 0.693 [0.670, 0.720] | 2.9/141.1 | 3.28x | equivalent |
| 02_aircraft_flights | coma_instances | 0.751 [0.738, 0.767] | 20.574 [20.364, 20.879] | 3.2/141.3 | 27.40x | equivalent |
| 02_aircraft_flights | cupid | 0.362 [0.356, 0.388] | 1.651 [1.626, 1.655] | 42.1/321.9 | 4.56x | equivalent |
| 02_aircraft_flights | distribution | 1.610 [1.580, 1.622] | 82.107 [77.493, 83.784] | 2.9/147.3 | 51.00x | equivalent |
| 02_aircraft_flights | jaccard | 0.357 [0.346, 0.385] | 1.624 [1.610, 1.667] | 2.9/141.9 | 4.55x | equivalent |
| 02_aircraft_flights | flooding | 0.623 [0.598, 0.627] | 224.615 [198.506, 232.825] | 3.8/142.4 | 360.45x | equivalent |

COMA without instances, Cupid, DistributionBased and Flooding are faster in Rust
on these completed workloads. COMA instances is faster for the small, medium and
two real cases, but **2.63× slower** on the large case. Jaccard is close on small,
faster on these real cases, **4.27× slower** on medium, and fails the large budget.
All three large Rust Jaccard workers timed out before finishing a warmup, while
all three Python workers completed. This remains a substantial scalability limit.

The initial batch had 28/30 complete comparisons, with three Rust timeouts each
for large COMA instances and Jaccard. Large COMA instances now completes, at
30.626 seconds median and 316.6 MiB process peak versus Python's 11.662 seconds
and 476.7 MiB. Medium Rust COMA instances changed 2,370.560 → 194.826 ms; medium
Jaccard changed 413.429 → 206.825 ms. These are separate-batch observations;
the alternating controls below provide stronger evidence for causal improvements.

Unfavorable cross-batch observations are also retained: penguin Jaccard grows
0.188 → 0.262 ms (+39.4%), aircraft Jaccard 0.288 → 0.357 ms (+23.8%), and large
Distribution grows 1,659.585 → 2,438.212 ms (+46.9%). Distribution's code path was
not changed; Python also slowed on that case. Shared-host variation therefore
limits attribution between batches. The kernel follow-up below separately checks
the real Jaccard regressions against alternating implementations.


## Retained optimizations

**COMA trigrams:** allocation profiling found trigram comparison accounted for
58–75% of allocations in the investigated schema workloads. Borrowing character
windows removes those strings and the gram vectors, with seven fewer production
lines. Lowercasing, short-string behavior and multiset Dice arithmetic are
unchanged. A 529-pair independent multiset oracle covers Unicode, case expansion,
combining marks, repeated grams, short/empty strings and NUL with exact-bit checks.

Whole-COMA allocation counts fell 217,245 → 136,605 for 64 short-name columns,
229,533 → 107,933 for a 64-column Unicode profile, and 390,119 → 112,231 for long
32-column names. Allocation instrumentation is separate from release timing.
[All timing observations](coma_trigrams_results.json) retain both measurement
batches: 110 fresh processes and 1,410 timed batches, including adverse results.
Ordered scores and component details remain bit-identical. A batch of independent
cases runs 17 matches; it is not one matching call.

| COMA workload, first five-process batch | Before ms | After ms | Change |
| --- | ---: | ---: | ---: |
| Short, 8 columns | 0.125 | 0.107 | −14.7% |
| Short, 64 columns | 16.216 | 14.204 | −12.4% |
| Short, 128 columns | 38.830 | 40.800 | +5.1% |
| Unicode, 32 columns | 5.371 | 3.534 | −34.2% |
| Long similar, 32 columns | 30.771 | 22.636 | −26.4% |
| Long unrelated, 32 columns | 28.058 | 13.595 | −51.5% |
| Mixed lengths, 32 columns | 13.529 | 7.964 | −41.1% |
| Tiny names, 32 columns | 0.601 | 0.568 | −5.5% |
| All 17 independent cases | 1.686 | 1.460 | −13.4% |

A separate five-process follow-up improved short128 by 37.7% but worsened the
independent batch by 11.6%. Both batches are retained; no reliable real-world
speedup or peak-RSS reduction is claimed for this optimization. The deterministic
allocation savings and substantial complex-name gains justify the simpler code.

**Uncapped table preparation:** after the same input validation, clean tables can
be borrowed directly. The eight-line branch avoids cloning every sample when no
sampling or empty-value filtering is needed. Explicit sampling and any empty
string retain the original path, including for custom matchers. A regression
checks the supplied values, oversized limits, final-empty fallback and immutability.

An isolated 320,000-value preparation probe went from **320,460 allocations to 7**,
and **23,785,016 bytes of additional peak live heap to 512 bytes**. These counts
measure requested Rust heap, not process RSS. Actual matcher results, ordered
score bits and details were checked in every fresh process. The pinned follow-up
used five alternating processes per variant, one warmup and five timed calls;
timings include matching and result destruction, not input generation.
The table uses medians of process medians, consistently with the head-to-head
comparisons. The raw preparation file also retains its original pooled summaries;
for example, pooling all 25 COMA before calls gives 67.55 ms instead of 60.71 ms.

| Two 8-column tables, 100,000 rows each | Before ms | After ms | Change | Before/after RSS MiB |
| --- | ---: | ---: | ---: | ---: |
| COMA | 60.71 | 1.63 | −97.3% | 257.93 / 136.49 |
| Exact Jaccard | 148.93 | 114.26 | −23.3% | 262.29 / 139.39 |

[Raw preparation results](preparation_results.json) include the initial 96-process
matrix, pinned follow-up and allocation probes. Tiny, all-empty, mixed and
final-empty controls were retained. Fallback allocation counts are unchanged;
their timing ranges overlap heavily, so apparent fallback speedups are noise.
The additional clean-input scan remains a cost when an empty value appears late.
Historical RSS here used `wait4`: small cases have the inherited-parent floor
limitation; large cases exceed it. The committed reproduction harness now also
reports its own `VmHWM`. The headline Python comparison times direct matching,
so this separate high-level preparation improvement is not credited to its ratios.

**Jaccard's Levenshtein kernel:** the shared scorer now uses the existing RapidFuzz
dependency instead of the scalar `strsim` implementation, with the same Unicode
character units and normalization. The committed oracle checks 17,188 pairs,
including bit-vector boundaries; an expanded standalone oracle checked 139,273.
Every result was bit-identical. Six whole-matcher controls use the existing
performance runner, with three alternating fresh processes and three timed calls
per process. Ordered scores, details and five selection variants have identical
fingerprints.

| Jaccard control | Before ms | After ms | Reduction |
| --- | ---: | ---: | ---: |
| Base | 18.646 | 4.246 | 77.2% |
| No value overlap | 17.137 | 3.917 | 77.1% |
| Complete value overlap | 17.084 | 3.902 | 77.2% |
| 512 distinct values | 4,671.747 | 1,102.038 | 76.4% |
| 512 distinct codes | 49.067 | 11.949 | 75.6% |
| 32 distinct codes | 3.132 | 0.819 | 73.8% |

Process RSS does not show a meaningful improvement. Direct scorer microbenchmarks
retain regressions: three-character ASCII grows roughly 33 → 42 ns, and a
two-character versus 152-character comparison grows 364 → 583 ns. Default Jaccard
prunes that unequal-length case before scoring. No additional dispatch or cache
was added to address nanosecond-scale cases. Distinct-set comparisons remain
quadratic; a faster distance kernel does not change that scaling.

A follow-up confirmed real-case regressions, using five alternating fresh
processes per variant and dataset, one warmup and five timed calls. Penguin
Jaccard grows **0.204741 → 0.268914 ms (+31.3%)**; aircraft grows
**0.308613 → 0.362437 ms (+17.4%)**. Process-median ranges do not overlap, and
every pair and score bit remains identical. These are actual small-workload
regressions, consistent with the short-string overhead above. The substitution
is retained for its substantial bulk-workload savings with no new dependency or
dispatch logic; the approximately **64 and 54 microseconds** of added latency on
these cases are an explicit tradeoff, not a universal performance improvement.

**COMA instances:** measured Cartesian document-cosine comparisons dominated the
medium workload. Thirteen production lines prove a column pair has disjoint token
vocabularies and return the same zero without visiting every document pair. A
shared token in the first document pair avoids constructing the vocabulary set.
Otherwise the original arithmetic and iteration order remain intact. The test
oracle covers 49 combinations of empty, repeated, disjoint and overlapping
documents, including zero-IDF vectors: those retain the original loop to preserve
negative-zero component bits. This oracle passes in debug and release builds.

| COMA instances, final three-process controlled batch | Before ms | After ms | Reduction |
| --- | ---: | ---: | ---: |
| Head-to-head medium input | 2,570.794 | 223.801 | 91.3% |
| Frozen penguin samples | 1.009 | 0.801 | 20.6% |
| Frozen aircraft/flight samples | 1.063 | 0.768 | 27.7% |
| Disjoint multiword control | 2.343 | 1.030 | 56.1% |

The extra check is not free. A fixed-CPU follow-up measured **12.802 → 14.498 ms
(+13.3%)** on dense overlapping multiword documents, and +1.6% on the 512-row
overlap control. These regressions are retained and reported. The small change
is worthwhile for its substantial disjoint-column gains; overlapping vocabularies
still use the quadratic loop, and no universal speedup or RSS reduction is claimed.

[Kernel measurements](kernel_optimization_results.json) preserve all 150 main
process runs and 20 real-Jaccard follow-up processes, both COMA implementations
tried, the fixed-CPU controls, microbenchmarks,
exact-output checks, source/binary/input hashes and reconstruction patches. This
kernel baseline already contains the trigram and preparation changes; the
initial head-to-head batch uses that same pre-kernel source state. The isolated
tables above compare kernels under the same conditions, rather than attributing
unrelated scheduling differences between complete benchmark batches to the code.

### Reproduce the isolated Rust workloads

These standalone programs link to the library being tested; they are not new
library APIs. Build the starting revision and current library in fresh directories
so dependency artifacts cannot be accidentally mixed:

```bash
before=$(mktemp -d /tmp/fieldkin-before.XXXXXX)
after=$(mktemp -d /tmp/fieldkin-after.XXXXXX)
git archive cf8c1bac4f730e06b77d3f38b3e990374f973beb | tar -x -C "$before"
cargo build --locked --release --lib --jobs 1 --manifest-path "$before/Cargo.toml" --target-dir "$before/target"
cargo build --locked --release --lib --jobs 1 --target-dir "$after/target"
for directory in "$before" "$after"; do
    serde_json_rlibs=("$directory"/target/release/deps/libserde_json-*.rlib)
    test "${#serde_json_rlibs[@]}" -eq 1 || exit 1
    rustc --edition=2024 -O validation/coma_trigrams_bench.rs \
        --extern fieldkin="$directory/target/release/libfieldkin.rlib" \
        --extern serde_json="${serde_json_rlibs[0]}" \
        -L dependency="$directory/target/release/deps" -o "$directory/trigrams"
    rustc --edition=2024 -O validation/preparation_bench.rs \
        --extern fieldkin="$directory/target/release/libfieldkin.rlib" \
        -L dependency="$directory/target/release/deps" -o "$directory/preparation"
done
for round in 1 2 3 4 5; do
    variants=("$before" "$after")
    if (( round % 2 == 0 )); then variants=("$after" "$before"); fi
    for directory in "${variants[@]}"; do
        "$directory/trigrams" long_unrelated 32 11
        "$directory/trigrams" independent 0 11 eval_independent
        taskset -c 0 "$directory/preparation" 100000 8 none coma 5
        taskset -c 0 "$directory/preparation" 100000 8 none jaccard 5
    done
done
```

Use a permitted CPU instead of `0` if the host's affinity mask excludes it.
`trigrams` prints timings, process peak RSS and ordered score/detail bits;
`preparation` emits one JSON object including timings and a result fingerprint.
Compare fingerprints before interpreting performance. All controls and repetition
counts appear in the committed raw results; the source comments list available
workloads. The preparation measurements isolated that change by keeping the
trigram implementation equal on both sides. The commands above compare the
complete starting and final libraries instead.

For allocation counts, separately compile `preparation_bench.rs` with
`--cfg 'feature="allocations"'` and run `10000 16 none noop 0`; compare `all`,
`mixed` and `last` as filtering controls. Instrumented executions must not supply
timing claims. The COMA trigram allocation counts use a separate counting allocator; its exact
source, binary hashes and six count outputs are retained in the trigram JSON.
Extract `allocation_profile.source` to compile that probe against each library;
run `short 64`, `unicode 64`, and `long 32`. Ignore its instrumented timings. The
committed COMA program supplies uninstrumented timing and exact-output checks.

For the Jaccard controls, build the existing `benchmark_performance` example in
both target directories above, then run:

```sh
python validation/benchmark_performance.py \
  --baseline-binary "$before/target/release/examples/benchmark_performance" \
  --binary "$after/target/release/examples/benchmark_performance" \
  --output target/jaccard-comparison.json --rounds 3 --repetitions 3 \
  --case jaccard-base --case jaccard-overlap0 --case jaccard-overlap100 \
  --case jaccard-distinct512 --case jaccard-codes512 --case jaccard-codes32
```

The COMA program also accepts `dataset_instances 0 5 PATH.json`; use the generated
head-to-head medium input or either frozen real dataset. The kernel JSON contains
the exact dense-control generator, parameters, measured harness and runner sources,
plus adapters for this same committed program. It also contains the patches needed
to reconstruct the intermediate baseline for an isolated kernel comparison.

## Validation

The development quality gate passes. Development and independent Valentine
accuracy JSON and the independent suggestion report are byte-identical before
and after. Frozen corpus files, labels and baselines are unchanged. Formatting,
**174 native/Polars tests**, **four doctests**, strict native/Polars Clippy, rustdoc,
verified crate packaging and a separate default-feature consumer pass. Eight
dependency-free Python methodology tests also pass. The four runtime edits add
**16 net production lines**; regression tests and development-only measurement
code account for the other code growth. All benchmark/evaluation tooling and
measurement artifacts remain outside the published crate. No dependencies or
public APIs were added.

ONNX-enabled tests and ordinary all-feature Clippy are blocked by the runtime
binary download returning HTTP 403 from `cdn.pyke.io`. Static all-feature Clippy
passes with `ORT_SKIP_DOWNLOAD=1`; that is not an ONNX execution test. No Windows
execution was available in this session; the existing Windows CI remains enabled.

Validation commands (the two all-feature commands require the runtime download):

```sh
cargo fmt --all -- --check
cargo test --locked --all-targets --features polars --jobs 1
cargo test --locked --doc
cargo clippy --locked --all-targets --features polars --jobs 1 -- -D warnings
cargo test --locked --all-targets --all-features --jobs 1
cargo clippy --locked --all-targets --all-features --jobs 1 -- -D warnings
ORT_SKIP_DOWNLOAD=1 cargo clippy --locked --all-targets --all-features --jobs 1 -- -D warnings
cargo run --locked --example evaluate_all -- --check
cargo run --locked --example evaluate_all -- --independent
cargo run --locked --example benchmark_accuracy -- --json
cargo run --locked --example benchmark_accuracy -- --independent --json
RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps --features polars
cargo package --locked --allow-dirty
python scripts/test_compare_valentine_parity.py
python validation/test_benchmark_valentine.py
```

`--allow-dirty` packages this uncommitted change for verification; it does not skip
the isolated package build. Omit it for a clean checkout.
