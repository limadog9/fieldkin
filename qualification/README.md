# Generated-case qualification

This isolated, unpublished package uses the latest stable Rust toolchain and calls only Fieldkin's public API.
It adds no dependencies beyond the library itself. Inputs are original synthetic
data under MIT OR Apache-2.0; no evaluation fixtures or held-out examples are read.

Build after freezing the candidate source revision, then run:

```text
cargo +stable run --locked --offline -p fieldkin-tools -- qualify-build --output target/qualification/candidate-v3
cargo +stable run --locked --offline -p fieldkin-tools -- qualify-run --output target/qualification/candidate-v3 --cases 1000000 --seed 20261003
```

The current v3 campaign distributes cases across seven round-robin categories.
The default one-million-case run gives each category 142,857 or 142,858 cases.
The earlier v2 campaign used six categories; its dated records remain archived.
A case counts once after its checks succeed; repeated calls, assertions,
candidate pairs and assignment-oracle search nodes do not increase the count.

| Category | Generation and checks |
| --- | --- |
| Normalization | ASCII case/separator/digit oracle and bounded Unicode fragments; repeatability, nonempty tokens and bounded expansion |
| Malformed schemas | Duplicate/empty/oversize identities and names, non-finite floats, oversized observations/text, invalid semantic hints; ordinary errors required |
| Configuration and limits | Invalid thresholds/margins/top-k/corroboration floors, each resource budget, and an exact accepted sample-byte boundary |
| Default report invariants | Small random schemas, all sample representations, missing/null-heavy samples, duplicated names and hints; randomly disabled/default/strict-name corroboration; valid scores/ranks/IDs, selected/unmatched complements, one-to-one uniqueness, exact report invariance under field reordering; gated candidates require positive built-in support and preserve ungated scores/evidence by target |
| Assignment oracle | Random 2–4 by 2–4 score matrices through a public custom matcher, thresholds and display top-k; compare total selected score with an independent exhaustive integer partial-assignment oracle; randomly enable diagnostic solve/work budgets and compare complete best-distinct-alternative analysis with full enumeration; validate witness assignments/objectives/gaps, exact budget use/completeness, reordering and structural invariants |

The sixth category tests caller-reviewed assignment on random 1–4 by 1–4 score
matrices with confirmations, forbidden pairs, explicit unmatched sources, both
assignment modes and display truncation. An independent exhaustive oracle checks
the remaining automatic objective in one-to-one mode; row maxima check independent
mode with ambiguity abstention disabled. Confirmations retain their original scores,
reserved targets cannot be reused, diagnostic witnesses respect review decisions,
unmatched lists are complements, and input/directive reordering preserves reports.
No gold evaluation labels are supplied to this generated-case campaign.

The seventh category exercises optional contextual evidence using generated
identifier samples under both assignments. It preserves distinctive supported
matches, explicitly abstains when identifier samples are missing or reused by a
competing field, remains invariant under field reordering, and rejects configured
gross/net and kg/lb contradictions despite sample overlap. It supplies no
evaluation fixtures or gold metadata to the matcher.

The generator uses wrapping 64-bit LCG constants recorded in source, seed
20,261,003 by default, and bounded small inputs. This is a deterministic generated
property campaign, not coverage-guided fuzzing, formal verification, or one
million independently sampled production schemas. Generated cases may repeat.
The assignment oracle disables local ambiguity abstention to test the global
objective; ordinary random-schema cases retain the public abstention default.
Diagnostic budgets use 0–4 solves and either zero or 1,024 work units, with
objective margins 0, 0.05 or 0.2. Exhaustive enumeration is independent of the
engine's selected-edge exclusion procedure. Incomplete diagnostic output is
checked for valid witnesses and honest completeness, without pretending it
establishes the absence of further alternatives.

The corrective-cycle extension randomly enables corroboration in built-in-engine
cases with name/sample floors `0.0/0.5` or `0.8/0.5`. Even at a zero name floor,
eligible and selected candidates must have positive active name evidence. The
comparison against an ungated call checks pair scores, signal evidence, retained
targets and eligibility. It does not require selected mappings to be a subset:
removing a weak competing candidate can resolve a tie and create a proposal.
The harness can inspect signal names here because it explicitly constructs the
built-in engine; applications must not authenticate custom matchers by name.
These additions change generated inputs at the same seed, so use new result
directories. Preserve earlier campaigns with their recorded source hashes.

The native runner defaults to 1,000,000 cases and seed 20,261,003. It records the
actual stable compiler and Cargo versions, platform, source/manifest/lockfile and
binary hashes, case totals, category totals, seed and elapsed time. Each build
uses a fresh Cargo target directory and copies the executable into its output
directory. Before and after build/run, it checks sources, compiler, Cargo
configuration and hashed build environment; it also checks the executable and
build record. Existing output directories or run records are rejected. Campaign
execution has a ten-minute wall-time limit. Records carry a checksum and explicit
trust boundary: they are local reproducibility records, not signatures or
hermetic builds, and cannot detect a change restored between checks. Source
hashes normalize CRLF to LF; executable hashes use raw bytes. Native v3 records
use Unix timestamps. Timings describe this run and are not performance benchmarks.
On failure it reports the case index/category and seed; replay with the same seed
and at least that many cases. Sample values are not printed.

Small CI smoke run and tool checks:

```text
cargo +stable fmt --manifest-path qualification/Cargo.toml -- --check
cargo +stable clippy --locked --manifest-path qualification/Cargo.toml --all-targets -- -D warnings
cargo +stable test --locked --manifest-path qualification/Cargo.toml
cargo +stable run --locked --release --manifest-path qualification/Cargo.toml -- --cases 5000 --seed 20261003
```

Capture a fresh dependency inventory and a pinned local RustSec review with the
native tooling. HTTPS metadata requests use `curl` (`curl.exe` on Windows), with
certificate verification and bounded request time; no Python runtime is used.

```text
cargo +stable run --locked --offline -p fieldkin-tools -- dependency-review --output target/dependency-review.json --audit C:/path/to/cargo-audit.exe --database C:/path/to/advisory-db
```

The review covers the root, evaluation, tooling, performance and qualification
manifests and their deduplicated lockfiles. It records all locked third-party
versions, declared licenses, license notice hashes, source manifest hashes,
upstream release metadata, audit executable hash and clean advisory database
commit. It preserves failed audit reports and exits unsuccessfully if any audit
fails. Historical qualification and dependency records retain their original
toolchain and source hashes; native tooling creates new records in fresh paths.
