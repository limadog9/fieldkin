# Stage 3: matching evidence

Stage 3 adds exact numeric samples, distinct-aware sample reliability, verified
semantic constraints and an optional sample-profile matcher. It does **not**
establish a general accuracy improvement: on the original development corpus,
default precision is nearly unchanged while recall falls. The new representation
and constraint APIs solve specific correctness problems; the quarter's quality
targets remain unmet. Fieldkin remains experimental and unpublished.

## Reproducibility and boundaries

Protocol and 12 new synthetic families were committed in `928c07f` before any
Stage 3 corpus scoring. Runtime and evaluator sources were then frozen at
`623859a`. No threshold, label, weight or ambiguity-margin changes were made
after seeing results. Original Stage 1 corpus, protocol and artifacts remain
unchanged. All fixtures are original MIT OR Apache-2.0 data.

The commands below use current latest stable Rust and the planned native strict
snapshot. They do not rewrite the archived Stage 3 record; older compiler
maintenance is retired and `--check` requires its snapshot to exist.

```text
cargo +stable run --locked --release -p fieldkin-eval -- --stage3 --output target/fieldkin-stage3
cargo +stable run --locked --release -p fieldkin-eval -- --stage3 --check --output evaluation/results/rust-native-v1/stage3
```

The [machine-readable report](https://github.com/limadog9/fieldkin/blob/main/evaluation/results/stage3-v1/report.json)
contains full source, manifest, lockfile and corpus hashes, counts, metrics and
configurations. The [generated tables](https://github.com/limadog9/fieldkin/blob/main/evaluation/results/stage3-v1/report.md)
include every operating point. The adjacent `predictions.jsonl` stores 2,408
SHA-256 digests of complete default-threshold reports, covering ranks, scores,
selections, alternatives, explanations and warnings without sample values.
`--check` compares all these artifacts, including source hashes. Historical
behavior is separately checked with the explicitly selected `Legacy` policy.

The original development partition has 32 families, 160 schema pairs and 960
labels: 730 unique matches, 190 no-match and 40 ambiguous decisions. The extension
has 12 families, 12 pairs and 32 labels: 20 unique, 11 no-match and one ambiguous.
They are reported separately. Concepts and labeling rationales never enter the
matcher; verified hints enter only where a fixture explicitly supplies them.
Seven models, two assignment policies and 21 thresholds produce 588 aggregate
rows from 50,568 calls. **No held-out family was scored.**

These are public synthetic development observations, not an independent test or
population accuracy estimate. Five variants within each original family are
correlated. Candidate recall@5 includes ineligible alternatives and is relatively
easy with small target schemas. A score is never a calibrated probability.

## Default behavior on the original development partition

Threshold remains 0.70 and ambiguity margin 0.08. Default weights remain
name/type/sample = 0.65/0.20/0.15. No caller hints exist in this partition.

| Model | Assignment | Correct/proposed | Precision | Unique recall | Unique coverage | No-match proposals |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Stage 3 default | independent | 321/422 | 76.07% | 43.97% | 44.66% | 96/190 |
| Original combined policy | independent | 333/438 | 76.03% | 45.62% | 46.30% | 100/190 |
| Name only | independent | 275/390 | 70.51% | 37.67% | 38.36% | 110/190 |
| Stage 3 default | one-to-one | 313/414 | 75.60% | 42.88% | 43.56% | 96/190 |
| Original combined policy | one-to-one | 325/430 | 75.58% | 44.52% | 45.21% | 100/190 |
| Name only | one-to-one | 270/385 | 70.13% | 36.99% | 37.67% | 110/190 |

Default candidate recall@5 is 100% for both combined policies and 97.53% for
name-only. All three abstain on all 40 ambiguous labels at this threshold.
The new default still makes five wrong-target proposals on unique labels in
addition to the 96 no-match errors. One-to-one matching can suppress legitimate
duplicate source facts: the corpus has 730 single-target source labels but only 715
distinct gold targets assignable across cases. Raw recall retains the 730
denominator; JSON also reports the assignment-adjusted denominator.

Cardinality attenuation removes four incorrect proposals and twelve correct
ones in each policy. The four improvements occur in `product_07` (three variants
of a currency mismatch) and `crm_03` (one namespace mismatch). The twelve lost
matches are three variants each in `product_01`, `product_03`, `product_05` and
`product_06`, involving valid Boolean or constant-currency renames. The small
precision difference is not evidence of a meaningful accuracy gain.

The distinct-aware policy remains the default because repeated constants should
not supply full value evidence. This is an explicit evidence-policy choice with
a measured recall cost. `SampleReliability::Legacy` remains available to callers
who require historical behavior. Neither policy is a semantic classifier.

## Comparisons at matched precision

The frozen grid is 0.50 through 1.00 in increments of 0.025. For each precision
floor, select the grid point with the most correct matches, then greater
precision, then higher threshold. Require at least one proposal. An infeasible
floor is reported as such, never as successful abstention. These selected grid
points are development analyses, **not the default configuration**.

Independent-mode examples on original development:

| Required precision floor | Model | Selected threshold | Correct/proposed | Precision | Recall |
| --- | --- | ---: | ---: | ---: | ---: |
| Original combined default: 76.0274% | Stage 3 | 0.500 | 498/644 | 77.33% | 68.22% |
| Original combined default: 76.0274% | Original combined | 0.500 | 505/651 | 77.57% | 69.18% |
| Original combined default: 76.0274% | Name only | none | — | — | — |
| Name-only default: 70.5128% | Stage 3 | 0.500 | 498/644 | 77.33% | 68.22% |
| Name-only default: 70.5128% | Name only | 0.500 | 405/570 | 71.05% | 55.48% |
| 80% | Stage 3 | 0.575 | 441/551 | 80.04% | 60.41% |
| 80% | Original combined | 0.550 | 465/581 | 80.03% | 63.70% |
| 80% | Name only | none | — | — | — |

At the new default's own precision floor (76.0664%), the same 0.500 points are
selected for Stage 3 and original combined; name-only remains infeasible. In
one-to-one mode at the Stage 3 default precision floor (75.6039%), Stage 3 selects
0.500 with 482/630 correct proposals, 76.51% precision and 66.03% recall; original
combined selects 0.500 with 489/637, 76.77% precision and 66.99% recall. Name-only
is again infeasible. The generated report includes both baseline anchors and all
models for both modes, without interpolation.

The historical policy therefore retains higher recall at these matched floors;
Stage 3 does not dominate it. No tested model/grid point reaches 90% or 95%
precision on original development with a nonempty proposal set. The new default
in one-to-one mode cannot reach even 80% on this grid. Increasing a threshold
does not necessarily improve precision: high-scoring hidden semantic mismatches
remain, and eligibility thresholds interact with ambiguity and assignment.

The 95% precision and 60% unique-coverage targets are not met by current defaults.
Candidate recall meets its 90% target on this development set only. No new
held-out quality claim or application-wide threshold recommendation follows.

## Signal ablations and failure cases

**Samples.** The default overlap score is typed distinct-value Jaccard times
lower non-null coverage times `min((lower distinct count - 1) / 2, 1)`. Constants
score zero, two-value columns at most half, and three distinct values allow full
support. The minimum remains three non-null observations. Null-heavy, empty,
missing and insufficient samples have dedicated regressions and explanations.
Repeated observations cannot increase distinct support. Three unrelated shared
codes can still score fully, and identical names/types can pass without samples.
Mixed-kind separation is covered by regression tests. The extension's constant,
Boolean and shared-code failures illustrate
this: default cardinality attenuation and `Legacy` make the same 23 independent
proposals, including seven false ones. Lower sample scores do not guarantee
abstention by the combined engine.

**Exact numbers.** `SampleValue::Integer(i128)` and `Decimal(ExactDecimal)` avoid
floating-point collapse of adjacent identifiers or amounts. Decimal coefficients
are signed 128-bit integers with scale 0..38, canonical trailing-zero equality
and no arithmetic/conversion API. Tests cover integer neighbors above 2^53,
coefficient bounds, invalid scales, exact decimal equality, signed floating zero,
non-finite floats, checked unsigned overflow and mixed kinds. The two dedicated
exact-value fixture families propose all four correct matches. This demonstrates
representation correctness, not a measured accuracy gain over an old binary:
the original API could not express these sample variants. The extension's
`no_cardinality` model uses new exact representations and hints with historical
sample weighting; it is not an execution of the Stage 1 binary. Integer, decimal,
float and text sample kinds are deliberately not coerced, so equal-looking
values in different representations provide no exact overlap.

**Profiles.** The optional model uses 0.55/0.20/0.15/0.10 for name/type/overlap/
profile. Its `no_profile` ablation retains the same weight scale but supplies
absent profile evidence. At 0.70 in original independent mode, profiles produce
303/396 correct proposals versus 291/384 without profiles: twelve more correct
matches and the same 93 incorrect proposals. This does not compare directly to
default weights. At the prespecified 80% precision floor, profiles give 62.47%
recall at 0.525, while their absence gives 64.52% at 0.500. Extension default
decisions are identical with or without profiles. A narrow benefit exists in
one-to-one mode at the 80% floor: profiles reach 61.23% recall versus 60.27% for
the original combined policy; the new default has no qualifying grid point.
This does not establish consistent improvement across policies. Kind and length distributions
can be identical for unrelated fields; a regression explicitly demonstrates
full profile agreement on disjoint values. There is insufficient evidence to
enable profiles by default. The matcher remains an opt-in experiment.

**Aliases.** Removing the two existing `amt -> amount` and `trans -> transaction`
aliases loses five correct original-development proposals and two extension
proposals at 0.70, without removing default false proposals. No new synonyms were
added. A checked builder and bounded, sorted explanation details make configured
aliases inspectable. Substitution is one-pass, not recursive. An alias can still
misrepresent a domain term and cannot establish units, scope or gross/net meaning.

**Verified hints.** Conflicting supplied units, currency or identifier scope
make a pair ineligible independently of signal weights. Missing hints are absent
evidence, agreeing hints add no score, and reports expose categories without
payloads. On extension independent mode, hints change 16/26 correct proposals
(61.54%) to 16/23 (69.57%): three false currency/unit/scope proposals are removed,
with no loss of correct proposals. The corresponding one-to-one result is 15/22
(68.18%). This uses extra authoritative caller information and is not a fair
claim of better inference from unchanged inputs. Incorrectly supplied hints can
reject valid matches. Seven extension no-match proposals remain where no supplied
contradiction distinguishes unrelated facts. Hidden gross/net meaning is still
unresolved.

## Application-specific tuning recipe

1. Label representative schemas before tuning, including no-match, ambiguity,
   null-heavy and misleading-name cases. Split by independent source/schema
   families, keeping variants together. Reserve an untouched final test set.
2. Choose authoritative semantic vocabularies and numeric representations before
   matching. Do not derive hints from proposed mappings. Measure hint-enabled and
   hint-free operation separately; keep labels/rationales out of matcher inputs.
3. On development data only, compare the current defaults, true name-only baseline
   and individual ablations. Predeclare small grids for `min_score`, matcher
   weights and `ambiguity_margin`; freeze them before measuring. This experiment
   swept thresholds and compared two declared weight configurations; it did not
   optimize an ambiguity-margin grid or search arbitrary weights.
4. Select using a required precision floor plus a minimum useful coverage, while
   tracking no-match and ambiguous false proposals separately. If no grid point
   qualifies, retain review/abstention and improve evidence; do not call an empty
   proposal set accurate. Evaluate one-to-one separately if the application needs it.
5. Freeze configuration and dependencies before final test evaluation. Publish
   counts and failures. Further tuning on test results requires fresh independent
   test families. Changing a heuristic threshold never calibrates a probability.

## Validation

Local validation used Rust 1.85.0 on `x86_64-pc-windows-msvc`. All 111 workspace
tests passed, including four doctests and numeric/profile property tests. One
separate performance-harness test exercised all 50 success/rejection workloads.
One preparation microbenchmark test remains intentionally ignored in ordinary
test runs. Formatting, strict clippy and warning-free documentation passed.

Current equivalents of those historical checks use latest stable Rust:

```text
cargo +stable fmt --all -- --check
cargo +stable clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo +stable test --locked --all-features
cargo +stable doc --locked --no-deps --all-features
cargo +stable run --locked --example baseline
cargo +stable run --locked --example verified_semantics
cargo +stable package --locked --allow-dirty -p fieldkin
cargo +stable fmt --manifest-path performance/Cargo.toml -- --check
cargo +stable clippy --locked --manifest-path performance/Cargo.toml --all-targets --all-features -- -D warnings
cargo +stable test --locked --manifest-path performance/Cargo.toml --all-features
cargo +stable run --release --locked --manifest-path performance/Cargo.toml -- --smoke
cargo +stable run --release --locked --manifest-path performance/Cargo.toml --features allocations -- --smoke
cargo +stable run --locked --release -p fieldkin-eval -- --check --output evaluation/results/rust-native-v1/baseline
```

Documentation used `RUSTDOCFLAGS=-D warnings`; the Stage 3 reproduction/check
commands above also passed. The two performance smoke runs exercise 50 workloads
each with and without allocation instrumentation. They check execution and
budget paths, not statistically useful latency changes. No new speedup is claimed;
the repeated [Stage 2 measurements](stage2-performance.md) remain historical.
Packaging verifies a crate archive without publishing it. API source changes
are documented in the [migration guide](stage3-migration.md).
