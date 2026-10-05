# Precision development evaluation

This experiment compares configurable name-conflict checks and an explicit
evidence-sufficiency decision with the current weighted default on the same
original development dataset. All library defaults remain unchanged. The new
checks are opt-in, and a higher precision figure must be assessed together with
preserved correct matches and coverage.

## Failure review and comparison dataset

The current Stage 3 default scores 32 original development families: 160 schema
pairs and 960 source decisions, comprising 730 unique-match, 190 no-match and 40
ambiguous labels. Each family has five correlated variants: base, reordered,
without samples, null-heavy and separator noise. These are public synthetic
development scenarios, rather than independent consumer validation.

The current-default reference is the `combined` model at threshold 0.70 in
the [Stage 3 report](../evaluation/results/continuation-v3/stage3/report.json).
It uses distinct-aware sample reliability. The original Stage 1 combined model
uses `Legacy` reliability and is a separate historical comparator; its scores
must not be substituted for today's default.

| Existing policy | Assignment | Correct/proposed | Precision | Unique recall | Unique coverage | Overall coverage | Wrong unique | No-match proposals | Ambiguous proposals |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Current default | Independent | 321/422 | 76.07% | 43.97% | 44.66% | 43.96% | 5 | 96/190 | 0/40 |
| Current default | One-to-one | 313/414 | 75.60% | 42.88% | 43.56% | 43.12% | 5 | 96/190 | 0/40 |
| Existing sample gate | Independent | 195/246 | 79.27% | 26.71% | 27.12% | 25.62% | 3 | 48/190 | 0/40 |
| Strict name/sample gate | Independent | 141/186 | 75.81% | 19.32% | 19.73% | 19.38% | 3 | 42/190 | 0/40 |

The two existing gates in this table are development-only slices of the
[corrective regression predictions](../evaluation/results/continuation-v3/corrective/original_regression-predictions.jsonl),
filtered to exactly the same 32 development families before analysis. The sample
gate requires positive name support and raw distinct-aware sample support of at
least 0.50; the strict ablation additionally requires a name score of 0.80.

The sample gate preserves 195 of the default's 321 correct independent matches,
loses 126 correct matches, removes 50 false proposals and adds no new correct
matches. Lost correct matches comprise 51 without samples, 51 null-heavy and
eight in each of the base, reordered and separator-noise variants. Its precision
gain comes with a substantial coverage loss. The stricter name floor loses
another 54 correct matches while lowering precision, demonstrating why raising
a lexical floor is not a general remedy.

The current default's 101 independent errors comprise 96 no-match proposals and
five wrong unique targets. Observable failures include a pressure field named
`Pressure_kPa` proposed against `pressure_pa`: direct matching must not silently
perform a unit conversion. Other false proposals involve identical names, types
and sampled values with hidden scopes. Examples include unrelated entities using
`ID`, `Name` and `Status`, and different measurements using `Value` and `Count`.
Historical names such as `boot_id` and `event_id` can also hide different facts.

These score-one errors explain the limits of the existing support gate. A name
conflict check can reject a visible unit or qualifier contradiction. It cannot
infer hidden entity scope from otherwise identical observable evidence. Label
concepts and rationales describe that limitation for evaluation; they are never
used to create matcher hints.

## Fixed experiment

The comparison uses four models on identical inputs: current default,
conflicts only, support only and conflicts with support. Every model retains
threshold 0.70, ambiguity margin 0.08, top-k five and both independent and
one-to-one selection. Name/type/sample weights remain 0.65/0.20/0.15, aliases and
type handling are preserved, and sample reliability remains distinct-aware.
The support requirement reuses the existing positive-name/0.50-sample preset.
There is no threshold or weight sweep.

Conflict rules are explicit caller-configured distinctions among qualifier or
unit alternatives. Equivalent spellings may share an alternative; normalized
phrases contain at most four tokens and longer overlapping phrases take
precedence. No scores are boosted or renormalized. A rule may change candidate
eligibility, followed by ordinary ambiguity and assignment recomputation.
`Decision::InsufficientEvidence` distinguishes support-driven abstention when an
otherwise viable edge lacks the required evidence.

The matcher receives only existing field IDs, names, declared types and samples.
No gold label, concept, rationale, family ID or scenario tag reaches matching.
The original development fixtures contain no supplied semantic hints, and this
experiment does not derive hints from their ground truth. Caller confirmations
are excluded from automatic quality counts.

No reserved holdout is scored in this experiment. Original holdout families,
the corrective holdout and reserved external tables are excluded from this
comparison. Settings are evaluated on development data; these results make no
held-out generalization claim. Historical result files are preserved, and new
outputs belong in a separate versioned directory.

## Results

The fixed [protocol](../evaluation/precision-protocol.json) was written before
the first precision run. Its vocabulary distinguishes gross/net, billing/shipping,
minimum/maximum and start/end qualifiers, plus explicit pressure, mass, length,
duration, temperature and currency representations. Configuration was not
changed after results were observed. This vocabulary is experiment configuration;
applications must select appropriate distinctions for their own data.

| Policy | Assignment | Correct/proposed | Precision | Unique coverage | Overall coverage | Preserved correct | Lost correct | Removed false |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Default | Independent | 321/422 | 76.07% | 44.66% | 43.96% | 321 | 0 | 0 |
| Conflicts | Independent | 321/417 | 76.98% | 44.66% | 43.44% | 321 | 0 | 5 |
| Support | Independent | 195/246 | 79.27% | 27.12% | 25.62% | 195 | 126 | 50 |
| Conflicts + support | Independent | 195/246 | 79.27% | 27.12% | 25.62% | 195 | 126 | 50 |
| Default | One-to-one | 313/414 | 75.60% | 43.56% | 43.12% | 313 | 0 | 0 |
| Conflicts | One-to-one | 313/409 | 76.53% | 43.56% | 42.60% | 313 | 0 | 5 |
| Support | One-to-one | 189/240 | 78.75% | 26.30% | 25.00% | 189 | 124 | 50 |
| Conflicts + support | One-to-one | 189/240 | 78.75% | 26.30% | 25.00% | 189 | 124 | 50 |

Every policy adds zero false proposals and zero newly correct matches here.
Conflict checks remove the five variants of the observable pressure-unit
contradiction while preserving every correct default proposal and unique coverage.
The improvement is 0.91 percentage points in independent precision, with five
fewer overall proposals. These are five correlated variants of one family,
not five independent discoveries. Qualifier behavior is additionally exercised
by separately authored unit tests; these corpus results do not establish a
qualifier-driven precision gain.

Conflict checks still leave five wrong unique selections and 91 no-match
proposals in independent mode. Every policy abstains on all 40 ambiguous labels;
some uniquely labeled fields also receive an ambiguity decision. Candidate
recall@5 remains 100% for every policy. The combined policy removes no extra
proposals beyond the support gate on this dataset. It reports 194 explicit
`InsufficientEvidence` abstentions per assignment mode, compared with 199 for
support alone; five otherwise viable candidates now have a unit-conflict reason.

The results support making conflict checks available to callers who know their
name vocabulary. They do not meet the quality targets or justify changing defaults.
The sample-support gate remains an optional coverage tradeoff. Identical generic
names and shared values can still conceal incompatible meanings.

The [machine-readable report](../evaluation/results/precision-v2/report.json)
contains all counts, metrics, transitions, decision counts, the exact protocol,
dataset and source hashes, and the locked dependency identity. The
[readable report](../evaluation/results/precision-v2/report.md) and adjacent
development prediction records provide reproducible evidence. All 320 complete
default reports and aggregate counts are verified against the frozen current
baseline before recording candidate results. Matching receives no gold-derived
hints or review decisions. This Cargo runner records checkout source hashes;
it does not claim independent executable attestation.

```text
cargo +stable run --locked --release -p fieldkin-eval -- --precision --output target/precision-development
cargo +stable run --locked --release -p fieldkin-eval -- --precision --check --output evaluation/results/precision-v2
```

The CLI rejects holdout, threshold and tuning options. Generation refuses to
overwrite different historical artifacts. CI checks the new precision snapshot
with its exact implementation provenance. The original
[precision-v1 snapshot](../evaluation/results/precision-v1/report.json) remains
archived; precision-v2 records the native Rust tooling graph and stable compiler
policy with the same outcomes. Current baseline, Stage 3, release-development,
corrective-development, external and Northix checks use exact new snapshots in
`evaluation/results/rust-native-v1/`. Historical behavior and legacy-decision
projection modes remain explicitly named compatibility tools, rather than
exceptions to the current snapshots. The full original default-report digests
are still checked before each precision or readiness report.

## Metrics and acceptance

Proposal precision is correct unique-target selections divided by every
selection, including errors on no-match and ambiguous labels. Unique coverage
is any selection on a unique-match label divided by all 730 such labels;
wrong-target selections count as coverage. Unique recall counts only correct
selections. Overall coverage is all proposals divided by 960 source decisions.
An ambiguous label requires abstention, even if one of its plausible targets is
selected. Undefined precision for zero proposals is reported as undefined.

Report all three false-proposal categories and correct-match transitions:
preserved correct matches, rejected correct matches, newly correct matches,
removed false proposals and newly false proposals. Name conflicts and support
requirements can change ambiguity or assignment, so proposal sets must be
compared as actual matcher outcomes rather than postfilters over old results.

Candidate recall@5 includes ineligible ranked alternatives. The current default
achieves 100% on this dataset with only five to nine targets per schema; that
retrieval result does not establish proposal precision. One-to-one mode has a
715-distinct-target ceiling for the 730 unique source facts, so report raw recall
alongside assignment-adjusted recall.

The existing targets remain at least 95% proposal precision, 60% unique coverage
and 90% candidate recall@5, with nonzero proposals. Empty output or improved
precision accompanied by unreported lost matches does not meet this bar.
Synthetic development results alone do not justify promoting the opt-in checks
to library defaults. See [the API guide](precision.md) for configuration and
review semantics.

## Validation

Local validation used Rust 1.85.0 on Windows x86_64. All 263 workspace tests,
including doctests, passed; one existing manual performance test remains ignored.
The feature-disabled library tests also passed. Thirteen new matcher regressions
cover qualifier/unit disagreements, useful aliases and synonyms, missing support,
ambiguity, hidden candidates beyond top-k, caller review, assignment and JSON.
Evaluator tests check baseline report equality, answer-metadata isolation,
holdout/tuning rejection and the narrow compatibility modes.

Formatting, strict workspace clippy and warning-free documentation passed.
All seven baseline/current-policy snapshot commands configured in CI passed
locally, including the exact new precision snapshot. Original datasets and
historical result artifacts were not rewritten. No new performance or
cross-platform validation claim is made.
