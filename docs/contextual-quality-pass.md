# Contextual matching quality pass — October 5, 2026

**Not qualified; main remains unchanged.** This working branch preserves the
original release requirements:
95% proposal precision, 60% aggregate unique-field coverage, 90% candidate
recall@5, nonempty output, both assignments and every required variant. Main
integration requires fresh qualification and all engineering/platform checks.
Development improvements alone cannot qualify the candidate. The final guarded
development acceptance command exits 1 because aggregate unique coverage fails.
The independently prepared fresh corpus has never been scored or used for tuning.

## Public policy and evidence

`Config::contextual_quality()` explicitly selects the experimental candidate.
Ordinary `Config::default()` and `ContextualEvidence::default()` keep their
existing behavior. The preset includes the same inspectable qualifier/unit
vocabulary used by the precision protocol. It does not perform conversions.

```rust
use fieldkin::{Config, MatchEngine};
let engine = MatchEngine::new(Config::contextual_quality())?;
# Ok::<(), fieldkin::MatchError>(())
```

The sampling assumption is explicit: populations can differ. Disjoint samples
are therefore inconclusive, rather than automatically incompatible. Informative
role evidence, compatible representations and separation from plausible choices
must support matching. Generic names, opaque codes, copied constants and sample
profiles do not independently establish meaning.

Relationships distinguish supported equivalence, contradiction and unresolved
wording. Only actual contradictions are excluded before calculating competition;
unresolved choices remain, including choices that fail current support thresholds.
Competition uses every compatible original pair before top-k and caller review.
Events, entities, polarity, direction, qualifiers, units/currencies and declared
Date/Timestamp representation remain safeguards. Scores are heuristics.

Structured `ContextualReason` values expose overlapping support and relationship
reasons without sample values. The evaluator attributes unselected unique matches
to the correct candidate, and separately records score/ambiguity/assignment
effects. Labels and slicing metadata reach only the scorer after matching.

## Controlled evidence

The starting remote main is `8a8568033269b5f15fba6cc3958760f24fd9b8ff`; the working
tree was clean. Stable Rust was checked with `rustup check`: Rust 1.99.0 and Cargo
1.99.0. The original development, hint-free extension, corrective development and
previously examined qualification are kept separate. Historical files remain
unchanged. Fresh qualification data stay outside policy development.

The baseline reproduction agrees with the recorded October 5 results. Individual
experiments include the existing relaxed identifier option, relationship repair,
independent populations and score ranking. Each experiment records configuration,
source/input/dependency identities, actual decisions, slices and changes. Failed
experiments are retained; proposed policy repairs were not assumed to work.

## Qualification and engineering discipline

`development-acceptance` checks numerical counts and required corpora/modes/
variants during iteration. `quality-freeze` binds the implementation, evaluator,
configuration, development evidence and opaque fresh inputs to a fresh executable.
`quality-qualify` requires an explicit reserved-run acknowledgement and preserves
the first-attempt start record even if execution fails. `release-acceptance`
verifies that frozen authorized record and its unrounded count-based gates.
Snapshot equality remains a separate reproducibility check.

The fresh corpus is independently instructed synthetic authorship, isolated by
instructions from implementation, results and existing answers. Agents share a
filesystem; this is not enforced sandbox isolation or production adjudication.
The independent author and reviewer must record their actual limitations.

The native performance comparison measures the complete selected public policy
against the original contextual policy using five alternating processes per
revision and latency/allocation mode, including success, ambiguity, independent
populations and four budget rejections in both assignment modes. Input/engine
construction is excluded; matching and report destruction are included. Allocator
requests are separate from timing and do not measure peak memory. The roadmap's
2x speed target remains an aspiration, not an achieved result.

No crates.io publishing, release, tag, credential changes, force push or protection
bypass was performed. A failed or unverifiable required gate leaves main unchanged.

## Final measured matching results

The runtime, evaluator, native tools, harnesses, contracts and README were committed
before final measurements at `b36290af851cbf18b9fa8f84474f9978e8b631cb`. Later commits
add evidence and documentation without changing that implementation. The freshly
built final evaluator SHA-256 is
`491f5de3dfde0a6960b992dcabfc062465f971d5fa151ae9474439039f974f52`.
The original baseline executable is independently recorded at SHA-256
`454ec72945f517649c7e8eb1fb961fd391e4c853e3c75551726d9594c4d3cbba`.

Here, before is the original strict contextual policy, and after is the complete
public quality preset. Ordinary defaults and both name-only comparators are
reported separately and unchanged. All required variants are included in each
aggregate. The hint-free extension is deliberately small; its outcomes have little
statistical headroom. Existing examined qualification is regression evidence,
never fresh evidence.

| Corpus | Assignment | Before correct/proposed | After correct/proposed | Precision before → after | Unique coverage before → after |
| --- | --- | ---: | ---: | ---: | ---: |
| Original development | Independent | 407/407 | 407/409 | 100% → 99.5110% | 407/730 (55.7534%) → 407/730 (55.7534%) |
| Original development | One-to-one | 402/402 | 402/404 | 100% → 99.5050% | 402/730 (55.0685%) → 402/730 (55.0685%) |
| Extension without hints | Independent | 13/13 | 12/12 | 100% → 100% | 13/20 (65%) → 12/20 (60%) |
| Extension without hints | One-to-one | 12/12 | 11/11 | 100% → 100% | 12/20 (60%) → 11/20 (55%) |
| Corrective development | Independent | 63/66 | 66/69 | 95.4545% → 95.6522% | 63/141 (44.6809%) → 66/141 (46.8085%) |
| Corrective development | One-to-one | 60/63 | 63/66 | 95.2381% → 95.4545% | 60/141 (42.5532%) → 63/141 (44.6809%) |
| Previously examined qualification | Independent | 78/78 | 76/76 | 100% → 100% | 78/210 (37.1429%) → 76/210 (36.1905%) |
| Previously examined qualification | One-to-one | 74/74 | 72/72 | 100% → 100% | 74/210 (35.2381%) → 72/210 (34.2857%) |

Candidate recall@5 is 100% in all eight before/after rows. Wrong unique-target
proposals are zero, so unique coverage happens to equal correct recall in this
comparison; the definitions remain distinct. After has two unmatched-label errors
on original development and the same three unmatched-label errors on corrective
development in each assignment; ambiguous-label errors remain zero. Thus high
precision has not hidden false proposals or established sufficient usefulness.

| Corpus | Assignment | Correct retained/lost/recovered | False proposals removed/introduced |
| --- | --- | ---: | ---: |
| Original development | Independent | 387 / 20 / 20 | 0 / 2 |
| Original development | One-to-one | 382 / 20 / 20 | 0 / 2 |
| Extension without hints | Independent | 12 / 1 / 0 | 0 / 0 |
| Extension without hints | One-to-one | 11 / 1 / 0 | 0 / 0 |
| Corrective development | Independent | 63 / 0 / 3 | 0 / 0 |
| Corrective development | One-to-one | 60 / 0 / 3 | 0 / 0 |
| Previously examined qualification | Independent | 74 / 4 / 2 | 0 / 0 |
| Previously examined qualification | One-to-one | 70 / 4 / 2 | 0 / 0 |

The opaque original matches lost under the concept safeguard are reported rather
than restored through an answer-derived synonym or sample-only meaning rule.
Typed integral counts remain supported. Recovering three corrective matches is
useful, but replacing 20 original correct matches with 20 different correct matches
and two new errors is not an aggregate matching-quality success. The extension
and examined corpus also regress. This preset remains experimental.

Full counts, correct recall, proposals on unmatched/ambiguous labels, transitions
and overlap-aware rejection reasons are in
[the selected report](../evaluation/results/quality-pass-v1/selected-final/report.json).
[The slice inventory](../evaluation/results/quality-pass-v1/slice-summary.json)
reports each domain, required variant and source-sample availability separately;
corpora are never pooled. Predictions and changes are adjacent to each report.

## Controlled policy experiments and attribution

The original existing `strict_identifier_samples=false` option alone recovered
20 original matches and introduced three unmatched-label errors: 427/430 and
422/425 proposals, 58.4932% and 57.8082% unique coverage. Other development corpora
did not improve. It missed coverage and was not selected.

On the final source, the independent-population repair alone produces 427/429 and
422/424 on original development, the unchanged 13/13 and 12/12 on the extension,
and 66/69 and 63/66 on corrective development. It still misses original and
corrective coverage. The relationship repair alone and regular identifier word
forms alone produce 387/387 and 382/382 on original development, 12/12 and 11/11
on the extension, and unchanged corrective results. Their conservative losses
are not counted as progress. Ranking preservation changes no observed selections;
the measured score-floor contribution to missed unique matches is zero in all
final rows, so the public preset retains the floors and ambiguity abstention.

Broad initial population rules failed the precision floor on the extension and
corrective development. Disabling scoped support also failed precision: the
corrective population/relaxed variant produced 67/91 (73.6264%). A sampled
identifier rename prototype failed the close negative `warehouse_code` to
`paint_id`; the independent-mode failure was observed and the single-edge global
outcome inferred. It was removed despite recovering known development renames.
The final negative contract is actually exercised in both assignments.

Evidence is retained under
[relaxed identifiers](../evaluation/results/quality-pass-v1/relaxed-identifier/report.md),
[first individual repairs](../evaluation/results/quality-pass-v1/individual-repairs-1/report.md),
[revised repairs](../evaluation/results/quality-pass-v1/individual-repairs-2/report.md),
[unscoped experiments](../evaluation/results/quality-pass-v1/unscoped-experiments/report.md),
[rejected rename patch](../evaluation/results/quality-pass-v1/rejected-sampled-rename/README.md)
and [final controlled repairs](../evaluation/results/quality-pass-v1/controlled-final/report.md).
Their configuration/source identities distinguish discarded implementations;
earlier results are not attributed to the final code.

Diagnostics attribute each missed uniquely labeled source to its actual gold
candidate, with overlapping reasons for unresolved relationships, contradictions,
missing identifiers, inadequate observations and competitors. Score threshold,
local ambiguity and assignment displacement are recorded separately. Consequently
these counts are not disjoint categories and must not be summed as source totals.
No postfiltering of old predictions is presented as a runtime improvement.

## Fresh qualification and acceptance

The fresh author and independent reviewer received separate instructions with no
conversation history, matcher code, tuning results or existing answers. They
prepared 12 synthetic families across four domains, with 32 unique, 24 no-match
and 16 ambiguous base labels: five required variants expand these to 60 cases and
360 decisions. Corpus SHA-256:
`143c931f4b5485ccefa08ecfd93a071e4048d2ff4012d9875ae2320adb7d0f0f`.
The schemas and labels remain local under `target/fresh-quality-reserved` and were
never read by policy developers or supplied to runtime development.

[Fresh status and provenance limits](../qualification/results/quality-pass-v1/fresh-status.json)
record the author/reviewer isolation and hashes. Instruction isolation on a shared
filesystem is not enforced isolation. Synthetic labels are not production
adjudication. No fresh qualification score or production guarantee is claimed.
[The planned protocol](../qualification/results/quality-pass-v1/qualification-protocol.json)
contains the full measured public configuration and unchanged acceptance contract.
It has not been bound to a reserved first attempt because development fails.

[Guarded development acceptance](../evaluation/results/quality-pass-v1/development-native/acceptance.json)
returns exit 1 after validating source, binary and input identities. It requires
95% precision, 60% aggregate unique coverage, 90% recall@5 and nonempty proposals
for each required corpus and assignment. Variant/domain slices reconstruct the
aggregate, but an individual variant is not given an invented 60% coverage rule.
An initial direct executable invocation was correctly rejected for differing Cargo
environment; the documented Cargo invocation verified provenance and then failed
the actual numerical gate. This is recorded separately from snapshot success.

When a later candidate legitimately passes development, the native workflow is:

```text
cargo +stable run --locked -p fieldkin-tools -- quality-freeze --protocol qualification/results/quality-pass-v1/qualification-protocol.json --corpus target/fresh-quality-reserved/qualification.json --provenance target/fresh-quality-reserved/provenance.json --development evaluation/results/quality-pass-v1/development-native/artifacts --build-dir target/quality-reserved-build
cargo +stable run --locked -p fieldkin-tools -- quality-qualify --build-dir target/quality-reserved-build --output target/quality-reserved-results --acknowledge-new-holdout
cargo +stable run --locked -p fieldkin-tools -- release-acceptance --build-dir target/quality-reserved-build --output target/quality-reserved-results
```

Use genuinely fresh paths and development records; do not reuse these identities
for different source. The gate rejects stale, missing, mismatched and incomplete
evidence, weakened protocols and inaccurate underlying counts. The first-attempt
ledger is keyed by the LF-normalized corpus digest and survives failures. Release
acceptance verifies the frozen record rather than rescoring the holdout. Ordinary
CI evaluates development only, never repeatedly opens a fresh holdout.

## Engineering and distribution evidence

[Final Windows engineering](../qualification/results/quality-pass-v1/engineering.json)
passes 23/23 recorded commands: all formatting, strict Clippy, warning-free default
and JSON documentation, 373 workspace all-feature tests, 226 no-default library
tests, both standalone performance feature configurations, qualification and scale
tests, six public consumer examples, 1,024 assignment compatibility cases, and
offline licensed T2D/Northix fixture reproduction. All 81 input hashes stayed fixed.
[Independent source review](../qualification/results/quality-pass-v1/independent-review.json)
found no remaining implementation/privacy/budget blocker after fixing opaque-ID,
custom-weight duplicate-role and sampled temporal-representation defects.

[Snapshot checks](../evaluation/results/quality-pass-v1/snapshot-checks.json) reproduce
all nine evaluation routes and the five-model development route; a separately
recorded [native external evaluation](../evaluation/results/quality-pass-v1/external-native)
retains source/binary guards. Positive external annotations cannot qualify precision
or unmatched-field safety.
[One million generated cases](../qualification/results/quality-pass-v1/generated/run.json)
passed on the final implementation, seed 20,261,005, with seven categories receiving
142,857 or 142,858 cases. This is a deterministic property campaign, distinct from
fresh labeled accuracy qualification.

[Dependency/advisory/license review](../qualification/results/quality-pass-v1/dependencies.json)
records 72 third-party package versions across three lockfiles. No dependencies
were added. The fetched RustSec database remained at commit
`ef6173cbc5c50ec8166f9a5b28f07834144373ee` (October 3); all three advisory audits
passed. This records the reviewed database state, not a guarantee against unknown
issues. License/notice hashes and package metadata are retained in the review.

## Actual contextual policy costs

[Full repeated measurements](../qualification/results/quality-pass-v1/performance/summary.md)
include all 24 successful, ambiguous, disjoint-population and budget-rejection
workloads. All builds and other checks finished before measurement. Five processes
per revision alternate order, with separate allocator instrumentation. No CPU
affinity/frequency control was imposed; the min/max process spread is not a
confidence interval. The baseline is the actual original contextual runtime at
`8a85680`, using the copied common harness, and the candidate is the actual public
preset at `b36290a`. Input/engine setup is excluded; matching and report destruction
are included. Disjoint workloads make different selections, so their cost change
includes changed behavior. The duplicate-role fixture abstains by different
documented diagnostic categories in the two policies.

| Representative workload | Before median µs [min–max] | After median µs [min–max] | Allocations before → after | Requested bytes before → after |
| --- | ---: | ---: | ---: | ---: |
| 128 sampled, independent | 66,836.90 [62,326.18–67,614.68] | 72,694.24 [67,295.02–73,102.26] | 207,088 → 214,121 | 36,912,376 → 37,385,375 |
| 128 sampled, one-to-one | 66,069.24 [61,801.18–69,623.38] | 72,643.12 [68,882.40–79,525.94] | 207,479 → 214,512 | 37,483,032 → 37,956,031 |
| 64 ambiguous, independent | 14,885.14 [14,203.26–19,540.62] | 16,791.66 [16,368.36–19,498.68] | 53,287 → 56,160 | 9,554,792 → 9,558,799 |
| Report rejection, independent | 54.31 [50.25–74.39] | 94.27 [91.34–135.62] | 305 → 819 | 52,808 → 68,808 |

The sampled 128-field cases are 8.76% and 9.95% slower. Additional semantic
preparation, full competitor handling and diagnostics cost time and allocation;
report rejection also does more bounded preparation before returning the same
budget error. This tradeoff is documented, not waived or described as faster.
The roadmap's aspirational 2x speed target is unmet. Cumulative allocator requests
are not peak memory measurements.

The unchanged 16 MiB explanation default rejects the candidate's 128x128 sampled
success fixture. Both benchmark revisions explicitly use 32 MiB for representative
success cases; report-rejection cases use one byte and require the exact error.
This is an application capacity choice, not a relaxed library default or disabled
budget. Raw observations, machine/compiler identities, exact executable and source
hashes are retained beside the summary. Historical timing is not reused.

## Packaging and remote integration

[Package verification](../qualification/results/quality-pass-v1/packages.json)
records fresh default and JSON package builds and publish dry-runs, archive/content
and license comparisons, and consumers compiled against the extracted archive in
independent workspaces. These consumers exercise the new preset and JSON fidelity;
they are smoke checks, not independent business adoption. No crate is uploaded.

The branch is `quality/contextual-release-20261005`. Its draft pull request retains
the separate development-quality job, which must fail while the documented counts
miss coverage. Platform engineering/reproducibility jobs remain mandatory on Linux,
Windows and macOS. They do not read the untouched fresh corpus. Inspect the current
PR commit's job results; local tests or successful snapshot equality are not release
acceptance. Remote results and exact revisions are reported with the PR.

Main integration is blocked by original and corrective development coverage in
both assignments, extension one-to-one coverage, and absent fresh qualification.
The examined corpus also remains far below coverage and regresses. A later matching
change must receive new engineering/measurement provenance and legitimately fresh
accuracy evidence before any release claim. Main is not updated by this pass.
