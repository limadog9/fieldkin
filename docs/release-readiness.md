# Contextual precision and release qualification

The [October 5 readiness pass](readiness-pass.md) records the current candidate,
same-input changes, packaging checks and platform validation. **The current
candidate remains not qualified.** The evidence below describes the frozen
October 4 candidate and preserves its failed first qualification; that examined
corpus is now used only for regression. The current runtime is described in the
new pass rather than inferred from these historical implementation details.

**Release decision: not qualified.** The frozen blind run reached 100% precision
and 100% candidate recall@5, but only 52.86% independent coverage and 49.05%
one-to-one coverage against the unchanged 60% target. No matcher settings were
changed after seeing these results.

The optional runtime candidate meets the development targets of at least 95%
proposal precision, 60% unique-match coverage and 90% candidate recall@5 on the
original comparison and both development stress sets. Defaults remain unchanged.
Development success is separate from prospective qualification on new reserved
families. The fixed synthetic qualification report is linked below;
it does not certify production accuracy or authorize publication.

## Same baseline, useful matches and errors

The original development dataset remains 32 families, 160 schema pairs and 960
source decisions: 730 unique matches, 190 unmatched and 40 ambiguous labels.
All five correlated variants stay together. No easier subset replaces it.

| Policy | Assignment | Correct/proposed | Precision | Unique coverage | Candidate recall@5 |
| --- | --- | ---: | ---: | ---: | ---: |
| Unchanged default | Independent | 321/422 | 76.07% | 44.66% | 100% |
| Conflict checks only | Independent | 321/417 | 76.98% | 44.66% | 100% |
| Runtime contextual candidate | Independent | 470/470 | 100% | 64.38% | 100% |
| Unchanged default | One-to-one | 313/414 | 75.60% | 43.56% | 100% |
| Runtime contextual candidate | One-to-one | 465/465 | 100% | 63.70% | 100% |

Independent matching preserves 279 correct default pairs, loses 42, recovers 191
additional correct pairs and removes all 101 false default proposals. One-to-one
matching preserves 274, loses 39, recovers 191 and removes 101 false proposals.
Both add zero new false proposals on this examined dataset. Losing useful pairs
is a real cost; the improvement is not described as preserving every match.
Conflict checks alone preserve all 321 correct independent pairs and remove five
false proposals, all related variants of one pressure-unit family.

The separate Stage3 extension has 12 development families and 32 decisions;
caller hints are stripped before every model. Corrective development has 12
families, 36 cases and 216 decisions; its reserved families are excluded.
These stress sets are not pooled with the original dataset.

| Development stress set | Assignment | Correct/proposed | Precision | Unique coverage | Candidate recall@5 |
| --- | --- | ---: | ---: | ---: | ---: |
| Extension without hints | Independent | 13/13 | 100% | 65% | 100% |
| Extension without hints | One-to-one | 12/12 | 100% | 60% | 100% |
| Corrective development | Independent | 93/96 | 96.88% | 65.96% | 100% |
| Corrective development | One-to-one | 90/93 | 96.77% | 63.83% | 100% |

Extension matching loses three correct default matches in each assignment mode
and removes ten false proposals. Corrective independent matching preserves 12,
loses eight, recovers 81 and removes 51 false proposals. Three incorrect proposals
remain: observed agreement cannot reveal an absent business-event distinction.
The [full development report](../evaluation/results/readiness-v3/report.json)
contains every model, counts, transitions and family/domain/variant slices. Its
[readable table](../evaluation/results/readiness-v3/report.md) retains earlier
development policies, including unsuccessful comparisons.

## Runtime configuration and evidence

Enable `Config::contextual_evidence = Some(ContextualEvidence::default())` and
select appropriate `Config::name_conflicts` for the application's vocabulary.
The default contextual preset requires strict identifier samples and scoped
support; the ordinary Config default leaves the option disabled. See the
[precision guide](precision.md) for configurable qualifier/unit checks and
caller-confirmation behavior.

The [runtime policy](../src/contextual.rs) uses observable names, declared types,
typed samples and supplied hints. It requires active concrete built-in name and
default distinct-aware sample signals. Signal names alone cannot authenticate a
custom matcher. No labels, concepts, family IDs, rationale, tags or stored answer
table enter matching. IDs address fields and caches.

Informative exact name cores can support lexical matches. Generic names alone
are insufficient. Identifier fields require at least three distinct values on
each side, at least 0.25 non-null coverage, Jaccard at least 0.90 and a margin of
0.10 over every hard-compatible alternative in both row and column. Exact copied
source observations share a competitor identity; target twins remain alternatives.
Non-identifier exact lexical matches require adequate observed overlap when both
sides have observations, without requiring a mutual margin. Empty/all-null
non-identifier samples count as unavailable. Boolean lexical roles remain useful.

Unqualified singleton measurements require representation evidence; declared
Integer fields with exclusively integral numeric observations are exempt from
that measurement heuristic. This exception does not prove that integers are
dimensionless. Physical integers can still have hidden units. Temporal sample
recovery requires a shared informative role. A recognized configured unit on
either name requires matching unit evidence on both. Supplied semantic/type
conflicts and configured name contradictions remain vetoes.

The optional contextual score is the maximum of the weighted score, 0.90 for
supported informative lexical agreement and 0.95 for adequate sampled overlap.
Original signal scores remain visible. These floors are heuristics, not calibrated
probabilities. `ContextualScoreAdjustment` identifies a change and
`InsufficientContextSupport` explains an exclusion; otherwise viable unsupported
sources explicitly abstain with `Decision::InsufficientEvidence`. Threshold 0.70,
ambiguity margin 0.08 and top-k five are unchanged. The entire compatible matrix
is assessed before caller restrictions, ambiguity, assignment and truncation.
Callbacks run once and all actual signal/explanation work is charged to budgets.

## Development iterations and freeze discipline

V1 distinctive-context rules were declared before their first development run.
They reached 99.47% precision / 77.12% coverage on the original dataset but only
81.25% precision on the extension and 80.58% on corrective development.
V2 added scoped sample, measurement and temporal-role checks based on those
development failures; strict identifiers reached 100% precision / 59.73% original
coverage, missing the coverage target. V3 refined exact non-identifier sample
adequacy, unavailable observations and integral Integer representation after
examining V2 useful-match losses. The runtime port and strict candidate nomination
were recorded before the first V3 development score. Earlier policies were retained.
These iterations are development tuning, not cross-validation or untouched tests.

All 320 original default reports and aggregate counts are verified against the
historical current-default baseline. A regression compares runtime selections,
rankings and eligibility with the fixed V3 prototype on every development case
under both assignments. The prototype's generated caller exclusions are not used
by the public runtime: the runtime emits genuine contextual evidence issues.

The new prospective protocol freezes one runtime candidate and two fixed name-only
comparators (thresholds 0.70 and 1.0), with the default also reported on exactly the
same inputs. There is no holdout threshold sweep. A separate author agent supplied
12 new families, three per original domain, without seeing matcher algorithms,
policies, fixtures or results. Five related variants yield 60 cases / 360 decisions.
The corpus was reserved during implementation and its author digest was recorded
before scoring. Every variant is explicitly assigned to holdout, independent of
the old corpus partition. Older published holdouts are retired evidence; corrective
and T2D reserved partitions remain unscored.

The [fixed qualification protocol](../evaluation/context-qualification-protocol.json)
and [native verified workflow](verified-evaluation.md) bind implementation,
protocols, data, configuration, compiler, lockfiles and a freshly built executable.
Before/after guards reject drift and output replacement. A one-time start record
is persisted before scoring; an interrupted attempt is still examined evidence.
The only new holdout route requires explicit acknowledgement and frozen digests.
Labels reach the scorer after matching observable schemas. No caller hints or
confirmations count as autonomous success.

## Metrics, tooling and limits

The [blind report](../evaluation/results/context-qualification-v1/artifacts/qualification.md),
[raw counts and scenario slices](../evaluation/results/context-qualification-v1/artifacts/qualification.json)
and [guarded run record](../evaluation/results/context-qualification-v1/run.json)
preserve the first prospective outcome:

| Policy | Assignment | Correct/proposed | Precision | Unique coverage | Candidate recall@5 |
| --- | --- | ---: | ---: | ---: | ---: |
| Default | Independent | 27/27 | 100% | 12.86% | 100% |
| Fixed name-only comparators | Independent | 15/15 | 100% | 7.14% | 90.91% |
| Frozen contextual candidate | Independent | 111/111 | 100% | 52.86% | 100% |
| Default | One-to-one | 24/24 | 100% | 11.43% | 100% |
| Frozen contextual candidate | One-to-one | 103/103 | 100% | 49.05% | 100% |

All 90 unmatched and 60 ambiguous decisions are safely abstained in both modes.
Of 210 uniquely labeled decisions, the candidate abstains on 99 independently
and 107 one-to-one. Independent coverage is 27/42 (64.29%) in each sampled variant
but 3/42 (7.14%) without samples; one-to-one is 25/42 (59.52%) and 3/42. All variants
remain included. Dropping the difficult variant would change the test. At least
126 unique proposals are needed for 60% coverage; the fixed candidate is short
by 15 independently and 23 one-to-one. These observations explain the failed
coverage gate; they were not used for another policy run or parameter selection.

The [frozen build record](../evaluation/results/context-qualification-v1/frozen-build.json)
binds the evaluated executable SHA-256
`5a733cde6abdf218df5c68a8242563acd18bc44f05347dbeac68c7c14d50f1fa`
to the source/data/configuration inventory and Rust 1.99.0 compiler. The new
holdout is now examined evidence and cannot be reused as a fresh test.

Precision counts every proposal, including wrong unique targets and unsafe
selections on unmatched or ambiguous fields. Ambiguous truth requires abstention
even when a chosen target belongs to the plausible set. Unique coverage counts
any proposal on a uniquely labeled source, so correct recall is reported separately.
Zero proposals give undefined precision and cannot qualify. Candidate recall@5
includes ineligible displayed alternatives; on these small target schemas high
retrieval alone does not demonstrate safe selection. Related variants are not
independent observations or a statistical confidence bound.

The repository uses current stable Rust, verified locally with Rust 1.99.0.
Fixture import, evaluation provenance, benchmarks, generated qualification and
dependency review are native Rust tools. New Python/Valentine COMA execution was
retired; existing archived score imports/comparisons remain available. Historical
artifacts retain their dated language/compiler facts rather than being rewritten.
The [dependency review](../qualification/results/rust-native-v1/dependencies.json)
records 72 package versions across five manifest graphs and three lockfile scans
with no reported vulnerabilities; library runtime dependencies are unchanged.
The [generated campaign](../qualification/results/rust-native-v1/campaign/run.json)
passed 1,000,000 cases across seven categories, including contextual support and
conflict checks. The workspace passed 331 tests with one existing manual-cost test
ignored; separate harness/example tests and the 1,024-case assignment compatibility
check passed. All eight current development/external accuracy snapshots passed
exact checks. These correctness checks do not replace the failed coverage gate.

Synthetic qualification can establish the fixed synthetic targets. One agent's
newly authored labels are not independent production adjudication. Hidden scopes,
units and event meanings remain unverifiable when absent from the inputs. The
maintainer owns publication, version and timing; local tests do not establish
remote Linux/macOS CI success.
