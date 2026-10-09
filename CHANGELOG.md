# Changelog

## Unreleased

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
