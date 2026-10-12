# Source-only pre-freeze audit

No matcher implementation, decision policy, old evaluation output, runtime-agent
messages, or predictions were accessed. The first pass checked each positive
against the official conversion map plus both schema definitions; each negative
was checked against its omitted R5 equivalent and every retained target concept.
All 90 retained literal sample origins and their primitive lexical types were
independently checked against the archived publisher XML.

A separate source-only audit identified an evidence-extraction defect before the
initial freeze: rich-text runs in the R4 resource spreadsheet joined header text
without spaces (for example, `Type` + `Data type...`). The parser initially failed
to populate some resource-level type/comment metadata. Header normalization was
corrected to recognize the official header prefixes, and evidence was rebuilt.
This changed documentation metadata only; case inputs and answer labels were
unchanged. All retained source documentation now includes its native schema type.

The migration's as-needed boolean branch is compared specifically as a boolean;
the distinct coded precondition branch is not mislabeled as its equivalent.
Quantities remain scoped to the same dose/fill/maximum role. Integer refill counts
exclude the original dispense in both versions. No population-total, role, or
aggregation transformation is silently treated as a rename.

The independent auditor additionally verified all 50 archived files byte-for-byte
against pinned upstream Git objects, all sample/origin selections against the full
38 R4 and 43 R5 numbered-example populations, all 62 unique quoted schema rows,
and every literal mapping quote/line. The reference-wrapper mappings explicitly
cite the `Reference2CodeableReference` assignments, alongside primitive leaf rules.
The independent semantic review found no label defects or changed quantities.
