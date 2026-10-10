# Comparative Valentine accuracy protocol

This protocol was fixed before running the comparative benchmark. It measures
agreement with the existing labels, not equivalence to Python Valentine and not
accuracy on an independent held-out corpus. No scores or defaults are changed.

## Data and provenance

Use all 15 schema pairs in `eval/` and `eval_realworld/`: 256 source fields,
204 `match`, 46 `no_match`, and 6 `ambiguous` labels. The existing
[`quality_baseline.json`](quality_baseline.json) freezes dataset membership and
SHA-256 hashes of parsed inputs **and answers**. Verify these before evaluating
any algorithm. Never regenerate labels or update that baseline in a benchmark.

`eval/` contains five small development examples (accounts payable, customers,
inventory, payments, telemetry). `eval_realworld/` contains ten development
examples named after airports, US DOT airfare, EPA fuel economy, FDIC BankFind,
Kepler KOI, NOAA NDBC buoys, NYC municipal energy, NYC taxi, San Francisco budget,
and USGS earthquakes. Their names suggest public sources, but the repository
does not supply acquisition URLs, source versions, raw-file hashes, dataset
licenses, or an independently audited annotation history. Do not infer a source
license from an agency name or apply Fieldkin's code license to external data.
They have already been used in development, parity tests, and the suggestion
quality gate. Both directories are therefore **development data with unverified
external provenance and data licensing**, not an independent evaluation set.
Their labels were already frozen before this benchmark; their correctness has
not been independently established.

The upstream [Valentine v1.1 README](https://github.com/delftdata/valentine/blob/v1.1/README.md)
points to a [research dataset archive](https://surfdrive.surf.nl/files/index.php/s/QU5oxyNMuVguEku).
On 2026-10-10 the environment proxy denied that archive with HTTP 403. Its contents,
annotations, and redistribution licenses could not be verified. No external
data was downloaded or represented as independently labeled. **Independent
evaluation was outstanding at that measurement.** The subsequently sourced
[`independent_accuracy.md`](independent_accuracy.md) protocol documents the
separate held-out corpus; it does not relabel these development cases. A corpus
needs documented acquisition,
versions, licensing, and independently supportable labels frozen before looking
at predictions; it should also avoid reusing these development pairs.

## Fixed inputs and configurations

Run each pair separately through `valentine_match`, with table names `source`
and `target`. Keep declared types and every supplied nonempty sample, in order;
do not infer new types, canonicalize types for Python, or cap the samples.
Target reuse is allowed: these labels do not assert a one-to-one assignment.
There is no Hungarian or other global selection step.

The benchmark explicitly pins the current default configurations, rather than
calling `Default` so later default changes cannot silently change this protocol:

| Algorithm | Configuration |
| --- | --- |
| COMA | `max_n=0`, `use_instances=false`, `use_schema=true`, `delta=0.15`, `threshold=0.0`, `instance_weight=1.0` |
| Cupid | `leaf_w_struct=0.2`, `w_struct=0.2`, `th_accept=0.7`, `th_high=0.6`, `th_low=0.35`, `c_inc=1.2`, `c_dec=0.9`, `th_ns=0.7`, `process_num=1` |
| DistributionBased | `threshold1=0.15`, `threshold2=0.15`, `quantiles=256`, `process_num=1`, `use_bloom_filters=false` |
| JaccardDistanceMatcher | `threshold_dist=0.8`, `distance_fun=Levenshtein`, `process_num=1`, `tversky_alpha=1.0`, `tversky_beta=1.0`; no embeddings |
| SimilarityFlooding | `coeff_policy=InverseAverage`, `formula=FormulaC`, `string_matcher=PrefixSuffix`, empty TF-IDF corpus, `max_iterations=100`, `residual_threshold=0.0001` |

These values come from the implementation, not searches against the answers.
COMA and SimilarityFlooding use schema evidence; Cupid also uses declared types;
Jaccard and DistributionBased use instances. Algorithms retain their native
candidate filtering. Ranking measures therefore concern **returned positive
candidates**, not hidden candidates discarded internally.

## Predictions and metrics

For each source, rank returned positive scores descending, breaking ties by
target name ascending (the existing result order for a fixed table pair). A
zero score is not evidence of a correspondence. Automatic selection is the
first candidate **only if its score is at least 0.50**; otherwise abstain.
This shared, inclusive cutoff was fixed before measuring predictions. Scores
are not calibrated probabilities across algorithms, so this is one documented
operating point, not an optimal or equally calibrated threshold for each one.

- **TP**: an automatic selection equals the unique `match` target.
- **FP**: every other automatic selection: a wrong target, a `no_match` row,
  or an `ambiguous` row. Choosing one possible ambiguous target cannot establish
  a unique match. This is conservative automatic-match precision, not a claim
  that every possible ambiguous target is semantically false.
- **FN**: a `match` row without the correct automatic selection. A wrong
  selection contributes both FP and FN.
- **Precision** = TP / (TP + FP). Correct `no_match` abstentions never contribute.
- **Recall** = TP / number of `match` rows. **F1** = 2 TP / (2 TP + FP + FN).
- **Top-1 accuracy** = `match` rows whose first positive candidate is the gold
  target / number of `match` rows, **before the 0.50 cutoff**.
- **Recall@5** = `match` rows with the gold target among the first five positive
  candidates / number of `match` rows, also before the cutoff.
- **Coverage** = rows with an automatic selection / all source rows, including
  `no_match` and `ambiguous`. High coverage alone does not imply good accuracy.
- Separately count automatic selections on `no_match` and `ambiguous` rows;
  report ambiguous rows with any possible target at rank <=5 and with **all**
  labeled alternatives at rank <=5. These are candidate-retrieval diagnostics,
  not correct ambiguity decisions. The five matchers do not emit such decisions.

An undefined fraction prints `n/a` (JSON `null`). Empty predictions are valid
abstentions, giving zero recall/top-1/recall@5 when positive labels exist. Report
micro totals and each dataset, plus predeclared `match`-only strata: unchanged
versus renamed columns (exact names), same versus different declared types.
No-match true negatives are not included in top-1 accuracy or recall.

Reject malformed/empty corpora, duplicate dataset/field/answer/prediction IDs,
missing labels, unknown targets, invalid ambiguous sets, unexpected table/column
identities, and invalid scores. An algorithm error fails the whole command;
it must not become an empty prediction or a skipped case. Output is ordered,
without runtime measurements or timestamps. No exact-score comparison or new
accuracy floor is imposed on the Valentine algorithms.

## Running and interpretation

From the repository root:

```sh
cargo run --locked --example benchmark_accuracy
```

Append `-- --details` to print field-level diagnostics, or `-- --json` to emit
counts, metrics, and those diagnostics as deterministic JSON. The committed `accuracy_report.md` records
actual development-corpus measurements and limitations. Repeating the command
uses local files only; no reference Python environment or model download is
needed. Test the evaluator with `cargo test --locked --example benchmark_accuracy`.
The existing Rust workflow includes these evaluator tests in its all-targets
test step.

Keep `cargo run --locked --example evaluate_all -- --check` for the original
suggestion API's frozen accuracy gate. Keep the fixtures and live reference
comparison described in `PORTING.md` for Valentine **parity**. Neither parity
success nor agreement with these development labels establishes generalization
to unseen, independently verified real-world mappings.
