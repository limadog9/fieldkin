# Dependency review — 2026-10-03

This is the preserved historical review. Current development supports only latest
stable Rust; the older compiler maintenance policy and Python review runner are
retired. The newer [native review](../qualification/results/rust-native-v1/dependencies.json)
inventories **72 third-party versions**, all five manifest graphs and all three
lockfiles, with no reported vulnerabilities. That is a dated advisory inspection,
not a security guarantee. The development-only `proptest` pin is now 1.11.0;
the historical table below deliberately retains the 1.6.0 decision made then.

The three committed lockfiles contain **42 distinct third-party package versions**
across the library, tests, evaluator and isolated measurement tools. The library
runtime graph remains `strsim` 0.11.1, `unicode-normalization` 0.1.25 and transitive
`tinyvec` 1.13.3. Qualification adds only a path dependency on Fieldkin. No
dependencies were upgraded or added to the library for this review.

The [machine-readable review](https://github.com/limadog9/fieldkin/blob/main/qualification/results/dependency-review-2026-10-03/review.json)
records every version, declared license, graph membership, registry-manifest and
bundled license-file hashes, direct-dependency registry/upstream observations,
lockfile hashes and complete audit output. It is a dated inspection, not an
assurance about future advisories or every reachable code path.

## Advisory checks

Installed `cargo-audit` **0.22.2** with its locked dependencies under the ignored
local tools directory using Rust 1.98.1. At that time Fieldkin built on Rust 1.85;
the audit tool is separate and never becomes a project dependency. Scans used
the [RustSec database](https://github.com/rustsec/advisory-db/tree/ef6173cbc5c50ec8166f9a5b28f07834144373ee)
at commit `ef6173cbc5c50ec8166f9a5b28f07834144373ee`, dated
2026-10-03T10:14:03+02:00, containing 1,290 advisories.

All scans included every lockfile entry, including development and platform-only
dependencies, with **no architecture/OS filtering, no ignored advisories, yanked
checks enabled and `--deny warnings`**:

| Lockfile | Entries reported by cargo-audit | Vulnerabilities | Warnings | Exit code |
| --- | ---: | ---: | ---: | ---: |
| Root, including tests and evaluator | 43 | 0 | 0 | 0 |
| Performance harness, including optional allocator instrumentation | 6 | 0 | 0 | 0 |
| Qualification harness | 5 | 0 | 0 | 0 |

These counts include local packages and duplicate packages across graphs;
42 is the deduplicated third-party version count. `cargo-audit` checks published
advisories against locked versions; it is not a source-code audit or proof that
dependencies are secure. See the [upstream tool documentation](https://github.com/rustsec/rustsec/tree/main/cargo-audit).

To reproduce the dated database snapshot, install the same tool, check out the
database commit above, then run from the repository root:

```text
cargo audit --json --db PATH_TO_DATABASE --no-fetch --file Cargo.lock --deny warnings
cargo audit --json --db PATH_TO_DATABASE --no-fetch --file performance/Cargo.lock --deny warnings
cargo audit --json --db PATH_TO_DATABASE --no-fetch --file qualification/Cargo.lock --deny warnings
```

For a new review, fetch the current database first and save a fresh artifact;
do not silently replace either dated result. The retired
`qualification/dependencies.py` produced the historical record. The current
native runner captures all three audits and five manifest inventories:

```text
cargo +stable run --locked -p fieldkin-tools -- dependency-review --output target/dependency-review.json --audit C:/path/to/cargo-audit.exe --database C:/path/to/advisory-db
```

Use actual local paths to the separately installed audit executable and database
checkout. The native runner also fetches current public crates.io and GitHub
metadata without authentication, so these observations change over time. No
account integrations or automated dependency updater were installed.

## Licensing and attribution

All 42 versions declare a permissive license choice. The inventory retains the
exact expressions instead of replacing dependencies' licenses with Fieldkin's
MIT OR Apache-2.0 license. Existing runtime copyright notices are in
[THIRD_PARTY.md](../THIRD_PARTY.md); no dependency source is vendored in this
repository.

Most versions use MIT OR Apache-2.0. Additional expressions include MIT-only
(`strsim`, `generic-array`, `stats_alloc`, `zmij`), Unlicense OR MIT (`memchr`),
Zlib OR Apache-2.0 OR MIT (`tinyvec`), BSD-2-Clause OR Apache-2.0 OR MIT
(`zerocopy` and its derive crate), and the alternative license choices declared
by `wasi`. The legacy `MIT/Apache-2.0` expression in `version_check` accompanies
both upstream license texts.

`unicode-ident` 1.0.26 declares **(MIT OR Apache-2.0) AND Unicode-3.0**. Its Unicode
notice is an additional obligation, not an alternative MIT choice; the package
ships `LICENSE-UNICODE`, whose hash is recorded. Preserve applicable dependency
notices when redistributing bundled sources or binaries. The primary
[Unicode license](https://www.unicode.org/license.txt) explains its notice terms.

One packaging limitation is recorded explicitly: `stats_alloc` 0.1.10 declares
MIT in its [published manifest](https://docs.rs/crate/stats_alloc/0.1.10/source/Cargo.toml)
but its package and inspected upstream root contain no standalone license file.
It is used only by unpublished allocation-measurement executables; those binaries
and dependency sources are not distributed by this repository. Obtain/preserve
the appropriate upstream notice before distributing such an instrumented binary.
The Fieldkin library package excludes the performance harness.

## Maintenance observations

These are registry and repository observations on the review date. A recent
commit is not a support commitment, and an old release is not itself a security
finding. None of the eight inspected upstream repositories was archived or
disabled; the all-dependency RustSec scans produced no unmaintained advisories.

| Dependency | Locked / registry latest | Upstream last push observed | Decision |
| --- | --- | --- | --- |
| [strsim](https://github.com/rapidfuzz/strsim-rs) | 0.11.1 / 0.11.1 | 2025-11-27 | Retain the small runtime dependency; monitor releases/advisories |
| [unicode-normalization](https://github.com/unicode-rs/unicode-normalization) | 0.1.25 / 0.1.25 | 2026-09-18 | Retain; Unicode data and transitive MSRV changes need review |
| [tinyvec](https://github.com/Lokathor/tinyvec) | 1.13.3 / 1.13.3 | 2026-09-16 | Runtime transitive dependency; retain locked version |
| [proptest](https://github.com/proptest-rs/proptest) | 1.6.0 / 1.11.0 | 2026-10-03 | Retain development pin compatible with the Rust 1.85 policy; newer release has a higher MSRV |
| [serde](https://github.com/serde-rs/serde) | 1.0.229 / 1.0.229 | 2026-09-22 | Evaluator-only; retain |
| [serde_json](https://github.com/serde-rs/json) | 1.0.151 / 1.0.151 | 2026-08-08 | Evaluator-only; retain |
| [sha2](https://github.com/RustCrypto/hashes) | 0.10.9 / 0.11.0 | 2026-09-22 | Evaluator provenance hashing only; review the new minor series separately |
| [stats_alloc](https://github.com/neoeinstein/stats_alloc) | 0.1.10 / 0.1.10 | 2024-01-18 | Older, optional development instrumentation; retain isolation and the notice caveat above |

All eight locked direct/runtime-transitive versions were not yanked in their
registry records. The maintained status of every transitive repository has not
been individually established; all their declared licenses, available notice
files and known RustSec advisory results were checked. Recheck the complete graph
before publishing a crate release and whenever a dependency or feature changes.
