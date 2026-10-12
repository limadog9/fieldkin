# DCSA equipment event schema revision holdout

This case compares the official DCSA Track and Trace 2.2.0 schema with the
publisher's **3.0.0 work-in-progress draft**, as archived in DCSA's own
Conformance-Gateway repository. The draft explicitly states that it is not ready
for general adoption (new schema lines 20–22). This is a real published draft
schema revision, not a claim about a released production API.

Official repository: https://github.com/dcsaorg/Conformance-Gateway

Pinned revision: `ffa064b01a452ab4fcd34e08086b53cf41635aa6` (dev head observed at
retrieval; immutable raw-file URLs, retrieval times, and SHA-256 values are in
`manifest.json`). The repository LICENSE and both OpenAPI documents identify
Apache 2.0. The archived LICENSE applies to these official repository sources.

## Selection and projection

The pre-existing `/tmp/fieldkin-blind-operations/SELECTION_POLICY.md` was read
before source inspection. No repository implementation, matcher, predictions,
or existing evaluation cases were accessed. Definitions and labels were frozen
before any prediction, as recorded in `FROZEN.json`.

Projection: equipment-event scalar leaves for event identity/time/classification,
container identifiers, event location/address, transport-call identifiers, vessel
identifiers, and seals. We preserve literal publisher object keys and represent
nested keys mechanically with dots and original array items with `[]`.
Equipment events are selected by old `eventType == EQUIPMENT` and new
`eventClassification.eventType == EQUIPMENT`. Full source/target schemas and
publisher fixtures remain archived. This projection is explicit and intentionally
omits unrelated shipment and reefer structures and many optional properties.

There are 34 source fields, 36 target fields, 31 positive correspondences, and 3
no-match labels. No-match means no equivalent in this target projection. Three
negative source concepts are technical creation time, address name, and the
transport-call facility's free-text fallback. The separately modeled equipment
event location does not identify the transport call's facility. Do not conflate
those two source locations merely because examples sometimes share values.

Target-only distractors include technical update time, event retraction, a
universal service reference, a universal export voyage reference, and vessel MMSI.
They are drawn from the original target schema with their original definitions.
No field has been hand-renamed to create a benchmark alias.

## Labels and samples

Every label includes a semantic rationale in `field_evidence.json`; each field
includes the original type, original definition text, component/property locator,
line range, and sample provenance. Correspondence is based on concept continuity,
including direction-bearing parent properties and explicit enum definitions,
not spelling alone. Examples include code vocabulary expansion and relocation of
transport and equipment details into nested groups.

Samples are publisher-authored **conformance fixtures and schema examples**, not
observations claimed to come from a real shipping operation. Only the success
fixtures are used; deliberately invalid publisher fixtures were archived during
source discovery but are excluded by the extractor. V3 fixtures contain literal
`EQUIPMENT_REFERENCE_PLACEHOLDER` values; these are preserved and flagged here.
The source seal example `string` is also preserved literally. Do not interpret
these as production samples.

The extractor first uses values from equipment-event success fixtures, preserving
original strings and canonicalizing primitive nonstrings to sample strings. If
none cover a field, it uses that field's publisher schema example, if present.
Otherwise its samples array is empty. The evidence distinguishes these cases.
Generic schema examples can be reused in distinct roles (e.g. import and export
voyage numbers), so role continuity must come from the parent-key definitions,
not these incidental equal values.

## Reproduction

Run `python /tmp/fieldkin-blind-operations/scan/dcsa_tnt/extract_case.py`.
It uses only Python's standard library and the pinned archived bytes. No network,
repository, or matcher access is required. It verifies every archived source hash
before rebuilding `case.json` and `field_evidence.json`, and checks field counts
and name uniqueness. The YAML reader deliberately scans only the explicitly
selected scalar nodes of these pinned OpenAPI files and refuses unsupported
schema/type shapes; it is not a general-purpose YAML parser.

`case.json` follows the requested EvalCase shape and normalized type vocabulary.
`field_evidence.json` retains original declared types and formats so this
normalization is transparent. `FROZEN.json` records hashes of the case, extractor,
field evidence, source manifest, and this note at the pre-prediction freeze.
