# Held-out healthcare schema cases

Two independently annotated HL7 FHIR version-migration projections, totaling 30
source fields and 27 target fields. These are **publisher illustrative examples,
not empirical real-world clinical observations**. No patient records were fetched.
CMS observations were attempted but the configured network policy denied the
hostname; the recorded fallback is the official HL7 GitHub publisher repositories.

`cases.json` uses Fieldkin EvalCase JSON. Every source field has a match or no_match
answer. No uncertain equivalences were included. These answers are concept-level
migration equivalences, not guarantees of lossless value-level conversion across
every possible input. Target projections are explicitly partial: a no_match label
means the relevant concept is absent from this target projection, even when a full
FHIR R5 resource has an equivalent field. Definitions of those omitted equivalents
are recorded to make the scope reviewable.

`evidence.json` records every source field's ground truth, exact publisher schema
locations and definition quotations, native types, literal-sample provenance,
conversion-map lines, and the semantically different nearby target fields.
`sources.json` records pinned commit URLs, retrieval times, licensing, byte counts,
and SHA-256 for every archived original. `selection-policy.md` was written before
field selection. No matcher source, pre-existing evaluation sets, or predictions
were inspected during sourcing or annotation.

FHIR primitive wire paths are preserved with their resource-name prefix removed;
XML `@value` attributes become scalar samples. Repeated elements are pooled in
publisher XML order. For each selected leaf, keep the first three distinct literal
values encountered in lexicographically sorted example filenames; never generate
or alter sample values. The archive retains every file contributing a sample.
Selection uses documentation and semantics, not matcher results. Conversion maps
come from the official HL7 FHIR Cross-Version Mapping Pack, pinned independently
from the pinned R4 and R5 schema/example revisions.

Native `dateTime` is normalized to Fieldkin `timestamp`, `decimal` to `decimal`, and
`string`/`code` to `text`. Native types remain in evidence. Partial dates are valid
FHIR dateTime values and are preserved literally.

Rebuild and verify offline from this directory:

```sh
python3 archive_sources.py
python3 build_cases.py
```

`archive_sources.py --fetch` can restore missing source bytes using pinned URLs;
it validates each download before writing and never overwrites existing archives.
The local `upstream-*` exploration clones are not part of this corpus and need not
be copied. Deliver `archive/`, `cases.json`, `evidence.json`, `sources.json`, the two
Python scripts, this README, selection policy, and `freeze.json` only.

HL7 publishes the FHIR specification under CC0; the mapping ImplementationGuide
explicitly declares CC0-1.0. Archived license pages preserve the qualification
that third-party terminology rights are not included. Full publisher examples
may contain such terms. The retained evaluation fields do not select external
terminology coding/display fields.
