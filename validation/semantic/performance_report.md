# Rejected corroboration prototype: measured runtime cost

This compares the unchanged original suggestion API with the frozen optional COMA/Jaccard corroboration candidate. It measures **additional cost, not a speed improvement**. No held-out data was used.

| Workload | Original median ms | Optional median ms | Optional / original | Original peak RSS KiB | Optional peak RSS KiB | Original median range ms | Optional median range ms |
| --- | ---: | ---: | ---: | ---: | ---: | --- | --- |
| exposed597 | 221.833 | 284.894 | 1.284× | 8096 | 9264 | 219.335–225.142 | 282.334–285.780 |
| aligned_small | 0.289 | 0.500 | 1.730× | 2012 | 2144 | 0.274–0.305 | 0.499–0.508 |
| aligned_medium | 24.973 | 27.308 | 1.093× | 2780 | 4156 | 24.899–25.234 | 27.097–27.774 |
| aligned_large | 779.902 | 789.757 | 1.013× | 9624 | 21672 | 774.101–784.163 | 789.522–810.729 |
| ambiguous_small | 1.499 | 4.887 | 3.260× | 2092 | 2336 | 1.491–1.522 | 4.815–4.900 |
| ambiguous_medium | 22.654 | 181.375 | 8.006× | 2800 | 3860 | 22.449–23.071 | 180.221–182.276 |
| ambiguous_large | 192.443 | 2671.062 | 13.880× | 5060 | 9272 | 187.354–192.632 | 2664.180–2687.461 |

Synthetic inputs are fixed SHA-256-derived values. Aligned small/medium/large use 8/24/64 source and target columns, 32/256/1024 sample values per column, and 8/64/256 distinct values. Source and target names and values are identical. Ambiguous small/medium/large use 8/16/32 source columns and twice that many target columns, 64/256/512 samples per column, and 16/64/128 distinct values. Each source has an identical nested target and a flat target sharing approximately four-fifths of its values. This deliberately activates the optional Jaccard branch while cross-column high-entropy values force costly negative comparisons. These synthetic schemas are performance inputs, not independently labeled semantic accuracy evidence.

Each workload/mode ran in three fresh release processes. Each process loaded/generated inputs, performed one matching warmup, then five measured matching runs. The table reports the median of the three process medians and their minimum/maximum. Mode order alternated between original-first and optional-first. Matching timers include result construction and destruction and exclude loading, generation, hashing, and process startup. Results were consumed through Rust black_box. Every process reproduced the previously verified full-output digest. Verification ran in separate processes, confirmed original candidate order and exact numeric bit patterns, and recorded intentional decision differences (19 exposed fields; zero aligned fields; 8/16/32 ambiguous fields). No timing failures or timeouts occurred.

Peak RSS is Linux VmHWM for each full process, including resident inputs and warmup. It is not an allocation count or isolated incremental matcher memory. The RSS table reports the median of the three fresh processes. Runtime ratios and RSS are hardware/environment-specific. The shared cloud environment was otherwise kept free of heavy builds during the timing window; no CPU pinning was used.

The corpus cost increased about 28%, while the large ambiguous workload increased 13.9-fold because the rejected prototype ran fuzzy Jaccard across the whole table pair. It inherited Jaccard quadratic distinct-value comparisons and offered no predictable large-input resource bound. This is an additional cost of the rejected experiment; no optional corroboration API is shipped.

The large aligned workload also exposed unnecessary memory cost: cloning full samples into tables for COMA even though default COMA uses only schema names. The aligned original peak of 9,624 KiB grew to 21,672 KiB. Constructing schema-only COMA tables, then cloning samples only when Jaccard is required, could reduce this prototype cost, but was not applied because the semantic policy failed independent validation. No such optimization or API was retained.

## Environment

Measured UTC: 2026-10-13T00:07:43.531625+00:00
Git base: `c6b5a2caa2c337ee63ff0dd2993bed352eff2637`.
CPU quota: `400000 100000`; allowed CPUs: `0-4`; container memory limit: 34359738368 bytes.
Rust toolchain:

```text
rustc 1.99.0 (b940084d7 2026-09-28)
binary: rustc
commit-hash: b940084d7eb6a299eb4bfeb8e34901bc051e7ac4
commit-date: 2026-09-28
host: x86_64-unknown-linux-gnu
release: 1.99.0
LLVM version: 23.1.1
```

Exact CPU/OS details, per-repeat times, RSS ranges, verification fingerprints and source hashes are in `performance_results.json`.

## Reproduce the rejected experiment

The benchmark is an archived experiment, not a retained runtime API. The runner
creates a temporary checkout of the measured base revision, applies the frozen
`candidate.patch`, installs the small benchmark example, and builds it with the
original locked dependencies, verifies every frozen runtime-file hash, and checks
all recorded output fingerprints. If the base revision is absent in a shallow
clone, fetch the revision printed in the runner error. It does not change the working tree or evaluation
labels. It uses only the previously exposed corpora from that revision.

```sh
python3 validation/semantic/benchmark_corroboration.py
```

The default output is `target/semantic_performance.json`; use `--output PATH` to
choose another path. `--verify-only` checks all frozen output fingerprints and
candidate bit patterns without timing runs. Rust/Cargo must be on PATH. Managed-cloud users may need the
session's existing `CARGO_HOME` and `RUSTUP_HOME` settings. The archived raw results
were measured using the original temporary harness and contain its exact hashes;
this reproduction harness calls the same frozen implementation through the
patched crate API instead of keeping a second implementation.

The semantic candidate was rejected after the two fully sealed packets produced
zero additional correct automatic matches and one additional false automatic
match. A partially exposed packet improved, but supplies no held-out generalization
evidence. No policy, labels, or gates were tuned after that result.
These timings document an experiment's cost; they do not justify adding its API.
