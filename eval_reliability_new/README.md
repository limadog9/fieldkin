# Second held-out reliability tranche

Three new cases contain 67 source fields and 58 target fields: 51 unique-match
labels, 16 no-match labels, and no ambiguous labels. They were sealed before
predictions and before the implementation author saw their fields or labels.
The final second candidate was frozen at 2026-10-12T19:42:24Z. Source inputs and
labels were not changed after predictions.

| Case | Source fields | Target fields | Matches | No match | Evidence |
| --- | ---: | ---: | ---: | ---: | --- |
| Treasury contract subawards → broker schema | 27 | 27 | 21 | 6 | Official crosswalk and declared schema; no samples |
| FHIR MedicationRequest R4 → R5 | 24 | 18 | 17 | 7 | Official definitions, conversion maps, and illustrative XML examples |
| GBFS station status 2.3 → 3.0 | 16 | 13 | 13 | 3 | Published schemas and literal specification examples |

These are schema declarations and publisher examples, **not production records**.
Financial CSV types remain unknown where the publisher declares no storage type;
target database TEXT declarations remain text, including amounts and dates.
Every retained healthcare and operations sample comes from an archived publisher
example. Target schemas are explicit partial projections; no-match labels refer
to those projections. Declared semantic matches may require representation
conversion. Neither tranche measures correct ambiguity handling.

The first tranche did not evaluate these schema pairs. Some publisher archives
and common metadata fields are reused, so this is not a new independent source
family. Financial and healthcare authors did not inspect runtime policies or
first-trial reports. The operations author had audited first-trial reports, which
limits independence; that author did not inspect the second candidate's code,
policy, or predictions and selected complete source-schema coverage under a
recorded policy.

Original bundle bytes, team freeze manifests, field-level evidence, licenses,
and source-specific extractors are under `../eval_reliability/sources/blind2/`.
`validation/reliability_followup_corpus.json` freezes the 84 artifacts and binds
each case to its original archived object or array. The first eight cases and
their manifest remain unchanged. The same verifier and evaluator handle both
tranches, without new CLI modes or dependencies.

```sh
python3 validation/reliability/verify_extraction.py
cargo test --locked --example evaluate_all reliability::tests
cargo run --locked --example evaluate_all -- --reliability
cargo run --locked --example evaluate_all -- --check-reliability
```

Byte-exact extraction was validated on Linux; the frozen scripts use native text
newlines. Git preserves archived bytes on all platforms. CI performs extraction
on Linux and Rust provenance/baseline checks on Linux and Windows.

The second candidate changed no decision on these 67 fields, but lost one
legitimate rename on the first tranche and was rejected. Both runtime candidates
were rejected; the original runtime remains. The accepted gate appends only the
new cases' original-runtime before counts and preserves every first-tranche
baseline entry. See `validation/reliability_followup_report.md` for the final
outcome, measured limitations, and rejected-experiment records.
