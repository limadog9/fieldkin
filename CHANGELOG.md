# Changelog

## 0.1.0 - 2026-10-07

- Initial experimental release for suggesting field correspondences between flat
  schemas.
- Combines field names, declared type compatibility, and optional sample overlap.
- Reports `Match`, `Ambiguous`, or `NoMatch` with heuristic scores for review.
- Prefers exact normalized names with compatible types.
- Uses fuzzy abbreviation matching only as a fallback after `NoMatch`.
