# Verified external evaluation runs

Use this workflow when recording new external development evidence. The original
evaluator calculates source hashes from the checkout at execution time; an old
executable can therefore report newer on-disk source hashes. The verified runner
binds a fresh build to its input inventory and executable, and refuses a run when
either changes. It adds no dependency or behavior to the Rust library.

The same recorded build can run the fixed Northix task through
`evaluation/record_northix.py`; see the
[Northix workflow and results](northix-evaluation.md). That route requires either
no external scores or both pinned comparison modes and records their exact bytes
alongside the shared executable and input hashes. It does not enable holdout flags.

## Build and run

Python 3.11 or newer and the chosen installed Rust toolchain are required. No
Python packages are needed. On Windows, use `py -3.11` if `python` selects an older
interpreter. Fetch the locked Cargo dependencies once if they are not cached;
the recorded build uses `--locked --offline`.

```text
cargo +1.85.0 fetch --locked
python evaluation/verified.py build --build-dir target/external-verified --toolchain 1.85.0
python evaluation/verified.py run --build-dir target/external-verified --output target/external-results
```

Both destinations must be new. Build directories are confined to the checkout's
`target/`; run outputs can also use a new directory under `evaluation/results/`.
The build uses an isolated Cargo target directory and copies the actual executable
reported by Cargo. `build.json` records the executable hash, source/fixture
inventory, compiler, Cargo, command and build configuration. The run invokes only
the external development evaluator. There is no arbitrary argument passthrough
or holdout option.

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

The original `--external --check` remains an exact snapshot check, including its
historical source metadata. Existing artifacts are not rewritten or relabeled as
verified builds. CI retains that check and additionally exercises the verified
runner and its failure-path tests on all supported OS/compiler combinations.

## Limits

This is a local reproducibility guard, not a signed attestation or a hermetic build.
It trusts the local operating system, Python, Cargo, compiler, linker and dependency
cache. It does not resist a process that maliciously changes files and restores
them between checks, or an actor who rewrites both evidence and hashes. A success
record describes the observed inputs and executable; it does not prove matching
accuracy or establish the original precision/coverage targets.

Direct evaluator commands retain their original behavior and do not acquire this
build/run guarantee. Other synthetic evaluation modes, qualification tools and
performance tools have their own recorded boundaries; this wrapper does not
retroactively verify them. The external experiment still uses positive-only labels,
scores 549 development tables, and leaves all 218 reserved tables unscored.

## Recorded qualification

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

Exact local validation commands (use fresh directory names to repeat):

```text
py -3.11 -m unittest discover -s evaluation -p test_verified.py
py -3.11 -m unittest discover -s evaluation -p test_import_t2d.py
py -3.11 evaluation/import_t2d.py --check
py -3.11 evaluation/verified.py build --build-dir target/external-verified-v1 --toolchain 1.85.0
py -3.11 evaluation/verified.py run --build-dir target/external-verified-v1 --output target/external-verified-results-v1
```

CI repeats the runner tests and fresh build/run on Linux, Windows and macOS with
Rust 1.85.0 and 1.99.0, in addition to the existing Rust and regression checks.
