# Python Valentine comparison and measured Rust optimizations

## Opt-in resource-check overhead (2026-10-14)

This hardening adds checked work counts, not a matching optimization. Baseline is
`7d21f37fbaeaacc55ca6cdfb3f63864a8d0415ab`; candidate is that revision plus the
additive `WorkLimits` changes committed in `92b31dd` (diagnostic hooks are
`cfg(test)` only). Both were built independently with Rust/Cargo
1.99.0, release defaults, one worker, no optional features. Machine: Xeon Platinum
8573C, five visible CPUs, four-CPU container quota, 32 GiB memory limit. There
were no concurrent builds/reference runs during measurement.

Each entry is the median of **five fresh-process medians**, alternating comparison
order, pinned to CPU 0. Each process warms up once, then times 7 calls for the
small conflicting Distribution case, 11 for wide assignments/empty Jaccard,
101 for the singleton assignment, and 21 otherwise (31 for exact Jaccard).
Matching includes preparation; inputs, output verification and disposal are
outside timing. Historical modes still include high-level table preparation;
new `-direct`/`-guarded` modes both use the direct batch APIs, so their boundaries
are comparable. `WorkLimits::default()` exercises checked arithmetic and all
preflights without rejecting work. C/R/D mean columns/rows/distinct values per
table; two tables and 50% overlap unless specified.

| Workload | Baseline/current unguarded ms | Current direct/guarded ms | Peak KiB baseline/current/guarded |
|---|---:|---:|---:|
| Jaccard 8C/128R/32D, hashes | 0.7706 / 0.7755 | 0.7787 / 0.7775 | 3024 / 3088 / 3084 |
| Jaccard 256C/0R | 0.3038 / 0.2990 | 0.2673 / 0.3144 | 2740 / 2704 / 2644 |
| Jaccard 3 tables × 8C/64R/16D | 0.7178 / 0.7212 | 0.7248 / 0.7257 | 3020 / 3052 / 3028 |
| Exact Jaccard 16C/128R/64D, disjoint hashes | 0.7317 / 0.7337 | 0.7248 / 0.7329 | 3092 / 3112 / 3112 |
| Distribution 8C/128R/32D | 1.9019 / 1.8833 | 1.8802 / 1.8628 | 3348 / 3360 / 3356 |
| Distribution 16C/64R/32D, 100% overlap | 3.7319 / 3.7454 | 3.7240 / 3.7253 | 3528 / 3504 / 3492 |
| Distribution ring4, 4C/64R/64D | 0.8509 / 0.8320 | 0.8242 / 0.8123 | 3088 / 3108 / 3116 |
| Hungarian 64×64 | 2.1740 / 2.2452 | 2.2153 / 2.1893 | 7716 / 7584 / 7584 |
| Hungarian 8×256 | 20.8730 / 20.6878 | 20.6358 / 20.7238 | 5308 / 5204 / 5216 |
| Hungarian 1×256 | 0.0012 / 0.0011 | 0.0011 / 0.0196 | 3076 / 3064 / 3052 |
| Threshold-aware Hungarian 8×256 | 21.0973 / 20.4357 | 20.6290 / 20.8101 | 5252 / 5172 / 5216 |

Peak RSS is Linux process-lifetime `VmHWM` measured before output hashing,
including input generation, warmup and retained results; it is **not incremental
matcher memory**. These small differences do not establish a memory improvement.
All 220 process comparisons across the initial unrestricted run and pinned
recheck preserved the complete ordered pair/score-bit/detail-bit and selection
digests, not merely the top correspondence.

The checks are opt-in. Empty-wide Jaccard pays about **47 µs (+18%)** for conservative
pair counting; the 1×256 assignment shortcut pays about **18 µs** for unique
identity/dimension checks that the unchecked shortcut avoids. Other accepted
checks were small relative to measurement variation. The original unrestricted
run showed large timing variation (Distribution-identical 3.68–8.19 ms and
Hungarian-wide 20–37 ms process medians), so the entire set was repeated on CPU 0.
The pinned 64×64 unchecked assignment median was 3.3% higher, with overlapping
process-median ranges (baseline 2.168–2.287 ms, current 2.150–2.287 ms). Wide
assignment outliers remained up to 33.8 ms. No speedup or universal zero-overhead
claim is made; guarded calls buy early rejection, not faster accepted execution.

Exact timed harness SHA-256: `24c328b7bee827486bd5c5230bdfc2f7d5951c5cd1dff22175770d03d193b876`.
The final harness also honors `--workers` for direct Distribution calls; the
measured calls all used one worker, so their work is unchanged.

Exact benchmark binary SHA-256 values:

- Baseline: `b85f831e8a7eda929dacf16be8eeeee64cbcee65c86094fd70f95db1dc3b216a`.
- Candidate: `fc52e6b5480c0c3230c50fbf29c776b789e9c2afc042322c71d1ae1290b1862b`.

Reproduce with separate source/target directories; the baseline uses its original
harness, whose existing modes have unchanged timing boundaries. For example:

```sh
mkdir -p target/hardening/before
git archive 7d21f37fbaeaacc55ca6cdfb3f63864a8d0415ab | tar -x -C target/hardening/before
CARGO_TARGET_DIR=/tmp/fieldkin-hardening-before-target cargo build --locked --release \
  --manifest-path target/hardening/before/Cargo.toml --example benchmark_performance --jobs 1
CARGO_TARGET_DIR=/tmp/fieldkin-hardening-after-target cargo build --locked --release \
  --example benchmark_performance --jobs 1
# Five fresh-process pairs, alternating order; repeat for the table's dimensions.
for round in 1 2 3 4 5; do
  order="before after"; [ $((round % 2)) = 0 ] && order="after before"
  for version in $order; do
    taskset -c 0 /tmp/fieldkin-hardening-$version-target/release/examples/benchmark_performance \
      --algorithm jaccard --columns 8 --rows 128 --distinct 32 --values hashes \
      --workers 1 --repetitions 21
  done
done
# Same candidate binary/direct batch boundaries; alternate these modes likewise.
for mode in jaccard-direct jaccard-guarded; do
  taskset -c 0 /tmp/fieldkin-hardening-after-target/release/examples/benchmark_performance \
    --algorithm "$mode" --columns 8 --rows 128 --distinct 32 --values hashes \
    --workers 1 --repetitions 21
done
```

## Distribution solver diagnosis (2026-10-14)

Fresh captures close the previously incomplete ring8/ring16 investigation on
`7d21f37` plus the additive guarded-call changes. No clustering formula or
search option changed; diagnostic logging was enabled. The default path uses
good_lp 1.15.3 / microlp
0.6.0. The Python reference is pinned to `d8fa9ee6d312b49ce2c45b62824df1fd31b94a72`,
with Python 3.12.14, PuLP 3.3.2 and CBC 2.10.3. Unmodified and explicitly corrected
reference captures are separate; the identity patch changes neither solution
on these punctuation-free inputs.

| Existing control | Nodes | Actual binary variables / triangles | Directed optimum | Native / CBC pairs | Differing directed arcs |
|---|---:|---:|---:|---:|---:|
| ring8 | 16 | 240 / 3,360 | 64 | 64 / 32 | 112 |
| ring16 | 32 | 992 / 29,760 | 128 | 256 / 64 | 160 |

The captured node order and sign matrices agree. PuLP's actual coefficients,
binary bounds and every constraint were checked against the directed formulation;
test-only native captures record the actual solver model counts, separation
matrix and termination evidence. CBC reports optimal, and microlp reports
`ProvenOptimal`, zero gap and best bounds -32/-576 before the objective constants
96/704 are restored. Independently, every binary solution satisfies every
directed triangle. An LP relaxation supplies **exact rational dual certificates**:
checked signs and stationarity prove lower bounds 64/128, attained by all four
Python and both native solutions. Equal reported objectives alone were not used
as proof of optimality. No exponential enumeration of the large rings was run.

These are different valid optimal **directed separation matrices**. Upstream
converts their zero arcs to undirected edges and takes weak components. Native
has one component in each case; CBC has two/four. The returned pairs are exactly
the cross-table expansions of those components. The directed objective is not
the cost of that final weak-component partition. Adding symmetry or forcing a
particular tied optimum would alter compatibility; neither is warranted here.
These controls remain **non-equivalent outputs**, excluded from equivalent-output
speedup claims, including against the corrected reference.

[Compact evidence](distribution_solver_diagnosis.json) records the graph,
separation matrices, partitions, ordered output score bits, rational duals,
versions and hashes. Full LP/model/log captures remain under the ignored
`target/solver-diagnosis`; the evidence records their hashes. Ordinary tests
exhaust all directed assignments on three nodes for 59 non-shortcut sign graphs,
check feasibility/objectives and repeat a conflicting eight-node solve. The
manual full-ring capture is ignored by normal offline tests.

Reproduce after the pinned reference setup in PORTING.md (use an external process
deadline; this is diagnosis, not a performance benchmark):

```sh
cargo build --locked --release --example benchmark_performance
mkdir -p target/solver-diagnosis
for n in 8 16; do
  target/release/examples/benchmark_performance --algorithm distribution --values ring \
    --columns "$n" --rows 128 --distinct 128 --overlap 100 \
    --dump-input "target/solver-diagnosis/ring$n.json"
done
# Preserve the historical byte-level input fingerprints, including JSON order.
python - <<'PYGEN'
import json
from pathlib import Path
for n in (8, 16):
    p = Path(f"target/solver-diagnosis/ring{n}.json")
    data = json.loads(p.read_text())
    ordered = {"name": f"generator_ring{n}"}
    for side in ("source", "target"):
        ordered[side] = {"fields": [{key: f[key] for key in ("name", "data_type", "samples")}
                                   for f in data[side]["fields"]]}
    p.write_text(json.dumps(ordered, separators=(",", ":")) + "\n")
PYGEN
for n in 8 16; do
  FIELDKIN_DISTRIBUTION_INPUT="target/solver-diagnosis/ring$n.json" \
  FIELDKIN_DISTRIBUTION_OUTPUT="target/solver-diagnosis/ring$n-native.json" \
    timeout 180s cargo test --locked --lib capture_distribution_problem -- --ignored
  timeout 180s python scripts/diagnose_distribution_solver.py \
    --dataset "target/solver-diagnosis/ring$n.json" \
    --native "target/solver-diagnosis/ring$n-native.json" \
    --output "target/solver-diagnosis/ring$n-upstream"
  timeout 180s python scripts/diagnose_distribution_solver.py --reference-fixes \
    --dataset "target/solver-diagnosis/ring$n.json" \
    --native "target/solver-diagnosis/ring$n-native.json" \
    --output "target/solver-diagnosis/ring$n-fixed"
done
python scripts/test_distribution_solver_diagnostic.py
```

## Previous follow-up: exact COMA document reuse and Jaccard rejection bounds

Measured **2026-10-13**, starting from `origin/main` at
`e82bdc5b901fc2ddefa0976a275a2046be6c38a9`, on branch
`perf/valentine-performance`. The sections below this follow-up are historical;
their large-input timeouts and unavailable ONNX execution describe those earlier
runs. No suggestion-engine behavior, dependencies, public APIs, algorithm
formulas, defaults, samples, labels or frozen gates changed in this follow-up.
[Machine-readable evidence](valentine_performance_20261013.json) preserves raw
matching/warmup timings, ranges, process peaks, hashes, failures and validation.

### Measured bottlenecks and retained changes

Temporary stage/call instrumentation identified large COMA instance matching as
28.628 s of cosine work versus 0.395 s of TF-IDF preparation and 0.0004 s of
result construction: **2,147,483,648 cosine evaluations**. The input contained
524,288 document occurrences but only 1,024 distinct documents per column.
COMA now prepares identical raw documents once per column and compares their
vectors once. IDF still counts every occurrence; maxima are expanded in original
sample order before summing, preserving floating-point addition order. The
lookup-only hash map never determines output order. Unique and tokenless inputs
remain supported. This is 38 net production lines, plus an expanded-document
oracle checking score bits, details, global multi-table IDF and boundary cases.

The large Jaccard profile made **453,725,688 Levenshtein calls**, despite its
existing length bound and partner-hit shortcut. Each distinct value now lazily
stores its Unicode scalar length and a 128-bit hashed adjacent-character bitmap.
An edit can change at most four presence bits, so `ceil(xor_popcount / 4)` is an
exact lower bound on edit distance. Hash collisions only weaken rejection.
The existing length bound and rounded inclusive cutoff remain authoritative;
pairs are skipped only when the bound proves they cannot qualify. Metadata is
shared only within the existing matching call, including its worker threads,
and is unnecessary on identical/exact/embedding/other-distance paths. It adds
24 bytes per distinct value on this host (about 1.5 MiB at the largest workload),
not an unbounded cache. This is 50 net production lines. Independent Unicode
edit-distance and Cartesian-score oracles cover collisions, empty values,
cutoff equality, ties and nonidentical parallel/batch inputs.

Rejected alternatives include eager tokenless-document caching, wider eager
preparation, and additional length/cutoff guards: measured regressions or
insignificant gains did not justify their complexity. DistributionBased, Cupid,
SimilarityFlooding and COMA name scoring were left unchanged.

### Comparable three-way measurements

Release Rust 1.99.0 / LLVM 23.1.1, Python 3.12.14, pinned Valentine 1.0.0
`d8fa9ee6d312b49ce2c45b62824df1fd31b94a72`, with the **existing two reference
fixes** described below. The unmodified reference is separately verified.
Hardware remains the shared Xeon Platinum 8573C Linux 6.18.44 container, five
visible CPUs, four-CPU quota, 32 GiB memory limit. Processes were pinned to CPU 0;
matcher/BLAS worker counts were one. No concurrent task builds or matching runs
were scheduled during timing. This is not a dedicated-host or multithreaded claim.

Small/medium/large retain the existing 4×32×16, 16×1,024×128 and
32×8,192×1,024 column×row×distinct controls; two frozen real inputs are unchanged.
Each side used three fresh processes, one recorded first-call warmup and three
timed matches per process, rotating before/after/Python order. The table reports
medians of process medians. `get_matches` includes internal preparation and
result construction; parsing, startup, constructors, disposal, selection,
hashing and serialization are outside timing on **both** sides. Python's prior
result is now released outside timing too. Warmups and setup remain in the JSON:
small Cupid first calls were about 60–64 ms native versus 2,057 ms Python, far
above warmed times. All inputs use every nonempty sample without a cap.

Memory is Linux **lifetime process `VmHWM`**, including setup, inputs, matching,
verification and allocator retention; Rust samples before final serialization,
Python after the preceding checkpoint. It is not incremental matcher memory.
**Python CBC children are excluded**, so total Distribution memory is unmeasured.

All **30/30** core comparisons complete and meet the unchanged Python pair,
score and ranking checks. Every native before/after output also agrees bit for
bit, including ordered candidates, component details and five result selections.
Times are ms; memory is before→after MiB with the qualified scope above. Changes
in untouched algorithms are controls, not optimization claims.

| Workload | Algorithm | Python ms | Rust before ms | Rust after ms | Native peak MiB | Before/after |
|---|---|---:|---:|---:|---:|---:|
| small | coma | 0.107 | 0.037 | 0.037 | 2.9→3.0 | 1.01× |
| small | coma_instances | 4.032 | 0.206 | 0.132 | 3.2→3.0 | 1.56× |
| small | cupid | 0.670 | 0.070 | 0.070 | 42.4→42.4 | 0.99× |
| small | distribution | 48.460 | 0.507 | 0.501 | 3.0→3.0 | 1.01× |
| small | jaccard | 0.210 | 0.141 | 0.068 | 2.9→3.0 | 2.07× |
| small | flooding | 23.328 | 0.117 | 0.116 | 3.2→3.1 | 1.00× |
| medium | coma | 2.294 | 0.543 | 0.538 | 6.8→6.8 | 1.01× |
| medium | coma_instances | 342.637 | 318.399 | 8.703 | 22.6→7.8 | 36.59× |
| medium | cupid | 4.008 | 0.332 | 0.341 | 44.5→44.7 | 0.97× |
| medium | distribution | 594.343 | 66.421 | 35.313 | 7.8→7.8 | 1.88× |
| medium | jaccard | 51.935 | 106.068 | 23.522 | 6.8→6.9 | 4.51× |
| medium | flooding | 709.777 | 1.136 | 1.131 | 6.9→6.8 | 1.00× |
| large | coma | 27.361 | 2.237 | 2.009 | 65.2→65.2 | 1.11× |
| large | coma_instances | 13331.926 | 28919.854 | 665.047 | 316.7→79.2 | 43.49× |
| large | cupid | 17.150 | 0.844 | 0.876 | 80.6→80.6 | 0.96× |
| large | distribution | 4460.680 | 1613.296 | 1503.257 | 69.2→69.2 | 1.07× |
| large | jaccard | 12359.852 | 44940.931 | 5542.306 | 65.2→65.2 | 8.11× |
| large | flooding | 4554.746 | 4.873 | 8.158 | 65.2→65.2 | 0.60× |
| 01_penguins | coma | 0.652 | 0.290 | 0.269 | 3.0→3.0 | 1.08× |
| 01_penguins | coma_instances | 40.221 | 0.840 | 0.737 | 3.2→3.3 | 1.14× |
| 01_penguins | cupid | 3.198 | 0.544 | 0.610 | 42.5→42.4 | 0.89× |
| 01_penguins | distribution | 60.099 | 1.393 | 1.448 | 3.1→3.2 | 0.96× |
| 01_penguins | jaccard | 1.387 | 0.225 | 0.172 | 2.9→2.9 | 1.31× |
| 01_penguins | flooding | 236.424 | 0.663 | 0.669 | 3.9→3.8 | 0.99× |
| 02_aircraft_flights | coma | 0.735 | 0.223 | 0.210 | 3.1→3.1 | 1.06× |
| 02_aircraft_flights | coma_instances | 21.070 | 0.804 | 0.699 | 3.3→3.4 | 1.15× |
| 02_aircraft_flights | cupid | 1.587 | 0.390 | 0.396 | 42.5→42.2 | 0.99× |
| 02_aircraft_flights | distribution | 138.964 | 1.575 | 1.640 | 3.1→3.1 | 0.96× |
| 02_aircraft_flights | jaccard | 1.732 | 0.363 | 0.250 | 2.9→3.1 | 1.45× |
| 02_aircraft_flights | flooding | 250.647 | 0.641 | 0.785 | 4.0→4.0 | 0.82× |

Large COMA instances improves **43.49× (97.7% less time)** and process peak drops
**316.7→79.2 MiB (75.0%)**, versus Python's 477.0 MiB excluding children.
Its process-median timing ranges are before **28.630–39.870 s**, after
**0.510–0.770 s**, Python **10.904–15.128 s**. Large Jaccard improves **8.11×
(87.7%)**: before **39.871–48.890 s**, after **5.332–6.005 s**, Python
**11.156–12.764 s**; process peak remains about 65.2 MiB. These formerly losing
Rust workloads now beat this pinned Python configuration, not every possible
Valentine workload.

### Controls, unfavorable results and remaining limitations

The same three-way method checks high-cardinality unique documents, tokenless
Unicode, partially overlapping Unicode, dense similar names, long shared
prefixes/suffixes, and disjoint deterministic hashes. All ten comparisons pass
native exact-output and Python parity checks. No controls are silently omitted.

| Control | Algorithm | Python ms | Before ms | After ms |
|---|---|---:|---:|---:|
| coma_unique | coma_instances | 395.064 | 297.139 | 285.798 |
| coma_tokenless | coma_instances | 37.813 | 12.130 | 9.110 |
| jaccard_unicode | coma_instances | 6.947 | 1.505 | 1.585 |
| jaccard_dense | coma_instances | 105.443 | 27.493 | 45.011 |
| jaccard_long | coma_instances | 8.808 | 1.257 | 1.435 |
| jaccard_disjoint | coma_instances | 8.454 | 1.920 | 2.083 |
| jaccard_unicode | jaccard | 3.396 | 13.140 | 1.860 |
| jaccard_dense | jaccard | 966.147 | 27.363 | 21.403 |
| jaccard_long | jaccard | 621.182 | 5.279 | 5.751 |
| jaccard_disjoint | jaccard | 25.805 | 205.205 | 7.792 |

Shared-host variation is substantial, sometimes about 2× even in unchanged
algorithms. Five fresh-process native rechecks with 21 timed calls each retain
all observations: COMA dense's apparent 27.493→45.011 ms regression became
**27.260→27.578 ms** (after range 27.093–36.067); unchanged large Flooding's
4.873→8.158 ms observation reversed to **6.524→4.646 ms**. Neither supports a
kernel speedup/regression claim. Jaccard long strings have a real preparation
tradeoff: three-way **5.279→5.751 ms (+8.9%)**; the recheck was less favorable,
**5.303→7.727 ms (+45.7%)**, with after range **5.760–11.002 ms** versus before
5.262–5.422 ms. Both are reported; timing noise does not erase the overhead.
COMA unique documents use slightly more memory (23.7→24.0 MiB) and no compelling
speed gain; its long/Unicode controls also have small absolute regressions.
The large repeat-heavy and disjoint-value wins justify the limited extra work,
but these optimizations are not uniformly beneficial. Dense or colliding Jaccard
values still permit quadratic comparisons; no implicit budgets/sampling are added.

Conflicting Distribution clustering remains a separate limitation. Existing
ring8/ring16 controls run unchanged in Rust before/after but differ from Python
(64/256 native pairs versus 32/64 Python pairs); **speedup ratios are withheld**.
A temporary ring8 diagnostic proved identical 16-node sign graphs, 240 directed
binary variables, 3,360 triangles, both optimal statuses, exactly feasible binary
solutions and recomputed objective **64** on both sides. Different tied optima
(112 differing directed arcs) produce different weak zero-edge components.
This is pre-existing solver-dependent behavior, not a performance regression.
That historical probe did not separately instrument ring16; the follow-up above now does. Adding symmetry or changing the
objective would change established behavior, so neither is included here.
The interleaved32 control is equivalent: Python **18,624 ms**, Rust before/after
**46.484/45.558 ms**; Distribution code is unchanged. Process peaks were
575.0 MiB Python excluding CBC and 4.6/4.4 MiB native. Neither solved controls
nor this result establishes predictable resource consumption for general MILPs.

A first benchmark batch was **discarded in full** after finding Cargo's shared
target reused the optimized library in the supposed baseline executable. The
accepted baseline was rebuilt in an isolated target and independently verified.
An early stress batch was also rejected by strict parameter validation because
an export-only flag appeared in normal JSON; serialization was corrected, then
the suite rerun. These measurements are not used as successful comparisons.
The first core batch was interrupted by an environment restart after 21 complete
comparisons; the remaining nine were run with the same immutable binaries/inputs.

### Reproduce this follow-up

Use the pinned dependencies/NLTK setup below. Build the baseline with the **same
measurement harness but an independent Cargo target directory**:

```sh
PERF_BEFORE="$(mktemp -d)/fieldkin"
git worktree add --detach "$PERF_BEFORE" e82bdc5b901fc2ddefa0976a275a2046be6c38a9
mkdir -p "$PERF_BEFORE/examples/support"
cp examples/valentine_parity.rs examples/benchmark_performance.rs "$PERF_BEFORE/examples/"
cp examples/support/performance.rs "$PERF_BEFORE/examples/support/"
cargo build --locked --release --jobs 1 --manifest-path "$PERF_BEFORE/Cargo.toml" \
  --target-dir "$PWD/target/performance-before" \
  --example valentine_parity --example benchmark_performance
cargo build --locked --release --jobs 1 --example valentine_parity --example benchmark_performance
taskset -c 0 python validation/benchmark_valentine.py \
  --baseline-binary target/performance-before/release/examples/valentine_parity \
  --baseline-revision e82bdc5b901fc2ddefa0976a275a2046be6c38a9 \
  --rounds 3 --repetitions 3 --warmups 1 --timeout 480 \
  --output target/valentine-performance-threeway.json
```

Select an available CPU if 0 is outside your affinity. Binary/source/input hashes
are recorded automatically. The new `--dataset` option reuses the same runner for
controls; the native example's `--dump-input PATH` exports its existing generator
without matching, so Python receives identical clustering stress data:

```sh
mkdir -p target/performance-controls
target/release/examples/benchmark_performance --algorithm distribution --values ring \
  --columns 8 --rows 128 --distinct 128 --overlap 100 \
  --dump-input target/performance-controls/ring8.json
taskset -c 0 python validation/benchmark_valentine.py \
  --baseline-binary target/performance-before/release/examples/valentine_parity \
  --baseline-revision e82bdc5b901fc2ddefa0976a275a2046be6c38a9 \
  --dataset target/performance-controls/ring8.json --algorithm distribution \
  --rounds 3 --repetitions 3 --warmups 1 --timeout 120 \
  --output target/performance-ring.json
```

The ring comparison intentionally exits 1 for non-equivalence. No tolerances or
selection rules are loosened. Native stress commands below use the existing
`valentine_match` boundary, including table preparation, with separate 30-second
fresh-process deadlines and a 512 MiB address-space limit.


Generate the additional controls from the existing synthetic helper (no labels):

```sh
python - <<'PY_CONTROLS'
import hashlib, json, pathlib, sys
sys.path.insert(0, "validation")
from benchmark_valentine import synthetic
out = pathlib.Path("target/performance-controls")
out.mkdir(parents=True, exist_ok=True)
profiles = {"coma_unique": (16, 1024), "coma_tokenless": (8, 4096),
            "jaccard_unicode": (4, 128), "jaccard_dense": (8, 512),
            "jaccard_long": (2, 128), "jaccard_disjoint": (2, 512)}
for name, (columns, rows) in profiles.items():
    case = synthetic(name, columns, rows, rows)
    for side, schema in enumerate((case["source"], case["target"])):
        for column, field in enumerate(schema["fields"]):
            if name == "coma_unique":
                continue
            field["data_type"] = "text"
            values = field["samples"]
            if name == "coma_tokenless":
                values = ["東京" + chr(0x4e00 + side * rows + i) + "県" for i in range(rows)]
            elif name == "jaccard_unicode":
                values = ["東京🙂" + v + "é" for v in values]
            elif name == "jaccard_long":
                values = ["a" * 256 + v + "b" * 256 for v in values]
            elif name == "jaccard_dense":
                values = [f"customer-account-{(column * 2 + (side if i >= rows * 3 // 4 else 0)) * rows + i:016}"
                          for i in range(rows)]
            elif name == "jaccard_disjoint":
                values = [hashlib.sha256(f"{side}:{column}:{i}".encode()).hexdigest()[:32] for i in range(rows)]
            field["samples"] = values
    (out / f"{name}.json").write_text(json.dumps(case, separators=(",", ":")) + "\n")
PY_CONTROLS
```

Reuse the three-way command with `--algorithm coma_instances` and six repeated
`--dataset target/performance-controls/NAME.json` flags; for `--algorithm jaccard`
use the four `jaccard_*` files. To reproduce native stress controls:

```sh
taskset -c 0 python validation/benchmark_performance.py \
  --baseline-binary target/performance-before/release/examples/benchmark_performance \
  --rounds 3 --repetitions 3 --timeout 30 --memory-limit-mib 512 \
  --case coma-base --case cupid-base --case distribution-base --case flooding-base \
  --case jaccard-base --case jaccard-tables --case jaccard-distinct512 --case jaccard-codes512 \
  --case jaccard-single2048-overlap0 --case jaccard-single2048-overlap50 \
  --case jaccard-single2048-overlap100 --case distribution-interleaved8 \
  --case distribution-interleaved32 --case distribution-interleaved64 \
  --case distribution-ring8 --case distribution-ring12 --case distribution-ring16 \
  --output target/performance-stress.json
```

All **102/102** native stress processes completed with exact ordered output,
details and selection hashes. Disjoint/half-overlapping 2,048-value hash controls
were **633.855→16.955 ms** and **683.856→17.105 ms**; identical sets remained
**0.226→0.222 ms**. Metadata adds roughly 0.13–0.16 MiB observed process peak on
the first two controls; no substantial reduction is claimed. Ring16 completed
within the budget but regressed **1,834→2,268 ms** in this batch (unchanged code);
interleaved32 similarly changed **48.111→73.476 ms** with overlapping ranges.
These are retained unfavorable controls, not new Distribution improvements.

A larger **ring32** case (64 columns across two tables) tested one cold call,
three fresh processes per runtime, a 20-second process deadline and **2 GiB
virtual-address-space limit inherited by CBC too**. All nine processes timed
out: baseline, optimized Rust and Python. There is no completed matching time,
peak RSS, correctness result or speedup for this case. The deadline includes
startup; it is not evidence for an exact 20-second matching duration. This
confirms a remaining resource/scalability limitation without changing the
optimization objective or accepting incomplete solutions. Reproduce separately:

```sh
target/release/examples/benchmark_performance --algorithm distribution --values ring \
  --columns 32 --rows 128 --distinct 128 --overlap 100 \
  --dump-input target/performance-controls/ring32.json
(
  ulimit -v 2097152
  taskset -c 0 python validation/benchmark_valentine.py \
    --baseline-binary target/performance-before/release/examples/valentine_parity \
    --baseline-revision e82bdc5b901fc2ddefa0976a275a2046be6c38a9 \
    --dataset target/performance-controls/ring32.json --algorithm distribution \
    --rounds 3 --repetitions 1 --warmups 0 --timeout 20 \
    --output target/performance-ring32-deadline.json
)
```

### Validation completed

Default tests, Polars tests, all-target/all-feature tests, four doctests, strict
all-feature Clippy, warning-free all-feature rustdoc, Python methodology/ranking
checks, reference-patch checks, all four original suggestion quality gates,
independent Valentine accuracy gate and development accuracy report passed.
Full current live parity: **90/90** against the explicit fixed reference, **60
numerically exact**, **89 exactly ranked**, all 9,199 pairs within existing
tolerances. Every before/after native output is exact across all 90 cases.
The unmodified upstream run completed but the comparator correctly exited 1:
**88/90 equivalent** here. USGS Distribution retains its three sanitized-name
mismatches, and customers Cupid's balance→account_balance differs by
0.0136752136752 due to its documented object-ID-dependent WordNet orientation.
Neither difference is new; the same Rust baseline results agree exactly with
the optimized build. The two existing source fixes are kept separate and the
upstream checkout remains clean; no fixture/tolerance/reference patch changed.

`cargo package --locked` verified the runtime commit without `--no-verify`.
The package excludes benchmark/evaluation tooling and measurements. Real optional
ONNX inference also passed with official ONNX Runtime 1.28.0 and a local,
checksum-verified Qdrant MiniLM cache (model revision
`d13954661f83248295ba75c1ed411eef3b7b936e`). Automatic model retrieval failed in
the proxy environment; curl retrieved the pinned weights with TLS verification,
so this is a download-client limitation, not an unavailable inference check.
Weights and runtime binaries remain outside Git/the published crate.
Linux validation executed locally; Windows execution belongs to the existing CI.
No cross-platform success is inferred from Linux tests.

Re-run validation with the commands in the historical section below, using
`--check-independent`, `--check-reliability` and `--check-semantic` for the
corresponding suggestion gates. A local ONNX installation can be selected with
`ORT_LIB_LOCATION`, `ORT_PREFER_DYNAMIC_LINK=1` and `LD_LIBRARY_PATH`; these checks
used the actual runtime, not `ORT_SKIP_DOWNLOAD`. To reproduce live parity:

```sh
target/release/examples/valentine_parity target/parity/current-rust.json
python scripts/run_valentine_parity.py --reference-fixes --output target/parity/current-python-fixed.json
python scripts/compare_valentine_parity.py --rust target/parity/current-rust.json \
  --python target/parity/current-python-fixed.json --output target/parity/current-fixed-comparison.json
python scripts/run_valentine_parity.py --output target/parity/current-python-upstream.json
python scripts/compare_valentine_parity.py --rust target/parity/current-rust.json \
  --python target/parity/current-python-upstream.json --output target/parity/current-upstream-comparison.json
```

The unmodified comparison may expose either or both documented defects; it is
not expected to pass unconditionally. Reproduce native exactness by running the
isolated baseline example on the same inputs and comparing `output_sha256` for
every dataset/algorithm. Expensive performance runs stay outside regular CI.


---

# Historical measurements (2026-10-11)

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
All three large Rust Jaccard workers timed out before completing the full
warmup-plus-three-measurement sequence, while all three Python workers completed.
The retained worker logs show completed warmups of 85.827, 59.771 and 57.057
seconds, followed by one or two completed measurements before the 180-second
process deadline. The incomplete sequences cannot support an accepted comparison.

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

## Follow-up: reuse Jaccard's Levenshtein patterns

Compared with `e96ab8e`, Jaccard now prepares a RapidFuzz pattern once per source
value when repeated comparisons remain useful, instead of rebuilding it for
every candidate. Its existing rounded cutoff also lets the scorer stop below
that cutoff. Setup is skipped for a single target or once all target values have
partners, preserving the cheap dense-match path. No thresholds, aggregation,
public APIs or other distance functions changed; the production change is 13
lines, using the existing dependency.

[Raw follow-up measurements](jaccard_pattern_reuse_results.json) retain all
exploratory and final observations, input/binary hashes, output fingerprints and
historical timeout logs. Final runs used the same release toolchain and machine
as above, pinned to CPU 0. Three fresh processes per version alternated order;
values below are medians of process medians, with the full process-median range.
Small and real inputs used one warmup and eleven measurements; medium used one
warmup and five measurements. Large used **one cold call per process**, avoiding
an incomplete four-call sequence. These large measurements are not a rerun of
the full head-to-head comparison or evidence that its 180-second deadline passes.

| Jaccard input | Before ms [range] | After ms [range] | Observed change |
|---|---:|---:|---:|
| Synthetic small | 0.193 [0.192, 0.194] | 0.138 [0.138, 0.142] | −28.3% |
| Synthetic medium | 560.475 [265.818, 580.002] | 159.452 [153.853, 200.562] | −71.6% |
| Synthetic large | 74,737 [66,152, 95,723] | 59,312 [41,522, 64,590] | −20.6% |
| Penguins | 0.244 [0.242, 0.253] | 0.246 [0.242, 0.253] | +0.9% |
| Aircraft/flights | 0.379 [0.340, 0.380] | 0.355 [0.346, 0.385] | −6.3% |

All three paired large rounds improved, but substantial within-batch variation
limits the precision of speedup claims, especially for medium. No meaningful RSS
improvement was measured: large process peaks were 65.19 versus 65.24 MiB.
Small real-data differences have overlapping ranges and provide no strong claim
of improvement. The algorithm still has quadratic distinct-value comparisons.

The existing 13 Jaccard performance profiles also ran before/after with three
fresh processes and five measurements each. Twelve improved in that batch.
The 256-distinct-value profile regressed 89.978 → 114.948 ms amid wide variation;
a focused five-process, nine-measurement follow-up measured 59.226 → 38.779 ms
(ranges 59.099–61.515 and 38.039–39.205), with unchanged results. Both batches are
retained. An earlier eager-pattern version regressed on dense codes32 samples
(2.607 → 3.187 ms). The final version skips setup after all targets find partners;
its dense-codes batch measured 0.863 → 0.761 ms. The interrupted exploratory
large sweep and every completed exploratory measurement are retained as well.

All measured output fingerprints agree with the baseline, including the existing
native benchmark's scores, details and selection fingerprints. The final large
output also matches the pinned Python output exactly: 32 pairs, identical scores
and rankings. The existing Cartesian regression now also covers cached patterns
at Unicode/bit-vector length boundaries and adjacent `f32` cutoffs; an additional
local scoring probe checked 458,119 threshold decisions against full scoring.

Follow-up validation passed 176 native/Polars tests, four doctests, formatting,
strict Polars Clippy, static all-feature Clippy, rustdoc, verified packaging and
the packaged README consumer. The frozen quality check and both accuracy reports
are byte-identical to the baseline. All 9,199 live Rust correspondence scores and
details retain their exact bits; the recorded pinned reference with source fixes
passes 90/90 cases. Unmodified upstream retains its documented USGS identifier
failure (89/90). All-feature execution and ordinary all-feature Clippy remain
blocked by the ONNX Runtime download's proxy HTTP 403; static linting does not
establish inference execution.

To repeat the native controls, build `e96ab8e` in a separate directory and the
current release examples, as in the build instructions above, then run:

```sh
cargo build --locked --release --lib --example valentine_parity --example benchmark_performance --jobs 1
taskset -c 0 python validation/benchmark_performance.py \
  --baseline-binary /tmp/fieldkin-before/target/release/examples/benchmark_performance \
  --output target/jaccard-pattern-controls.json --rounds 3 --repetitions 5 \
  --case jaccard-base --case jaccard-width --case jaccard-rows \
  --case jaccard-distinct --case jaccard-tables --case jaccard-overlap0 \
  --case jaccard-schema0 --case jaccard-schema50 --case jaccard-overlap100 \
  --case jaccard-sampled --case jaccard-distinct512 \
  --case jaccard-codes32 --case jaccard-codes512
```

For the focused follow-up use `--case jaccard-distinct --rounds 5 --repetitions 9`.
For the five schema pairs, reuse the synthetic inputs generated by
`benchmark_valentine.py` and the two frozen real inputs. Run each release parity
binary in three alternating fresh processes with the warmup/repetition counts
above; for example, the large invocation is:

```sh
taskset -c 0 target/release/examples/valentine_parity target/jaccard-large.json \
  --direct --dataset target/valentine-performance-final/large.json \
  --algorithm jaccard --warmups 0 --repetitions 1
```
