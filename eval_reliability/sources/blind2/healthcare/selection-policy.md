# Blind healthcare tranche 2 selection policy

Recorded before field selection or prediction exposure. Select at most one resource
migration distinct from the first healthcare tranche. Use only official publisher
schema definitions, conversion maps, and unmodified publisher example records.
Select fields for documented concept-preserving directional migration equivalents,
including renamed wire paths, and semantically distinct no-match roles absent from
an explicitly partial target projection. Include only documented labels; exclude
uncertain equivalences or changed concepts. Use the first three distinct literal
sample values in lexicographically sorted publisher example filenames. Archive
original schema/example/map/license bytes at pinned revisions with SHA-256.
No matcher implementation, decision policy, prior evaluation reports/predictions,
or runtime-agent messages may be inspected. Freeze all selected fields and labels
before exposure. This tranche uses publisher examples, not empirical observations.

Before freeze, publisher-example selection was fixed to numbered scenario files
matching `medicationrequest[0-9]+[a-z]?.xml` at each pinned version, preserving a
consistent scenario family across versions. Auxiliary/legacy named examples are
outside this projection. All chosen primitive paths were checked against their
actual versioned schemas; no predictions or runtime policy informed this choice.
