# Changelog

## Unreleased experimental release candidate

No crate has been published. Final version and publication remain @limadog9's
decision. See the release scorecard for qualification evidence and limitations.

- Added opt-in name/sample corroboration requirements with typed exclusion reasons,
  unchanged default matching behavior and no additional runtime dependencies.
- Added a fresh synthetic corrective evaluation, compact reproducible reports,
  strict source verification before fresh holdout scoring and a compiling example.

- Added bounded optional global alternative analysis with explicit completeness,
  objective gaps and stable-ID mapping witnesses. Diagnostics do not change
  selection and distinguish budget exhaustion from a completed search.
- Added typed candidate/source reasons and target competition records.
- Replaced string-only `MatchError` with typed non-exhaustive errors, preserving
  human-readable messages and suppressing arbitrary matcher error details.
- Added importer and catalog examples with application-owned review decisions.
- Added cross-platform minimum/current compiler CI and deterministic generated
  qualification tooling. Holdout evaluation is explicitly gated and kept out of CI.
- Documented source-breaking changes, usage and deferred serialization/adapters.

## Earlier unpublished stages

- Stage 3: distinct-aware sample reliability; exact integer/decimal samples;
  verified semantic constraints; inspectable aliases and optional profiles.
- Stage 2: per-call evidence preparation, bounded report retention and repeated
  before/after cost measurements.
- Stage 1: synthetic evaluation corpus, family partitions, baseline comparisons
  and reproducible regression artifacts.
- Initial implementation: ranked candidates, normalized names, type/sample
  evidence, local ambiguity and partial one-to-one matching.

Migration instructions: [migration guide](migration.md).
