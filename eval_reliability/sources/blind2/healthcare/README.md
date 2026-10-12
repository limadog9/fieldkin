# Blind healthcare validation tranche 2

One new HL7 FHIR MedicationRequest R4-to-R5 schema-migration projection: 24 source
fields and 18 target fields. Inputs are **publisher illustrative examples**, not
real-world clinical observations. This is a resource different from the first
healthcare tranche; no previous case or answer set was reused.

`cases.json` is a list containing one Fieldkin EvalCase. `evidence.json` documents
every label, source/target schema definition, native leaf type, literal sample
origin, explicit official migration-map statements, and contrasting target roles
for each no_match. `sources.json` records original publisher URLs pinned to Git
commits, revisions, retrieval timestamps, licensing, byte counts, and SHA-256.
`archive/` contains the original bytes. `selection-policy.md` was recorded before
field selection. All selected fields and answers are frozen together.

The target schema is an explicitly partial projection. A no_match means that the
source concept is absent from this projection, even if a complete R5 resource has
an equivalent field. The omitted equivalent and all possible target fields were
reviewed from documentation. Roles and scopes remain distinct: a prescribed
supply amount is not a dose amount, a lifetime maximum is not a per-administration
maximum, and writing a prescription is not the same event as its validity window
or administration schedule. No aggregates, narrower measured populations, or
similar-looking values were used to infer equivalence.

Select only numbered publisher MedicationRequest scenario XML files matching
`medicationrequest[0-9]+[a-z]?.xml`. For each projected primitive leaf, pool repeated
elements in XML order and keep the first three distinct literal `@value` strings
from lexicographically sorted scenario filenames. Do not alter or invent samples.
Wire paths retain exact nesting; only the resource-name prefix is omitted.
Native `dateTime` maps to Fieldkin `timestamp`, integer-family primitives to
`integer`, `decimal` and `boolean` to their same-named types, and `string`/`code` to
`text`. Partial FHIR dateTime values remain literal partial dates.

Official cross-version maps support directional concept-preserving migration;
this does not assert equality of every permitted value domain in both directions.
The source-only audit compares the actual selected branch and semantic role, not
just the shared field name or allowed reference entity type.

Run offline:

```sh
python3 archive_sources.py
python3 build_cases.py
```

`archive_sources.py` verifies every original source checksum. Its optional `--fetch`
restores missing originals from pinned URLs after validating their expected bytes.
`build_cases.py` writes only `cases.json` and `evidence.json`, using Python stdlib
and local archives. Byte-for-byte regeneration was checked before freeze.
`freeze.json` records the SHA-256 of every deliverable and the blindness statement.
Neither script imports a matcher or accesses predictions.

HL7 publishes FHIR and the cross-version mapping guide under CC0-1.0. Archived
license pages explicitly retain exclusions for separately licensed third-party
terminologies, including codes/displays in full publisher examples. This corpus
retains those notices and makes no broader licensing claim.
