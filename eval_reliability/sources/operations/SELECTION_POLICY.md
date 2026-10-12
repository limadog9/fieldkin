# Operational holdout selection policy

Recorded before source inspection or matcher access.

1. Select 2–4 pairs of real, official public transport/logistics/operations schema revisions or interoperable publisher specifications. Prefer changed concepts or version migrations over easy exact-name copies.
2. Preserve publisher field names and declared types. Extract samples only from publisher-provided examples or actual public records; never invent values, hand-rename columns, or fabricate missing evidence. Identify examples honestly. Empty sample arrays are permitted where publisher examples do not cover a field.
3. Use explicit, documented projections of relevant records, aiming for 15–40 fields per case. Flatten nested structures only with mechanically reproducible original key paths, identifying every projection.
4. Freeze matching labels from specification prose, migration notes, and schema definitions before any predictions. Renamed positives require concept continuity; identical spelling alone is insufficient. Missing concepts are no-match. Ambiguity requires documented multiple valid targets.
5. Archive source bytes and evidence with URLs, revision, retrieval time, license and SHA-256. Provide a Python standard-library-only extraction script and per-field evidence.
6. No repository changes, matcher code inspection, prediction runs, or existing evaluation-data inspection. Do not disclose labels or field names to the implementation agent before implementation freeze.
7. Access failures and unsupported claims are recorded honestly. Do not bypass network restrictions.
