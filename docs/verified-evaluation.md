# Verified external evaluation runs

Use this workflow when recording new external development evidence. The original
evaluator calculates source hashes from the checkout at execution time; an old
executable can therefore report newer on-disk source hashes. The verified runner
binds a fresh build to its input inventory and executable, and refuses a run when
either changes. It adds no dependency or behavior to the Rust library.

The same recorded build can run the fixed Northix task through the native Rust
`fieldkin-tools record-northix` command; see the
[Northix workflow and results](northix-evaluation.md). That route requires either
no external scores or both pinned comparison modes and records their exact bytes
alongside the shared executable and input hashes. It does not enable holdout flags.

## Build and run

Latest stable Rust is required, with 1.99.0 used for October 4, 2026 validation.
Fixture import and build/run recording use Rust directly. Fetch the locked Cargo
dependencies once if they are not cached; the recorded evaluator build uses
`--locked --offline`. The importer checks only the vendored pinned archives and
does not execute a matcher.

```text
cargo +stable fetch --locked
cargo +stable run --locked -p fieldkin-tools -- import-t2d --check
cargo +stable run --locked -p fieldkin-tools -- import-northix --check
cargo +stable run --locked -p fieldkin-tools -- verify-build --build-dir target/external-verified --toolchain stable
cargo +stable run --locked -p fieldkin-tools -- verify-run --build-dir target/external-verified --output target/external-results
```

Both destinations must be new. Build directories are confined to the checkout's
`target/`; run outputs can also use a new directory under `evaluation/results/`.
The build uses an isolated Cargo target directory and copies the actual executable
reported by Cargo. `build.json` records the executable hash, source/fixture
inventory, compiler, Cargo, command and build configuration. The external run
invokes only the development evaluator. There is no arbitrary argument
passthrough or external holdout option.

The Northix route uses the same build record and a separate fresh output:

```text
cargo +stable run --locked -p fieldkin-tools -- record-northix --build-dir target/external-verified --output target/northix-results
```

To compare the archived external matcher scores, provide exactly two distinct
files, one for each recorded mode:

```text
cargo +stable run --locked -p fieldkin-tools -- record-northix --build-dir target/external-verified --output target/northix-comparison --scores evaluation/results/continuation-v1/valentine/valentine-schema_only.json --scores evaluation/results/continuation-v1/valentine/valentine-schema_and_samples.json
```

Local Python/Valentine COMA execution is retired. This command imports historical
scores, verifies their bytes and reapplies the Rust selector; it does not produce
new COMA measurements.

The separate prospective contextual route uses one fixed new synthetic corpus,
candidate and comparison protocol. It requires the same frozen build and a fresh
output, explicit acknowledgement, and the executable/build-record digests supplied
by the wrapper:

```text
cargo +stable run --locked -p fieldkin-tools -- record-context --build-dir target/context-candidate-v1 --output evaluation/results/context-qualification-v1 --acknowledge-new-holdout
```

Use it only after completing implementation and development checks. It persists
`target/context-qualification-v1.started.json` before scoring, refuses a second
prospective attempt, and records the new reserved partition accurately in
`run.json`. A failed or interrupted score still examines the holdout. No other
reserved partition is enabled. See the [fixed protocol](../evaluation/context-qualification-protocol.json)
and [qualification limits](release-readiness.md).

Run success requires matching inventories and executable hashes before and after
execution, a successful evaluator exit, and exactly the three expected development
files in `artifacts/`. Only then is the adjacent `run.json` written with their
hashes. If a build or run
fails, keep its partial directory for diagnosis and choose a fresh destination;
the runner never upgrades or overwrites an earlier attempt. An artifact directory
without its successful run record is incomplete evidence.

## What is checked

The input inventory covers library and evaluator sources recursively, manifests
and lockfile, optional build/toolchain files, corpus and protocol files, pinned
external data and importer/runner code. It detects added and deleted files as
well as changed content. Files use SHA256 over their exact bytes; different
checkout line endings therefore require a new build. Symlinks are rejected.
The README is included because `src/lib.rs` embeds it. When adding a build input
outside the recorded directories, extend the inventory and its tests before
relying on a verified record; arbitrary file reads by build scripts are not tracked.

Cargo configuration is inspected at the checkout, its ancestors and Cargo home.
Relevant build environment settings are recorded as hashes rather than raw values.
Unsupported source replacements, local dependency overrides, configuration
includes and cross-compilation are rejected instead of being silently omitted
from the recorded input boundary. Compiler-wrapper overrides are also rejected.
The recorded workflow is intentionally narrow.

`--external --check` compares an exact native development snapshot, including
source metadata. Historical artifacts remain archived with their original
provenance. CI exercises current native snapshots, importer/runner tests and a
fresh verified build/run on Linux, Windows and macOS using latest stable Rust.

## Limits

This is a local reproducibility guard, not a signed attestation or a hermetic build.
It trusts the local operating system, Rust tooling, Cargo, compiler, linker and dependency
cache. It does not resist a process that maliciously changes files and restores
them between checks, or an actor who rewrites both evidence and hashes. A success
record describes the observed inputs and executable; it does not prove matching
accuracy or establish the original precision/coverage targets.

Direct evaluator commands retain their original behavior and do not acquire this
build/run guarantee. Other synthetic evaluation modes, qualification tools and
performance tools have their own recorded boundaries; this wrapper does not
retroactively verify them. The external experiment still uses positive-only labels,
scores 549 development tables, and leaves all 218 reserved tables unscored.

## Historical qualification: October 3, 2026

Implementation and tests were committed at
`d1668412685b051080c32dc9d8b89aa3825e733b` before the real build and run.
On Windows with Python 3.11 and Rust 1.85.0:

- All 24 runner tests and five importer tests passed. The runner tests exercise
  stale sources/binaries, recursive additions/deletions, build and run drift,
  configuration changes, manifest source paths, symlinks/reparse points,
  incomplete/oversized output, existing destinations and prohibited CLI arguments.
- A fresh offline build inventoried 45 files and produced executable SHA256
  `68b24f02cea7a36895174b292af0a3f237115bd6b674705cf116d51e39ffeceb`.
- The verified development run succeeded. Its JSON, JSONL and Markdown outputs
  exactly match the raw Git blobs of `evaluation/results/t2d-v1` at `eb95408`,
  including all source metadata. Historical files were neither rewritten nor
  duplicated. The new records reference those identical artifact bytes.
- Changing `CARGO_PROFILE_RELEASE_DEBUG` after the build caused the real runner
  to reject before creating any evaluation artifacts or success record. Retrying
  that partial output directory was also rejected.

The [build/run records and exact comparison](../qualification/results/verified-external-v1/)
include file and executable hashes; `rejection.json` records the negative check.
The recorded build interval was 27.16 seconds and run interval 7.77 seconds. These
are single execution records, not matching benchmarks or performance comparisons.
No library or evaluator Rust source, dependency, scoring policy or historical
snapshot changed in this phase; matching benchmarks were not rerun.

Those records describe the earlier Python runner and Rust 1.85.0 checkout. They
remain historical evidence; current native runs use the commands above and a new
destination. The active project maintains latest stable Rust only. The native
workflow does not relabel or retroactively verify the older artifacts.
