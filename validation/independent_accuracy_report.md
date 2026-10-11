# Independently sourced Valentine accuracy: measured report

Measured on 2026-10-10. Fifteen cases, **91 source fields: 40 unique matches,
51 no-matches, zero ambiguity labels**. Inputs and annotations were committed
before the first prediction in `637e3f2d6652a197a59dbc4d82e28eb9e7be9139`. Matching library
code is unchanged from `f41c627868a3362ca689eb286c50659ed30e80e4`.

These cases were held out from Fieldkin development, with publisher evidence for
every label. They are public, mostly historical studies and related lookup
tables, not a representative sample of production schemas. Single-annotator
labels and shared dataset families limit the strength of generalization claims.

Reproduce (now includes the separately reported extension below):

```sh
cargo run --locked --example benchmark_accuracy -- --independent
```

Add `--details` for errors or `--json` for full per-dataset results. The committed
[machine-readable output](independent_accuracy_results.json) contains every
metric/count, field diagnostic and input/label fingerprint. The
[protocol](independent_accuracy.md), [manifest](independent_corpus.json) and
[publisher notices](../eval_independent/NOTICE.md) document extraction, ground
truth, versions, sources, licensing and restrictions.

## Automatic selection and ranking are different measurements

Automatic top-1 selection uses the unchanged inclusive **0.5** cutoff. All five
fixed configurations are identical to the development benchmark. Rank-only
top-1/recall@5 use the 40 uniquely matched fields before the cutoff. Coverage
uses all 91 fields. Scores are not calibrated probabilities; the common cutoff
does not express equal confidence across algorithms. No thresholds were tuned.

| Algorithm | Precision | Recall | F1 | Rank top-1 | Recall@5 | Coverage |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| COMA | 84.21% | 80.00% | 82.05% | 82.50% | 82.50% | 41.76% |
| Cupid | 54.29% | 47.50% | 50.67% | 47.50% | 47.50% | 38.46% |
| DistributionBased | 96.43% | 67.50% | 79.41% | 67.50% | 67.50% | 30.77% |
| JaccardDistanceMatcher | 100.00% | 75.00% | 85.71% | 85.00% | 85.00% | 32.97% |
| SimilarityFlooding | 100.00% | 5.00% | 9.52% | 80.00% | 97.50% | 2.20% |

| Algorithm | Correct / automatic | False positives | Missed unique matches | Automatic on no-match / 51 |
| --- | ---: | ---: | ---: | ---: |
| COMA | 32 / 38 | 6 | 8 | 6 / 51 |
| Cupid | 19 / 35 | 16 | 21 | 13 / 51 |
| DistributionBased | 27 / 28 | 1 | 13 | 1 / 51 |
| JaccardDistanceMatcher | 30 / 30 | 0 | 10 | 0 / 51 |
| SimilarityFlooding | 2 / 2 | 0 | 38 | 0 / 51 |

Observed precision of 100% for Jaccard is based on **30** selections; for
SimilarityFlooding it is based on just **2**. Neither is a general guarantee.
Correct no-match abstentions do not inflate precision or ranking accuracy.
There is no ambiguity performance estimate because no trustworthy ambiguity
case was established; the category remains covered by existing metric tests.

## Per-case comparison

Each cell is **correct automatic / automatic selections; rank top-1 / positive
labels**. Zero positive labels indicate an all-negative control; its ranking
accuracy is undefined, not 100%. Full precision, recall, F1, top-1, recall@5 and
coverage for every row/algorithm are in the JSON output.

| Case | COMA | Cupid | DistributionBased | Jaccard | SimilarityFlooding |
| --- | ---: | ---: | ---: | ---: | ---: |
| 01_penguins | 7/7; 7/7 | 5/7; 5/7 | 5/5; 5/7 | 5/5; 5/7 | 0/0; 7/7 |
| 02_aircraft_flights | 1/2; 1/1 | 1/7; 1/1 | 0/0; 0/1 | 0/0; 0/1 | 0/0; 1/1 |
| 03_carriers_flights | 1/1; 1/1 | 1/2; 1/1 | 1/1; 1/1 | 1/1; 1/1 | 0/0; 1/1 |
| 04_weather_flights | 6/7; 6/6 | 5/8; 5/6 | 4/5; 4/6 | 4/4; 5/6 | 0/0; 6/6 |
| 05_aircraft_weather | 0/2; 0/0 | 0/1; 0/0 | 0/0; 0/0 | 0/0; 0/0 | 0/0; 0/0 |
| 06_longley | 5/5; 6/7 | 1/1; 1/7 | 2/2; 2/7 | 5/5; 5/7 | 0/0; 5/7 |
| 07_nile | 0/0; 0/2 | 1/1; 1/2 | 2/2; 2/2 | 2/2; 2/2 | 0/0; 1/2 |
| 08_sunspots | 0/0; 0/2 | 1/1; 1/2 | 0/0; 0/2 | 0/0; 2/2 | 0/0; 1/2 |
| 09_engel | 2/2; 2/2 | 0/0; 0/2 | 2/2; 2/2 | 2/2; 2/2 | 2/2; 2/2 |
| 10_grunfeld | 5/5; 5/5 | 1/1; 1/5 | 5/5; 5/5 | 5/5; 5/5 | 0/0; 5/5 |
| 11_stackloss | 4/4; 4/4 | 1/1; 1/4 | 4/4; 4/4 | 4/4; 4/4 | 0/0; 2/4 |
| 12_road_casualties | 0/0; 0/1 | 0/1; 0/1 | 1/1; 1/1 | 1/1; 1/1 | 0/0; 0/1 |
| 13_arrests_states | 1/3; 1/1 | 1/2; 1/1 | 1/1; 1/1 | 1/1; 1/1 | 0/0; 1/1 |
| 14_geyser_ocean | 0/0; 0/0 | 0/0; 0/0 | 0/0; 0/0 | 0/0; 0/0 | 0/0; 0/0 |
| 15_airmiles_economy | 0/0; 0/1 | 1/2; 1/1 | 0/0; 0/1 | 0/0; 1/1 | 0/0; 0/1 |

Case identities and complete semantic definitions are in the frozen manifest;
the numbers above do not weight dataset families equally. Shared NYC targets,
parallel R/statsmodels editions and controls are correlated.

## What the evidence supports

* **Jaccard** has the highest automatic F1 here (85.71%), with 30 correct
  selections and no false positives. It gets Nile, Engel, Grunfeld, stackloss
  and the driver-KSI correspondence correct, despite renamed generic fields
  in several pairs. It misses encoded penguin species/sex, aircraft tail
  registrations with little sample overlap, and some differently scaled
  Longley fields. For sunspots it ranks both true correspondences first but
  abstains at the common cutoff; changing time-span samples affects overlap.
* **COMA** has the highest automatic recall (32/40, 80%) and gets all seven
  penguin variables and all six weather lookup coordinates correct. It ranks
  every unchanged-name positive correctly (17/17). Its six false positives
  include aircraft manufacture `year` -> flight/weather `year`, and arrests
  `Murder` -> murder-offense `Murder`. Thus names can recover publisher renames
  while also accepting well-documented semantic traps. Generic renamed
  `time`/`value` series are often missed.
* **DistributionBased** has 27/28 correct automatic selections (96.43%
  precision). It recovers all Nile, Engel, Grunfeld and stackloss fields and
  the driver-KSI series, but only 2/7 Longley fields with different scaling
  and no sunspot correspondences with the different spans/16-row samples.
  Its one false positive is weather `precip` -> flight `dep_delay` (score 1):
  precipitation and delay are different quantities regardless of distribution.
* **Cupid** has 19/40 correct matches and 16 false positives. It recovers
  tail-number and carrier keys, but all ten positives with different declared
  types are missed. Its penguin float-to-integer cases select bill dimensions
  instead of flipper length/body mass, and the differently typed weather
  `hour` selects `day`. Several unrelated aircraft strings select `carrier`.
  These are observations of the fixed configuration, not a threshold study.
* **SimilarityFlooding** has the best candidate recall@5 (39/40, 97.5%),
  including 22/23 renamed positives; raw top-1 is 32/40 (80%). Automatic
  recall is only 2/40 because most scores remain below 0.5. It ranks all
  seven penguin, all six weather and all five Grunfeld positives first, but
  generic names and tied scores can put another field first. The one top-5
  miss is airmiles `time` -> Longley `YEAR`. Its low automatic F1 is not
  evidence that its candidate ranking is equally poor.

## Condition slices

Positive-label groups overlap across name/type dimensions. Each cell is
**rank top-1; recall@5; automatic recall**, each divided by the group size.

| Condition (labels) | COMA | Cupid | DistributionBased | Jaccard | SimilarityFlooding |
| --- | ---: | ---: | ---: | ---: | ---: |
| unchanged_name (17) | 100.00%; 100.00%; 100.00% | 58.82%; 58.82%; 58.82% | 76.47%; 76.47%; 76.47% | 88.24%; 88.24%; 82.35% | 100.00%; 100.00%; 11.76% |
| renamed (23) | 69.57%; 69.57%; 65.22% | 39.13%; 39.13%; 39.13% | 60.87%; 60.87%; 60.87% | 82.61%; 82.61%; 69.57% | 65.22%; 95.65%; 0.00% |
| same_declared_type (30) | 76.67%; 76.67%; 73.33% | 63.33%; 63.33%; 63.33% | 60.00%; 60.00%; 60.00% | 83.33%; 83.33%; 70.00% | 73.33%; 96.67%; 0.00% |
| different_declared_type (10) | 100.00%; 100.00%; 100.00% | 0.00%; 0.00%; 0.00% | 90.00%; 90.00%; 90.00% | 90.00%; 90.00%; 90.00% | 100.00%; 100.00%; 20.00% |

These ten cross-type cases include publisher-declared unknown CSV types as
well as actual integer/double differences. They are not ten independent
experiments, nor proof of a universal type-handling advantage.

## Validation and limits

* `cargo fmt --all -- --check`: passed.
* `cargo test --locked --all-targets --features polars --jobs 1`: **136 passed**,
  including all five algorithms' existing reference-fixture comparisons, the
  suggestion gate tests and 11 benchmark/integrity/metric tests.
* `cargo test --locked --doc`: **3 passed**.
* `cargo run --locked --example evaluate_all -- --check`: passed against the
  unchanged frozen baseline, **215/256 (83.98%)**, 170/179 automatic precision.
* Independent benchmark: summary and JSON modes passed; two separate JSON runs
  were **byte-identical**. The importer verified all extracted cases twice.
* Strict all-features Clippy passed with the supported lint-only environment:
  `ORT_SKIP_DOWNLOAD=1 cargo clippy --locked --all-targets --all-features --jobs 1 -- -D warnings`.
* Standard `cargo test --locked --all-targets --all-features --jobs 1` and strict
  all-features Clippy could not complete: existing optional `ort-sys` attempted
  its ONNX binary download from `cdn.pyke.io` and received proxy **403**.
  The lint-only setting does not validate linking or executing embeddings.
  This benchmark uses no embeddings and runs without that feature.

The original suggestion quality baseline and all Valentine reference fixtures
and tolerances remain intact. Live Python parity could not be rerun because this
checkout has no upstream Valentine/Python reference environment; the existing
pinned reference-fixture tests passed. Passing parity establishes reproduction
of an algorithm, not the accuracy measured in this report.

This corpus is about 5 MB including 76 exact publisher snapshots, with only
16 sampled rows per view. It is dominated by explicit publisher transformations,
documented lookup keys and historical statistical examples. Some targets are
small, making recall@5 easier; 51 no-match fields partly arise from intentionally
unrelated controls. Unknown production schemas, large-scale instance evidence
and independently adjudicated ambiguity remain outstanding. Primary government
endpoints were network-restricted; the actual acquisition routes are pinned
publisher archives and the R CSV mirror, not claimed primary downloads.
Licenses are upstream declarations; the complete historical licensing chain
has not been independently audited. See the protocol for all annotation, type,
period/unit and provenance limitations.

## 2026-10-11 follow-up: suggestions and an aggregation control

The original 15 cases above remain unchanged. They are now previously evaluated
inputs, not a new blind trial. Case 16 was independently annotated from the
publisher's annual/monthly temperature dictionaries and processing script, then
frozen in `0445a57898243aa01ebc67fbba21827fc7e87aa8` **before** its first
prediction. It contributes one match and two hard negatives: provider `Source`
matches, while annual `Year`/`Mean` are not monthly `Year`/`Mean`. See the
[protocol extension](independent_accuracy.md#2026-10-11-extension) and notices for
mirror provenance, licensing and stale publisher-description caveats.

Run the existing commands for the current 16 cases:

```sh
cargo run --locked --example evaluate_all -- --independent
cargo run --locked --example benchmark_accuracy -- --independent --details
```

The first command now evaluates the original suggestion API with unchanged
`Config::default()`. It verifies the same inputs, labels and provenance before
predicting, reports per-dataset decisions and errors, and treats both `NoMatch`
and `Ambiguous` as abstentions from automatic selection. Correct ambiguity
requires the exact labeled alternative set. The second command preserves the
five fixed Valentine configurations and the common 0.5 automatic cutoff.

Combined measurements: **94 fields, 41 matches, 53 no-matches, zero ambiguities**.
Ranking metrics apply only to the 41 unique matches, before the selection cutoff.

| Matcher | Precision | Recall | F1 | Rank top-1 | Recall@5 | Coverage |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Suggestions (explicit decisions) | 85.37% | 85.37% | 85.37% | — | — | 43.62% |
| COMA | 80.49% | 80.49% | 80.49% | 82.93% | 82.93% | 43.62% |
| Cupid | 54.05% | 48.78% | 51.28% | 48.78% | 48.78% | 39.36% |
| DistributionBased | 96.43% | 65.85% | 78.26% | 65.85% | 65.85% | 29.79% |
| JaccardDistanceMatcher | 100.00% | 75.61% | 86.11% | 85.37% | 85.37% | 32.98% |
| SimilarityFlooding | 60.00% | 7.32% | 13.04% | 80.49% | 97.56% | 5.32% |

| Matcher | Correct / automatic | Incorrect automatic | Missed matches | Abstentions / 94 |
| --- | ---: | ---: | ---: | ---: |
| Suggestions | 35/41 | 6 | 6 | 53 |
| COMA | 33/41 | 8 | 8 | 53 |
| Cupid | 20/37 | 17 | 21 | 57 |
| DistributionBased | 27/28 | 1 | 14 | 66 |
| JaccardDistanceMatcher | 31/31 | 0 | 10 | 63 |
| SimilarityFlooding | 3/5 | 2 | 38 | 89 |

On **case 16 alone**, Jaccard correctly selects `Source` and abstains for both
negatives. Suggestions and Cupid correctly select `Source` and abstain for
`Year`, but incorrectly select `Mean`. COMA and SimilarityFlooding incorrectly
select both same-named negatives. DistributionBased abstains for all three,
missing the positive. Matching labels and units do not establish equal temporal
aggregation. These failures were recorded without changing weights or thresholds.

Suggestion decision accuracy is **81/94 (86.17%)**: 35/41 expected matches,
46/53 expected no-matches, and no supported ambiguity denominator. Its 53
abstentions comprise 52 `NoMatch` decisions and one incorrect `Ambiguous` decision.
Failures include manufacture year versus flight/weather year, airframe speed
versus wind speed, annual versus monthly mean, and renamed `time` coordinates.
The CLI prints each affected source, expected decision, actual decision and
candidate components; full Valentine per-dataset results remain available with
`--json`.

The blank-sample correction was justified by a separate synthetic reproduction,
not by these labels: `country=["us", ""]` versus `category=["us", ""]` previously
received positive sample credit and an automatic 0.814286 match. Ignoring blanks
restores the one-observation result, `NoMatch` at 0.664286. A measured tradeoff on
the original 15 cases is **80/91 → 79/91 correct decisions**: all-missing aircraft
`speed` now has unavailable sample evidence, so existing weight redistribution
changes `NoMatch` to `Ambiguous` against flight times. Correct automatic matches
remain 34/39, false positives remain five, and six positives remain missed.
Another existing incorrect `speed` → `wind_speed` match rises from 0.7875 to
0.926471. Missingness consistency is not a claim of aggregate accuracy gain.
The development gate remains 215/256, with 9 incorrect automatic matches.

All original 15 Valentine per-dataset metrics, diagnostics and rankings were
identical before/after this pass; the complete development benchmark JSON was
byte-identical. The historical machine-readable report above is left intact.
Jaccard's observed precision rests on only 31 selections; DistributionBased
trades coverage for precision, and Flooding's strong retrieval does not imply
safe automatic matches at this shared cutoff. These are corpus-specific findings.

An attempted ambiguity extension used published UTC/CET equivalence evidence,
but its observation download was blocked; no synthetic observations or ambiguity
labels were substituted. Independent ambiguity accuracy remains unmeasured.
The small, correlated corpus, single annotation author, mirror provenance and
shared uncalibrated cutoff still limit generalization. Both independent commands
run in CI as reports with integrity checks, not newly calibrated accuracy gates.
