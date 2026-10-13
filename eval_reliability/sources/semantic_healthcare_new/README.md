# Additional unseen source-only healthcare validation

This packet contains one complete official OHDSI/OMOP clinical encounter schema migration: PostgreSQL VISIT_OCCURRENCE from `v5.2.2` to `v5.4.1`. Both schemas contain 17 actual declared fields. Every source field has a documented intended counterpart; 13 names persist and four transition names change. The target retains 27 individually reviewed directed hard negatives involving distinct semantic roles.

The packet was selected and extracted **after the root reported its runtime candidate frozen**. Selection criteria, derived from the original predeclared complete clinical-table criteria, were frozen before extraction. The author did not inspect Fieldkin code, proposals, predictions, existing fixture labels, or other implementation work while preparing this new packet. No new table/field names or labels were sent to root before sealing. The source reviewer read only the official raw publisher files and kept its final status free of field names and labels.

This is **schema-only with published vocabulary illustrations**, with no observed clinical records. Exact integer vocabulary illustrations are retained for one field on each side, with every value attributed in `sample_evidence.json`. The source publisher gives one explicit coded example; the target publisher lists eleven named Visit-domain concepts with exact official concept URLs. These are documentation examples, not patient observations. Every other `samples` array is empty. Labels rely on published field roles, declared temporal precision, reference entities, and vocabulary domains, not names or sample equality alone.

## Evidence and limits

The case distinguishes admission origin from discharge destination, standardized concepts from original source strings, standardized visit kind from source-record provenance, present-visit identity from the preceding visit reference, provider from patient and care site, start from end, and calendar dates from timestamps. All target confounders are actual published fields. There are no fabricated observations or fields and no forced ambiguous/no-match labels.

This case tests intended **schema-role correspondence**. The transition concept vocabularies evolve from source Place of Service descriptions to target standard Visit-domain concepts, so a real ETL may need vocabulary remapping. It does not establish unconditional value copying. There are no source roles missing from the complete target, so this packet does not independently test source no-match abstention.

Five source Markdown rows are malformed: one has an extra blank cell, and five transpose Type and Required. Extraction recovers the named description and corroborates all field identities, physical types, and requiredness against official PostgreSQL DDL. Source convention prose includes stale undeclared aliases; these remain as verbatim publisher prose and are not manufactured into schema fields. Target ETL prose also contains date/datetime and record-provenance examples; actual field declarations and field-specific semantic descriptions govern the labels.

## Files

- `case.json`: standard `{name, source, target, answers}` format.
- `schemas.json`: independently extracted physical declared names/types and published illustration values.
- `contexts.json`: independently extracted official table/field descriptions, ETL conventions and declared FK/domain metadata. No source-to-target answer links are encoded in it.
- `sample_evidence.json`: exact provenance for every included vocabulary illustration.
- `labels.json`, `evidence.json`, `hard_negatives.json`: sealed semantic labels, quotations/citations for every source field, and the 27 specifically reviewed target confounders.
- `provenance.json`: official publisher URLs, pinned commits/tags, retrieval date, full-source hashes/byte counts, source-specific licenses, translations and publisher discrepancies.
- `raw/`: precise source DDL/documentation and parsed target CSV row excerpts, plus licenses. Full repositories and complete schema CSV archives are not retained.
- `SELECTION_CRITERIA.json` and checksum: criteria frozen before extraction, recording the original criterion hash and runtime-freeze notice.
- `SOURCE_CONTEXT_FROZEN.json`: publisher schema/context/example hashes fixed before this author constructed answer files.
- `extract.py`: source-only extraction; downloads seven pinned files, validates full-source SHA256 hashes, and regenerates schema/context/examples/provenance/excerpts without reading labels or cases.
- `assemble.py`: joins the independently extracted schema to the sealed labels, validating full source coverage and actual target references.
- `FROZEN.json`: final SHA256 manifest for every retained file except itself.

Both complete official PostgreSQL definitions and the target CSV contain 17 fields, independently checked by a source-only reviewer. That reviewer supports all 17 intended counterparts and 27 explicitly checked target confounders, with no unresolved role ambiguity. Extraction reproduction was verified in a separate directory using exact-byte comparisons of all generated schema, context, example, provenance and excerpt artifacts. Fieldkin was not run.

Reproduce publisher input using Python's standard library:

```sh
python extract.py --output-dir /tmp/ohdsi-new-reproduction
cmp schemas.json /tmp/ohdsi-new-reproduction/schemas.json
cmp contexts.json /tmp/ohdsi-new-reproduction/contexts.json
cmp sample_evidence.json /tmp/ohdsi-new-reproduction/sample_evidence.json
```

Reassemble the standard case from existing frozen input:

```sh
python assemble.py
```

Identifiers preserve each literal declared table/field spelling: source `visit_occurrence`, target `VISIT_OCCURRENCE`. The publisher's `@cdmDatabaseSchema` substitution placeholder is omitted from target paths. PostgreSQL resolves the unquoted table spelling case-insensitively; provenance records that fact. Physical `INTEGER`, `DATE`, `TIMESTAMP` and `VARCHAR(n)` become `integer`, `date`, `timestamp` and `text`; logical documentation types remain separately available in context.

The source documentation publishes a public-domain dedication / CC0-1.0 statement; the target repository LICENSE declares Apache-2.0. Both exact statements are retained. Every upstream URL uses a fixed official OHDSI commit, with the full-file SHA256 and original byte count, so the small excerpts can be regenerated without preserving full archives.

All source selections and labels are final at the package's seal. Do not revise them using predictions. The earlier partially exposed medical packet remains unchanged and is a separate artifact.
