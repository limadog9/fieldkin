# Blind operational schema corpus

Prepared independently of matcher implementation and predictions. Selection policy
was recorded in `SELECTION_POLICY.md` before inspecting candidate source schemas.
No matcher, evaluation code, or previous predictions were inspected by the corpus
authors. Labels and resource hashes are frozen in `FREEZE.json` before release to
the implementation agent.

## Contents and provenance

Three cases use original published field paths, declared schema types, and
publisher-authored examples. They are **illustrative specification/fixture data,
not live fleet or shipping observations**. No sample values or field renames were
invented. Dotted paths preserve nesting; `[]` is mechanical array traversal.

| Case | Source fields | Target fields | Match | No equivalent | Ambiguous |
| --- | ---: | ---: | ---: | ---: | ---: |
| GBFS vehicle 2.3 → 3.0 partial consumer view | 17 | 15 | 14 | 3 | 0 |
| GBFS geofencing 2.3 → 3.0 rule scopes | 15 | 21 | 12 | 3 | 0 |
| DCSA Track & Trace equipment events 2.2 → 3.0 draft | 34 | 36 | 31 | 3 | 0 |
| Total | 66 | 72 | 57 | 9 | 0 |

The DCSA 3.0 schema is explicitly **WORK IN PROGRESS**, a published draft rather
than a generally available released API. Its official source archive and README
retain that status. Placeholder strings in official fixtures are preserved and
identified as publisher data, never presented as live observations.

`cases/` holds the two GBFS EvalCase objects. `dcsa_tnt/case.json` holds the DCSA
EvalCase. `cases.json` is their combined list. GBFS field evidence is in `evidence/`;
DCSA field evidence is `dcsa_tnt/field_evidence.json`. Evidence includes the raw
schema declarations, source locations, sample origins and answer rationales.

GBFS specifications are pinned to tag commits 4f0cc7024ff5ba71d3c3e861c3ff23a281ef71f8
(2.3) and 457cb71121348ad6c07384b7743ea7df994a4ebb (3.0). The separately maintained
JSON schemas are pinned to 2e974fd51ae1db9c8006c97375d53e0b882cfedf. Full source URLs,
revision dates, retrieval times and SHA-256s are in `gbfs-provenance.json` and the
DCSA bundle's provenance manifest. GBFS documentation is CC-BY-3.0; the JSON
schemas and DCSA materials use Apache-2.0. Original license texts are archived.

## Label semantics and deliberately partial projections

A match identifies the same operational concept; ordinary representation
conversions may be needed (for example POSIX seconds to RFC3339, short enum codes
to expanded spellings, or a localized text wrapper). This is not a promise that
raw values can be copied without conversion.

The GBFS vehicle source is the complete set of scalar leaves of the vehicle
record. The target deliberately omits three counterpart fields and includes the
feed update timestamp. This documents partial-schema behavior with closely related
states and identifiers present, rather than silently relabeling a full schema.

The GBFS geofencing source is every scalar leaf of the endpoint. The target omits
two local rule leaves while retaining the identically named global defaults.
The complete projection lists and reasons are stored in each evidence file.
The old joint start-and-end predicate has no single equivalent among the new
independent predicates; it is labeled no-match, not manufactured ambiguity.

No case supplies a genuine pair of interchangeable target fields, so there are
no ambiguous labels. This corpus does not measure ambiguity handling.

## Sample and type limitations

* Samples are canonical strings from publisher JSON values or explicit schema
  examples; no rows are synthesized or repaired. Empty sample arrays indicate
  absent sample evidence. The GBFS extractor keeps at most eight distinct values
  per field in publisher order.
* The GBFS 2.3 carsharing example contains an undeclared `home_station` key. It is
  ignored rather than corrected; the declared `home_station_id` sample comes
  from another publisher record.
* GBFS equipment item schemas have an enum but no explicit type, retained as
  `unknown`. Physical JSON schema types are normalized to the allowed type names;
  string `date-time` formats become timestamp. Integer timestamp encodings remain
  integer, and the evidence preserves the publisher's semantic definitions.
* The geofencing name maps to localized text content; operational migration also
  needs a locale choice. Geometry coordinates are mechanically flattened numeric
  values, not fabricated longitude and latitude columns.
* DCSA fixture and schema-example samples may cover different equipment events;
  they are field samples, not synthetic joined records. The bundle documents
  which evidence source supplied each value.

## Reproduce

From this directory, run:

```sh
python3 extract_gbfs.py
python3 dcsa_tnt/extract_case.py
python3 assemble.py
```

All scripts use only Python's standard library and the archived resource bytes.
Source checksums are verified before extraction. No network or application
dependency is required. `assemble.py` validates field/label coverage and recreates
`cases.json`; it does not import or invoke the matcher. Verification performed
before freeze: both extraction scripts completed, all declared projection paths
resolved, labels covered every source field exactly once, match targets existed,
and repeated extraction produced byte-identical case and evidence files.
