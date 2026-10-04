# JSON reports and persisted review

Enable `fieldkin = { path = "../fieldkin", features = ["json"] }` to use the
optional `fieldkin::json` module. It adds Serde and serde_json only to builds that
enable the feature. Default builds keep the existing matching dependency graph.
The library still performs no filesystem or network I/O. These documents are an
experimental versioned boundary; the library remains unpublished.

The [persisted-review example](../examples/persisted_review.rs) demonstrates a
concrete reference importer: export an ambiguous report, record an explicit
reviewer choice, save bytes in application-owned storage, then resume review after
reordering schemas. It rejects a stale application revision before matching.
Run `cargo run --locked --features json --example persisted_review`.
This is a synthetic reference consumer, not evidence of independent adoption.

## Two distinct document kinds

`report_to_json(&report, &ReportJsonOptions::default())` returns UTF-8 bytes with
`"format":"fieldkin.report"`, `"version":1`, `"text_included":false`, and a
`report` object. There is no JSON import into `MatchReport`, and a report document
cannot be loaded as review instructions. Exported decisions describe the supplied
report; they do not authenticate its origin or authorize a migration.

The report object preserves source IDs, ordered candidates, the separately
selected candidate (including selections outside displayed top-k), decisions,
ambiguity alternatives, unmatched IDs, typed reasons, target competition and all
global diagnostic status/work/objective/witness fields. Scores remain heuristic
scores. A zero-score, automatically ineligible selection can still be explicitly
`confirmed`. `budget_exhausted` stays distinct from `complete`.

Decision/status/reason codes use lowercase snake_case. Semantic reasons contain
`code` and `axis`; source competition/displacement reasons contain `code` and
`target`. Missing selections, scores and objectives use JSON `null`. Arrays
retain supplied order. Engine reports already have deterministic stable-ID order;
the exporter does not reorder caller-mutated reports. Numbers are JSON numbers;
consumers must preserve their needed integer/floating-point range. Integer work
counts can exceed JavaScript's exact integer range if callers raise budgets or
construct their own reports.

`review_to_json(&constraints, context, &limits)` creates this separate format:

```json
{"format":"fieldkin.review","version":1,"context":{"source_revision":"import-v4","target_revision":"catalog-v9"},"confirmed":[{"source":"total","target":"gross"}],"forbidden":[],"unmatched_sources":[]}
```

All fields are required. Unknown fields and duplicate object keys are rejected.
The three directive lists preserve order and repeated identical entries. Raw
entries count toward budgets; nothing is deduplicated or implicitly accepted.

`review_from_json(&bytes, expected_context, &limits)` returns `MatchConstraints`
only after the format, version, expected revision context and JSON limits pass.
Pass those constraints to `match_schemas_with_constraints`: the engine remains
the authority for known IDs, conflicting directives, declared types, semantic
hints and all original matching budgets. Export/import deliberately does not
perform schema validation or invoke matcher callbacks. A review document can
therefore parse successfully and subsequently fail normal matching validation.

## Revisions and the trust boundary

The application supplies `ReviewContext` from its own trusted state, separately
from a submitted document. Both revisions must be nonempty. Advance them when
field meaning, metadata or matching policy changes enough to invalidate review.
Do not read the document's revisions and pass them back as the expected context;
that would remove the stale-review check.

Equality of revision strings is not authentication, a content fingerprint or
proof that a human approved a pair. The application must authorize reviewers and
own persistence. A user with permission to submit review decisions can select a
different pair within normal engine constraints. IDs reused for a different
meaning remain undetectable if the application incorrectly reuses its revision.
The API creates no accounts, storage, revision registry or background process.

## Privacy and limits

No `Schema`, `Field`, `SampleValue` or semantic-label serialization is provided.
Default report export omits arbitrary matcher names, explanation strings and
warning strings. Signal order, weight and optional numeric score remain visible.
Set `ReportJsonOptions::include_text` only after reviewing extension output: the
library cannot identify private values embedded in text. Stable IDs and review
revision strings are intentionally visible application metadata and can themselves
be sensitive if the application puts sensitive material there.

`JsonLimits` defaults to 8 MiB of input/output bytes, 262,144 visited record/string
items and 4,096 UTF-8 bytes per accepted/emitted caller string. Limits are separate
from matching limits and configurable explicitly. Review contexts and IDs must be
nonempty; report IDs must be nonempty. Report scores and signal weights must be
finite and in `[0,1]`; objectives/gaps must be finite and nonnegative. The exporter
does not certify other cross-field consistency in a caller-mutated public report.

Input bytes are checked before parsing. The normal serde_json recursion guard
remains enabled; the accepted review format has a shallow fixed structure.
Item/string checks run after parsing. The input byte cap bounds possible parsing
work and allocations, but is not an exact peak-memory cap. Decoding and converting
owned strings/vectors can use more memory than the input size. Output uses borrowed
adapters and a bounded byte sink, without building a second JSON value tree.
It returns a complete byte vector or an error; no partial document escapes.
Skipped free-form report text is neither inspected nor copied.

`JsonError` contains typed categories and never retains parser messages, supplied
IDs, revision strings or other payload text. Invalid documents, unsupported
formats/versions, context mismatches and byte/item/string failures remain distinct.
An application can inspect these categories without logging private input.

The feature does not change matching scores, assignment, review constraints,
sampling or engine defaults. Its tests exercise privacy sentinels, malformed and
oversized documents, explicit zero-score confirmations, stale revisions,
reordering, full diagnostic witnesses and retained raw-directive budgets.
