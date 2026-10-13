# Semantic reliability investigation — rejected corroboration policy

**No runtime change met the acceptance criteria.** The strongest candidate
improved exposed reliability data but added a false automatic match on new
financial data without recovering any additional held-out matches. Its worst
measured workload also cost 13.9 times as much. The runtime, public API, five
Valentine algorithms, defaults and all earlier labels/baselines remain unchanged
from `c6b5a2caa2c337ee63ff0dd2993bed352eff2637`.

The retained improvement is validation: four sourced migration cases, an
original-runtime regression gate using the existing metrics, and the same fixed
five-algorithm benchmark on those inputs. CI runs the new gate on Linux and Windows.
The experimental patch is archived, not part of the library.

## Root causes and alternatives

The [per-field analysis](semantic_failure_analysis.md) classifies all 20 wrong
automatic matches and 62 missed renamed matches in the existing 236-field corpus.
Eight fields occur in both groups. The biggest usable-evidence categories are
hierarchical/camelCase role distinctions (8 wrong automatic, 25 missed rename),
explicit code/description distinctions (3 and 6), and abbreviations rejected by
the missing-sample evidence gate (13 missed). Another 14 missed renames require
semantic/entity aliases not established by the current flat signals.

The correct target already ranks first for 29 missed renames and second for 18.
Ranking and acceptance are therefore different problems. Thirty-six missed
renames have Unknown source types and no samples. Five wrong automatic matches
lack the distinguishing code/description role entirely in their supplied inputs;
publisher definitions establish it, but name equality cannot recover absent facts.
Contrasting periods, units or entity scopes cannot be inferred safely from a
generic name or a few coincidentally equal observations.

All five native implementations and their selection semantics were inspected.
COMA ranks 173/187 unique gold matches first on the exposed reliability corpus,
versus 166 for Flooding, 90 for Cupid, 68 for Jaccard and 67 for DistributionBased.
These are candidate-ranking counts, not automatic-match accuracy. COMA and
Flooding supply useful lexical alternatives but also repeat name biases. Cupid's
WordNet relatedness can prefer related but different business concepts. The
instance algorithms have little evidence on schema-only cases. Vetoing original
matches unless COMA agrees loses 42 correct automatic matches across all 597
exposed fields to remove 16 errors; Cupid agreement loses 104 to remove 29.
Blind averaging or mandatory agreement is unjustified.

Two preliminary, exposed-data experiments were rejected before the held-out
trial. Reusing existing camelCase token boundaries reduced wrong matches from
20 to 18 but reduced correct decisions from 145 to 144 and recovered renames
from 52 to 51. Leaf-only path scoring gained some reliability renames but reduced
development accuracy from 215/256 to 212/256 and increased its wrong matches from
8 to 10. Neither repeated the earlier global token-overlap reduction or exact-name
floor removal; neither justified replacing existing behavior.

The principal candidate instead separated ranking from acceptance. An optional,
fallible method retained every original candidate score, component, rank and
default decision. A sole positive default COMA correspondence could rescue an
original NoMatch only when it agreed with the original top candidate and that
candidate met the existing minimum score. Other decision changes additionally
required agreement with Jaccard's unique positive top candidate. Ties and zero
evidence could not authorize a change. No scores were averaged and no new
thresholds or field-name exceptions were introduced. Jaccard ran only when needed.

This was 86 lines of candidate logic using existing implementations. It was
frozen before new financial/new medical contents were opened. The candidate
patch, runtime/dependency hashes, source-packet seals and acceptance rule are in
[`semantic/candidate_freeze.json`](semantic/candidate_freeze.json). No policy
tuning followed held-out prediction exposure.

## Before and after

Here “candidate” means the **rejected prototype**, not retained behavior.
Counts and complete decisions are archived in `semantic/exposed_metrics.json`,
`exposed_predictions.jsonl`, `heldout_metrics.json` and `pinned_predictions.jsonl`.

| Corpus | Fields | Correct automatic | Wrong automatic | Precision | Unique-match recall | Renamed recall | Exact decision accuracy |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Development, original → candidate | 256 | 170 → 170 | 8 → 8 | 95.51% → 95.51% | 83.33% → 83.33% | 66/99 → 66/99 | 215/256 → 215/256 |
| Earlier independent, now exposed | 105 | 35 → 35 | 8 → 8 | 81.40% → 81.40% | 85.37% → 85.37% | 17/23 → 17/23 | 91/105 → 91/105 |
| Existing reliability, exposed | 236 | 118 → 137 | 20 → 18 | 85.51% → 88.39% | 63.10% → 73.26% | 52/114 → 71/114 | 145/236 → 164/236 |
| **New fully held-out** | **55** | **30 → 30** | **2 → 3** | **93.75% → 90.91%** | **60.00% → 60.00%** | **7/26 → 7/26** | **34/55 → 33/55** |
| New medical, partly exposed | 13 | 2 → 5 | 2 → 2 | 50.00% → 71.43% | 16.67% → 41.67% | 2/12 → 5/12 | 3/13 → 6/13 |

All 19 changed exposed decisions become correct; no exposed decision becomes
incorrect. Nevertheless, ten false automatic matches on expected no_match in
the existing reliability corpus remain ten. Gains on positive labels do not prove
safe abstention. On genuinely held-out no_match labels, errors rise from 0/4 to 1/4.

| New case | Independence | Correct decisions | Correct automatic | Wrong automatic |
| --- | --- | ---: | ---: | ---: |
| CFPB modified LAR 2017 → 2018, 17 fields | Fully held-out | 10 → 10 | 8 → 8 | 0 → 0 |
| Adyen Payment v52 → Checkout v72, 21 fields | Fully held-out | 20 → 19 | 18 → 18 | 1 → 2 |
| OHDSI VISIT_OCCURRENCE 5.2.2 → 5.4.1, 17 fields | Fully held-out | 4 → 4 | 4 → 4 | 1 → 1 |
| OHDSI PROCEDURE_COST 5.0.1 → COST 5.2.2, 13 fields | Partly exposed | 3 → 6 | 2 → 5 | 2 → 2 |

The decisive regression is Adyen `enableRealTimeUpdate`: its documented role
controls a real-time account updater, not storage for recurring payments. Gold is
no_match. Original behavior abstains; the candidate selects `enableRecurring` at
0.7803921568627451. Complementary lexical agreement did not establish equivalence.
The new gate exits 1 with this field and the affected dataset/category metrics;
[`semantic/rejected_candidate_gate.txt`](semantic/rejected_candidate_gate.txt)
records the executed failure. This rejection holds despite mixed-pool accuracy
increasing from 37/68 to 39/68. Partial-exposure gains must not hide held-out loss.

Automatic coverage on old reliability rises 138/236 → 155/236 and abstentions
fall 98 → 81. Fully held-out coverage rises 32/55 → 33/55 solely through the wrong
match; abstentions fall 23 → 22. Gold ambiguity correctness remains 0/1 on new
fully held-out data. Original and candidate both select Adyen `deliveryDate`
instead of expressing uncertainty between the documented old/new alternatives.

Precision is correct unique automatic matches divided by all automatic matches;
no_match successes cannot inflate it. Recall divides by unique match labels.
Exact decision accuracy requires the exact target or exact ambiguous target set.
Abstention includes NoMatch and Ambiguous. The established renamed metric uses
literal source/target name inequality: case/namespace differences count. An
additional diagnostic ignoring punctuation/case in the final path component has
3/13 held-out renamed matches recovered in both modes, and 1/7 → 4/7 in the partly
exposed case. Neither is evidence of general semantic inference.

## Independent sourcing and freeze protocol

Source-only authors and separate source-only reviewers received no proposed
policy, predictions or known failure list. They selected official publishers,
froze inclusion/abort criteria, extracted publisher schema/context, then annotated
every included source field with evidence. Schema, labels, selection and source
bytes were sealed before predictions. The implementation author did not see the
financial or new VISIT_OCCURRENCE labels/contents until the policy was frozen.
Author/review work was performed by separate AI agents, not external humans;
publisher documentation is independent evidence, but annotation mistakes remain
possible. These are small, purposive migration projections, not random production
samples or a statistically powered reliability claim.

An agent-status listing accidentally exposed several PROCEDURE_COST mappings
before the policy freeze. Its annotations were already sealed, but the entire
13-field packet is conservatively classified partly exposed and excluded from
the held-out aggregate. A fresh complete VISIT_OCCURRENCE packet was then sourced
without exposing its contents to the implementation author until its own seal.

The strict existing parser rejected a numeric JSON sample in that fresh packet
before its first prediction: Fieldkin samples require strings. Its author changed
only twelve actual publisher numeric illustrations to exact decimal strings,
without changing schema identities, types, context, labels or evidence. The
original seal/input and correction binding are retained. The corrected inputs
were reverified before the first matching run. This was serialization repair,
not prediction-driven relabeling.

The initial scratch evaluator resolved serde_json 1.0.154; the project pins
1.0.151. All comparisons were repeated using the project's locked dependencies.
The first 51 field records were identical, and the final 68-field archive is from
the locked repeat. The historical per-row key `candidate_sha256` identifies the
case fingerprint, not the runtime; runtime hashes are in `candidate_freeze.json`.
Source URLs, revisions, retrieval dates, full/excerpt SHA-256,
licenses, exclusions and per-field evidence are recorded under
`eval_reliability/sources/semantic_*`. CFPB snapshots use CC0; Adyen specifications
use MIT; OHDSI schema/documentation snapshots retain their Apache/public-domain
statements. Monetary roles and general event identifiers do not imply lossless
ETL or equal units. Samples are publisher illustrations, not patient/customer
observations. No synthetic observations or labels were added.

The fully held-out clinical case has 17 unique-match labels and no no_match or
ambiguity labels. It tests clinical migration retrieval, not clinical negative
abstention. All four held-out no_match labels and the sole gold ambiguity come
from finance. Healthcare hard-negative decision reliability remains unestablished.

## Five algorithms, separately measured

The existing configurations and common automatic cutoff 0.5 are unchanged.
Ranked candidates are evaluated separately from accepting their top pair.
Fully held-out results (55 fields, 50 unique match labels):

| Algorithm | Automatic precision | Recall | F1 | Top-1 | Recall@5 | Coverage |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| COMA | 90.20% | 92.00% | 91.09% | 92.00% | 98.00% | 92.73% |
| Cupid | 91.49% | 86.00% | 88.66% | 86.00% | 86.00% | 85.45% |
| DistributionBased | 50.00% | 2.00% | 3.85% | 2.00% | 2.00% | 3.64% |
| JaccardDistanceMatcher | 50.00% | 2.00% | 3.85% | 4.00% | 4.00% | 3.64% |
| SimilarityFlooding | n/a | 0.00% | 0.00% | 88.00% | 98.00% | 0.00% |

COMA has the strongest retrieval/recall here; Cupid accepts fewer pairs with
slightly higher precision. Distribution/Jaccard lack useful instance evidence
on most fields. Flooding ranks many correct pairs but accepts none at the common
cutoff; it is not evidence of a failed implementation. Scores are not calibrated
across algorithms, so that cutoff cannot establish their optimal operating
points. No algorithm recognizes the documented ambiguity as an explicit
decision. Per-case counts, all incorrect/missed examples and limitations are in
[`semantic/native_accuracy.txt`](semantic/native_accuracy.txt) and JSON. Reference
parity is separate from these semantic measurements; native scores were unchanged.

## Cost, reproduction and retained scope

The [release cost report](semantic/performance_report.md) contains three fresh
processes per mode/workload, a warmup and five measured matches per process,
variation and Linux peak RSS. The exposed corpus rose 221.833 → 284.894 ms (+28.4%).
A 32-source/64-target ambiguous workload rose 192.443 → 2671.062 ms (13.88×),
peak RSS 5060 → 9272 KiB. A 64-column aligned case rose 779.902 → 789.757 ms but
RSS 9624 → 21672 KiB from extra sample cloning. Loading/startup are excluded from
times; RSS includes resident inputs and warmup. These are rejected-prototype
costs, not retained performance regressions. Original candidate ordering and
numeric component bits were verified unchanged on all seven fixed workloads.

Run retained validation from the repository root:

```sh
cargo run --locked --example evaluate_all -- --check-semantic
cargo run --locked --example benchmark_accuracy -- --semantic --details
cargo run --locked --example benchmark_accuracy -- --semantic --json
python3 validation/semantic/benchmark_corroboration.py --verify-only
```

To reproduce the rejected gate using a snapshot of the reviewed working tree
(GNU tar; Rust/Cargo on PATH), leaving the runtime untouched:

```sh
trial=$(mktemp -d)
git ls-files --cached --others --exclude-standard -z |
  tar --null -T - -cf - | tar -xf - -C "$trial"
git -C "$trial" apply validation/semantic/candidate.patch
git -C "$trial" apply validation/semantic/evaluator_candidate.patch
(cd "$trial" && cargo run --locked --example evaluate_all -- --check-semantic)
# Expected exit 1, including enableRealTimeUpdate -> enableRecurring.
```

The new baseline records the original runtime's actual 37/68 correct decisions,
32/36 correct automatic matches and existing errors, not a claimed quality target.
The existing per-dataset and per-category rules allow improvements and reject
accuracy/precision losses or additional wrong automatic/ambiguity decisions. No
old baseline or fingerprint was regenerated. Archive/tooling/data additions are
excluded by the unchanged Cargo package include list; no production dependency,
public API or configuration was added.

The retained diff adds 104 net Rust lines to the existing evaluation runners and
shared loader, plus a 287-line archived performance replay (Rust/Python).
Publisher extraction/assembly scripts add 459 Python lines. The complete addition
is approximately 1.35 MB, mainly source snapshots, annotations and frozen results;
production Rust growth is zero and the verified package still contains 45 files.
The extra artifacts make provenance, rejected decisions and measurements
reviewable; no second runtime implementation or evaluation framework is maintained.

The main unresolved weakness remains semantic evidence: neither strong names
nor correlated matcher agreement prove compatible field roles. Context/definitions
would be necessary for several diagnosed errors, but an unvalidated metadata API
or semantic rules engine was not added. These cases are now exposed for future
development; a future policy still needs fresh validation before a generalization
claim. The rejected experiment materially strengthens that evidence, but does
**not** improve runtime semantic reliability.

## Engineering verification

Formatting, 179 default all-target tests, 191 Polars all-target tests, four doc
tests, strict Polars Clippy, warning-free Rustdoc and verified
`cargo package --locked --allow-dirty` passed. All existing development,
independent and reliability gates passed, as did the new original-runtime gate.
All 90 pinned reference comparisons passed within unchanged tolerances (60 exact,
89 exact source rankings and 90 rankings equivalent allowing rounding ties).
These compare against the previously executed pinned Python 1.0.0 reference with
its two already documented source fixes; Python inference was not rerun. The
native repeat is bit-identical to the previous commit's 9,199 pairs and ordering.

Normal all-feature tests and Clippy were blocked by `ort-sys` downloading ONNX
binaries from `cdn.pyke.io`: CONNECT proxy 403. Static all-feature Clippy with
`ORT_SKIP_DOWNLOAD=1` passed; it does not validate inference execution. Windows
was not run locally; its existing CI matrix includes the new gate. The rejected
gate's exit 1 and all seven archived release-output fingerprints were verified.
`semantic/engineering_checks.json` records exact commands/outcomes. Validation
completed on `improve/semantic-reliability` before integration into `main`.
