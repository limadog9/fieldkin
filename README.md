# Fieldkin

Fieldkin matches columns across tabular datasets in native Rust. It ports all
five current [Valentine](https://github.com/delftdata/valentine) algorithms:
COMA, Cupid, DistributionBased, JaccardDistanceMatcher and SimilarityFlooding.
The original Fieldkin suggestion API remains available.

Use `try_match_schemas` when you want ranked suggestions with explicit `Match`,
`Ambiguous`, or `NoMatch` decisions. Use `valentine_match` with a chosen algorithm
when you want its candidate pairs and scores, then select matches using
`MatcherResults`. These APIs have different decision rules; Valentine scores
do not imply the suggestion API's decisions.

The matchers execute in Rust. Python is used only for development fixtures and
reference verification. Cupid bundles WordNet; optional sentence embeddings use
native ONNX Runtime. Similarity scores are not probabilities.

## Install

Install the release from [crates.io](https://crates.io/crates/fieldkin):

```toml
[dependencies]
fieldkin = "0.2.1"
```

Optional features: `polars` for native Polars DataFrames and `embeddings` for
FastEmbed model inference. Both are disabled by default.

The supported minimum Rust version is 1.94, including optional features. CI runs
default-feature tests and checks all targets with all features on that compiler,
using the committed `Cargo.lock`. Current stable Rust is also tested on Linux and
Windows. Dependency updates may raise this minimum; Polars 0.55 currently uses
standard-library APIs introduced in Rust 1.94.

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
The original suggestion API accepts duplicate and empty names. Empty or
punctuation-only names normalize to the same identifier and can match; validate
field names before using that API.
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

DistributionBased uses microlp instead of Python's CBC solver. Conflicting graphs
can have multiple optimal directed solutions that produce different correspondence
sets. Equal objectives do not establish output parity; see the
[documented solver boundary](https://github.com/limadog9/fieldkin/blob/main/PORTING.md#interface-and-behavior-differences).

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

### Opt-in work limits

Use `match_tables_one_to_one_with_limits` to carry the same explicit limits through
Jaccard or DistributionBased batch matching and legacy Hungarian selection. This
path supports only those two checked matchers; it never falls back to unchecked
matching. Individual `get_matches_with_limits` / `get_matches_batch_with_limits`
and Hungarian `_with_limits` calls remain available for other compositions.

The caps below are illustrative; choose them for your workload.

```rust
use fieldkin::{CheckedMatcher, MatchOptions, Table, WorkLimits, match_tables_one_to_one_with_limits};
use fieldkin::algorithms::JaccardDistanceMatcher;
# fn main() -> Result<(), fieldkin::Error> {
let limits = WorkLimits {
    max_assignment_dimension: 256,
    max_matrix_cells: 65_536,
    max_clustering_variables: 4_096,
    max_clustering_constraints: 262_144,
    max_jaccard_comparisons: 1_000_000,
};
let source = Table::from_csv("source", "id\n1\n".as_bytes())?;
let target = Table::from_csv("target", "id\n1\n".as_bytes())?;
let matcher = JaccardDistanceMatcher::default();
let selected = match_tables_one_to_one_with_limits(
    &[source, target],
    CheckedMatcher::Jaccard(&matcher),
    MatchOptions { instance_sample_size: None },
    Some(0.5),
    &limits,
)?;
# assert_eq!(selected.len(), 1);
# Ok(())
# }
```

`WorkLimits::default()` leaves counts uncapped but still checks overflow.
These are work counts, not hard memory limits or solver deadlines.
Assignment checks the maximum source/target identity count and their product,
including candidates below the threshold. Jaccard checks total column-pair slots
and distinct-value Cartesian comparisons across the whole batch before scoring
shortcuts. For example, two tables with two columns and two distinct nonempty
values per column require four slots and at most 16 comparisons. Input parsing,
sampling and value deduplication precede that check; string lengths,
preparation allocations and embedding-provider work are not capped.

Distribution limits apply to each distance matrix over populated columns and each
conflicting clustering model: a component of size `n` has `n*(n-1)` directed
binary variables and `n*(n-1)*(n-2)` triangle constraints. These are not
cumulative batch limits. Row sampling alone does not bound assignment dimensions
or clustering complexity. These scopes also apply to the combined checked path;
they do not become cumulative budgets. Existing matching, selection, and metric
entry points remain unbounded. The combined path computes no metrics;
`get_metrics` afterward can perform an unchecked assignment. The packaged
[`valentine` example](https://github.com/limadog9/fieldkin/blob/main/examples/valentine.rs)
demonstrates finite limits for both
matching and assignment (`cargo run --locked --example valentine`).
Use an isolated process with external memory/time limits for hard containment.
For example, on Linux with GNU `timeout`, build first, then run the example with
a 30-second deadline and an illustrative 1-GiB virtual-address-space limit:

```sh
cargo build --locked --example valentine
timeout --kill-after=5s 30s bash -c 'ulimit -v 1048576; exec "$@"' _ target/debug/examples/valentine
```

The OS address-space limit includes mappings, not just resident matcher memory;
choose limits for the whole application. Other platforms need their own process
containment. This recipe does not turn unguarded library calls into checked calls.

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

When the best candidate has unavailable samples, its unnormalized name/type
contribution (`0.60 * name_score + 0.25 * type_score`) must also meet `min_score`.
This prevents missing samples from promoting weak evidence into a decision.
The ranking formula is unchanged, so `NoMatch.best_score` can exceed `min_score`;
consume `decision` rather than thresholding candidate scores. An additional
`NoMatch` can trigger the existing abbreviation fallback and change the returned
ranking.
This conservative check can abstain instead of choosing a lower-ranked candidate.
Exact names with identical known types and no samples qualify at the default
minimum (0.72), but supply only 0.85 absolute evidence.

Unknown types provide neutral compatibility, even when both are unknown.
Name-token containment can still score highly for semantically different fields;
scores are ranking heuristics, not probabilities of correctness. Exact normalized
names with compatible types receive a score floor of 0.98, even with disjoint
samples. For example, annual and monthly columns both named `Mean` need not
represent the same measurement. Units, aggregation periods and business meaning
are not supplied by `Field`; review these using domain context before accepting
automatic suggestions.

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

The quality gate and comparative benchmark reject unknown schema/field keys,
including misspelled `samples` properties, instead of silently ignoring evidence.

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
cargo run --locked --example benchmark_accuracy -- --independent --check
cargo run --locked --example evaluate_all -- --check-independent
```

The [independent protocol](https://github.com/limadog9/fieldkin/blob/main/validation/independent_accuracy.md) documents the
17 frozen cases and limitations; the [measured comparison](https://github.com/limadog9/fieldkin/blob/main/validation/independent_accuracy_report.md)
separates the initial cases from later climate and epoch-representation controls.
`benchmark_accuracy` reports all five algorithms; add `--json` for full
per-dataset metrics or `--details` for incorrect/missed-match examples.
`evaluate_all` reports the original suggestion API's decisions, automatic-match
precision/recall, abstentions and failures on the same verified inputs. Both
checks enforce the pre-change frozen independent baseline in CI, per dataset and
decision category, while preserving the development quality baseline and parity
checks. Omit the check flag (use `evaluate_all -- --independent`) for report-only
runs. The two ambiguity labels cover equivalent epoch representations;
they do not establish reliability on ambiguous business roles.

A further 11 financial, healthcare and operational schema-migration cases
(236 source fields) test suggestion reliability in two separately frozen trials:

```sh
cargo run --locked --example evaluate_all -- --check-reliability
```

The [reliability report](https://github.com/limadog9/fieldkin/blob/main/validation/reliability_report.md)
records both rejected decision safeguards: fewer false matches did not reliably
preserve legitimate renames. CI protects the original per-case quality floor.
These are documented publisher schemas/examples, not production observations;
they contain no independently supported ambiguity labels. Source archives,
field-level evidence and extraction checks are excluded from the published crate.

A later semantic trial adds CFPB, Adyen and OHDSI migrations, including a
documented ambiguity. Run `cargo run --locked --example evaluate_all -- --check-semantic`
for the original-runtime gate, or
`cargo run --locked --example benchmark_accuracy -- --semantic` for all five
algorithms. The [trial report](https://github.com/limadog9/fieldkin/blob/main/validation/semantic_reliability_report.md)
separates 55 genuinely held-out fields from 13 partially exposed fields and
explains why the tested corroboration policy was rejected. No new runtime policy
was accepted.

For release performance/scaling measurements, run
`cargo build --locked --release --example benchmark_performance --jobs 1`, then
`python3 validation/benchmark_performance.py --output target/performance.json`.
The [performance report](https://github.com/limadog9/fieldkin/blob/main/validation/performance.md) documents fixed workloads,
before/after measurements, exact-output checks, and memory limitations. These
expensive benchmarks run separately from regular tests.
For pinned Python-versus-Rust comparisons, including correctness checks, warmups,
fresh processes and memory scopes, see the
[head-to-head report](https://github.com/limadog9/fieldkin/blob/main/validation/python_performance.md).

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
