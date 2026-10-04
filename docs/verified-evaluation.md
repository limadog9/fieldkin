# Verified external evaluation runs

Use this workflow when recording new external development evidence. The original
evaluator calculates source hashes from the checkout at execution time; an old
executable can therefore report newer on-disk source hashes. The verified runner
binds a fresh build to its input inventory and executable, and refuses a run when
either changes. It adds no dependency or behavior to the Rust library.

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
artifacts. Only then is `run.json` written with their hashes. If a build or run
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
