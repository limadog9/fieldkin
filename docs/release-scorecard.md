# Experimental candidate qualification

The remaining roadmap implementation and qualification work is complete in this
candidate. **The release quality bar is not met.** Held-out default precision is
37.5% and unique-field coverage is 40%, below the frozen 95%/60% targets.
Fieldkin remains an experimental library that produces proposals for review.
No crates.io publication, release tag, publishing credential or new maintainer
was created. @limadog9 retains the final publication/version decision.

## Frozen candidate and evidence

Runtime, evaluators, generated-case campaign and benchmark code were committed
at `5d5c3746032ac743a9130d46b2794ed4e4efa074` before final scoring. Protocol-only
commit `eec7622` then recorded that exact feature-freeze revision. Subsequent
qualification does not tune weights, thresholds, aliases or labels against the
holdout. Artifact hashes identify the source files, manifests, lockfiles and
protocol actually used. No library runtime dependencies were added.

- [Release protocol](https://github.com/limadog9/fieldkin/blob/main/evaluation/release-protocol.json)
- [Development report](https://github.com/limadog9/fieldkin/blob/main/evaluation/results/rc-v1/release/development.md)
- [Held-out report and scenario slices](https://github.com/limadog9/fieldkin/blob/main/evaluation/results/rc-v1/release/holdout.md)
- [Full held-out counts, configuration and predictions](https://github.com/limadog9/fieldkin/blob/main/evaluation/results/rc-v1/release/holdout.json)
- [Million-case campaign](https://github.com/limadog9/fieldkin/blob/main/qualification/results/rc-v1/run.json)
- [Dependency review](dependency-review.md)

Stage 4 adds typed explanations of local ambiguity, competition, displacement
and unmatched sources, plus optional bounded alternative assignments. Stage 5
adds typed errors and two compiling consumers with application-owned review
state. Stage 6 freezes features, evaluates development and held-out families,
exercises generated invariants, audits dependencies and verifies supported builds
and packaging. Completing these activities is not a claim that their aspirational
accuracy targets were achieved.

## Held-out quality

The original eight held-out families remain together across their five variants:
40 schema pairs and 240 labels (140 single-target, 90 no-match, 10 ambiguous).
These public synthetic families were reserved from Stage 3 tuning; their initial
baseline aggregate had been published in Stage 1. They are not a secret or
independent third-party test. Counts are descriptive; variants are correlated.

Both assignment policies produce these same default-threshold results:

| Model | Correct/proposed | Precision | Unique recall | Unique coverage | Candidate recall@5 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Frozen current combined | 51/136 | 37.50% | 36.43% | 40.00% | 96.88% |
| True name-only | 45/135 | 33.33% | 32.14% | 35.71% | 90.62% |

The combined engine proposes on 80 of 90 no-match labels and selects five wrong
targets on uniquely labeled sources. It abstains on all ten ambiguous labels.
The original combined baseline had the same aggregate held-out results; the
new sample policy has not solved the missing semantic evidence in these cases.
The original development results also remain exactly those measured in Stage 3:
321/422 correct independent proposals, 76.07% precision and 43.97% recall.

Candidate retrieval passes its target, but includes ineligible candidates in
small target schemas; retrieving a field is much easier than proposing it
correctly. The opaque-identifier slice retrieves 30/35 relevant candidates
(85.71%), including only 20/25 unique gold targets (80%), below the overall
retrieval target on that slice. Opaque/disjoint-sample cases receive no proposals. Financial fixtures
yield 0/40 correct proposals, product/import 5/35, CRM 25/40 and telemetry 21/21.
These small, uneven scenario results are published rather than averaged away.
Misleading qualifiers, reused codes and unrelated same-name facts remain serious
failure cases. No hidden gross/net, currency or identifier-scope inference is
claimed. Supplied authoritative hints are separate inputs, not part of this
hint-free quality score.

The prespecified threshold grid is .50 through 1.00 in .025 steps. At the
combined default precision floor of 37.5%, name-only has **no feasible nonempty
grid point**. Combined's descriptive frontier selects .50 (46.54% precision,
52.86% recall and 56.43% coverage). Neither model has a feasible 95% point under
either assignment policy. These held-out frontiers are comparisons, not tuning
or recommended settings. The default remains .70 and no released configuration
is chosen from held-out results.

## Reliability and reproducibility

The frozen generated campaign passed **1,000,000 cases**, seed `20261003`, with
200,000 each for normalization, malformed schemas, configuration/limits, default
report invariants and small assignment oracles. It took 17.46 seconds in the Rust
process on the recorded Windows x86-64 machine. Compilation of other tools
overlapped this campaign, so its duration is not a latency benchmark.

Assignment cases use independently enumerated integer objectives and alternative
mappings, random diagnostic budgets and reordered inputs. Repeated API calls and
oracle search nodes count within a case, not as extra cases. The campaign is
deterministic generated property testing, not coverage-guided fuzzing, formal
verification or a million independent real-world schemas. It found no failures;
that is not proof of absence of bugs. Tooling records and checks source/binary
hashes before and after execution.

Local Rust 1.85.0 and 1.99.0 validation passes 149 workspace tests including five
doctests. Two qualification tests and one performance-harness test bring the
total to **152**; one preparation microbenchmark stays intentionally ignored.
Strict clippy, formatting and warning-free docs pass. Both realistic consumer
examples run, and historical decisions plus current full development reports are
checked separately. CI requires every Linux/Windows/macOS and minimum/current
compiler matrix job through the existing required `test` gate. Holdout scoring
is never part of CI. There are no optional library feature combinations; the
performance tool independently tests its allocator feature.

The dependency review inventories 42 third-party versions and records zero
RustSec vulnerabilities or warnings across all three lockfiles on the review
date. This is a dated advisory/license inspection, not a security proof. The
optional unpublished allocator tool's notice-file limitation is documented.

## Cost measurements

The existing 50-workload protocol was rerun against Stage 1 `00b12b5` and the
frozen candidate `eec7622` using Rust 1.85.0 on an AMD Ryzen 5 5625U (6 cores,
12 logical processors), Windows x86-64. There are five independent processes per
revision and measurement mode, alternating revision order. Normal timing and
allocation instrumentation are separate. No builds, tests or qualification
campaign ran concurrently with these measurements. No CPU affinity or fixed
frequency was imposed. Values below are medians of process means; brackets show
the minimum and maximum of those five means.

| Combined 128-field sampled workload | Stage 1 ms | Candidate ms | Speedup |
| --- | ---: | ---: | ---: |
| Independent | 115.38 [104.55–137.06] | 40.24 [36.05–41.38] | 2.87x |
| One-to-one | 112.93 [104.63–121.68] | 53.42 [51.64–55.11] | 2.11x |

The 2x stretch target is met on this machine under both policies, with global
alternative diagnostics disabled. All 24 core workload medians improve against
Stage 1. This compares complete current APIs, including new report structures and
the changed sample policy, not an isolated optimization. Stage 2's older session
is not used to claim an additional improvement.

Costs and regressions remain visible. Independent sampled allocated bytes fall
from 65,180,112 to 21,355,600; these are allocator request totals, not peak live
memory. Report-budget rejection rises from 6.33 to 57.69 microseconds because of
eager preparation. Empty-target independent calls rise from 31.96 to 39.76
microseconds; added structured per-field reasons also increase allocations from
281 to 409 in that fixture. Several tiny or rejection workloads are slightly
slower. The [complete timing/allocation table](https://github.com/limadog9/fieldkin/blob/main/performance/results/rc-v1/summary.md)
and adjacent raw runs include all 50 workloads and spreads. Its generated title
retains the Stage 2 protocol name; `build.json` records the actual revisions above.

Optional diagnostic costs were measured separately in five release processes,
each with one warmup and three calls per workload. On the dense equal-score
128×128 graph, disabled analysis takes 18.65 ms [18.35–24.09]; enabling analysis
with the default work cap permits two probes and takes 36.46 ms [31.97–42.84],
reporting exhaustion. Complete 128-probe analysis costs 956.51 ms
[941.80–1,097.50], charging 536,870,912 algorithmic work units. This makes the
reason for opt-in budgets concrete. The timing includes `match_schemas` and
report construction, excluding setup, assertions and returned-report destruction.
These intentionally contested fixtures illustrate expensive enabled work, not
a universal wall-clock bound. See [all 20 workloads](https://github.com/limadog9/fieldkin/blob/main/performance/results/diagnostics-v1/summary.json)
and the [algorithmic bounds](stage4-diagnostics.md).

## Reproduce the checks

Normal development and CI use only the development partition:

```text
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo +1.85.0 test --locked --all-features
cargo +1.85.0 doc --locked --no-deps --all-features
cargo +1.85.0 run --locked --example import_review
cargo +1.85.0 run --locked --example catalog_review
cargo +1.85.0 package --locked -p fieldkin
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --check-behavior --output evaluation/results/baseline-v1
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --stage3 --check --output evaluation/results/rc-v1/stage3-development
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --release --check --output evaluation/results/rc-v1/release
```

Set `RUSTDOCFLAGS=-D warnings` for docs. Repeat compiler checks with `+1.99.0`.
Run `fmt`, `clippy` and `test` with the standalone `performance/Cargo.toml`
(`--all-features`) and `qualification/Cargo.toml` manifests as configured in CI.
From a clean frozen checkout, these commands reproduce qualification artifacts
in fresh directories:

```text
python qualification/run.py build --output target/qualification-reproduction
python qualification/run.py run --output target/qualification-reproduction --cases 1000000 --seed 20261003
python performance/run.py build --baseline .local/stage2-baseline --candidate . --output target/cost-reproduction
python performance/run.py run --output target/cost-reproduction
python performance/diagnostics.py build --output target/diagnostics-reproduction
python performance/diagnostics.py run --output target/diagnostics-reproduction
```

The before/after baseline is a separate detached checkout of `00b12b5`, created
with `git worktree add --detach .local/stage2-baseline 00b12b5` if absent. Builders
record exact sources and binaries; measurements refuse stale artifacts and do
not overwrite prior runs. Timings will vary by machine. The final held-out run
was explicitly authorized after freezing and used:

```text
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --release --output evaluation/results/rc-v1/release --acknowledge-holdout
```

Do not use those now-examined families for future tuning and still call them an
untouched final test. Preserve them as regression evidence and reserve new
independently labeled families before another claim of generalization.

## Release decision and corrective work

The candidate is ready for maintainer review as an experimental artifact, with
the failed quality targets visible. It is not qualified against the roadmap's
95% precision/60% coverage release goal. Publication remains unperformed.
No code-owner rule, CI requirement or security policy was relaxed for this work.
The owner's previously requested administrator exception remains in place.

The next corrective cycle should collect independently labeled, appropriately
licensed consumer schemas and freeze new family-level holdouts before further
scoring changes. Evaluate explicit evidence sufficiency for generic names and
hidden semantic distinctions on development data, with per-domain precision
and no-match penalties. Preserve current behavior as a regression comparator and
retain useful coverage requirements. Do not infer semantic labels from proposals
or raise confidence based on a deterministic assignment. A score threshold alone
did not close the observed quality gap.

Serialization, externally imposed confirmed/forbidden pairs, public prepared
schemas, larger-schema pruning, richer numeric arithmetic, adapters, downstream
adoption targets and an external Valentine performance/accuracy comparison are
explicitly deferred. Nested transforms, automatic rewriting, embeddings, Python
bindings, Polars and hosted services remain outside scope. See the
[usage](usage.md), [migration](migration.md) and [diagnostics](stage4-diagnostics.md)
guides for the delivered API.
