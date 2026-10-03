# Fieldkin

Fieldkin is an independent Rust library that proposes correspondences between two
flat schemas. Embed it in an importer, migration tool, catalog, or integration
service. It returns ranked candidates, individual signal explanations, ambiguous
alternatives, and unmatched fields. It does not rewrite data.

**Status:** first-release implementation; not yet published to
crates.io. The API is experimental. Rust 1.85 or later; MIT OR Apache-2.0.
The sole maintainer and final decision-maker is [@limadog9](https://github.com/limadog9).

## Quick start

Use a local checkout of `main` and `fieldkin = { path = "../fieldkin" }`.
Run `cargo run --example basic` to see
the complete report. Rust examples below are compiled as library doctests.

```rust
use fieldkin::{Config, DataType, Field, MatchEngine, Schema};

let source = Schema::new(vec![
    Field::new("source-date", "TransDate", DataType::Date),
    Field::new("source-gross", "GrossAmt", DataType::Decimal),
]);
let target = Schema::new(vec![
    Field::new("target-amount", "amount", DataType::Decimal),
    Field::new("target-date", "transaction_date", DataType::Date),
]);
let engine = MatchEngine::new(Config::default())?;
let report = engine.match_schemas(&source, &target)?;
let date = report.fields.iter().find(|f| f.source.0 == "source-date").unwrap();
assert_eq!(date.selected.as_ref().unwrap().target.0, "target-date");
// "Gross amount" is not established to mean this target's "amount".
assert!(report.unmatched_sources.iter().any(|id| id.0 == "source-gross"));
# Ok::<(), fieldkin::MatchError>(())
```

Duplicate names remain distinct because identity comes from caller-supplied IDs:

```rust
use fieldkin::{Config, DataType, Decision, Field, MatchEngine, Schema};

let source = Schema::new(vec![Field::new("s", "amount", DataType::Decimal)]);
let target = Schema::new(vec![
    Field::new("net", "amount", DataType::Decimal),
    Field::new("gross", "amount", DataType::Decimal),
]);
let report = MatchEngine::new(Config::default())?.match_schemas(&source, &target)?;
assert_eq!(report.fields[0].decision, Decision::Ambiguous);
assert!(report.fields[0].selected.is_none());
assert_eq!(report.fields[0].alternatives.len(), 2);
# Ok::<(), fieldkin::MatchError>(())
```

## Evidence and decisions

The default score is `0.65 × name + 0.20 × type + 0.15 × sample`.
These are **heuristic scores, not calibrated probabilities**. Missing signals
contribute zero without redistributing their weight, so absent samples do not
inflate the other evidence. The default eligibility threshold is 0.70; a margin
of 0.08 between eligible alternatives triggers abstention. Every proposal still
needs domain review.

| Signal | Evidence | Limits |
| --- | --- | --- |
| Name | Unicode NFKC, lowercase, separator/camel-case/acronym/digit tokenization; token Jaccard plus Jaro-Winkler | Only `amt → amount` and `trans → transaction` are default aliases; no ontology or language model |
| Type | Exact coarse types, numeric compatibility, date/timestamp compatibility | Unknown types supply no evidence; incompatible known types veto selection by default; compatibility is not a conversion guarantee |
| Samples | Exact typed value-set overlap, attenuated by non-null coverage and distinct support | Requires at least three non-null observations per side; constants score zero, two-value columns receive half support; sample overlap does not establish meaning |

Use `Field::with_samples` with `SampleValue::{Null, Boolean, Number, Integer, Decimal, Text}`.
`None`, empty samples, and insufficient non-null values are explained separately.
Text values compare exactly; Fieldkin does not parse dates or cast strings to numbers.
`Number` retains `f64` measurements. Use `Integer(i128)` and
`Decimal(ExactDecimal)` for exact values; numeric kinds are not implicitly cast.
See [exact numeric samples](docs/exact-numbers.md) for scale and range rules.
Callers are responsible for representative sampling and truthful declared types.

`SKU_Code → product_id` needs domain evidence, such as caller-defined token aliases
or a custom `Matcher`. Shared sample values alone are deliberately insufficient
under the defaults. Likewise, gross/net amounts, currencies, units, timezones,
identifier scope, and similarly shaped unrelated data can fool lexical or value
evidence. Fieldkin has no general semantic guarantee. See
[design and failure cases](docs/design.md).

Verified metadata can rule out an otherwise convincing candidate:

```rust
use fieldkin::{Config, DataType, Field, MatchEngine, Schema, SemanticHints};

let source = Schema::new(vec![Field::new("s", "price", DataType::Decimal)
    .with_hints(SemanticHints { currency: Some("USD".into()), ..Default::default() })]);
let target = Schema::new(vec![Field::new("t", "price", DataType::Decimal)
    .with_hints(SemanticHints { currency: Some("EUR".into()), ..Default::default() })]);
let report = MatchEngine::new(Config::default())?.match_schemas(&source, &target)?;
assert!(!report.fields[0].candidates[0].eligible);
assert!(report.fields[0].selected.is_none());
# Ok::<(), fieldkin::MatchError>(())
```

`SemanticHints` accepts caller-verified `unit`, `currency` and `identifier_scope`.
Conflicts exclude a pair; missing hints imply no agreement. Labels compare
exactly, without conversions or guessing. `NameMatcher::with_alias` validates
explicit token replacements, which are listed in explanations. Optional
`SampleProfileMatcher` compares sample shapes but is disabled by default because
unrelated fields can have identical profiles. See [Stage 3 migration and limits](docs/stage3-migration.md).

## API and customization

- `Schema`, `Field`, `FieldId`, `DataType`, and `SampleValue` describe inputs. IDs
  must be nonempty and unique within each schema; names can repeat.
- `MatchEngine::new(Config)` selects conservative defaults.
  `MatchEngine::with_matchers` accepts weighted built-in or custom `Matcher`s.
  Signals return an optional bounded score and an explanation. Invalid scores,
  duplicate IDs, invalid configuration, and exceeded budgets return `MatchError`.
- `MatchReport.fields` contains ranked `Candidate`s, their `SignalReport`s,
  ambiguity alternatives, a selected proposal, and a `Decision`. Unmatched IDs
  are reported on both sides. Ranking includes weak/ineligible candidates so the
  caller can inspect why they were rejected.
- Set `Config::one_to_one = true` for partial maximum-total-score assignment.
  The solver can leave every source unmatched. Ambiguous sources are excluded
  by default; `abstain_on_ambiguity = false` explicitly opts into deterministic
  selection while preserving the alternatives. Global ties receive a stable
  algorithmic choice, not extra confidence; competing targets carry a warning.

Candidate truncation never affects eligibility, ambiguity, or assignment. A
globally selected candidate is preserved separately even if it falls outside the
displayed top-k. Output uses stable ID order, descending scores, then target IDs
for ranking ties. Reordering inputs does not change results. Custom matchers must
also be deterministic and side-effect-free.

The defaults bound each schema to 128 fields, 16,384 pairs, 256 bytes per name/ID,
256 samples per field, 1,024 bytes per text sample, and 8 MiB total sample text.
Inputs over budget return errors instead of silent truncation. Raising limits
opts into more work. The built-ins perform no I/O and do not log sample values;
sample `Debug` output is redacted. Custom matchers are trusted code, not sandboxed.

## Scope and prior work

Fieldkin is a Rust library, with no database, network, Polars, LLM, API key, or
runtime service requirement. It focuses on small and medium flat schemas and
auditable partial proposals. Nested transformations, automatic data rewriting,
embeddings, Python bindings, and elaborate adapters are deferred.

[Valentine](https://github.com/delftdata/valentine) provides established schema
matching methods and an experimental framework. Fieldkin does not reimplement
Valentine or claim to be the first or fastest matcher. Its contribution here is
a small Rust API with explicit missing evidence, abstention, bounded inputs,
stable identities, and partial global assignment. See
[landscape and attribution](docs/landscape.md) and [THIRD_PARTY.md](THIRD_PARTY.md).

## Validation and development

```text
cargo fmt --all -- --check
cargo clippy --locked --all-targets --all-features -- -D warnings
cargo test --locked --all-features
cargo test --locked --doc
cargo doc --locked --no-deps --all-features
cargo run --locked --example baseline
cargo bench --locked --bench matching
```

CI runs format, clippy, tests, and documentation checks on Rust 1.85.0 with
read-only workflow permissions. Fixtures are synthetic and included under this
repository's license. The baseline example compares name-only matching with the
default engine on small labeled cases; it is a regression illustration, not a
real-world accuracy claim. Benchmarks generate fixed inputs in memory and use
`std::hint::black_box`; report toolchain, hardware, and workload alongside timings.
See [benchmark results](docs/benchmarks.md), [CONTRIBUTING.md](CONTRIBUTING.md),
[ROADMAP.md](ROADMAP.md), and [MAINTAINERS.md](MAINTAINERS.md).

Stage 1 now adds a separate development-only evaluation package with 40 synthetic
families, 200 schema pairs and 1,200 labeled source decisions. The
[evaluation report](docs/evaluation.md) records the unchanged matcher's results,
including substantial false-proposal rates; the roadmap's precision and coverage
targets are not yet met. The full reproduction guide is in
[evaluation/](https://github.com/limadog9/fieldkin/tree/main/evaluation).

Stage 2 prepares built-in names and samples once per field within each call.
The [performance report](docs/stage2-performance.md) records five-run before/after
measurements, allocation costs and the slower report-budget rejection path.
The frozen development decisions remain identical; performance work does not
change the matching-quality limitations above.

## License

Licensed under either [MIT](LICENSE-MIT) or [Apache License 2.0](LICENSE-APACHE),
at your option. Dependency code retains its original copyright and license.
