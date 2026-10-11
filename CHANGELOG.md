# Changelog

## Unreleased

- Ignore blank samples in suggestions so missingness cannot create positive
  sample evidence; absent evidence retains the existing weight redistribution.
- Reuse the shared Levenshtein scorer in SimilarityFlooding, removing duplicate
  edit-distance code while preserving exact matching outputs.
- Evaluate suggestions on the verified independent corpus and add a separately
  frozen annual/monthly climate aggregation control with publisher evidence.

- Avoid sampling-index overflow on 32-bit targets for large columns.
- Verify packaged crates in CI and update checkout to its Node.js 24 runtime.
- Clarify when to use decision-oriented suggestions versus Valentine candidate scores.

- Reject unknown schema/field keys in labeled evaluation inputs, keeping public
  schema deserialization compatible.
- Reject impossible DistributionBased quantile allocations before matching.
- Skip Levenshtein comparisons that cannot reach the existing Jaccard cutoff
  based on Unicode character lengths, preserving exact matching outputs.

- Add `try_match_schemas` for recoverable suggestion configuration errors while
  preserving `match_schemas` and its existing behavior.
- Reject duplicate match identifiers and metric names instead of overwriting
  results; validate table identities consistently in direct and batch matching.
- Return errors for out-of-range Polars calendar values, invalid timezones and
  worker creation failures. Valid matching scores, ordering, and defaults are unchanged.
- Clarify validation and optional integration behavior and source-only tooling.

- Add live comparisons across all evaluation datasets, recording input and
  reference source hashes and checking original column identifiers.
- Add two explicit Python reference source fixes for reproducible Cupid WordNet
  ordering and Distribution solver identity preservation, with regression tests.
  Native Rust matching behavior remains unchanged.

## 0.2.0 - 2026-10-08

- Port all five current Valentine matcher families to native Rust, including
  global statistics, WordNet, graph propagation, quantile EMD, integer
  clustering, fuzzy/Tversky set matching and optional ONNX embeddings.
- Add immutable table-aware results, selectors, metrics, CSV/JSON loading,
  optional native Polars and a JSON-output matching CLI.
- Preserve the original suggestion API and evaluation datasets.
- Add pinned Python-reference fixtures, offline tests and cross-platform CI.
- Include Apache-2.0/WordNet licenses, attribution and porting notes.

## 0.1.0 - 2026-10-07

- Initial experimental release for suggesting field correspondences between flat
  schemas.
- Combines field names, declared type compatibility, and optional sample overlap.
- Reports `Match`, `Ambiguous`, or `NoMatch` with heuristic scores for review.
- Prefers exact normalized names with compatible types.
- Uses fuzzy abbreviation matching only as a fallback after `NoMatch`.
