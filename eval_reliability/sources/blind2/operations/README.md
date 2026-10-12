# Second blind tranche: one operational case

This package contains one new GBFS station-status migration case, distinct from
the earlier vehicle-status, geofencing, and shipping cases. The source includes
all 16 scalar leaves of the complete publisher endpoint. The target is an explicit
13-field partial consumer projection. There are 13 matches (two changed field
names) and three no-match labels; no ambiguity is invented.

Every field has literal publisher example values and an archived declared type.
The examples are illustrative specification JSON, not observed production data.
Dotted names retain publisher keys; `[]` mechanically traverses an original array.
Sample values are represented as strings required by EvalCase, with no invented
values, hand-renamed fields, or repaired publisher data.

`case.json` has the existing EvalCase shape. `evidence.json` contains every field's
schema pointer, raw declaration, ancestor descriptions, documentation lines,
sample origin, and answer rationale. `provenance.json` records immutable URLs,
commit dates, license references, and SHA-256 hashes. Six original resources are
archived: both full specifications, both endpoint schemas, and the CC-BY-3.0 and
Apache-2.0 license texts. The existing pinned source archive was reused; the prior
tranche did not evaluate this endpoint.

The source-only audit found an inconsistent v3.0 sentence for the per-compatible-
dock quantity. Its parent and JSON schema describe docks, but the sentence says
vehicles. That target leaf is excluded; no positive depends on the discrepancy.
The source v2.3 dock count and retained v3.0 vehicle count are unambiguous. Other
no-match controls separate disabled from functional dock points and rental
permission from return permission or street installation. Omitting required
target fields means this partial projection is not itself a valid complete feed.

Matches denote the same concept; POSIX timestamps require conversion to RFC3339.
No claim of value-copy compatibility is made.

Reproduce offline using Python standard library only:

```sh
python3 extract.py
```

The extractor first verifies every source checksum, then rewrites only
`case.json` and `evidence.json`. Repeated extraction was checked byte-for-byte.
`FREEZE.json` hashes all package artifacts and seals the labels before any new
predictions or disclosure to the implementation author.

Independence limit: the author previously audited first-tranche reports and
therefore has seen those results. No second-tranche candidate code, policy, or
predictions were inspected. The selection policy was recorded before selecting
this endpoint; complete source coverage and documentary meanings govern labels.
The second tranche has not been authorized for runtime evaluation.
