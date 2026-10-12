# Blind financial publisher-schema cases

This corpus was sourced and annotated without inspecting the fieldkin matching
implementation, existing evaluation fixtures, or any predictions. The selection
policy was recorded before source retrieval in `SELECTION_POLICY.md`.

All financial cases are **schema-only missing-evidence controls**. Every sample
array is empty; there are no real observations or generated example records.
Official schema/type declarations and an official concept crosswalk support the
fields and labels. Unknown storage types remain `unknown`.

The cases use explicit projections of publisher schemas. A `no_match` label
means that no equivalent column exists **within the target projection**, even
when a counterpart exists in the full publisher schema. Projection exclusions
are documented, so a superficially similar field can represent a different
financial quantity, event, representation, or entity role.

- Stripe Plan to recurring Price: the publisher explicitly documents replacement
  and backwards compatibility. Names preserve original property paths, with `.`
  for nested fields and `[]` for array elements. Only one representation of each
  retained base or tier per-unit quantity is included. Alternate decimal-string
  representations were excluded before predictions because their possible
  fractional precision prevents an unconditional equivalence decision from
  schema-only evidence; they are not treated as distinct economic facts.
- Treasury USAspending assistance and contract downloads to documented database
  download fields: the official crosswalk maps exact CSV and database field
  names to named concepts and definitions. Current publisher compatibility-model
  declarations establish target types. The crosswalk does not declare source CSV
  storage types, so those are `unknown`.

Sources are immutable GitHub revisions. The Stripe archive retains complete
component definitions relevant to the case, plus publisher README and MIT
license. Treasury retains the original crosswalk workbook, relevant publisher
model declarations, custom type declaration, and CC0 license. Manifests give
resource URLs, revisions, SHA-256 checksums, field-level evidence, and limitations.
No upstream test fixtures are included.

Regenerate with Python 3, no third-party dependencies:

```
python3 extract_stripe.py
python3 treasury/extract_cases.py
```

`extract_stripe.py --download` also fetches the original immutable OpenAPI and
verifies its full checksum before rebuilding the archived excerpt. Default
regeneration uses only the archived resources. Treasury regeneration parses XML
and Python AST as data; it never imports or executes publisher code.

`financial_cases.frozen.json` is the combined EvalCase array. `FROZEN.json`
records the final pre-prediction snapshot and checksums. Reviewer annotations in
`ANNOTATION_REVIEW.md` describe source-only checks performed before release.
`ANNOTATION_CHANGES.json` records the source-only correction between the initial
and final freezes; superseded case files are retained outside the active bundle
only for audit.
