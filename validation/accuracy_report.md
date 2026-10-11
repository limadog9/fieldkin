# Measured Valentine accuracy on development data

Measured on 2026-10-10 with Fieldkin 0.2.0. The matcher implementation is
`bb5a04aabc4cbb32964418b2596921cb1fc312fb`; this change only adds evaluation
code/documentation and shares the existing label parser. The 15 pairs contain
256 labeled source fields: 204 unique matches, 46 no matches, and 6 ambiguities.
Dataset membership, schema/sample values, and labels are unchanged and verified
against the parsed-data SHA-256 fingerprints in `quality_baseline.json`.

**These are development-corpus measurements, not independent real-world accuracy
claims.** External provenance, data licensing, and annotation independence are
unverified. The accessible upstream documentation pointed to an archive blocked
by the environment proxy. Independent evaluation was outstanding at this
measurement; the later [independent report](independent_accuracy_report.md)
evaluates a separately sourced corpus. See the [protocol](accuracy_protocol.md)
for provenance, pinned configurations, full metric definitions, and limitations.

Reproduce the tables with:

```sh
cargo run --locked --example benchmark_accuracy
```

Append `-- --json` for per-dataset precision/recall/F1/coverage, exact counts, input
fingerprints, and field diagnostics, or `-- --details` to read the diagnostics.
The following output is from the actual run. No settings were changed after
inspecting its predictions.

Development-corpus accuracy; independent evaluation outstanding.
Fixed defaults; positive candidates ranked per source; automatic cutoff >= 0.50.
Ambiguous automatic selections count as FP. Top-1 and recall@5 use only match labels, before cutoff.

| Algorithm | Precision | Recall | F1 | Top-1 | Recall@5 | Coverage |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| COMA | 78.72% | 72.55% | 75.51% | 75.49% | 82.84% | 73.44% |
| Cupid | 86.67% | 63.73% | 73.45% | 63.73% | 64.71% | 58.59% |
| DistributionBased | 85.08% | 75.49% | 80.00% | 75.49% | 79.90% | 70.70% |
| JaccardDistanceMatcher | 87.60% | 51.96% | 65.23% | 83.82% | 92.16% | 47.27% |
| SimilarityFlooding | n/a | 0.00% | 0.00% | 81.86% | 90.20% | 0.00% |

| Algorithm | TP / automatic | No-match automatic / labels | Ambiguous automatic / labels | Ambiguous any / all alternatives @5 |
| --- | ---: | ---: | ---: | ---: |
| COMA | 148 / 188 | 12 / 46 | 6 / 6 | 6 / 5 (of 6) |
| Cupid | 130 / 150 | 6 / 46 | 6 / 6 | 6 / 4 (of 6) |
| DistributionBased | 154 / 181 | 10 / 46 | 5 / 6 | 5 / 5 (of 6) |
| JaccardDistanceMatcher | 106 / 121 | 6 / 46 | 5 / 6 | 5 / 5 (of 6) |
| SimilarityFlooding | 0 / 0 | 0 / 46 | 0 / 6 | 6 / 6 (of 6) |

Per-dataset top-1 correct / match labels (before cutoff; full metrics in --json):

| Dataset | COMA | Cupid | DistributionBased | JaccardDistanceMatcher | SimilarityFlooding |
| --- | ---: | ---: | ---: | ---: | ---: |
| eval/accounts_payable.json | 4 / 8 | 5 / 8 | 8 / 8 | 8 / 8 | 7 / 8 |
| eval/customers.json | 3 / 5 | 1 / 5 | 5 / 5 | 5 / 5 | 4 / 5 |
| eval/inventory.json | 7 / 8 | 5 / 8 | 8 / 8 | 8 / 8 | 6 / 8 |
| eval/payments.json | 4 / 8 | 3 / 8 | 7 / 8 | 7 / 8 | 6 / 8 |
| eval/telemetry.json | 7 / 9 | 3 / 9 | 9 / 9 | 9 / 9 | 6 / 9 |
| eval_realworld/airports_publication_export.json | 12 / 12 | 12 / 12 | 9 / 12 | 9 / 12 | 12 / 12 |
| eval_realworld/dot_airfare_change_to_markets.json | 5 / 7 | 7 / 7 | 6 / 7 | 7 / 7 | 7 / 7 |
| eval_realworld/epa_fuel_economy_exports.json | 9 / 28 | 5 / 28 | 18 / 28 | 25 / 28 | 8 / 28 |
| eval_realworld/fdic_bankfind.json | 21 / 23 | 21 / 23 | 11 / 23 | 17 / 23 | 23 / 23 |
| eval_realworld/kepler_koi.json | 23 / 25 | 25 / 25 | 12 / 25 | 18 / 25 | 25 / 25 |
| eval_realworld/ndbc_buoy.json | 7 / 15 | 3 / 15 | 14 / 15 | 12 / 15 | 10 / 15 |
| eval_realworld/nyc_municipal_energy_versions.json | 8 / 9 | 8 / 9 | 6 / 9 | 7 / 9 | 9 / 9 |
| eval_realworld/nyc_taxi_annual_versions.json | 11 / 11 | 11 / 11 | 7 / 11 | 5 / 11 | 11 / 11 |
| eval_realworld/sf_budget_to_actuals.json | 20 / 20 | 20 / 20 | 20 / 20 | 20 / 20 | 20 / 20 |
| eval_realworld/usgs_earthquakes.json | 13 / 16 | 1 / 16 | 14 / 16 | 14 / 16 | 13 / 16 |

Match-only conditions; groups overlap across name/type dimensions:

| Algorithm | Condition | Labels | Top-1 | Recall@5 | Automatic recall |
| --- | --- | ---: | ---: | ---: | ---: |
| COMA | different_declared_type | 4 | 100.00% | 100.00% | 100.00% |
| COMA | renamed | 99 | 56.57% | 64.65% | 50.51% |
| COMA | same_declared_type | 200 | 75.00% | 82.50% | 72.00% |
| COMA | unchanged_name | 105 | 93.33% | 100.00% | 93.33% |
| Cupid | different_declared_type | 4 | 0.00% | 0.00% | 0.00% |
| Cupid | renamed | 99 | 26.26% | 28.28% | 26.26% |
| Cupid | same_declared_type | 200 | 65.00% | 66.00% | 65.00% |
| Cupid | unchanged_name | 105 | 99.05% | 99.05% | 99.05% |
| DistributionBased | different_declared_type | 4 | 25.00% | 25.00% | 25.00% |
| DistributionBased | renamed | 99 | 82.83% | 85.86% | 82.83% |
| DistributionBased | same_declared_type | 200 | 76.50% | 81.00% | 76.50% |
| DistributionBased | unchanged_name | 105 | 68.57% | 74.29% | 68.57% |
| JaccardDistanceMatcher | different_declared_type | 4 | 25.00% | 25.00% | 0.00% |
| JaccardDistanceMatcher | renamed | 99 | 88.89% | 93.94% | 62.63% |
| JaccardDistanceMatcher | same_declared_type | 200 | 85.00% | 93.50% | 53.00% |
| JaccardDistanceMatcher | unchanged_name | 105 | 79.05% | 90.48% | 41.90% |
| SimilarityFlooding | different_declared_type | 4 | 100.00% | 100.00% | 0.00% |
| SimilarityFlooding | renamed | 99 | 62.63% | 79.80% | 0.00% |
| SimilarityFlooding | same_declared_type | 200 | 81.50% | 90.00% | 0.00% |
| SimilarityFlooding | unchanged_name | 105 | 100.00% | 100.00% | 0.00% |

## What these results support

- **DistributionBased** has the highest automatic-selection F1 at the fixed
  0.50 operating point: 80.00%, with 154 correct automatic matches, 27 FP, and
  50 missed unique matches. It ranks 37/38 small `eval/` matches first and leads
  COMA/Cupid on renamed columns (82/99 versus 56/99 and 26/99). It is weaker on
  the development pairs named after FDIC (11/23) and Kepler (12/25), where schema
  matchers rank more labeled targets first. This supports its usefulness on
  these instance-bearing pairs, not a claim about arbitrary distributions.
- **JaccardDistanceMatcher** has the highest aggregate top-1 (171/204, 83.82%),
  recall@5 (188/204, 92.16%), and automatic precision (106/121, 87.60%). It ranks
  88/99 renamed matches first and 25/28 EPA-labeled matches first. Its automatic
  recall is only 51.96% at 0.50; good rankings often fall below that cutoff.
  It ranks only 5/11 NYC-taxi-labeled matches first, while COMA, Cupid, and
  SimilarityFlooding rank all 11 first. Instance ranking is useful on many
  of these renamed development columns but does not dominate schema evidence.
- **COMA**, in its pinned schema-only default mode, ranks 98/105 unchanged-name
  matches first and all four differing-declared-type matches first. It ranks
  all 12 airport-labeled matches first, but only 9/28 EPA-labeled matches and
  56/99 renamed matches first. It has the highest automatic coverage (73.44%)
  and the most automatic selections on no-match labels (12/46), so its coverage
  comes with a measurable false-match cost.
- **Cupid** ranks 104/105 unchanged-name matches first, including all 25 Kepler
  and all 11 NYC-taxi-labeled matches. It ranks only 26/99 renamed matches and
  none of the four differing-declared-type matches first. With its existing
  acceptance filtering, recall@5 (64.71%) adds little to top-1 (63.73%). The
  observed limitation concerns these particular labels and returned candidates;
  this benchmark does not expose candidates discarded inside Cupid.
- **SimilarityFlooding** ranks all 105 unchanged-name matches first, all labeled
  matches in FDIC, Kepler, NYC municipal energy, NYC taxi, and SF budget first,
  and 138/166 matches first in the `eval_realworld/` development group (83.13%,
  the highest top-1 in that group). It ranks 62/99 renamed matches first and
  only 8/28 EPA-labeled matches first. Every returned source maximum falls
  below the common 0.50 operating point, so automatic coverage and recall are
  zero and precision is undefined. This is evidence that **a shared absolute
  cutoff is not a calibrated cross-algorithm decision rule**, not evidence
  that its rankings are useless; aggregate recall@5 is 184/204 (90.20%).

There is no single winner across selection metrics, rankings, and dataset
conditions. The five small `eval/` cases favor instance rankings: both Jaccard
and DistributionBased rank 37/38 matches first. Among the ten development pairs
in `eval_realworld/`, SimilarityFlooding leads top-1 (138/166), followed by
Jaccard (134/166), COMA (129/166), DistributionBased (117/166), and Cupid
(113/166). Pooling the groups hides this distinction. All comparisons concern
the same supplied inputs with the configurations fixed in the protocol.

## Limits of the measurement

Precision treats a single automatic choice on an ambiguous label conservatively
as FP. Finding a possible target is not a verified unique mapping. There are
only six ambiguous cases and four differing-declared-type matches: the latter
are the NDBC `time`, NYC municipal energy's GHG-intensity field, and USGS
`time`/`updated` mappings. These small, potentially confounded groups cannot
establish broad ambiguity handling or datatype robustness. Algorithms do not
emit explicit ambiguity decisions, so the any/all-alternatives counts measure
retrieval only. Correct no-match abstentions never inflate automatic precision.

The corpus is small, previously used in development, and unevenly distributed
across conditions. Label correctness, source representativeness, licensing,
and independence remain unaudited. Sample-based algorithms depend on the
provided samples and transformations; the results do not measure all rows of
original external datasets. Scores are algorithm-specific heuristics and the
shared cutoff is one untuned operating point. Native candidate pruning and
fixed tie-breaking also affect the rankings. No statistical confidence or
out-of-corpus generalization is asserted.

Valentine reference fixtures and tolerances remain unchanged. Reference parity
is separate from agreement with these development labels. The original
`match_schemas` frozen quality gate also remains unchanged; its 215/256 correct
decisions are not scores for any of the five algorithms in this report.

## Validation of this change

- Formatting and whitespace checks passed.
- `cargo test --locked --all-targets --features polars --jobs 1`: 132 tests
  passed, including seven evaluator tests, ten unchanged quality-gate tests,
  and the existing pinned Valentine reference fixtures.
- `cargo test --locked --doc`: all three doctests passed.
- `cargo run --locked --example evaluate_all -- --check`: passed at the
  unchanged 215/256 decision baseline.
- Two complete `benchmark_accuracy --json` runs produced byte-identical output,
  SHA-256 `6c45e05148fc9e72f8df910d21da80744f03178dc36caef55ceca8d0eb3b95c0`.
- All-features, all-targets Clippy with `-D warnings` passed using
  `ORT_SKIP_DOWNLOAD=1`, a lint-only build that does not link the ONNX runtime.

The standard all-features test and Clippy commands could not complete because
`ort-sys` 2.0.0-rc.13's optional embedding-runtime binary download from
`cdn.pyke.io` was rejected by the proxy with HTTP 403. Actual embedding-feature
tests were not executed; the five benchmark configurations do not use that
feature. The live Python reference comparison was not rerun because the pinned
`.upstream/valentine` checkout and reference Python dependencies were absent.
Reference fixture tests passed without any tolerance or fixture changes.
