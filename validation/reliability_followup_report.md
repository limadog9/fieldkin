# Final reliability outcome: both runtime candidates rejected

**No runtime accuracy improvement was accepted.** The first candidate lost 13
documented renamed matches. The narrower second candidate changed no decision on
the new 67-field tranche, then failed the preserved first-tranche gate by losing
one legitimate shipping rename. Both were reverted. The accepted change is the
source-verified evaluation corpus and regression gate; the runtime and defaults
remain as before this work.

The current original-runtime baseline covers 11 cases and 236 source fields:
145/236 correct decisions, 118/187 correct unique matches, and 20 wrong automatic
matches. These are existing limitations, not improved accuracy claims.

The narrow candidate had reduced development wrong automatic matches 8→7 while
preserving 170 correct automatic matches and 215 correct decisions. The previously
seen independent corpus was entirely unchanged. Those exposed-data checks were
not blind validation, and they did not predict the first-tranche rename loss.
Both candidates' apparent development benefits were rejected; final runtime
behavior on all these corpora remains unchanged.

## Second trial and its first results

The final second runtime was frozen at 2026-10-12T19:42:24Z, before the
implementation author saw any second-tranche fields, labels, or predictions.
A source-blind review corrected an initials-recognition defect before that final
freeze; the earlier draft was never evaluated on the new cases. No runtime or
label tuning followed first predictions. The second-tranche source manifest is
`b2e87426e0a48eaec8f33aa015be6d2c8413503e7326752e321c237f8ef6d810`.
The final runtime freeze is
[`fallback_implementation_freeze.json`](reliability/fallback_implementation_freeze.json),
SHA-256 `af1b5430d4d89cb1a3d97633b231abb92af5b06a068e3597213a074fd6731cd6`.

Original-runtime predictions began at 19:43:23Z. The new cases' original counts
were appended to the gate at 19:43:24Z, before the candidate comparison. Every
first-tranche dataset fingerprint and baseline count remained unchanged.

All second-tranche decisions were unchanged:

| Metric | Original and second candidate |
| --- | ---: |
| Exact decision accuracy | 44/67 (65.67%) |
| Automatic-match precision | 33/34 (97.06%) |
| Automatic-match recall | 33/51 (64.71%) |
| Wrong automatic matches | 1 |
| Correct no-match decisions | 11/16 |
| Abstentions | 33: 28 NoMatch, 5 Ambiguous |
| Missed unique matches | 18 |
| Exact-name recall | 25/27 (92.59%) |
| Renamed recall | 8/24 (33.33%) |

| New case | Accuracy | Correct auto / unique matches | Wrong auto | Renamed recall |
| --- | ---: | ---: | ---: | ---: |
| Treasury contract subawards → broker schema | 11/27 | 5/21 | 0 | 2/18 |
| FHIR MedicationRequest R4 → R5 | 22/24 | 17/17 | 1 | 4/4 |
| GBFS station status 2.3 → 3.0 | 11/16 | 11/13 | 0 | 2/2 |

This supports nonregression on those cases only. It supplies **no independent
accuracy gain** and leaves weak schema-only rename recall visible.

## Why the second candidate also failed

On the first tranche, `transportCall.importVoyageNumber` stopped matching
`transportDetails.transportCall.importVoyageNumberOrReference.carrierVoyageNumber`.
The frozen DCSA definitions identify the same carrier-specific import voyage;
the lost match became NoMatch. First-tranche correct unique matches fell 85→84,
with wrong automatic matches unchanged at 19. This is a regression on already
exposed data, not a new blind result.

| Scope | Original/current restored | Rejected second candidate |
| --- | ---: | ---: |
| First 8 cases: correct decisions | 101/169 | 100/169 |
| New 3 cases: correct decisions | 44/67 | 44/67 |
| All 11 cases: correct decisions | 145/236 | 144/236 |
| Pooled automatic precision | 118/138 (85.51%) | 117/137 (85.40%) |
| Pooled unique-match recall | 118/187 (63.10%) | 117/187 (62.57%) |
| Pooled wrong automatic matches | 20 | 20 |
| Pooled exact-name recall | 66/73 | 66/73 |
| Pooled renamed recall | 52/114 | 51/114 |

The gate failed before restoration; its diagnostic is retained in
[`followup_gate_failure.txt`](reliability/followup_gate_failure.txt). Restoring the
original runtime makes the gate pass and exactly reproduces its full baseline.
The rejected candidate never became a lower quality floor.

An external temporary harness compared complete candidate arrays, with
`max_candidates=usize::MAX`, across all 236 fields. All **5,384 candidate pairs**
preserved target order and exact `f64::to_bits()` values for total, name, type, and
optional sample scores. Only the one decision changed. This proof uses full
numeric values, not the evaluator's rounded diagnostic display.

## Preserved records and limits

[`reliability_followup_results.json`](reliability_followup_results.json) records
every case, each tranche, pooled metrics, exact-name/renamed strata, timestamps,
report hashes, and the bit-comparison summary. Raw first outputs are
[`followup_before.txt`](reliability/followup_before.txt) and
[`followup_current_all.txt`](reliability/followup_current_all.txt). **“Current” in
that filename means the rejected candidate at measurement time**, not the final
restored runtime. The original [first-trial report](reliability_report.md), raw
outputs, corpus manifest, and source artifacts remain intact.

The second experiment is reproducible from the recorded baseline commit plus
[`fallback-candidate.patch`](reliability/fallback-candidate.patch) and optional
[`fallback-candidate-tests.patch`](reliability/fallback-candidate-tests.patch).
Applying those patches in a temporary baseline copy reproduced every frozen
runtime hash and candidate test byte. The runtime-patch SHA-256 is
`e0c564abf27db46eaeb55380093fe5ea95435f972fb8762a203516bdd127d6eb`.

All 217 source artifacts across both tranches passed checksums, and all six
source bundles regenerated byte-for-byte offline in temporary copies. The
second tranche contains schema-only Treasury declarations and official FHIR/GBFS
examples, not production observations. Publishers and some metadata are reused.
The operations author had seen first-trial reports; the other two source authors
had not. No source author saw the second runtime policy or predictions before
sealing their cases. There are no gold ambiguity labels. These small deliberate
projections cannot establish representative production reliability.

The existing development and independent corpora and their baselines are
unchanged. The accepted commands remain:

```sh
python3 validation/reliability/verify_extraction.py
cargo run --locked --example evaluate_all -- --reliability
cargo run --locked --example evaluate_all -- --check-reliability
```

Final validation of the retained changes passed 21 checks: 172 default and 184
Polars tests, four doctests, formatting, strict Polars Clippy, static all-feature
Clippy with `ORT_SKIP_DOWNLOAD=1`, rustdoc with warnings denied, all existing
evaluation gates and the new reliability gate, verified packaging, and Python
corruption, extraction, parity, and benchmark-helper checks. All 90 reference
cases agree with the saved corrected pinned Python fixture; all 9,199 candidate
pairs and scores equal the pre-task Rust output. Runtime and existing test files
are byte-identical to the original commit. All-feature test execution and normal
all-feature Clippy remain externally blocked by an HTTP CONNECT 403 from the
ONNX download host `cdn.pyke.io`; the static check does not establish execution
of those features.
