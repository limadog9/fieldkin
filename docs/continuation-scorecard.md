# Phases 11–14 qualification

This continuation covers assignment allocation reuse, bounded scale measurements,
optional JSON reports and saved review, an independent Northix diagnostic, and a
pinned Valentine COMA comparison. Fieldkin remains experimental and unpublished.
The original 95% precision and 60% unique-coverage targets are unchanged.

## Frozen acceptance and measurement plan

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
The 218 reserved T2D tables remain unscored. The pinned comparator runs with
`PYTHONHASHSEED=0` and `OMP_NUM_THREADS`, `OPENBLAS_NUM_THREADS` and
`MKL_NUM_THREADS` each set to `1`; it records those environment values by hash.

Qualification requires both Rust 1.85.0 and 1.99.0, JSON enabled and disabled,
strict formatting/Clippy/documentation checks, the importer and runner failure
tests, packaging, and a fresh 1,200,000-case generated campaign with seed
`20261007`. CI must pass on Linux, Windows and macOS. Historical result files stay
unchanged; new strict snapshots must preserve all outcomes while recording the
changed implementation and optional-dependency provenance explicitly.

## Status

Implementation and qualification are in progress. Results will be recorded here
after the frozen runs; no accuracy or performance improvement is claimed yet.

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
