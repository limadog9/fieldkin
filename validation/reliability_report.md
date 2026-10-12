# First blind reliability comparison

Final outcome: both proposed runtime changes were rejected and the original
runtime remains. See the [second-trial and final report](reliability_followup_report.md)
for the 236-field accepted baseline and the separate second frozen experiment.
This page preserves the first experiment's measurements.

The frozen candidate reduces wrong automatic matches from **19 to 8**, but loses
**13 correct renamed matches**. Automatic precision improves from 81.73% to 90.00%;
recall falls from 62.50% to 52.94% and exact decision accuracy from 59.76% to 54.44%.
This is a precision/recall tradeoff, not an unqualified reliability improvement.
The candidate was **rejected** because it suppresses legitimate documented
renames. The runtime was restored; the new regression gate retains the original
before-comparison baseline, not the candidate's reduced recall.

Earlier results on already exposed data had looked better: development wrong
automatic matches fell 8→1 while correct automatic matches stayed at 170, and
correct decisions rose 215→219. On the previously seen independent corpus,
wrong automatic matches fell 8→6, correct automatic matches stayed at 35, and
correct decisions rose 91→93. Those were development/regression measurements,
not new blind evidence; the unseen cases exposed the lost legitimate renames.

The runtime was frozen at 2026-10-12T19:18:49Z before the implementation author saw
these fields, labels, or predictions. The last source-only annotation correction
was independently re-frozen at 19:19:19Z; it removed two competing decimal source
and target representations with unsupported unique labels. The corpus wrapper
hash is `ebc103f5e97b840517ada20f55aebf5aa5eddbfd8118e94923b99281ef3049ee`.
The unchanged before binary was built from commit
`073127592d7e9099c5aa8c3f7f9f2ffc3131a431`. Runtime file hashes, policy,
and the before-binary hash are preserved in
[`reliability/implementation_freeze.json`](reliability/implementation_freeze.json).
The runtime, inputs, labels, and case selection were unchanged during the first
comparison. After rejecting the candidate, the runtime was restored. A separate
second candidate was later frozen, measured, and rejected as documented in the
follow-up report. First-trial corpus artifacts and raw results remain unchanged.

The eight cases contain 169 source fields: 136 unique matches, 33 no-matches, and
no ambiguous labels. They use publisher examples and schema-only declarations,
not observed production records. Financial cases have no samples; several
operations fields also lack samples. DCSA 3.0 is an official published draft.

## Overall results

| Metric | Before | Frozen candidate |
| --- | ---: | ---: |
| Exact decision accuracy | 101/169 (59.76%) | 92/169 (54.44%) |
| Automatic-match precision | 85/104 (81.73%) | 72/80 (90.00%) |
| Automatic-match recall | 85/136 (62.50%) | 72/136 (52.94%) |
| Automatic-match coverage | 104/169 (61.54%) | 80/169 (47.34%) |
| Correct no-match decisions | 16/33 (48.48%) | 20/33 (60.61%) |
| Automatic selections on no-match labels | 9/33 (27.27%) | 5/33 (15.15%) |
| Wrong automatic matches | 19 | 8 |
| Abstentions | 65 | 89 |
| NoMatch predictions | 34 | 58 |
| Ambiguous predictions, all incorrect | 31 | 31 |
| Missed unique matches | 51 | 64 |
| Exact-name recall | 41/46 (89.13%) | 41/46 (89.13%) |
| Renamed recall | 44/90 (48.89%) | 31/90 (34.44%) |

Exact-name means the source name equals the gold target name byte-for-byte,
among unique-match labels only. Renamed covers the remaining unique-match labels.
All 13 lost correct matches are renames; exact-name recall is unchanged. The
candidate changes 24 Match decisions to NoMatch: 11 were wrong automatic matches
and 13 were correct. Of the 11 wrong automatic matches, four become correct
no-match decisions; seven remain incorrect abstentions on positive labels.
Ambiguity behavior is unchanged and this corpus has no gold ambiguities.

## Every case

| Case | Accuracy before → candidate | Correct auto / match labels | Wrong auto |
| --- | ---: | ---: | ---: |
| Stripe Plan → Price | 15/21 → 12/21 | 13/16 → 8/16 | 2 → 0 |
| Treasury assistance → database | 15/27 → 13/27 | 12/22 → 10/22 | 3 → 3 |
| Treasury contracts → database | 11/25 → 9/25 | 9/20 → 7/20 | 5 → 4 |
| FHIR Encounter R4 → R5 | 13/15 → 14/15 | 9/10 → 9/10 | 2 → 1 |
| FHIR Procedure R4 → R5 | 11/15 → 12/15 | 9/11 → 9/11 | 1 → 0 |
| GBFS geofencing 2.3 → 3.0 | 8/15 → 8/15 | 8/12 → 8/12 | 2 → 0 |
| GBFS vehicle 2.3 → 3.0 | 10/17 → 10/17 | 8/14 → 8/14 | 2 → 0 |
| DCSA equipment events 2.2 → 3.0 draft | 18/34 → 14/34 | 17/31 → 13/31 | 2 → 0 |

The schema-only financial domain loses nine correct positives while removing
three wrong automatic matches. Healthcare loses none and removes two wrong
automatic matches. Operations loses four correct positives while removing six
wrong automatic matches. All lost operational positives are in the DCSA case.

## Evidence for the 13 lost correct correspondences

These classifications use the already frozen publisher evidence; they are not
proposals for tuning against these cases. All 13 become NoMatch.

| Publisher evidence | Lost correspondence(s) |
| --- | --- |
| Stripe identical definitions under the documented Plan → Price replacement | `interval` → `recurring.interval`; `interval_count` → `recurring.interval_count`; `meter` → `recurring.meter` |
| Stripe identical transformation definitions with renamed parent property | `transform_usage.divide_by` → `transform_quantity.divide_by`; `transform_usage.round` → `transform_quantity.round` |
| Treasury official concept crosswalk, Public rows 287 and 294 | `primary_place_of_performance_code` → `place_of_performance_code`; `primary_place_of_performance_scope` → `place_of_performance_scope` |
| Treasury official concept crosswalk, Public rows 99 and 413 | `current_total_value_of_award` → `current_total_value_award`; `type_of_set_aside_code` → `type_set_aside` |
| DCSA same facility and transport-call sequence roles with expanded nesting | `eventLocation.facilityCode` → `eventLocation.facility.facilityCode`; `transportCall.transportCallSequenceNumber` → `transportDetails.transportCall.transportCallSequenceNumber` |
| DCSA transport-call reference and carrier import-voyage identifier definitions | `transportCall.transportCallID` → `transportDetails.transportCall.transportCallReference`; `transportCall.importVoyageNumber` → `transportDetails.transportCall.importVoyageNumberOrReference.carrierVoyageNumber` |

The archived Stripe label evidence quotes both definitions; Treasury evidence
identifies each original crosswalk row and target model declaration; DCSA evidence
retains both schema declarations, line numbers, parent references, and sample
origins. The healthcare labels describe documented directional R4 → R5 migration;
allowed reference populations expand in some R5 schemas. Procedure.subject is
not claimed to be a bidirectional identity over every R5 focus configuration;
the selected examples do not have a differing focus.

## Reproduction and limitations

The strict labeled parser and source adapter verified all cases before prediction.
All 133 preserved artifacts passed checksums. Source-specific extractors rebuilt
the three bundles byte-for-byte in temporary directories. Three Rust integrity
tests and three Python corruption tests passed. Source-only healthcare audit
also checked 128 literal samples and all 57 normalized field types.

Raw first outputs are [`before.txt`](reliability/before.txt) and
[`after.txt`](reliability/after.txt). [`reliability_results.json`](reliability_results.json)
contains every case and domain count, recall strata, all 24 changed decisions,
and raw-report hashes. Its current counts were checked against an evaluator
snapshot; that diagnostic snapshot was removed from active gating. The accepted
`reliability_quality_baseline.json` instead records the original before counts,
including this tranche's 101/169 correct decisions and 85/136 correct unique
matches. The second trial appended only its new cases' original-runtime counts;
every first-tranche baseline entry remains unchanged. CI uses
the evaluator's unchanged per-case and per-category regression rules, so the
rejected candidate fails the new gate. Linux CI also verifies byte-exact source
regeneration; Rust provenance and baseline checks run on Linux and Windows.

```sh
python3 validation/reliability/verify_extraction.py --self-test
python3 validation/reliability/verify_extraction.py
cargo test --locked --example evaluate_all reliability::tests
cargo run --locked --example evaluate_all -- --reliability
cargo run --locked --example evaluate_all -- --check-reliability
```

The before report used `/tmp/fieldkin-evaluate-before-0731275` with the absolute
`eval_reliability` directory path. That temporary binary is not a distributable
artifact; its commit and hash are recorded for a fresh build from the baseline
revision. Both reports used the identical frozen case files and default config.

The rejected runtime is preserved exactly in
[`reliability/candidate.patch`](reliability/candidate.patch), SHA-256
`61fe27877d540bc0a1c95cf83c5f3a458390ce28b3f04341d4e49460b07a69a4`.
Applying it to the recorded baseline revision reproduced every runtime hash in
`implementation_freeze.json` in a temporary copy. The patch is an experiment
archive, not an active implementation change; the current evaluator can be used
with it to reproduce the candidate's corpus report.
The corresponding integration-test additions are archived separately in
[`reliability/candidate-tests.patch`](reliability/candidate-tests.patch).

This is one small, intentionally challenging schema-migration sample. It cannot
establish representative production accuracy or ambiguity handling. The data is
now exposed development feedback; rerunning it is not a second blind trial.
Existing development and independent inputs, labels, manifests, and baseline
files remain unchanged.
