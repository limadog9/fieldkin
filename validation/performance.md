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

## Follow-up: Jaccard length bound (2026-10-11)

Levenshtein similarity cannot exceed
`1 - abs(character_count(a) - character_count(b)) / max(character_count(a), character_count(b))`.
Jaccard now skips comparisons whose bound falls below its existing rounded
cutoff. Counts use Unicode scalar values, matching the scorer. Equal lengths
skip the bound calculation; zero cutoffs, other distances, and embeddings retain
their previous path. The existing hit-count accumulation and arithmetic are
unchanged. Storage adds one `usize` per value in the larger distinct-value set
of the current column pair. Disjoint values of similar lengths still require
quadratic work.

Compared against `9a431ca0fe27e9046246f2eb5d16cb1dbba35a0b`, using the same
unmodified harness, release profile, and hardware described above (Rust 1.99.0,
Linux x86-64, Xeon Platinum 8573C, four-CPU quota, 32 GiB). This follow-up uses
**five fresh processes and 15 timed repetitions per version/case**, with one
warmup per process and alternating version order. There were no concurrent
builds or test runs, but host scheduling and CPU frequency were not controlled.
All **12 cases / 120 processes / 1,800 timed invocations** completed with no
errors and identical final-result fingerprints across versions and rounds.

Median of process medians; negative percentages mean reduced elapsed time.
RSS is median process-lifetime peak memory, not incremental matcher storage.

| Case | Before ms | After ms | Time change | Before → after RSS MiB |
| --- | ---: | ---: | ---: | ---: |
| COMA base | 0.242 | 0.241 | −0.31% | 3.02 → 3.08 |
| Cupid base | 0.217 | 0.218 | +0.55% | 42.50 → 42.47 |
| DistributionBased base | 2.236 | 2.240 | +0.16% | 3.71 → 3.72 |
| SimilarityFlooding base | 0.405 | 0.383 | −5.41% | 3.21 → 3.21 |
| Jaccard base | 24.562 | 30.031 | +22.27% | 2.96 → 2.96 |
| Jaccard, 32 columns | 351.058 | 268.702 | −23.46% | 4.09 → 4.05 |
| Jaccard, 4 columns, 256 distinct values | 418.159 | 322.920 | −22.78% | 2.93 → 2.92 |
| Jaccard, four tables | 158.098 | 104.847 | −33.68% | 3.32 → 3.31 |
| Jaccard, zero overlap | 21.567 | 17.090 | −20.76% | 2.97 → 2.96 |
| Jaccard, complete overlap | 22.827 | 20.057 | −12.14% | 2.93 → 2.96 |
| Jaccard, 32 distinct dense codes | 3.219 | 3.274 | +1.69% | 2.99 → 2.99 |
| Jaccard, 512 distinct dense codes | 50.360 | 50.118 | −0.48% | 3.96 → 3.95 |

The base-case slowdown is retained in this report. Its five process medians
ranged from 21.431–38.850 ms before and 17.069–31.861 ms after: this shared-host
measurement does not establish a universal speedup. The 32-column case ranged
from 346.966–361.967 ms before and 267.913–328.860 ms after; four tables ranged
from 129.366–187.338 ms before and 101.491–126.042 ms after. The change is retained
for those larger-workload gains. Dense-code cases have little opportunity to
prune and pay length-counting overhead. No meaningful memory reduction, speed
gain for the other algorithms, or change in accuracy is claimed.

To reproduce, build the baseline revision in a separate worktree and target
directory, then use the existing runner (all cases use its unchanged inputs):

```sh
git worktree add --detach /tmp/fieldkin-length-before 9a431ca0fe27e9046246f2eb5d16cb1dbba35a0b
(cd /tmp/fieldkin-length-before && cargo build --locked --release --example benchmark_performance --jobs 1 --target-dir target)
cargo build --locked --release --example benchmark_performance --jobs 1
python3 validation/benchmark_performance.py \
  --baseline-binary /tmp/fieldkin-length-before/target/release/examples/benchmark_performance \
  --output target/length-bound-performance.json --rounds 5 --repetitions 15 \
  --case coma-base --case cupid-base --case distribution-base --case flooding-base \
  --case jaccard-base --case jaccard-width --case jaccard-distinct --case jaccard-tables \
  --case jaccard-overlap0 --case jaccard-overlap100 --case jaccard-codes32 --case jaccard-codes512
```

The new regression compares exact score bits with a full Cartesian calculation
for Unicode, unequal lengths, empty values, rounded cutoff boundaries, both
directions, and asymmetric penalties. Existing lexical/embedding oracles and
reference fixtures remain unchanged.

Follow-up validation passed 162 tests with Polars, four doctests, formatting,
strict rustdoc, and all-feature Clippy with `ORT_SKIP_DOWNLOAD=1`. The suggestion
gate remains 215/256; development and independent accuracy JSON are byte-identical
to the baseline, including the committed independent report. Ordinary all-feature
tests/Clippy remain blocked by the ONNX download's HTTP 403; live Python parity
could not start without `.upstream/valentine`. Native reference fixtures passed.

## Follow-up: reuse Flooding's shared Levenshtein scorer (2026-10-11)

SimilarityFlooding's optional `StringMatcher::Levenshtein` now uses the existing
shared scorer. This removes 20 production lines and replaces a newly allocated
DP row per character with one working row. Unicode character handling, graph
propagation, defaults, and output scores are unchanged. The former scorer remains
only in the existing exact-output test oracle, extended with Unicode, combining
marks, unrelated names, and unequal lengths (864 full configuration/schema checks).

Compared Flooding's implementation from `770c096930fd29592b5b6f5124a25c89060431c0`
with the shared-scorer change on the Linux/Rust hardware described above.
The fixed [harness](flooding_levenshtein_bench.rs) uses
8 source and 12 target columns, alternating integer/text types, no samples, and
five synthetic name patterns. These are performance workloads, not labeled
accuracy cases. Only the selected string scorer differs from Flooding's defaults.
The direct `get_matches` call is timed; schema generation, output comparison, and
printing are excluded. Five fresh processes per version/case each perform one
warmup and 11 timed calls, alternating version order. All **100 processes / 1,100
timed matches** completed; complete ordered score bits agreed across versions and
rounds. Each process also checks repeated results for equality.

Median of process medians, with all controls retained:

| Scorer | Names | Before ms | After ms | Time change |
| --- | --- | ---: | ---: | ---: |
| Levenshtein | Short | 0.343 | 0.285 | −16.9% |
| Levenshtein | Unicode / combining marks | 0.446 | 0.397 | −10.9% |
| Levenshtein | Long similar | 10.428 | 7.632 | −26.8% |
| Levenshtein | Long unrelated | 16.030 | 4.564 | −71.5% |
| Levenshtein | Mixed lengths | 6.680 | 3.405 | −49.0% |
| Default prefix/suffix | Short | 0.457 | 0.431 | −5.8% |
| Default prefix/suffix | Unicode / combining marks | 0.362 | 0.368 | +1.7% |
| Default prefix/suffix | Long similar | 10.761 | 9.143 | −15.0% |
| Default prefix/suffix | Long unrelated | 0.706 | 0.709 | +0.3% |
| Default prefix/suffix | Mixed lengths | 0.544 | 0.608 | +11.9% |

No concurrent builds/tests ran, but the shared host was noisy: long-unrelated
Levenshtein process medians ranged 11.65–34.20 ms before and 4.44–16.34 ms after.
Short-name ranges were 0.327–0.396 and 0.272–0.296 ms; Unicode ranges were
0.436–0.455 and 0.391–0.412 ms. No default-mode improvement is claimed. Median
process-lifetime peak RSS was 1,624–1,920 KiB across these small cases, with no
meaningful reduction established. Worst-case edit-distance complexity remains
quadratic. The correctness fixtures passed without tolerance changes.

To reproduce on Linux, build the same standalone harness against each library.
It lives under `validation/`, outside the crate and regular CI benchmark runs:

```sh
mkdir -p /tmp/fieldkin-flooding-source
git archive 770c096930fd29592b5b6f5124a25c89060431c0 | tar -x -C /tmp/fieldkin-flooding-source
cargo build --locked --release --lib --jobs 1 \
  --manifest-path /tmp/fieldkin-flooding-source/Cargo.toml \
  --target-dir /tmp/fieldkin-flooding-source/target
cargo build --locked --release --lib --jobs 1
rustc --edition=2024 -C opt-level=3 validation/flooding_levenshtein_bench.rs \
  --extern fieldkin=/tmp/fieldkin-flooding-source/target/release/libfieldkin.rlib \
  -L dependency=/tmp/fieldkin-flooding-source/target/release/deps -o target/flooding-before
rustc --edition=2024 -C opt-level=3 validation/flooding_levenshtein_bench.rs \
  --extern fieldkin=target/release/libfieldkin.rlib \
  -L dependency=target/release/deps -o target/flooding-after
python3 - target/flooding-before target/flooding-after target/flooding-comparison.json <<'PY'
import hashlib, json, pathlib, statistics, subprocess, sys
binaries = dict(zip(("before", "after"), sys.argv[1:3]))
report = {"binaries": {v: hashlib.sha256(pathlib.Path(p).read_bytes()).hexdigest()
                       for v, p in binaries.items()}, "runs": []}
reference = {}
for round_number in range(5):
    for scorer in ("default", "levenshtein"):
        for case in ("short", "unicode", "long_similar", "long_unrelated", "mixed"):
            versions = list(binaries) if round_number % 2 == 0 else list(reversed(binaries))
            for version in versions:
                lines = subprocess.check_output(
                    [binaries[version], scorer, case, "11"], text=True, timeout=180).splitlines()
                assert len(lines) == 3, "incomplete benchmark output"
                assert reference.setdefault((scorer, case), lines[2]) == lines[2], "changed output"
                times = json.loads(lines[0])
                assert len(times) == 11, "incomplete timing measurements"
                report["runs"].append(dict(round=round_number, scorer=scorer, case=case,
                    version=version, times_ms=times, median_ms=statistics.median(times),
                    peak_rss_kib=int(lines[1].split()[0]),
                    output_sha256=hashlib.sha256(lines[2].encode()).hexdigest()))
                pathlib.Path(sys.argv[3]).write_text(json.dumps(report, indent=2) + "\n")
for scorer, case in reference:
    medians = {v: statistics.median(r["median_ms"] for r in report["runs"]
               if (r["scorer"], r["case"], r["version"]) == (scorer, case, v)) for v in binaries}
    print(scorer, case, medians)
PY
```
