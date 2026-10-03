# Generated-case qualification

This isolated, unpublished Rust 1.85 package calls only Fieldkin's public API.
It adds no dependencies beyond the library itself. Inputs are original synthetic
data under MIT OR Apache-2.0; no evaluation fixtures or held-out examples are read.

Build after freezing the candidate source revision, then run:

```text
python qualification/run.py build --output qualification/results/candidate-v1
python qualification/run.py run --output qualification/results/candidate-v1 --cases 1000000 --seed 20261003
```

The five round-robin categories each execute 200,000 generated cases in the full
campaign. A case counts once after its checks succeed; repeated calls, assertions,
candidate pairs and assignment-oracle search nodes do not increase the count.

| Category | Generation and checks |
| --- | --- |
| Normalization | ASCII case/separator/digit oracle and bounded Unicode fragments; repeatability, nonempty tokens and bounded expansion |
| Malformed schemas | Duplicate/empty/oversize identities and names, non-finite floats, oversized observations/text, invalid semantic hints; ordinary errors required |
| Configuration and limits | Invalid thresholds/margins/top-k, each resource budget, and an exact accepted sample-byte boundary |
| Default report invariants | Small random schemas, all sample representations, missing/null-heavy samples, duplicated names and hints; valid scores/ranks/IDs, selected/unmatched complements, one-to-one uniqueness, exact report invariance under field reordering |
| Assignment oracle | Random 2–4 by 2–4 score matrices through a public custom matcher, thresholds and display top-k; compare total selected score with an independent exhaustive integer partial-assignment oracle; randomly enable diagnostic solve/work budgets and compare complete best-distinct-alternative analysis with full enumeration; validate witness assignments/objectives/gaps, exact budget use/completeness, reordering and structural invariants |

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

The runner records compiler, platform, source/manifest/lockfile and binary hashes,
actual case totals, category totals, seed and elapsed time. It rejects changed
sources/binaries and existing result files; campaign execution has a ten-minute
wall-time limit. Timings describe this run and are not performance benchmarks.
On failure it reports the case index/category and seed; replay with the same seed
and at least that many cases. Sample values are not printed.

Small CI smoke run and tool checks:

```text
cargo +1.85.0 fmt --manifest-path qualification/Cargo.toml -- --check
cargo +1.85.0 clippy --locked --manifest-path qualification/Cargo.toml --all-targets -- -D warnings
cargo +1.85.0 test --locked --manifest-path qualification/Cargo.toml
cargo +1.85.0 run --locked --release --manifest-path qualification/Cargo.toml -- --cases 5000 --seed 20261003
```
