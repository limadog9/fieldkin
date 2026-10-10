# Fieldkin

Fieldkin matches columns across tabular datasets in native Rust. It ports all
five current [Valentine](https://github.com/delftdata/valentine) algorithms:
COMA, Cupid, DistributionBased, JaccardDistanceMatcher and SimilarityFlooding.
The original Fieldkin suggestion API remains available.

The matchers execute in Rust. Python is used only for development fixtures and
reference verification. Cupid bundles WordNet; optional sentence embeddings use
native ONNX Runtime. Similarity scores are not probabilities.

## Install

Install the release from [crates.io](https://crates.io/crates/fieldkin):

```toml
[dependencies]
fieldkin = "0.2.0"
```

Optional features: `polars` for native Polars DataFrames and `embeddings` for
FastEmbed model inference. Both are disabled by default.

## Match tables

```rust
use fieldkin::{Table, valentine_match};
use fieldkin::algorithms::{Coma, ComaConfig};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let source = Table::from_csv(
    "customers", "customer_id,email\n1,alice@example.org\n2,bob@example.org\n".as_bytes(),
)?;
let target = Table::from_csv(
    "orders", "customerId,email_address\n1,alice@example.org\n2,bob@example.org\n".as_bytes(),
)?;
let matcher = Coma::new(ComaConfig { use_instances: true, ..Default::default() })?;
let matches = valentine_match(&[source, target], &matcher)?;

for (pair, score) in &matches {
    println!("{}.{} -> {}.{}: {:.3}",
        pair.source_table, pair.source_column,
        pair.target_table, pair.target_column, score);
    println!("{:?}", matches.get_details(pair));
}
# Ok(())
# }
```

Batch matching visits every unique pair of named tables. COMA, DistributionBased,
Flooding TF-IDF, and embeddings share statistics/vocabulary across the batch.

`Table::new(name, columns)` accepts existing `Field` values with declared
`DataType` and string samples. `from_schema` wraps an existing `Schema`.
`from_rows`, `from_csv`, and `from_json` infer boolean/numeric types and otherwise
use text. JSON input is an array of flat records; nulls are omitted. Supply
date/timestamp types explicitly when constructing fields.

The table API requires nonempty names, distinct columns within a table and
distinct table names in a batch. Empty columns and schemas are supported.
The original suggestion API retains its duplicate/empty-name behavior.
Direct native matcher calls validate table identities too. `from_schema` and
Serde deserialization construct tables without validation; matching validates
them, or callers can explicitly use `Table::validate()`.

`valentine_match` uses all supplied values. `match_tables` with `MatchOptions`
applies evenly spaced, deterministic per-column sampling to every matcher.
The options default limits columns to 1,000 nonempty values; `None` keeps all
values and zero clears samples. See [PORTING.md](https://github.com/limadog9/fieldkin/blob/main/PORTING.md) for Python differences.

## Algorithms

| Matcher | Evidence and controls |
| --- | --- |
| `Coma` | Complex name similarity, optional global TF-IDF instances, weighted combination and bidirectional max-N/delta/threshold selection. |
| `Cupid` | Typed tokens, all-sense WordNet Wu-Palmer semantics, datatype compatibility and structural reinforcement. |
| `DistributionBased` | Global ranks, quantile histograms, EMD, distribution/attribute discovery, exact integer correlation clustering and optional Bloom filters. |
| `JaccardDistanceMatcher` | Exact/fuzzy value sets with six lexical distances or cosine embeddings; Tversky penalties support Dice and containment. |
| `SimilarityFlooding` | Labelled schema graphs, both coefficient policies, all four fixpoint formulas and three initial string matchers. |

All implement `Matcher`; applications can implement custom algorithms.
Invalid input/configuration returns `Error`. Configure matchers using `Default`
and struct updates or validated constructors. Native worker counts default to one.
If the operating system cannot start a worker, matching returns `Error::Io`.

## Select and evaluate

`MatcherResults` is immutable and ranked by descending score with deterministic
ties. `ColumnPair` identifies both tables and columns. Transformations retain
only the selected component details. Constructors reject duplicate `ColumnPair`
identifiers instead of choosing one score silently.

```rust
# use fieldkin::{ColumnPair, MatcherResults};
use fieldkin::metrics::GroundTruth;
# fn main() -> Result<(), fieldkin::Error> {
# let matches = MatcherResults::new(vec![(ColumnPair::new("a","id","b","identifier"),1.0)])?;
let top = matches.take_top_n(5);
let top_quarter = matches.take_top_percent(25.0)?;
let confident = matches.filter(0.7)?;
let per_source = matches.take_top_n_per_source(3);
let optimal = matches.one_to_one_hungarian(Some(0.5))?;
let eligible = matches.one_to_one_hungarian_threshold_aware(Some(0.5))?;
let greedy = matches.one_to_one_greedy(Some(0.5))?;
let mutual = matches.one_to_one_mutual_top(1)?;

let truth = GroundTruth::Names(vec![("id".into(), "identifier".into())]);
let metrics = matches.get_metrics(&truth)?;
assert_eq!(metrics["F1Score"], 1.0);
# Ok(())
# }
```

Use `GroundTruth::Columns(Vec<ColumnPair>)` for table-aware evaluation.
Core metrics: precision, recall, F1, precision at top 10%, recall at ground-truth
size and reciprocal rank per source. `Metric`, `get_metrics_with`, and
`OneToOneMethod` support custom metrics/configurations.
`metrics::precision_increasing_n` evaluates cutoffs from 10% to 100%.
The predefined `METRICS_CORE`, `METRICS_ALL`, `METRICS_PRECISION_RECALL` and
`METRICS_PRECISION_INCREASING_N` sets can be passed to `get_metrics_with`.
`NamedMetric` assigns distinct keys to differently configured metrics.
`get_metrics_with` rejects duplicate metric names rather than overwriting results.

For Hungarian/greedy selectors, `None` uses Valentine's distinct-score cutoff
(descending unique scores indexed at ceil(count/2), clamped to the last index).
Equal-score collections still receive true one-to-one selection. Hungarian
maximizes total similarity before threshold filtering.
`one_to_one_hungarian_threshold_aware` is an opt-in alternative: it first maximizes
the number of pairs at or above the threshold, then their total similarity. For
scores A→X=0.90, A→Y=0.80, B→X=0.81 and B→Y=0.79, threshold 0.80 yields only
A→X through the original selector, while the alternative selects A→Y and B→X.
Metrics continue to use the original Valentine-compatible selectors.

Hungarian and greedy selectors constrain each source and target (table, column)
identity once across the entire collection. A column shared by multiple table pairs
can therefore retain a correspondence for only one pair. Mutual-top selection
applies its `n` limit globally per source/target identity. Disjoint pairs do not compete
for assignment, but `None` computes its cutoff from all scores, so high scores in
one pair can exclude lower scores in another. Select each table pair's results
separately when constraints and default cutoffs should apply independently.
Serialization emits a `matches` array containing each pair, score and any
component details.

## Polars and sentence embeddings

With `features = ["polars"]`, `Table::from_polars(name, &frame)` supports native
Polars 0.55 scalar columns, preserves types and omits null/NaN values. Nested
and binary columns are rejected. Dates/timestamps outside the calendar range
supported by Polars and invalid timezones return an input error.

With `features = ["embeddings"]`, construct `embeddings::FastEmbedProvider`,
wrap it in `Arc` and call `JaccardDistanceMatcher::with_embedding_provider`.
`all_minilm_l6_v2()` selects the upstream default sentence-transformer model.
`new(TextInitOptions, batch_size)` controls model/cache, threads, tokenizer limits
and supported ONNX execution providers. Model construction downloads missing
files; lexical matching never loads a model. CPU inference is the default.
CUDA/MPS support depends on the ONNX runtime/provider, rather than PyTorch's
device autodetection.

Without that feature, applications can implement `EmbeddingProvider` or use
`PrecomputedEmbeddings`. Real vectors are validated and normalized, and the
vocabulary is encoded once per batch.

## Original suggestion API

```rust
use fieldkin::{Config, DataType, Decision, Field, Schema, match_schemas};
let source = Schema { fields: vec![Field {
    name: "customer_id".into(), data_type: DataType::Integer, samples: vec![],
}] };
let target = Schema { fields: vec![Field {
    name: "customer-id".into(), data_type: DataType::Integer, samples: vec![],
}] };
let report = match_schemas(&source, &target, Config::default());
assert!(matches!(&report.fields[0].decision, Decision::Match { .. }));
```

This API returns ranked candidates and `Match`, `Ambiguous` or `NoMatch` for
each source independently. Names, types and optional normalized sample overlap
contribute evidence; target reuse is allowed. Abbreviation fallback runs after
the usual matching returns `NoMatch`. `match_schemas_with` instead returns
`MatcherResults` from a chosen Valentine algorithm.

`match_schemas` preserves its original panic behavior for invalid configuration.
Use `try_match_schemas` to handle user-supplied thresholds without a panic:

```rust
use fieldkin::{Config, Error, Schema, try_match_schemas};
let schema = Schema { fields: vec![] };
let config = Config { min_score: f64::NAN, ..Config::default() };
assert!(matches!(try_match_schemas(&schema, &schema, config), Err(Error::InvalidConfig(_))));
```

Both functions require finite thresholds and a nonnegative ambiguity margin.
Finite minimum scores outside `[0, 1]` remain supported.

## CLI and development

The evaluation commands, benchmarks, and reference-regeneration scripts below
require a source checkout; their corpora and tooling are not part of the published
crate. The crate includes the library, CLI, `valentine` example, tests, and licenses.

```sh
cargo run --bin fieldkin -- coma source.csv target.csv
cargo run --bin fieldkin -- distribution source.json target.json sales orders
cargo run --example valentine
cargo test --locked --all-targets --all-features --jobs 1
cargo test --locked --doc
cargo clippy --locked --all-targets --all-features --jobs 1 -- -D warnings
```

The CLI writes ranked JSON to stdout and errors to stderr. Builds with embeddings
download ONNX Runtime. The explicit model test is excluded from offline runs:

```sh
cargo test --locked --features embeddings --jobs 1 real_minilm_model -- --ignored
```

Checked-in fixtures come directly from pinned Valentine source and cover matcher
variants, distribution phases, semantic behavior, lexical distances, fuzzy sets
and Tversky penalties. Regeneration instructions are in [PORTING.md](https://github.com/limadog9/fieldkin/blob/main/PORTING.md).

The suggestion accuracy gate evaluates every labeled source field in `eval/` and
`eval_realworld/` using `match_schemas` and `Config::default()`. Run it with:

```sh
cargo run --locked --example evaluate_all -- --check
```

The same check runs on every push and pull request in the Rust workflow. With no
arguments, `evaluate_all` also runs the gate. The frozen
[baseline](https://github.com/limadog9/fieldkin/blob/main/validation/quality_baseline.json) records the existing implementation's
actual performance: **215/256 correct decisions (83.98%)**, across 15 datasets.
Expected matches are correct in 170/204 cases, `no_match` in 40/46, and ambiguity
in 5/6. These measurements use the existing defaults: minimum score 0.72,
ambiguity margin 0.05, and five reported candidates. They are a regression floor,
not a claim that this accuracy is sufficient.

Metric definitions and rules:

- A correct decision must match the label's kind and target. Ambiguity requires
  exactly the labeled target set, independent of order.
- Accuracy is `correct / total`. Correct counts cannot decrease overall, in any
  dataset, in any expected decision category, or within a dataset's categories.
- Incorrect automatic matches are explicit `Match` decisions with a wrong target
  or with a `no_match`/`ambiguous` label. Their counts cannot increase in any of
  those scopes. Automatic-match precision is correct matches divided by all
  explicit `Match` decisions: **170/179 (94.97%)**, with **9 incorrect matches**.
  Correct `no_match` decisions never enter this fraction; no automatic predictions
  means precision is undefined (`n/a`). The match-correctness floor and incorrect
  match ceiling together protect precision.
- Missed matches are expected `match` cases without the correct automatic match,
  including wrong targets and abstentions: **34/204**. The per-category accuracy
  floor prevents this count from increasing.
- Incorrect ambiguities are `Ambiguous` predictions with the wrong kind or target
  set: **4**. Their counts also cannot increase in any scope. Missed expected
  ambiguities are protected by the `ambiguous` category's correctness floor.

The gate compares integer counts without tolerances, so improvements pass without
requiring identical predictions. Compensating changes within the same dataset and
category can pass if all protected metrics hold. Failures show metric changes and
current incorrect fields in affected datasets, with expected and actual decisions.
Missing labels, invalid target references, duplicate names/labels, malformed JSON,
and empty datasets/directories fail. Dataset membership and SHA-256 fingerprints
of parsed inputs/labels are frozen; JSON formatting and checkout line endings do
not affect fingerprints. The baseline is never updated by a check.

For a deliberately reviewed baseline refresh, `--print-baseline` prints current
measurements as JSON without changing the saved baseline. Do not lower it merely
to pass CI. The labeled corpora remain limited coverage (including only six
expected ambiguities); this gate measures the original suggestion API, separately
from the unchanged Valentine reference-parity tests.

For a separate comparison of all five Valentine algorithms against the same
labels, run:

```sh
cargo run --locked --example benchmark_accuracy
```

For separately sourced, held-out real-world inputs with archived provenance and
per-field evidence, run:

```sh
cargo run --locked --example benchmark_accuracy -- --independent
```

The [independent protocol](https://github.com/limadog9/fieldkin/blob/main/validation/independent_accuracy.md) documents the
15 frozen cases and limitations; the [measured comparison](https://github.com/limadog9/fieldkin/blob/main/validation/independent_accuracy_report.md)
reports all five algorithms. Add `--json` for full per-dataset metrics or
`--details` for incorrect/missed-match examples. This does not change the
suggestion quality baseline or Valentine parity checks.

For release performance/scaling measurements, run
`cargo build --locked --release --example benchmark_performance --jobs 1`, then
`python3 validation/benchmark_performance.py --output target/performance.json`.
The [performance report](https://github.com/limadog9/fieldkin/blob/main/validation/performance.md) documents fixed workloads,
before/after measurements, exact-output checks, and memory limitations. These
expensive benchmarks run separately from regular tests.

The [accuracy protocol](https://github.com/limadog9/fieldkin/blob/main/validation/accuracy_protocol.md) fixes configurations,
ranking, abstention, ambiguity handling, and metric definitions before measuring
predictions. The [measured report](https://github.com/limadog9/fieldkin/blob/main/validation/accuracy_report.md) compares
precision, recall, F1, top-1 accuracy, recall@5, coverage, and dataset/condition
results. Append `-- --details` for field diagnostics or `-- --json` for complete metrics
and frozen input hashes. This benchmark preserves declared types and uses all
supplied nonempty samples; it does not perform one-to-one assignment or impose
an accuracy floor on these algorithms.

`eval/` and `eval_realworld/` remain development data. Their external provenance,
data licensing, and annotation independence are not verified; the public-source
names in `eval_realworld/` do not establish those properties. The separately
frozen `eval_independent/` corpus supplies held-out measurements with the
limitations documented above. Valentine reference parity, the suggestion
accuracy gate, and comparative accuracy reports measure separate things.

Individual directories can still be evaluated in report-only mode:

```sh
cargo run --example evaluate_all -- eval
cargo run --example evaluate_all -- eval_realworld
```

## License

Existing Fieldkin code is MIT. Valentine-derived ports are Apache-2.0.
Princeton WordNet data retains its redistribution license. See [LICENSE-MIT](https://github.com/limadog9/fieldkin/blob/main/LICENSE-MIT),
[LICENSE-APACHE](https://github.com/limadog9/fieldkin/blob/main/LICENSE-APACHE), [NOTICE](https://github.com/limadog9/fieldkin/blob/main/NOTICE) and
[assets/WORDNET-LICENSE](https://github.com/limadog9/fieldkin/blob/main/assets/WORDNET-LICENSE).
