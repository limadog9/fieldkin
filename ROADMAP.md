# Three-month roadmap

**Planning window: October 3, 2026 through January 3, 2027.**
**Owner and final decision-maker: [@limadog9](https://github.com/limadog9).**

Our goal is to make Fieldkin a dependable choice for developers who need
explainable schema matching inside a Rust application: better evidence, fewer
unsupported proposals, predictable costs, and an API that is easy to adopt.
Success includes knowing when to leave a field unmatched.

This is the intended development sequence, not a claim that these capabilities
already exist or a guarantee of release dates. **Implementation and qualification
work for stages 1 through 6 is complete as of October 3, 2026.** The final
[candidate scorecard](docs/release-scorecard.md) records missed quality targets:
37.5% held-out precision and 40% unique coverage. The library remains experimental
and unpublished; publication/version remain the maintainer's decision. See the
[evaluation evidence and limitations](docs/evaluation.md) and
[performance measurements](docs/stage2-performance.md), and
[Stage 3 evidence evaluation](docs/stage3-evaluation.md). Stages may finish
earlier than their planning dates.

The subsequent [corrective evidence cycle](docs/corroboration-evaluation.md) adds
an opt-in sample-support gate, 24 fresh synthetic families, source-verified final
evaluation and extended qualification. On the new holdout, independent precision
improves from 26.67% to 38.46%, while unique coverage remains only 16.30%.
The weighted default is preserved, and the original 95%/60% targets remain unmet.
Independent consumer data and richer verified semantic evidence are still needed;
these results do not establish production readiness or authorize publication.

## Phases following the corrective cycle

The subsequent build cycles deliver bounded follow-up phases while retaining the
original release targets and experimental status:

| Phase | Delivered scope | Evidence and remaining gap |
| --- | --- | --- |
| 7. Bring in independent annotations | Pinned, explicitly Apache-licensed original T2D correspondences; offline importer; 549 development tables; 218 tables reserved by whole class | [External evaluation](docs/external-evaluation.md). This induced annotation-only task supplies real original headers and positive labels, but lacks independent target schemas, sample values and negative/unmatched labels; it cannot certify precision. |
| 8. Close the application review loop | Per-call confirmations, forbidden pairs and explicit unmatched sources; partial reassignment respecting reviewed mappings; typed errors, unchanged evidence scores and conditional global diagnostics | [Review API](docs/review-constraints.md) and [importer example](examples/reviewed_import.rs). Human input remains distinct from automatic proposals and is never counted as autonomous accuracy. |
| 9. Bind evaluation records to their executable | Fresh isolated evaluator builds; recorded executable and input hashes; before/after drift checks; development-only runs with success records | [Verified evaluation](docs/verified-evaluation.md). This detects stale builds and changed inputs in the recorded workflow; it is not a signed attestation or validation of matching accuracy. |
| 10. Reduce partial-assignment work | Skip unused target columns and empty solves; retain source/dummy order, reports and diagnostic budget semantics; before/after measurements | [Assignment costs](docs/assignment-performance.md). Pair scoring remains complete and all-pairs; this reduces solver work without adding evidence or changing matching policy. |

Further accuracy work must use the reserved external classes only after a new
policy freeze, and report their positive-only limits. Production-oriented
validation still needs independently supplied source/target schemas and explicit
negative/unmatched labels. Richer verified meaning should be evaluated on that
evidence before changing defaults. More synthetic cases or counting caller
confirmations as matches would not close this gap. Scale experiments, reusable
prepared schemas, serialization and external-matcher comparisons remain deferred
until a concrete consumer or measurement justifies them. Publication remains a
separate maintainer decision.

## Original three-month plan

We will run one primary workstream at a time, reserve capacity for regression
fixes and documentation, and review scope at each stage boundary. @limadog9 owns
each milestone; outside contributions are welcome but are not a staffing
assumption. Stretch work starts only after the core stage meets its exit criteria.

## Starting point before Stage 1

The implementation on main already provides stable field IDs, normalized names,
coarse type compatibility, exact sampled-value overlap, missing-evidence
explanations, ranked alternatives, local ambiguity, and optional partial global
assignment. It has 38 passing tests including README doctests, a seven-case
synthetic baseline comparison, and 24 deterministic timing workloads. It is not
yet published to crates.io. See the [current benchmark record](docs/benchmarks.md).

The principal gaps are a small evaluation corpus, repeated name/sample preparation
for every pair, coarse numeric representation, unstructured diagnostic strings,
and limited explanation of competing global assignments. The existing seven
cases and one timing run do not establish general accuracy or performance.

## Stages at a glance

| Stage | Target dates | Main outcome |
| --- | --- | --- |
| 1. Establish evidence | Oct 3-16; completed Oct 3 | A reproducible evaluation corpus and comparison harness; [results](docs/evaluation.md) |
| 2. Reduce repeated work | Oct 17-30; completed Oct 3 | Prepared names/samples and measured performance improvements without changing decisions; [results](docs/stage2-performance.md) |
| 3. Improve matching evidence | Oct 31-Nov 13; completed Oct 3 | Distinct-aware samples, exact numeric options, explicit hints and development ablations; [results](docs/stage3-evaluation.md) |
| 4. Explain global decisions | Nov 14-27; completed Oct 3 | Bounded alternatives and typed reasons; [diagnostics](docs/stage4-diagnostics.md) |
| 5. Harden the public API | Nov 28-Dec 11; completed Oct 3 | Typed errors, two consumers and [migration guide](docs/migration.md) |
| 6. Qualify a release | Dec 12-25; qualification completed Oct 3 | Frozen holdout results, million-case campaign, platform checks and candidate archive; quality targets missed |
| Release buffer and review | Review recorded Oct 3 | [Scorecard and corrective work](docs/release-scorecard.md); experimental status retained; no publication |

Stages 1-2 establish the measurement and execution foundation. Stages 3-4 improve
decision quality. Stages 5-6 turn those improvements into a maintainable library
release. Later stages depend on the earlier exit criteria; missed dates do not
justify skipping evaluation or lowering quality requirements.

## Stage 1: establish evidence

**Completed:** 40 separately specified families, 200 pairs and 1,200 labels;
32/8 family development/holdout split; six models under two assignment policies;
frozen protocol, reproducible artifacts and development-only snapshot CI. The
matching algorithm is unchanged. This completes measurement infrastructure, not
the quarter's accuracy targets. External Valentine comparison remains stretch
work and was not included. The requirements below are retained as the stage's
original acceptance criteria.

Replace the demonstration-sized comparison with a repeatable evaluation system.

- Build at least **200 labeled schema pairs** across at least **40 independently
  authored fixture families** and four settings: product/import data, financial
  records, CRM/contact data, and operational telemetry. Aim for at least **1,000
  labeled source-field decisions**, including unmatched and ambiguous outcomes.
- Cover renamed/reordered columns, duplicate names, misleading qualifiers,
  inconsistent types, opaque identifiers, empty/null-heavy/constant samples,
  unrelated schemas, unequal sizes, Unicode names, and assignment collisions.
  Include both cases the matcher can solve and cases with insufficient evidence.
- Start with synthetic fixtures. Every public fixture must have an explicit
  source, redistribution license, attribution, and labeling rationale. A code
  repository's license is not assumed to cover a separately distributed dataset.
- Split by fixture family into development and held-out sets before tuning.
  Reserve at least 20% of families for evaluation; renamed variants of the same
  schema must stay in the same partition. Keep the original seven cases as
  regression examples, not hidden evaluation data.
- Produce a machine-readable result file and a readable report with proposal
  precision/recall, candidate recall@5, proposal coverage, incorrect proposals on
  no-match cases, and expected-abstention accuracy. Report counts per scenario,
  not just one aggregate number. Treat inherently ambiguous labels separately
  from single-target ground truth.
- Compare the existing combined engine, a true name-only baseline, and signal
  ablations on identical inputs. Freeze fixture IDs, generator seeds, configuration,
  dependency versions, and baseline commit in every result.

**Exit criteria:** one documented command reproduces the corpus and reports;
fixture provenance and family partitions are reviewed; the unchanged current
engine's results are saved. Metric definitions and the quality targets below are
frozen before changing scoring. Examining a held-out failure for tuning turns it
into a regression case; replenish the holdout with a new independent family.

**Stretch:** compare selected traditional
[Valentine matchers](https://github.com/delftdata/valentine) through a separate
development-only experiment. Use the same fields, samples, labels and assignment
policy, record algorithm versions/settings, and distinguish its experimental
suite from the current package. This must not add Python or a service dependency
to the Rust library. Lack of a fair external comparison does not block the core.

## Stage 2: reduce repeated work

**Completed:** private per-call preparation for names and samples; fewer report
allocations; identical frozen development predictions; 50 timing/allocation
workloads with five independent runs per revision and measurement mode. The
128-by-128 sampled combined workload improved 3.84× in independent mode and
2.28× with assignment, meeting the stretch target on the recorded machine.
All 24 core medians improved. Report-budget rejection became slower because of
eager preparation; this tradeoff is explicitly recorded. No public prepared-schema
API or candidate pruning was added. The requirements below remain the original
acceptance criteria.

Prepare reusable evidence once per field instead of rebuilding it for every pair.

- Cache normalized tokens, alias expansion and sample summaries within a match
  call. Measure whether reusable prepared schemas are worthwhile before exposing
  a new public abstraction. Preserve the simple custom `Matcher` path.
- Reduce avoidable string/report allocations while preserving explanations,
  unmatched fields, and the rule that top-k display limits do not change decisions.
- Extend timing workloads with short/long names, sparse/dense samples, mostly
  rejected pairs, dense competition, and unequal schemas. Measure peak memory or
  allocations as well as latency; report the measurement method.
- Establish repeated release-build measurements on a recorded reference machine:
  warm-up, at least five independent runs, median and spread, fixed compiler and
  inputs. Record before/after results from the same session. Shared CI timing is
  informational, not a hard microbenchmark gate.
- Keep input, pair, signal-evaluation and report budgets enforced. Exercise both
  successful calls and budget rejection paths; do not silently sample or prune.

**Exit criteria:** identical ranked decisions and assignment results on the
frozen regression corpus, no privacy regression, and published cost measurements.
The stretch performance target is **at least 2x lower median latency** for the
128-by-128 sampled workload against the same-session Stage 1 implementation,
with no more than a 10% unexplained median regression on other core workloads.
If the target is missed, retain only measured improvements and document why;
do not claim the speedup from the existing noisy one-run table.

**Stretch:** study opt-in candidate generation for 512-1,000 fields. Keep exact
all-pairs behavior as the reference. Any pruned mode must explicitly disclose
incomplete search, measure candidate recall and assignment differences, and obey
its own work budget. Do not raise the default 128-field limit until measurements
justify the change.

## Stage 3: improve matching evidence

**Completed:** exact integer/decimal samples, distinct-aware overlap, optional
bounded profiles, checked/inspectable aliases, verified semantic constraints and
source migration guidance. A protocol and 12 new development families were frozen
before scoring; seven models, two assignment modes and 21 thresholds are recorded
alongside full-report regression digests. Holdout families were not scored.
On original development, default independent precision is nearly unchanged
(76.03% to 76.07%) while recall falls (45.62% to 43.97%); matched-precision results
do not establish superiority over the original combined policy. Profiles remain
optional. The 95% precision target is still unmet. The original acceptance
criteria below are retained; completing the evidence stage is not release
qualification.

Make evidence more informative while keeping uncertainty visible.

- Distinguish observation count, distinct count and null coverage in sample
  reliability. Evaluate conservative handling of constant and low-cardinality
  columns so repeated identical values do not masquerade as strong evidence.
- Add an exact integer sample option and design an exact decimal representation
  with explicit scale semantics. Check compatibility and dependency cost before
  selecting the decimal approach. Retain explicit behavior for non-finite floats,
  signed zero, out-of-range integers and mixed sample kinds; avoid silent casts.
- Evaluate bounded sample-profile signals, such as value-kind proportions and
  string-length summaries, against exact overlap. They should remain optional
  unless ablation results show added value without a safety regression.
- Make caller-supplied aliases and semantic hints deliberate and inspectable.
  Define a small way to supply verified units, currency or identifier scope and
  report explicit conflicts. Missing hints are absent evidence, not agreement.
  Do not infer arbitrary synonyms or transform values.
- Tune weights, thresholds and ambiguity margins only on the development split.
  Document conservative defaults and an application-specific tuning recipe;
  continue calling scores heuristics, never probabilities.

**Exit criteria:** each new signal has failure cases and an ablation report;
default changes are compared at matched precision against both the initial
combined engine and name-only baseline. Contradictory supplied semantics cause a
documented rejection or caution, and null-heavy/constant samples have dedicated
regression coverage. Hidden gross/net distinctions remain an acknowledged limit
when the caller provides no distinguishing evidence.

**Stretch:** additional exact sample formats or richer type details only when a
concrete consumer needs them. General parsing, units conversion and automatic
data rewriting remain outside scope.

## Stage 4: explain global decisions

**Completed:** typed local/global reasons, target competition records and opt-in
edge-exclusion probes with explicit solve/work accounting, completeness and
stable-ID witnesses. Oracle tests cover tied, fractional and rectangular graphs;
diagnostics preserve selections and ignore displayed top-k limits. Five-process
cost measurements expose the expensive dense 128-field case (about 957 ms for
complete analysis on the recorded machine); default diagnostics remain disabled.
Confirmed/forbidden pairs remain deferred. Original acceptance criteria follow.

Show when one-to-one assignment is supported and when several mappings remain
equally plausible.

- Separate local ambiguity, target competition, displacement by another source,
  and no eligible target in the public diagnostic model.
- Add bounded checks for alternative assignments on contested mappings. Report
  equal or near-equal objective totals and relevant alternatives without claiming
  the objective gap is a confidence probability.
- Give those diagnostics an explicit solve/work budget. If it is exhausted,
  report that analysis is incomplete; never describe an unchecked assignment as
  unique. Keep deterministic choices and partial matching as the default contract.
- Extend brute-force oracle tests to constrained and contested small graphs;
  exercise rectangular schemas, ties, abstention, truncated reports and budget
  exhaustion. Confirm that diagnostic work does not secretly change selections.

**Exit criteria:** a caller can distinguish a local lexical tie from a global
competition issue, identify why a source stayed unmatched, and tell whether
alternative-assignment analysis finished. Reordered inputs produce the same
decisions and diagnostics. Publish worst-case work measurements for enabled
global diagnostics.

**Stretch:** caller-confirmed and forbidden field pairs for incremental review.
Conflicting confirmations must return an error, never overwrite a decision.
This is mapping metadata only; it does not execute a migration or rewrite data.

## Stage 5: harden the public API

**Completed:** non-exhaustive typed errors and important candidate/source reasons,
privacy-preserving error formatting, importer/catalog review examples, supported
compiler policy, migration notes and a usage guide. Custom matchers retain their
pairwise contract. Source breaks are accepted within the delegated unpublished
implementation scope and documented; this is not a stable-API or publication
decision. Optional serialization is deferred because the examples need no wire
format, and accidental sample export deserves a separate report-only design.
Independent downstream adoption remains a learning target, not a completed claim.

Make the richer engine straightforward to embed and maintain.

- Replace string-only errors and important reason strings with documented,
  structured variants. Keep human-readable explanations, stable field references,
  explicit diagnostic completeness and privacy-safe error formatting.
- Review the schema/config/report construction APIs and extension contract.
  Separate evidence preparation from evaluation only where Stage 2 justified it.
  Audit budget validation for new options and document custom matcher obligations.
- Add optional report/config serialization behind a feature if it benefits the
  integration examples. Version the representation deliberately; keep sample
  export an explicit caller action and leave default builds lightweight.
- Build two end-to-end embedding examples: an importer that asks for review of
  ambiguous proposals, and a migration/catalog tool that preserves unmatched
  fields and keeps caller review decisions in application-owned state. Use synthetic in-memory data and public
  APIs; demonstrate application code, not a new adapter framework.
- Write a migration guide from the initial checkout, document supported Rust
  versions and feature combinations, and review public API changes against the
  [Cargo compatibility guidance](https://doc.rust-lang.org/cargo/reference/semver.html).
  Do not promise 1.0 API stability before downstream experience exists.

**Exit criteria:** both consumer examples compile using only public APIs; the
default build still requires no network, database, LLM or credentials; optional
features are independently tested; public errors and reports are documented.
Every breaking change has a migration example and a recorded maintainer decision.

**Stretch:** incorporate feedback from two independent downstream users if they
are available. Adoption is a learning target, not a release promise or reason to
add collaborators, services or integrations without the owner's authorization.

## Stage 6: qualify a release

**Qualification performed:** features frozen at `5d5c374` before held-out scoring;
full scenario results and name-only comparisons published without tuning;
1,000,000 generated cases passed; dependency licenses/advisories inspected; Rust
1.85/1.99 platform CI, packaging and consumer checks established. The sampled
128-field runtime target is met against Stage 1 (2.87x independent, 2.11x global
median speedup in the same session). Precision and coverage targets fail on the
holdout, so the candidate remains experimental. The final scorecard records
release limits, deferred features and corrective work; no crate was published.
The original acceptance criteria below remain visible.

Prove that the selected feature set is supportable before expanding it again.

- Freeze features and run the full evaluation on the untouched holdout. Publish
  scenario-level successes, failures, counts and configuration; make regressions
  visible instead of changing the benchmark to make the release look better.
- Run formatting, clippy, tests and docs on the supported minimum Rust version
  and current stable. Cover Linux, Windows and macOS, plus every supported feature
  combination. Retain read-only CI permissions and the existing ownership rules.
- Add bounded fuzz/property campaigns for normalization, malformed inputs,
  configuration limits and assignment invariants. Target at least **one million
  generated cases** in a recorded campaign; report seed/corpus, duration, platform,
  reached cases and failures. This is bug-finding evidence, not proof of safety.
- Review dependency licenses, maintenance and advisories; update attribution,
  changelog, examples, failure-case docs and package contents. Verify a clean
  checkout and the packaged crate, including the README examples.
- Prepare a release candidate and an operator-free usage guide: how to choose
  samples, inspect ambiguity, set budgets and evaluate a custom matcher.

**Exit criteria:** no unresolved known implementation-correctness, panic-on-ordinary-input,
sample-disclosure or budget-enforcement blocker; cross-platform checks pass;
quality/performance reports and migration notes are complete. Resolve or explicitly
defer each remaining feature before the maintainer decides whether to release.

## Quarter-end scorecard and release decision

These are **targets to evaluate**, not current measurements or calibrated
guarantees. Freeze the evaluation definitions in Stage 1 and publish misses.

| Area | Target by January 3 | Evidence required |
| --- | --- | --- |
| Evaluation | 200 schema pairs, 40 independent fixture families, 1,000 labeled source decisions | Versioned corpus, provenance, family split, reproducible result files |
| Proposal quality | Without caller hints: at least 95% overall proposal precision, while proposing for at least 60% of held-out fields with a labeled unique match | Count false proposals on no-match cases and unsupported selections on ambiguous cases as errors; publish raw counts and scenario slices; compare name-only at the same precision |
| Candidate usefulness | Without caller hints: at least 90% recall@5 on held-out fields with a labeled correspondence | Eligible/candidate definitions fixed in advance; separate results for opaque renames and supplied domain hints |
| Runtime | Aim for 2x lower median latency on the fixed 128-field sampled workload | Same-machine repeated before/after runs, spread, memory and full workload results |
| Reliability | All supported platform/feature checks pass; one million generated cases exercised | CI links and reproducible campaign record, including discovered failures |
| Usability | Two realistic consumer examples and documented migration path | Examples built against public APIs; no hidden runtime services |
| Distribution | Release candidate and package verification complete | Changelog, license review, crate archive and maintainer release decision |

Do not achieve precision by abstaining on everything: coverage is measured on
fields with known unique matches, and false proposals on no-match/ambiguous cases
must remain visible. Compare defaults without hints separately from explicitly
configured domain hints. If a target lacks enough independent examples for a
useful conclusion, report that limitation instead of presenting a precise-looking
general accuracy claim.

The final buffer is for regression fixes, documentation and a review of the
scorecard, not another feature sprint. If quality targets are missed, keep the
experimental label, explain the shortfall and plan corrective work. If correctness
or privacy blockers remain, postpone release. The preferred quarter-end outcome
is a small, well-supported pre-1.0 crate with honest evidence, not a rushed 1.0.
Crates.io publication, release version and timing remain @limadog9's decision;
this roadmap does not publish a crate or create publishing credentials.

## What moves first if capacity is tight

Protect corpus quality, correctness, abstention, bounded execution and release
documentation. Drop the external matcher comparison and large-schema experiment
first, then richer sample profiles and optional serialization. Carry unfinished
work into the next quarter rather than parallelizing several unreviewed API
changes. Review scope and dates at each stage boundary and record changes with
their reason. Changing a frozen quality target requires an explicitly versioned
plan; still report results against the original scorecard. Never silently lower
targets after seeing held-out results.

Nested transformations, automatic data rewriting, embeddings, Python bindings,
Polars extensions, a hosted service and a generic ETL platform remain outside this
quarter. No new maintainer, write collaborator, publishing owner or third-party
integration is assumed. Planning this work also does not schedule automations or
commit the maintainer to recurring external meetings.
