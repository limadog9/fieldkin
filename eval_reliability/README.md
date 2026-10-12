# Blind reliability corpus

Eight separately sourced schema-migration cases contain 169 source fields and
173 target fields: 136 match labels and 33 no-match labels. Three domain teams
selected and annotated the inputs using publisher documentation without reading
the matcher implementation, previous evaluation fixtures, or predictions. Each
team froze its case arrays, extraction scripts, evidence, and publisher archives
before disclosure to the implementation author. Original freeze manifests and
all their artifact bytes are retained under `sources/`.

These are publisher examples and schema declarations, **not production records**:

| Domain | Cases | Source fields | Evidence |
| --- | ---: | ---: | --- |
| Financial | 3 | 73 | Stripe Plan → Price and Treasury USAspending crosswalks; schema only, all samples empty |
| Healthcare | 2 | 30 | HL7 FHIR R4 → R5 definitions, official conversion maps, and illustrative XML examples |
| Operations | 3 | 66 | GBFS versioned specifications and DCSA Track & Trace schemas with publisher examples |

Some operations fields also have no samples. DCSA 3.0 is an official **WORK IN
PROGRESS draft**; the corpus does not represent it as a released API. Publisher
placeholder strings and partial FHIR date/time values remain unchanged. Source
manifests distinguish schema examples, specification examples, and conformance
fixtures. No rows, aliases, or example values were invented.

Cases retain original property names, with documented mechanical dotted/array
paths for nesting. Target schemas are deliberate partial projections; a no-match
label means no equivalent exists within that target projection. Some positive
matches require an ordinary representation conversion. Labels describe semantic
equivalence, not guaranteed direct value copying. Each bundle documents these
choices and gives evidence for every selected field and label.

There are **no ambiguous labels**: no team found a documented pair of genuinely
interchangeable target fields. This corpus cannot establish ambiguity quality,
production reliability, or performance across a representative distribution.
Financial schema-only cases specifically exercise missing sample evidence.

`validation/reliability_corpus.json` preserves the case fingerprints, original
bundle array/index, provenance references, and every archived artifact checksum.
The small `examples/support/reliability.rs` adapter reuses strict case parsing and
fingerprints from the existing evaluator. It verifies label validity, case-set
completeness, unchanged original case arrays, and all frozen source/evidence bytes
before predictions. It does not reinterpret arbitrary source schemas.

Verify the original source-specific extraction offline (Python standard library;
all work takes place in temporary copies). Byte-exact regeneration was validated
on Linux; the frozen extractors use native text-file newlines. Git attributes
preserve archived bytes on all platforms, and Rust integrity checks are portable:

```sh
python3 validation/reliability/verify_extraction.py
cargo test --locked --example evaluate_all reliability::tests
```

Evaluate and check the separate suggestion baseline:

```sh
cargo run --locked --example evaluate_all -- --reliability
cargo run --locked --example evaluate_all -- --check-reliability
```

The first blind candidate reduced wrong automatic matches but lost 13 correct
renames and was rejected; see `validation/reliability_report.md`. The gate retains
the original runtime's before-comparison baseline, including its 101/169 correct
decisions and 85/136 correct unique matches. It uses the existing per-dataset and
per-category regression rules without lowering recall to the rejected candidate.
This baseline preserves existing errors; it is not an accuracy target. The
original development and independent corpora, manifests, and baselines remain
unchanged. This directory is excluded from the published crate by Cargo's
existing explicit package include list.

The same commands also load the separately frozen second tranche in
`eval_reliability_new/`. Both runtime candidates were rejected; the accepted
combined gate has 236 source fields with original-runtime counts, preserving all
eight entries from this directory. See `validation/reliability_followup_report.md`
for the final outcome. No runtime accuracy improvement was accepted.

See each source bundle's README and archived license texts for source terms.
GBFS documentation is CC-BY-3.0; its schemas and DCSA material use Apache-2.0;
Stripe uses MIT; Treasury and HL7 use CC0 with the original HL7 third-party
terminology qualification retained.

Before first predictions, the financial source-only audit removed two positive
source fields and two competing target decimal representations whose unique
labels depended on an unsupported integrality assumption. No remaining labels
changed. `sources/financial/ANNOTATION_CHANGES.json` records the correction and
re-freeze; `audit/financial_initial/` retains the initial case array and freeze
for annotation history only and is never loaded by the evaluator.
