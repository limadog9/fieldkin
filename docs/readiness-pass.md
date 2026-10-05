# October 5, 2026 candidate readiness pass

**Release decision: not qualified.** This pass preserves the release gates of
at least 95% proposal precision, 60% unique coverage and 90% candidate recall@5,
with nonzero proposals, both assignment modes and every required variant.
Development and examined regression results cannot establish a new qualification.
The [first failed prospective qualification](release-readiness.md) remains archived.

## Changes and macOS root cause

The clean starting checkout was the latest `main`,
`55b614f99067eabb1efc832992bfa8080db7e9e9`. No newer changes had landed. Work uses
the feature branch `readiness/bounded-normalization-ci`; version remains 0.1.0.

The inspected [main CI run](https://github.com/limadog9/fieldkin/actions/runs/37297803362)
passed Linux and Windows. Its macOS job failed exactly three native tooling tests:
checked executable paths, recursive input inventory and persisted qualification
records. The latter two errors explicitly report the symlink/junction path guard;
the executable test failed its expected-success assertion. All three fixtures
used `std::env::temp_dir()` directly. macOS commonly exposes the temporary path
through `/var` to `/private/var`, while `checked_path` correctly rejects symlink
ancestors before production canonicalization.

Only trusted **test** temporary roots now canonicalize. Cleanup stores and compares
against that same canonical root. Production guards and their negative tests are
unchanged. A Unix regression creates a macOS-shaped symlinked ancestor, verifies
the raw executable path is rejected, verifies a fixture beneath the resolved root
works, and checks cleanup preserves unrelated ancestor contents.

Contextual matching now shares a small word-form layer with the built-in name
scorer. Ordinary defaults retain their complete reports. Temporal evidence permits
`expiry`/`expiration`; temporal or Boolean evidence permits `settled`/`settlement`
and `reversed`/`reversal`. These token rules compose with entity prefixes rather
than recognizing complete benchmark names. Structural `is`/`flag` and temporal
suffixes are removed only in their supported type scope. Custom one-pass name
aliases are shared with contextual cores, including a caller-cleared alias map.

Full canonical cores preserve explicit entity, event and Boolean predicate scope.
Directional roles retain token order. Negation and estimated/actual modifiers
remain evidence rather than disappearing through samples. `enabled` remains a
meaningful distinction from `active`, but bare `enabled` is generic and cannot
gain unsupported certainty from low-cardinality Boolean samples. Explicit Date
and Timestamp declarations cannot silently become a representation conversion.
Existing units, currencies, semantic hints, identifier contrast and sample
adequacy checks remain in force. Adversaries include received/dispatched events,
created/updated events, consent scopes, reversed polarity, swapped transfer
directions, qualifiers, identifier entities and missing-evidence opaque fields.

## Same-input regression and evidence boundaries

The baseline library is built from the starting commit in an isolated checkout.
Both revisions run the same regression evaluator and configurations on identical
observable schemas. A separate scorer fingerprint binds labels and family/domain/
variant identities; answers never enter the matcher. The report rejects changed
inputs, protocols, settings, missing/extra cases and any complete-report drift in
the unchanged default or fixed name-only comparators.

The original development corpus retains 32 families, 160 cases, 960 decisions and
730 unique labels. The extension retains 12 families, 32 decisions and 20 unique
labels, with caller hints stripped. Corrective development retains 12 families,
36 cases, 216 decisions and 141 unique labels. The previously examined contextual
qualification retains all 12 families, 60 cases, 360 decisions, 210 unique,
90 unmatched and 60 ambiguous labels. All five correlated variants and both
assignments remain included where defined. These datasets are reported separately.

The [before report](../evaluation/results/readiness-pass-v1/before/report.md)
and [after report](../evaluation/results/readiness-pass-v1/after/report.md) carry precision, coverage, recall@5, wrong proposals,
abstentions, per-family/domain/variant slices and retained/lost/recovered correct
decisions. Changes also record selected source/target names for inspection.
`introduced_wrong` counts a source that was previously correct or abstained and
now proposes incorrectly; a wrong-target change remains visible in the changes
artifact even if that source was already wrong.

| Corpus | Assignment | Before correct/proposed | After correct/proposed | After precision | Coverage before → after | Abstentions before → after | Retained | Lost | Recovered | New wrong |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Original development | Independent | 470/470 | 407/407 | 100% | 64.38% → 55.75% | 490 → 553 | 402 | 68 | 5 | 0 |
| Original development | One-to-one | 465/465 | 402/402 | 100% | 63.70% → 55.07% | 495 → 558 | 397 | 68 | 5 | 0 |
| Extension without hints | Independent | 13/13 | 13/13 | 100% | 65% → 65% | 19 → 19 | 13 | 0 | 0 | 0 |
| Extension without hints | One-to-one | 12/12 | 12/12 | 100% | 60% → 60% | 20 → 20 | 12 | 0 | 0 | 0 |
| Corrective development | Independent | 93/96 | 63/66 | 95.45% | 65.96% → 44.68% | 120 → 150 | 63 | 30 | 0 | 0 |
| Corrective development | One-to-one | 90/93 | 60/63 | 95.24% | 63.83% → 42.55% | 123 → 153 | 60 | 30 | 0 | 0 |
| Examined qualification | Independent | 111/111 | 78/78 | 100% | 52.86% → 37.14% | 249 → 282 | 63 | 48 | 15 | 0 |
| Examined qualification | One-to-one | 103/103 | 74/74 | 100% | 49.05% → 35.24% | 257 → 286 | 59 | 44 | 15 | 0 |

Candidate recall@5 is 100% in every candidate row before and after. The extension
still meets all three examined-data gates in both modes; original and corrective
development now fail coverage in both modes. Original development needs 438 unique
proposals for 60% coverage (gaps 31/36); corrective needs 85 (gaps 22/25). The
examined qualification needs 126 (gaps 48/52). These acceptance thresholds were
not changed. All 90 unmatched and 60 ambiguous examined-qualification decisions
remain safely abstained. Three existing corrective errors remain in each mode;
precision falls from 96.88%/96.77% to 95.45%/95.24% because fewer correct proposals
remain alongside those same errors. No new wrong proposal is introduced.

**Supported lexical recovery improves; net coverage does not.** The original five
recoveries are `expiration_date` → `expiry_on` across every variant, including
no samples. The examined set recovers the three investigated word-form roles
across all five variants. Prefix/type and adversarial tests establish broader
composition than those full field names, but the small correlated counts cannot
establish general accuracy.

The scope safeguards deliberately lose useful sampled relationships such as
`account_ref` → `external_account_id`, `employee_code` → `worker_id`,
`SKU_Code` → `product_id`, `permit_ref` → `application_id` and
`billing_month` → `period_month`. Distinct copied values cannot justify deleting
explicit roles, qualifiers or entity distinctions. The stricter identifier guard
also loses a sampled opaque `x01` → `entry_id` pair. These are real costs, not
reclassified annotations. The three retained errors are `session_start` →
`session_start` across corrective base/reordered/null-heavy variants: identical
Timestamp names and samples conceal instrument-session startup versus frame
acquisition. The supplied inputs lack that business distinction. The
[change inventory](../evaluation/results/readiness-pass-v1/after/changes.jsonl)
and [complete predictions](../evaluation/results/readiness-pass-v1/after/predictions.jsonl)
retain every selection change and unchanged error.

Development evaluation briefly exposed unsupported bare `enabled` proposals after
it stopped being stripped as structural syntax. Classifying it as generic while
retaining its semantic distinction repaired those development errors before the
final freeze. Examined qualification results were not used for a subsequent
policy adjustment. The final source, configuration and protocols stayed fixed
through evaluation and the invariant campaign.

Coverage counts every proposal on a uniquely labeled source, including a wrong
target; correct recall is reported separately. Precision includes unsafe
unmatched/ambiguous proposals. Ambiguous labels require abstention even on a
plausible target. Zero-proposal precision is undefined. Candidate recall includes
ineligible displayed alternatives, so it cannot establish proposal safety.

No new independently authored qualification is claimed. The implementation and
all synthetic regression evidence were available in this development environment;
another pass over the known qualification answers would manufacture independence.
The reserved 218-table T2D holdout and corrective reserved partitions remain
unscored. T2D positive-only annotations cannot certify precision or unmatched-field
safety. No real consumer, adoption, business adjudication, unit conversion or
production guarantee is claimed.

## Validation and packaging

The final source passed the following local Windows checks with Rust/Cargo 1.99.0:

- Workspace all-features tests (344 passed), library no-default-features tests
  (209 passed), the `scale_cost`
  example tests, standalone performance/qualification tests and all three format
  checks. One existing manual-cost test remains ignored; no test was disabled.
- Workspace and both standalone clippy suites with warnings denied; strict JSON
  rustdoc; five workflow examples; the 1,024-case assignment compatibility check.
- All eight freshly generated evaluation snapshots passed exact `--check`, as did
  the ninth same-input regression snapshot. The default and fixed name-only full
  reports stayed identical to the baseline library across all four corpora. The
  historical V3 prototype also matches every archived development full report.
- Offline native Northix/T2D importer checks; a separately recorded evaluator
  [build](../qualification/results/readiness-pass-v1/verified-external/build.json)
  and [guarded development run](../qualification/results/readiness-pass-v1/verified-external/run.json),
  with no reserved scoring. The CI-sized generated campaign also passed 5,000 cases.
- The native [generated campaign](../qualification/results/readiness-pass-v1/campaign/run.json)
  passed 1,000,000 cases at seed 20261003 across seven invariant categories. Its
  [build record](../qualification/results/readiness-pass-v1/campaign/build.json)
  binds the actual working-tree source map, including the new semantic module,
  compiler and executable. Its Git revision is the pre-commit base; it is not
  rewritten to imply that the later documentation commit existed at build time.
- Frozen-source working-tree Cargo package verification and Cargo publish `--dry-run` passed with empty
  default features and with JSON. Both isolated extracted-archive consumers
  passed; license/notice checks and 37 rendered README link targets passed.

Generation and exact verification used the documented `cargo +stable run --locked
--release -p fieldkin-eval` modes for baseline, `--stage3`, `--release`,
`--corrective`, `--precision`, `--readiness`, `--external` and `--northix`.
The new comparison uses:

```text
cargo +stable run --locked --release -p fieldkin-eval -- --readiness-regression --before evaluation/results/readiness-pass-v1/before --check --output evaluation/results/readiness-pass-v1/after
cargo +stable run --locked --offline -p fieldkin-tools -- qualify-build --output target/readiness-final-campaign
cargo +stable run --locked --offline -p fieldkin-tools -- qualify-run --output target/readiness-final-campaign --cases 1000000 --seed 20261003
```

Remote results for the final commit are recorded in the draft PR, separately from
local checks and the historical run above. Historical evidence files are preserved;
CI uses fresh strict snapshots under `evaluation/results/readiness-pass-v1/current`
and the same-input regression snapshot under `readiness-pass-v1/after`.
Clean-commit package and publish dry-run checks are performed after the commit
and recorded in that PR; they are not inferred from the `--allow-dirty` checks.

[Package and documentation checks](readiness-package-checks.md) detail the default
and optional JSON archives, Cargo publish dry runs, standalone archive consumers,
license/notice checks, compiler policy and rendered rustdoc links. docs.rs now
enables the optional JSON feature. README repository links use absolute URLs that
survive inclusion in rustdoc. Publication, tags, release timing and merging remain
the maintainer's decisions.
