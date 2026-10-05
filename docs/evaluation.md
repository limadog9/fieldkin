# Stage 1 evaluation: initial baseline

Completed on 2026-10-03. The evaluation stage is complete; the quarter's matching
quality goals are **not** met. The matching implementation and its runtime
dependency versions were not changed for this work.

The corpus, partition, metric definitions and model configuration were committed
at [`9bab62e`](https://github.com/limadog9/fieldkin/commit/9bab62e) before the first
corpus scoring run. The reference library revision is
[`7c4da25`](https://github.com/limadog9/fieldkin/commit/7c4da25).

## What was delivered

- 40 separately specified synthetic fixture families across product/import,
  finance, CRM/contact and telemetry data; five variants each produce 200 schema
  pairs and 1,200 labeled source decisions.
- A frozen family split: development has 32 families, 160 pairs and 960 decisions;
  holdout has 8 families, 40 pairs and 240 decisions. Related variants never cross
  partitions. Every source has a mapping, no-match or ambiguity label and rationale.
- A development-only Rust evaluation package that compares six configurations
  under both independent and one-to-one assignment: 2,400 total schema-matching
  runs for the full baseline.
- Deterministic JSON results, readable slice tables, development predictions,
  exact configuration, seeds, source/corpus SHA-256 hashes, dependency lockfile
  and compiler environment. Fixtures are original synthetic work under the
  project's MIT OR Apache-2.0 license; no external datasets are bundled.
- Fourteen additional tests for labels, partitions, metrics, undefined ratios,
  ablation behavior and holdout opt-in, plus a development snapshot check in CI.

The [evaluation guide](https://github.com/limadog9/fieldkin/blob/main/evaluation/README.md) defines every metric and model.
[Development results](https://github.com/limadog9/fieldkin/blob/main/evaluation/results/baseline-v1/development.md) and
[holdout results](https://github.com/limadog9/fieldkin/blob/main/evaluation/results/baseline-v1/holdout.md) include domain,
variant and overlapping scenario slices. The adjacent JSON files carry raw counts.

## Main comparison

All models use the frozen 0.70 threshold and 0.08 ambiguity margin. Name-only
disables type/sample evidence and the type veto; it uses the same name aliases.
No threshold, label, split or signal was changed after these results were observed.

| Partition | Engine | Assignment | Correct / proposals | Precision | Unique recall | Unique coverage | Candidate recall@5 |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |
| Development | Combined | Independent | 333 / 438 | 76.03% | 45.62% | 46.30% | 100.00% |
| Development | Name-only | Independent | 275 / 390 | 70.51% | 37.67% | 38.36% | 97.53% |
| Development | Combined | One-to-one | 325 / 430 | 75.58% | 44.52% | 45.21% | 100.00% |
| Development | Name-only | One-to-one | 270 / 385 | 70.13% | 36.99% | 37.67% | 97.53% |
| Holdout | Combined | Independent | 51 / 136 | 37.50% | 36.43% | 40.00% | 96.88% |
| Holdout | Name-only | Independent | 45 / 135 | 33.33% | 32.14% | 35.71% | 90.62% |
| Holdout | Combined | One-to-one | 51 / 136 | 37.50% | 36.43% | 40.00% | 96.88% |
| Holdout | Name-only | One-to-one | 45 / 135 | 33.33% | 32.14% | 35.71% | 90.62% |

Coverage is not correctness: it includes wrong-target selections on uniquely
labeled fields. Precision counts every proposal on a no-match or ambiguous label
as an error. Candidate recall includes weak/ineligible candidates among the first
five ranks. With 5-9 targets per fixture, top-five recall is comparatively easy;
it does not mean the selected mapping is reliable.

The combined model made 100 proposals on 190 development no-match labels, and
80 on 90 holdout no-match labels. It abstained on all 40 development and all 10
holdout ambiguous labels, but overall expected-abstention accuracy was only
56.52% and 20.00%, respectively. This is a concrete limitation of the current
selection policy, not a result to hide behind candidate retrieval.

The development partition contains 730 unique, 190 no-match and 40 ambiguous
decisions; holdout contains 140 unique, 90 no-match and 10 ambiguous decisions.
Partitions are balanced by domain family count, **not by difficulty or label
mix**. Holdout has substantially more no-match labels proportionally. Comparing
its precision directly with development does not isolate a generalization effect.
These are deliberately challenging synthetic scenarios, not a random sample of
production schemas or a population accuracy estimate.

## What the measurements support

The combined model recovers more correct proposals than name-only on these fixed
fixtures, but neither is close to the roadmap's 95% precision and 60% unique
coverage targets. The top-five retrieval target is met here, with the limited
target-set size caveat above. No statistical superiority or general accuracy
claim follows from these small, correlated synthetic families.

The development examples expose misleading same-name fields, weak evidence for
opaque renames, and semantic differences absent from names and values. Some
distinctions are intentionally not recoverable from the supplied evidence.
Additional lexical cleverness cannot guarantee those hidden semantics; callers
may need explicit domain evidence or manual review. Do not silently relabel those
cases as correct matches to improve a benchmark.

One-to-one assignment does not repair incorrect evidence. It also lowers raw
recall when redundant source facts share one target. The reports retain ordinary
recall and a separate feasible-cardinality ceiling so that constraint cost is
visible. Local ambiguity abstention works on this corpus, but these cases do not
establish detection of all forms of global uncertainty.

The four ablations preserve the default score scale; they are not separately
optimized matchers. In particular, removing the name signal leaves a maximum
score of 0.35, below the fixed threshold. Its lack of proposals is expected from
configuration and cannot establish that type or sample evidence is useless.
All ablation counts are published rather than selecting only favorable comparisons.

## Reproduce and verify

The measurements above are the archived October 3 baseline. Its Rust 1.85.0
compiler and source hashes remain part of that record; current development
supports only latest stable Rust. The original held-out families are now
regression evidence, not a fresh test set.

Current development checks use the native evaluator and the planned strict
snapshot at `evaluation/results/readiness-pass-v1/current/baseline`:

```text
cargo +stable run --locked --release -p fieldkin-eval -- --check --output evaluation/results/readiness-pass-v1/current/baseline
cargo +stable test --locked --all-features
cargo +stable clippy --locked --workspace --all-targets --all-features -- -D warnings
```

The evaluator command checks development only and requires the snapshot to exist.
For a new record, use a fresh `target/` directory without `--check`. The historical
`--check-behavior` mode allowed engine/evaluator source hashes to differ while
comparing behavior; it does not make the old provenance a current strict snapshot.
Exact reproduction of the archived record requires its frozen checkout. The
original result files remain unchanged.
Baseline generation used Rust 1.85.0 on Windows x86_64. Source hashes normalize
line endings so CI can verify the same deterministic artifacts on Linux. Compiler
metadata is recorded separately and is not required to be byte-identical across
platforms. Timing benchmarks from the initial release remain a separate record;
this evaluation introduces no new performance claim.

Keep holdout out of routine tuning. Its initial report is aggregate only; no
individual holdout prediction artifact is produced. This is a public corpus, so
the opt-in flag does not create a concealed test set. If a held-out family informs
a scoring change, move it to development and replace it in a new corpus version.

Stage 2 should use the development predictions as an exact behavior reference
while reducing repeated preparation work. Stage 3 can then evaluate scoring
changes on development data and report their tradeoffs. No matching improvement,
95% accuracy claim, external Valentine comparison or real-world validation is
being declared complete by finishing Stage 1.
