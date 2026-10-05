# Phases 11–14 qualification

This continuation evaluates assignment allocation reuse, bounded scale measurements,
optional JSON reports and saved review, an independent Northix diagnostic, and a
pinned Valentine COMA comparison. Fieldkin remains experimental and unpublished.
The original 95% precision and 60% unique-coverage targets are unchanged.

## Frozen acceptance and measurement plan

This section preserves the continuation's historical acceptance plan. Current
development supports only latest stable Rust; Rust 1.85 maintenance and the
Python runners, including local Valentine COMA execution, are retired. The
archived COMA scores can still be imported by native Rust tools.

The baseline is main commit `104b8e9e41cdcf31efdc268ad58c81514dd3568d`.
Matching weights, thresholds, ambiguity rules and default resource limits do not
change. Scratch buffers may be reused only with complete report/callback equality
on the 1,024-case compatibility corpus and all existing regression decisions.
The expected allocation reduction is two allocations per additional source row
in each nonempty dense assignment solve. This is not a predicted latency gain.

Before measuring, freeze the implementation, harnesses, datasets and protocols.
Build every executable before starting timings. Run the existing 27 assignment
workloads and 50 default workloads with their five-process protocols, retaining
every result. An unexplained core-workload median regression above 10% fails the
optimization acceptance gate; preserve failed evidence and qualify any subsequent
revision separately. The earlier phase-10 regressions remain part of the record.
Run all 18 scale cases in five fresh processes, with explicit raised caller limits
at 512 and 1,000 fields. Those timings characterize these fixed cases, not worst
case complexity, memory usage or production capacity.

The [Northix protocol](../evaluation/northix-protocol.json) fixes all 84 native
table pairs, deterministic samples, published-class label interpretation and the
same selector settings for Fieldkin and imported COMA scores. Independent
selection is primary: one-to-one selection can recover at most 27 of 28 positive
edges in this task. There is no model tuning, threshold search or holdout claim.
The 218 reserved T2D tables remain unscored. The archived pinned comparator ran with
`PYTHONHASHSEED=0` and `OMP_NUM_THREADS`, `OPENBLAS_NUM_THREADS` and
`MKL_NUM_THREADS` each set to `1`; it records those environment values by hash.

The historical qualification required both Rust 1.85.0 and 1.99.0, JSON enabled and disabled,
strict formatting/Clippy/documentation checks, the importer and runner failure
tests, packaging, and a fresh 1,200,000-case generated campaign with seed
`20261007`. CI must pass on Linux, Windows and macOS. Historical result files stay
unchanged; new strict snapshots must preserve all outcomes while recording the
changed implementation and optional-dependency provenance explicitly.

## Status

The first candidate, `6d3e4a716d214ea41b362650092af816fa5ff48d`, passed functional
qualification but failed performance acceptance. Eight of 27 assignment workloads
and ten of 50 default-suite workloads exceeded a 10% median regression. All runs
remain in `performance/results/continuation-v1`. The optimization reduced
allocations in 18 workloads, but those savings do not waive the latency gate.

Scratch-buffer reuse is rejected for this build. The solver is restored exactly
to main's `104b8e9` implementation; the optional JSON API, external comparison and
scale tools remain. The retained freeze is
`5bfaa295de68178e801b79e158ea06f9a8279e74`, with separate qualification
and measurement records in `continuation-v2`. Scores from the fixed Valentine
run remain unchanged in `evaluation/results/continuation-v1/valentine`; no matching
policy is being tuned from the inspected Northix results.

Implementation and functional qualification for phases 11–14 are complete.
Accuracy and latency acceptance targets remain unmet; this is an experimental
build, not a production-readiness or publication decision.

Packaging subsequently exposed unanchored Cargo include patterns selecting local
tool README files. Freeze 1176e6f anchors all 13 patterns to the crate root.
[Parsed manifest comparison](../qualification/results/continuation-v3/packaging-change.json)
confirms no other manifest, dependency or runtime change. Final evaluator,
compatibility and campaign records use continuation-v3; performance remains the
explicitly measured 5bfaa29 revision in continuation-v2. The failed packaging
attempt remains in the v2 record.

The [first PR CI run](https://github.com/limadog9/fieldkin/actions/runs/37224658937)
then exposed two Python fixture failures on macOS: its temporary directory passes
through a system symlink, correctly rejected by the production path guard.
Resolving the tests' own trusted fixture roots fixes that portability issue
without changing the guard. The [v4 Northix refresh](../evaluation/results/continuation-v4/northix/run.json)
uses the same verified v3 executable. Predictions and the readable table are byte
identical; only the test file's metadata hash changes in the result JSON. Other
qualification records remain at their actual measured revisions.

## Retained API and qualification

Enable `features = ["json"]` for bounded `json::report_to_json` exports and
`review_to_json` / `review_from_json` persistence. The application supplies trusted
source/target revisions and passes loaded constraints to
`MatchEngine::match_schemas_with_constraints`. Default exports omit arbitrary
matcher text; reports cannot be imported as approvals. The compiling
[persisted-review importer](../examples/persisted_review.rs) demonstrates resuming
explicit review and rejecting stale revisions.

The [runtime comparison](../qualification/results/continuation-v2/runtime-comparison.json)
confirms every existing matching source file is identical to the baseline.
`lib.rs` only adds the feature-gated JSON module declaration. Default builds keep
the same two direct dependencies; optional Serde edges add no package version or
checksum changes to the existing lockfile.

| Check | Result |
| --- | --- |
| Rust 1.85.0 and 1.99.0 | 242 workspace tests each, including 19 JSON tests and eight doctests; two scale-example tests, one performance test and two qualification tests: 247 distinct tests per compiler |
| JSON disabled | 169 library/integration/doctests per compiler, separately passed |
| Formatting, Clippy, docs and consumer examples | Passed on both compilers; warnings denied for Clippy and rustdoc |
| Python protocol tests | 67 evaluation/importer/runner/comparator tests and ten performance-driver tests passed |
| Generated campaign | 1,200,000 cases, seed 20261007; six categories of 200,000, all passed |
| Baseline compatibility | All 1,024 full reports and callback traces exactly equal, including original/reversed inputs; 5,536,669-byte transcripts |
| Development/regression evidence | Outcomes, predictions and corpus unchanged across baseline, Stage 3, release, corrective and T2D development snapshots |
| Northix repeat on retained runtime | All 156 summary rows and complete predictions identical to the first freeze; original Valentine score files reused |
| Rust dependency review | All three lockfiles: zero known vulnerabilities and zero warnings using the recorded RustSec snapshot; 42 third-party versions reviewed |

Exact commands, compiler selections and counts are in
[checks.json](../qualification/results/continuation-v2/checks.json),
[campaign build](../qualification/results/continuation-v3/build.json),
[campaign result](../qualification/results/continuation-v3/run.json),
[compatibility](../qualification/results/continuation-v3/compatibility.json),
[regression comparison](../qualification/results/continuation-v3/regression-comparison.json),
[snapshot checks](../qualification/results/continuation-v3/snapshot-checks.json), and
[dependency review](../qualification/results/continuation-v1/dependencies.json).
The archived CI matrix ran both compilers on Linux, Windows and macOS. Current
CI uses latest stable Rust on those three operating systems.
[Packaging verification](../qualification/results/continuation-v4/packaging.json)
records all four compiler/feature combinations and the complete archive inventory
after the final documentation and portable test-fixture updates.

The first baseline-recording attempt caught a separate environment difference:
the evaluator's ambient `rustc -Vv` reports the machine's default 1.98.1, whereas
the executable was built with verified 1.85.0. That environment-only difference
and the initial failed recording are retained. All other environment fields and
all outcome bytes match. New snapshots record current implementation and root
Cargo provenance without rewriting historical evidence. The historical corrective
`--check` exempted its two implementation-hash maps; that cycle's separate
recording comparison verified those maps against the frozen checkout. Current
native snapshots check their own implementation provenance strictly.

## Measured costs and failed performance gate

Both freezes ran the same 27 assignment and 50 default workloads, with five
processes per revision and separate timing/allocation executables for the default
suite. Each also ran the 18 scale cases in five fresh processes. All builds and
tests stopped before timing; no outlier was removed and no measurement was
repeated to select a more favorable result.

The rejected candidate saved exactly 30, 126 or 254 allocations per call in 18
default workloads, consistent with `2 * (source_rows - 1)`. Its eight assignment
and ten default-suite regressions above 10% remain archived in
[v1 assignment results](../performance/results/continuation-v1/solver-cost/summary.json)
and [v1 default results](../performance/results/continuation-v1/default-regression/summary.md).

With the prior solver restored, all six allocation metrics and their ranges are
identical to baseline for all 50 workloads. Final timings still include six
regressions above 10%; the latency target is not demonstrated by this experiment:

| Suite / workload | Before median µs | Retained median µs | Change |
| --- | ---: | ---: | ---: |
| Assignment, 16 all-excluded | 121.030 | 145.810 | +20.474% |
| Assignment, 16 mixed | 117.830 | 139.130 | +18.077% |
| Assignment, 64 sparse | 3,348.200 | 3,854.900 | +15.134% |
| Default, dense assignment | 11,887.280 | 14,769.680 | +24.248% |
| Default, dense independent | 8,607.480 | 9,782.160 | +13.647% |
| Default, sparse assignment | 9,121.400 | 10,118.780 | +10.935% |

Full medians, ranges and every comparison remain in
[v2 assignment results](../performance/results/continuation-v2/solver-cost/summary.json)
and [v2 default results](../performance/results/continuation-v2/default-regression/summary.md).
Fifteen of 27 assignment medians and 21 of 50 default medians were slower;
none of the 24 explicitly named `core-*` default cases exceeded 10%.
Identical runtime source does not imply identical binary layout or wall-clock
measurements. These runs do not identify a cause, establish latency equivalence,
or justify dismissing the regressions as noise. The earlier
[phase-10 regressions](assignment-performance.md) also remain unresolved evidence.
No assignment optimization or speedup claim is retained from this cycle.

Builtin scale results, milliseconds per call, median [minimum–maximum]:

| Fields per schema | Independent | One-to-one |
| ---: | ---: | ---: |
| 128 | 30.028 [28.390–39.064] | 44.938 [42.034–52.125] |
| 512 | 460.251 [450.589–523.484] | 740.336 [714.852–760.131] |
| 1,000 | 1,794.606 [1,702.710–1,957.206] | 3,102.716 [3,010.299–3,665.044] |

The [scale protocol](scale-experiment.md) and
[all 18 results](../performance/results/continuation-v2/scale/summary.json) cover
synthetic diagonal, partial and builtin evidence. Limits are explicitly raised
for the larger calls; defaults stay at 128 fields and 16,384 pairs. Global probes
are disabled, all pairs are scored, and one displayed candidate does not bound
the internal graph. These are neither worst-case nor peak-memory measurements.

The archived runs used the retired Python runners after separate recorded builds.
Their original commands, binary hashes and inputs remain in each directory's
`build.json`; this does not make them current development instructions.

For new native measurements, first build into fresh directories:

```text
cargo +stable run --locked -p fieldkin-tools -- perf-assignment build --baseline .local/continuation-baseline --candidate . --output target/continuation-solver-native
cargo +stable run --locked -p fieldkin-tools -- perf-run build --baseline .local/continuation-baseline --candidate . --output target/continuation-default-native
cargo +stable run --locked -p fieldkin-tools -- perf-scale build --output target/continuation-scale-native
cargo +stable run --locked -p fieldkin-tools -- qualify-build --output target/continuation-campaign-native
```

The paired baseline must be a detached checkout of the recorded baseline above;
create it once with `git worktree add --detach .local/continuation-baseline 104b8e9`.
After builds and tests finish, measure sequentially without concurrent work:

```text
cargo +stable run --locked -p fieldkin-tools -- perf-assignment run --output target/continuation-solver-native
cargo +stable run --locked -p fieldkin-tools -- perf-run run --output target/continuation-default-native
cargo +stable run --locked -p fieldkin-tools -- perf-run summarize --output target/continuation-default-native
cargo +stable run --locked -p fieldkin-tools -- perf-scale run --output target/continuation-scale-native
cargo +stable run --locked -p fieldkin-tools -- qualify-run --output target/continuation-campaign-native --cases 1200000 --seed 20261007
```

These native records use distinct protocols and latest stable Rust. The generated
campaign now includes a seventh contextual-evidence category; no new full campaign
measurement is claimed here. Runners reject overwriting evidence. Raw assignment CSV and
scale JSONL hashes survive Git archival; the older default driver records
workload inventories without raw-file hashes. These are local drift records,
not signed or hermetic attestations.

## Accuracy and adoption outcome

Northix supplies 84 correlated table pairs, 469 source occurrences and 28 positive
edges. The weighted default proposes nothing, both with and without samples:
precision is undefined and positive-field recall is zero. The name-only baseline
selects 14 correct pairs out of 20 proposals (70% precision, 50% positive recall).
Schema-only COMA selects 17/31 independently (54.84% precision, 60.71% recall),
or 17/27 one-to-one (62.96% precision). COMA with samples also proposes nothing
at the fixed threshold. Every no-match error, abstention and candidate outcome
remains in the [full results](northix-evaluation.md).

The common 0.70 threshold and 0.08 ambiguity margin do not calibrate model scores.
Published classes are coarse: Northix even groups rental/return dates with payment
dates. These results cannot certify business semantics or production precision.
The 218-table T2D holdout remains reserved. The previously reported 95% precision
and 60% unique-coverage release targets remain unmet; this cycle does not supply
independent downstream adoption or authorize publication.

## Boundaries

The [JSON API](json.md) persists explicit application review, not automatic
approval or sample values. Its revision check is neither authentication nor a
schema fingerprint. The reference importer is maintained in this repository;
independent consumer adoption has not been established.

[Northix](northix-evaluation.md) is independently authored demonstration data
with injected rows and reused, correlated tables. Published classes define this
diagnostic's correct and incorrect proposals; they cannot certify every business
meaning. The [Valentine comparison](valentine-comparison.md) holds selection rules
constant but does not calibrate different score scales or reproduce a paper's
benchmark. Runtime Fieldkin still needs no Python, network, database or API key.

Prepared-schema caching, candidate pruning, transformations, embeddings, Python
bindings and elaborate adapters remain outside this cycle. A concrete consumer
and fresh evidence should justify changes to that scope. Publication remains the
sole maintainer's decision.
