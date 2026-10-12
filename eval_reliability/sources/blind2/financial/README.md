# Second blind financial tranche

One new contract-subaward reporting case, sourced only from the U.S. Treasury's
published USAspending crosswalk and declared database schema. It contains 27
source fields and 27 target fields, with an answer for every selected source.
No source field name or answer pair repeats the first financial tranche.

This is a **schema-only** case: samples are empty. Source CSV types remain
`unknown`; target types preserve the publisher's explicit `TEXT` storage
declarations even for dates and monetary amounts. No observations are invented.

Positive correspondences use the same named concept row in the official
crosswalk. Negative labels apply only to the explicit projection: exact
counterparts are omitted while fields for different recipient roles, reporting
identifiers, address purposes, and financial quantities remain. Alternate
representations are not treated as distinct facts merely because storage or
format differs.

The source-selection policy preceded second-tranche selection. Sourcing and
annotation did not access matcher implementation, proposed runtime policy,
evaluation reports, predictions, or prior runtime conversations. Primary-source
archives from the first tranche are reused; its results are not consulted.

`manifest.json` and `resources.json` record licensing, pinned URLs, acquisition
times and checksums. `evidence.json` gives every field's concept definition,
crosswalk cells, declared type, and label rationale. `raw/` contains the original
workbook, publisher type declarations, and full CC0 license. `FROZEN.json` records
the completed pre-prediction snapshot.

Run `python3 extract.py` offline to regenerate exactly:

- `financial_cases.frozen.json`
- `evidence.json`

Extraction uses the Python standard library. Publisher Python is parsed as data
using AST, never imported or executed. Copied-tree regeneration and field/label
integrity are checked before the final freeze.
