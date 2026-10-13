# Sealed source-only healthcare hold-out

This package contains one new, complete medical billing schema migration case from the official **OHDSI/CommonDataModel** publisher: PostgreSQL `PROCEDURE_COST` in tag `v5.0.1` to PostgreSQL `COST` in tag `v5.2.2`. It contains 13 source fields, 22 target fields, 12 intended field-role matches and one direct-equivalence no-match. There are no ambiguous labels and no observed values: every `samples` array is empty. The case is **schema-only**.

The evaluator did not inspect Fieldkin source, predictions, runtime proposals, implementation changes, or existing fixture labels. The root agent confirmed only that OHDSI/OMOP was not an existing corpus publisher. Criteria were frozen before extracting fields or making labels (`SELECTION_CRITERIA.json` and its checksum). Publisher context and schema output were independently extracted and frozen before label construction (`SOURCE_CONTEXT_FROZEN.json`). A second source-only reviewer read the official documentation and confirmed the principal semantic distinctions; it did not read label files.

## What is evaluated

Labels describe intended equivalence of published **field roles**, rather than universal equality of values or a complete ETL plan. COST replaces the prior event-specific cost tables. Its event reference is generalized through the published domain discriminator. The target also has a physical spelling error in the revenue source field; the package retains its actual PostgreSQL identifier and separately records the publisher's logical schema spelling.

The no-match distinguishes a secondary-payer component of the source record from the target's primary-payer component and from the payment by the payer represented by each target record. Capturing the source secondary payment requires another payer-specific row; it is not a direct replacement field in the original row. Real target fields retain hard negative roles such as primary versus secondary payer, a standard revenue code versus its original source value, and individual payment components versus their aggregate.

Publisher inconsistencies are disclosed rather than repaired silently. The source copay sentence contradicts its aggregate formula. The procedure total field and the procedure convention's `Copay + Coinsurance + Deductible` formula both support the intended patient-share role, which is also described in the target. Target conventions distinguish claim responsibility from verified actual patient payments and also alternate between expected and actual reimbursement. The case therefore does **not** establish universally lossless copying of monetary values. A scoring task that requires unconditional value equality should exclude this entire case.

Complete DRUG_COST and VISIT_COST candidates were excluded before labels were constructed because publisher definitions did not support an unquestionable answer for every field. In particular, AWP is expressly permitted in target COST but no exact amount field is designated, and VISIT_COST excludes copay in a total while inheriting a formula that includes it. These exclusions follow the original abort rule and were not based on model behavior.

## Files and reproduction

- `case.json`: the requested standard top-level `{name, source, target, answers}` case.
- `schemas.json`: physical declared names/types with empty sample arrays, independent of labels.
- `contexts.json`: separately extracted publisher table descriptions, field descriptions, logical names, physical types and requiredness, with source-document identifiers. It has no cross-schema correspondences or answer-derived roles.
- `labels.json` and `evidence.json`: sealed labels and per-field semantic justifications with source quotations and citations.
- `provenance.json`: original URLs, pinned commits/tags, retrieval date, full-source hashes, byte counts, licenses and publisher discrepancies.
- `raw/`: precise schema/PDF excerpts and license text. The 1.09 MB original PDF and full repository are not retained; their pinned URL and full-file SHA256 are recorded.
- `extract.py`: reproducible publisher-only extraction. It fetches six pinned files, verifies full-file checksums, uses `pdftotext -layout`, and regenerates schemas, context, provenance and excerpts without reading any case or answer file.
- `assemble.py`: joins the schema output to independently authored labels and validates coverage and actual target identifiers.
- `FROZEN.json`: SHA256 manifest sealing all package contents except itself.

Extraction used Python 3's standard library and Poppler `pdftotext` 25.03.0. A deterministic extraction reproduction was checked in a separate temporary directory by comparing the generated schemas, contexts, provenance and all retained excerpts. No Fieldkin code was run.

To regenerate and compare publisher input without reading answers:

```sh
python extract.py --output-dir /tmp/ohdsi-source-reproduction
cmp schemas.json /tmp/ohdsi-source-reproduction/schemas.json
cmp contexts.json /tmp/ohdsi-source-reproduction/contexts.json
```

To reassemble the case from existing frozen input:

```sh
python assemble.py
```

Actual PostgreSQL `INTEGER`, `NUMERIC`, and `VARCHAR(n)` declarations are normalized to `integer`, `decimal`, and `text`. The publisher's logical monetary declaration (`float`) is retained separately. Publisher documentation identifies the source revenue field as `revenue_code_source_value`; target PostgreSQL declares `reveue_code_source_value`. Both identifiers are retained, with the identical published role, companion revenue concept field, declared type, and schema position serving as evidence of the physical/logical discrepancy. The target Markdown also swaps Type and Required cells for its final two DRG rows; extraction recognizes the Yes/No cell and corroborates physical types from DDL.

Source PDF page 2 and the target documentation License.md publish a public-domain dedication / CC0-1.0 declaration. The source repository LICENSE declares Apache-2.0. The raw license statements are included so these source-specific declarations remain reviewable. Exact original URLs and SHA256 values are in `provenance.json`; they are anchored to official OHDSI repository commits rather than moving branches.

The package must remain sealed until the root evaluator freezes its candidate implementation. Selection, schemas, context, and labels are then fixed; prediction-driven relabeling or selection is not permitted.
