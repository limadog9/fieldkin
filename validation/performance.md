# Performance and scalability

These are performance measurements of unchanged matching behavior, not accuracy
measurements. The development/independent accuracy corpora, frozen labels,
suggestion quality gate, and Valentine reference fixtures are unchanged.

## Reproduce

```sh
cargo build --locked --release --example benchmark_performance --jobs 1
python3 validation/benchmark_performance.py --output target/performance.json
```

The suite is opt-in and is not run by regular CI. It uses existing Rust
dependencies; the small process runner uses only Python 3.11+'s standard library.
`--list` lists cases, and repeated `--case NAME` options select a subset. For one
workload without Python:

```sh
cargo run --locked --release --example benchmark_performance -- \
  --algorithm jaccard --columns 8 --rows 512 --distinct 512 --overlap 100
```

Use `--help` for all controls. To compare this change with its original library
implementation, build the **same harness** against the previous revision:

```sh
git worktree add --detach /tmp/fieldkin-before e8e2de1e5f74edd7caf51c3a5648e41b93be12ed
cp examples/benchmark_performance.rs /tmp/fieldkin-before/examples/
(cd /tmp/fieldkin-before && cargo build --locked --release --example benchmark_performance --jobs 1 --target-dir /tmp/fieldkin-before/target)
cargo build --locked --release --example benchmark_performance --jobs 1 --target-dir target
python3 validation/benchmark_performance.py \
  --baseline-binary /tmp/fieldkin-before/target/release/examples/benchmark_performance \
  --output target/performance-comparison.json
```

Use separate Cargo target directories for different revisions to avoid stale
artifacts. The runner rejects identical comparison binaries and fails on
execution errors, timeouts, parameter mismatches, or changed
output fingerprints within/between versions. Timing is not a CI pass/fail gate:
cloud scheduling and hardware variation would make such a gate unreliable.
Raw times, fingerprints, peak RSS, hardware, revision, binary hashes, and failures
are saved after every process. Preserve those records when comparing changes.

## Method

All five algorithms use their existing defaults, with one worker where
configurable, through the public table-matching API. Full-data matching uses
`valentine_match`; only explicitly named sampled cases use a limit of 128.
Consequently table preparation and result construction are timed. Input
generation, matcher construction, hashing, and result selection are outside the
timer. One warmup precedes five timed runs in each process; results are dropped
between runs, outside the timer. Each version runs in three fresh processes,
alternating before/after order. Reported time is the median of the three process
medians (15 timed runs per version/case); no runs are discarded.

Linux `/proc/self/status` supplies process-lifetime peak resident memory
(`VmHWM`), read **before** output hashing/selection. The median of three process
peaks includes generated input, warmup, matcher state, and allocator retention.
It is not a count of allocations or incremental matcher memory. Other platforms
report `null`. Small RSS differences should not be interpreted as improvements.

The 61 fixed synthetic workloads vary columns (8/32 and focused widths through
512), rows (128/8,192; ingestion through 100,000), distinct values (32/256/512),
tables (2/4), value overlap (0/50/100%), and column-name overlap (0/50/100%).
Numeric/text columns alternate; deterministic high-entropy strings avoid making
every text value a fuzzy match. Duplicates repeat in input order. Value overlap
controls the shared prefix of distinct values; schema-name overlap separately
controls the fraction of corresponding target column names retained, with every
third retained target name renamed. The rest have per-table hashed names. Types
remain fixed. These synthetic relationships are workload controls, not semantic
ground truth. CSV/JSON ingest one table with identical string-valued samples.
Two additional Jaccard controls use `--values codes`: all-text identifiers with
a shared `customer-account-` prefix. These deliberately create dense fuzzy hits
to measure the hit-skipping path. They do not imply that similar account codes
are semantically equivalent. Default `mixed` cases retain high-entropy text and
short numeric strings, predominantly producing exact hits or no hits.
The periodic sample order also means the 128-of-8,192 sampling profile selects
one distinct value per column (its stride is 64, versus a period of 32).
Comparing that profile with full data changes both row count and diversity; it
must not be interpreted as an optimization of identical matcher inputs.

Default COMA ignores instances; Cupid and SimilarityFlooding use schema evidence.
Their row-scaling measurements mainly reflect table preparation. Jaccard and
DistributionBased consume samples. Distribution's wide cases use one distinct
value per column and mostly isolated/per-column components: they expose matrix
allocation, **not** difficult integer correlation-clustering scalability.

The final timed result of each process is fingerprinted (not every iteration).
Output SHA-256 includes ordered column pairs, exact IEEE-754 score/component bits,
and filtered, top-10, greedy, Hungarian, and threshold-aware Hungarian selections.
Ingestion hashes the entire table: names, types, order, and sample strings.
This detects last-bit changes rather than accepting score tolerances.

## Changes and limits

- **Jaccard:** skip a value-pair calculation only when both values already have
  qualifying partners; finish a source row when all target values have partners.
  The aggregate depends only on those hit counts. This also removes the source
  hit vector. Cutoff conversion, distance functions, embeddings, and Tversky
  arithmetic are unchanged. Worst-case disjoint comparisons remain quadratic in
  distinct values; no persistent cache is introduced.
- **SimilarityFlooding:** index the original graphs' incoming/outgoing edges and
  enumerate their products in the original order. This avoids repeatedly
  searching/scanning a materialized product graph. Original edges are unique by
  endpoints, so their products cannot duplicate endpoints. Coefficient overwrite
  order and floating-point operation order remain unchanged. Product-sized
  scores and propagation weights are still needed; this is not unbounded scaling.
- **DistributionBased:** fill the sequential distance matrix directly, omitting
  temporary pair/result vectors. The symmetric matrix, calculation direction,
  parallel path, histograms, thresholds, and integer solver are unchanged. Final
  matrix storage remains quadratic and difficult clustering remains expensive.
- **JSON ingestion:** borrow keys to discover sorted headers, then move owned
  values directly from parsed records into columns. This removes repeated key
  clones, string clones, and the nullable row matrix. Parsing still retains the
  JSON object array; this is not streaming/bounded JSON ingestion. Null/empty
  filtering, number formatting, all-value type inference, and error precedence
  are unchanged.

COMA, Cupid, CSV ingestion, and sampling were inspected and left unchanged.
The latter two already stream CSV records and clone only retained samples.
No matching formulas, thresholds, defaults, public APIs, or dependencies changed.

## Measured results

Measured 2026-10-10 in a Linux 6.18.44 x86-64 cloud container, glibc 2.41,
Intel Xeon Platinum 8573C, five visible logical CPUs with a four-CPU cgroup quota,
32 GiB memory limit, Rust 1.99.0, default release profile, no optional features.
Before: `e8e2de1e5f74edd7caf51c3a5648e41b93be12ed`. After: the production changes
accompanying this report. Both builds used the same harness and separate target
directories. Binary fingerprints and every measured run are in
[`performance_results.json`](performance_results.json).

All **61 cases / 366 fresh processes / 1,830 timed invocations** completed.
Every final-result fingerprint agrees across versions and repeated processes;
there were no errors, timeouts, or mismatches. Expensive builds and other test
runs were stopped during measurement. The host was not otherwise isolated and
CPU affinity/frequency were not fixed. Some unchanged controls varied markedly;
these are observations on this host, not universal speed guarantees.

The main changes, with median times and process peak RSS:

| Workload | Before ms | After ms | Time change | Before MiB | After MiB | RSS change |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Jaccard, 8 columns, 32 distinct dense codes | 73.512 | 6.189 | −91.58% | 2.99 | 3.08 | +3.00% |
| Jaccard, 8 columns, 512 distinct dense codes | 14,627.643 | 61.664 | −99.58% | 3.95 | 3.93 | −0.49% |
| Jaccard, 8 columns, 512 distinct mixed values | 8,308.250 | 9,361.314 | +12.67% | 3.79 | 3.81 | +0.62% |
| SimilarityFlooding, 64 columns, zero rows | 169.640 | 19.999 | −88.21% | 6.54 | 5.91 | −9.50% |
| SimilarityFlooding, 128 columns, zero rows | 3,034.770 | 82.540 | −97.28% | 17.61 | 14.91 | −15.37% |
| DistributionBased, 256 columns, 32 rows, 1 distinct value | 39.735 | 41.181 | +3.64% | 13.85 | 8.99 | −35.11% |
| DistributionBased, 512 columns, 32 rows, 1 distinct value | 123.646 | 127.492 | +3.11% | 39.39 | 19.65 | −50.12% |
| JSON, 8 columns, 10,000 rows | 24.599 | 21.848 | −11.18% | 23.45 | 20.24 | −13.69% |
| JSON, 8 columns, 100,000 rows | 328.046 | 244.182 | −25.56% | 200.74 | 183.54 | −8.57% |

Negative percentages mean reductions, not throughput increases. Jaccard's
benefit is conditional on dense fuzzy hits: the mixed-value control was slower,
and worst-case complexity is unchanged. No meaningful RSS reduction is claimed
for its small hit-vector saving. Distribution's change is retained for its
consistent memory reduction, **not a speed claim**; the observed wide-case time
cost is about 3–4%. The existing parallel path does not get this saving.

For scale and dispersion: the three process medians for dense Jaccard/512 were
14,224–19,183 ms before versus 60.92–85.84 ms after. Mixed Jaccard/512 ranged
7,168–8,369 versus 8,191–10,821 ms. Flooding/128 ranged 2,758–3,070 versus
81.77–121.84 ms; JSON/100,000 ranged 308.35–397.04 versus 230.08–246.25 ms.
Report all these observations rather than selecting the fastest runs.

All-five overview, before → after milliseconds. Base: two tables, eight columns,
128 rows, 32 distinct values, 50% value overlap, default schema-name overlap.
Each other column changes only its named dimension:

| Algorithm | Base | 32 columns | 8,192 rows | Four tables |
| --- | ---: | ---: | ---: | ---: |
| COMA | 0.256 → 0.244 | 3.076 → 3.455 | 10.936 → 4.134 | 1.326 → 1.894 |
| Cupid | 0.219 → 0.228 | 1.547 → 1.466 | 3.902 → 3.883 | 1.049 → 1.084 |
| DistributionBased | 2.178 → 2.241 | 40.435 → 40.323 | 48.828 → 75.712 | 7.176 → 7.002 |
| JaccardDistanceMatcher | 20.899 → 49.939 | 457.122 → 438.110 | 54.896 → 36.197 | 127.366 → 163.911 |
| SimilarityFlooding | 0.501 → 0.411 | 15.725 → 4.492 | 4.211 → 4.465 | 2.815 → 2.029 |

The large COMA row-time variation occurred without changing COMA or sampling;
do not attribute it to these optimizations. Likewise no general speedup is
claimed for the mixed Jaccard profiles. Complete overlap, distinctness, sampling,
timing, and memory measurements are retained in the raw artifact. CSV/100,000
rows, an unchanged control, was 30.023 → 30.567 ms and 67.03 → 66.97 MiB.

## Correctness and validation

Regression tests compare against the previous implementations, not new scoring
targets:

- Jaccard: exact score bits across 7,350 lexical and 432 precomputed-embedding
  cases, including Unicode, empty/duplicate samples, both directions, cutoff
  boundaries, and asymmetric/zero Tversky penalties.
- Flooding: exact propagation edges, coefficients, and final scores across 25
  schema pairs, both policies, all four formulas and three string matchers,
  including empty/rectangular graphs and identifier/self-loop collisions.
- Distribution: exact matrix bits against former pair collection, including
  infinite distances, numeric aliases, duplicates, constants, disjoint values,
  empty/singleton inputs, Bloom on/off, and sequential/parallel result equality.
- JSON: full tables and errors compared with former row-matrix conversion,
  including formatting, large numbers, missing/null/empty values, Unicode,
  late fields/type changes, duplicates, and malformed/nested input precedence.
- Benchmark: invalid arguments, deterministic generation, independent name/value
  overlap controls, dense-code types, and CSV/JSON sample equivalence.

Passed:

```sh
cargo fmt --all -- --check
cargo test --locked --all-targets --features polars --jobs 1
cargo test --locked --doc
ORT_SKIP_DOWNLOAD=1 cargo clippy --locked --all-targets --all-features --jobs 1 -- -D warnings
cargo run --locked --example evaluate_all -- --check
cargo run --locked --example benchmark_accuracy -- --json
cargo run --locked --example benchmark_accuracy -- --independent --json
```

The executable suite passed **146 tests**, including unchanged Python-derived
Valentine reference fixtures, plus **three documentation tests**. The suggestion
quality gate remains **215/256 (83.98%)**, with automatic-match precision
**170/179 (94.97%)**. The complete development accuracy JSON is byte-identical
to a run of the original revision; independent accuracy JSON is byte-identical
to the committed report. No frozen labels, baselines, parity fixtures, or
tolerances changed. Runner smoke checks also verified equal-output success,
changed-output/malformed-report failures, and identical-binary rejection.

`cargo test --locked --all-targets --all-features --jobs 1` and the unmodified
all-feature Clippy command could not complete because the environment proxy
returned HTTP 403 for `ort-sys`'s optional ONNX Runtime binary download from
`cdn.pyke.io`. `ORT_SKIP_DOWNLOAD=1` allows all-feature linting, **not runtime
testing of that backend**. Precomputed-embedding equivalence tests did execute.
The live Python parity script was attempted but could not start without its
pinned `.upstream/valentine` checkout. Existing native reference-fixture tests
passed; no new live Python parity result is claimed.
