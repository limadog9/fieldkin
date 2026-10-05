# Fieldkin

Fieldkin is an independent Rust library that proposes correspondences between two
flat schemas. Embed it in an importer, migration tool, catalog, or integration
service. It returns ranked candidates, individual signal explanations, ambiguous
alternatives, and unmatched fields. It does not rewrite data.

**Status:** first-release implementation; not yet published to
crates.io. The API is experimental. Latest stable Rust is required
(Rust 1.99.0 for the October 4, 2026 validation); MIT OR Apache-2.0.
The sole maintainer and final decision-maker is [@limadog9](https://github.com/limadog9).

**Release decision: not qualified.** The current [readiness pass](https://github.com/limadog9/fieldkin/blob/main/docs/readiness-pass.md)
records matching regressions, platform and package checks, and remaining evidence gaps.

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
Text sample equality is exact, with no value conversions.
`Number` retains `f64` measurements. Use `Integer(i128)` and
`Decimal(ExactDecimal)` for exact values; numeric kinds are not implicitly cast.
See [exact numeric samples](https://github.com/limadog9/fieldkin/blob/main/docs/exact-numbers.md) for scale and range rules.
Callers are responsible for representative sampling and truthful declared types.

`SKU_Code → product_id` needs domain evidence, such as caller-defined token aliases
or a custom `Matcher`. Shared sample values alone are deliberately insufficient
under the defaults. Likewise, gross/net amounts, currencies, units, timezones,
identifier scope, and similarly shaped unrelated data can fool lexical or value
evidence. Fieldkin has no general semantic guarantee. See
[design and failure cases](https://github.com/limadog9/fieldkin/blob/main/docs/design.md).

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
unrelated fields can have identical profiles. See [Stage 3 migration and limits](https://github.com/limadog9/fieldkin/blob/main/docs/stage3-migration.md).

An application can require sampled support before accepting a pair as eligible:

```rust
use fieldkin::{CandidateIssue, Config, Corroboration, DataType, Decision, Field, MatchEngine, Schema};

let source = Schema::new(vec![Field::new("s", "amount", DataType::Decimal)]);
let target = Schema::new(vec![Field::new("t", "amount", DataType::Decimal)]);
let report = MatchEngine::new(Config {
    corroboration: Some(Corroboration::default()),
    ..Config::default()
})?.match_schemas(&source, &target)?;
assert!(report.fields[0].selected.is_none());
assert_eq!(report.fields[0].decision, Decision::InsufficientEvidence);
assert!(report.fields[0].candidates[0].issues.contains(&CandidateIssue::InsufficientSampleSupport));
# Ok::<(), fieldkin::MatchError>(())
```

This opt-in gate requires positive built-in name evidence and distinct-aware
sample evidence scoring at least 0.5. Scores and ranking remain unchanged;
unsupported candidates are retained for review. Unavailable or disjoint samples
lose eligibility, and coincidental shared values can still mislead. The weighted
default remains unchanged. See [corroboration and its tradeoffs](https://github.com/limadog9/fieldkin/blob/main/docs/corroboration.md)
and the [complete example](https://github.com/limadog9/fieldkin/blob/main/examples/corroboration.rs).

`Config::name_conflicts` can separately exclude conflicting qualifiers or unit
phrases using caller-configured alternatives and synonyms. It inspects normalized
names before aliases and preserves scores and ranked evidence. The default rule
list is empty. On the unchanged 960-decision development baseline, the fixed
conflict vocabulary removes five false proposals while retaining all 321 correct
independent matches: precision rises from 76.07% to 76.98%, with unique coverage
unchanged at 44.66%. See [configuration](https://github.com/limadog9/fieldkin/blob/main/docs/precision.md) and the
[precision and coverage report](https://github.com/limadog9/fieldkin/blob/main/docs/precision-evaluation.md).

`Config::contextual_evidence` enables an experimental policy that combines
informative name agreement with adequate or distinctive sampled support:

```rust
use fieldkin::{Config, ContextualEvidence, DataType, Field, MatchEngine, SampleValue, Schema};

let samples = vec![SampleValue::Integer(10), SampleValue::Integer(20), SampleValue::Integer(30)];
let source = Schema::new(vec![Field::new("s", "customer_id", DataType::Unknown)
    .with_samples(samples.clone())]);
let target = Schema::new(vec![Field::new("t", "customer_id", DataType::Unknown)
    .with_samples(samples)]);
let report = MatchEngine::new(Config {
    contextual_evidence: Some(ContextualEvidence::default()),
    ..Config::default()
})?.match_schemas(&source, &target)?;
assert!(report.fields[0].selected.is_some());
assert!(report.fields[0].selected.as_ref().unwrap().score >= 0.95);
# Ok::<(), fieldkin::MatchError>(())
```

This option defaults to `None`. Original signal evidence and weights are preserved;
the overall score becomes the maximum of their weighted sum, a 0.90 supported
lexical floor and a 0.95 adequate-sample floor. These are heuristic choices,
not confidence probabilities. Contextual support additionally governs eligibility:
unsupported viable pairs return `InsufficientEvidence` with
`InsufficientContextSupport`, while `ContextualScoreAdjustment` identifies a raised
score. Strict identifiers require distinctive samples; scoped support checks
observed agreement, measurement representation and temporal roles. Context uses
every original hard-compatible pair before caller review, ambiguity, assignment
and display truncation. It requires active concrete built-in name and default
distinct-aware sample signals; custom signal names cannot substitute. See
[release-readiness evidence and limits](https://github.com/limadog9/fieldkin/blob/main/docs/release-readiness.md).

## API and customization

Applications can feed reviewed decisions back into matching:

```rust
use fieldkin::{Config, DataType, Decision, Field, FieldPair, MatchConstraints, MatchEngine, Schema};

let source = Schema::new(vec![Field::new("s", "amount", DataType::Decimal)]);
let target = Schema::new(vec![
    Field::new("gross", "amount", DataType::Decimal),
    Field::new("net", "amount", DataType::Decimal),
]);
// An application reviewer has verified which amount this source represents.
let review = MatchConstraints {
    confirmed: vec![FieldPair::new("s", "gross")],
    ..Default::default()
};
let report = MatchEngine::new(Config::default())?
    .match_schemas_with_constraints(&source, &target, &review)?;
assert_eq!(report.fields[0].decision, Decision::Confirmed);
assert_eq!(report.fields[0].selected.as_ref().unwrap().target.0, "gross");
# Ok::<(), fieldkin::MatchError>(())
```

`MatchConstraints` also accepts forbidden pairs and explicitly unmatched sources.
Confirmations retain actual heuristic scores; they are caller decisions, not
stronger automatic evidence. In one-to-one mode they reserve targets before the
remaining proposals are computed. Invalid or contradictory directives return
typed errors. See [review semantics and limits](https://github.com/limadog9/fieldkin/blob/main/docs/review-constraints.md) and
the [reviewed importer](https://github.com/limadog9/fieldkin/blob/main/examples/reviewed_import.rs).

- `Schema`, `Field`, `FieldId`, `DataType`, and `SampleValue` describe inputs. IDs
  must be nonempty and unique within each schema; names can repeat.
- `MatchEngine::new(Config)` selects the built-in name, type and sample signals.
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
[landscape and attribution](https://github.com/limadog9/fieldkin/blob/main/docs/landscape.md) and [THIRD_PARTY.md](https://github.com/limadog9/fieldkin/blob/main/THIRD_PARTY.md).

## Embedding and diagnostics

The [usage guide](https://github.com/limadog9/fieldkin/blob/main/docs/usage.md) and compiling
[importer](https://github.com/limadog9/fieldkin/blob/main/examples/import_review.rs) / [catalog](https://github.com/limadog9/fieldkin/blob/main/examples/catalog_review.rs)
examples show how to preserve unmatched fields and keep review decisions in your
application. `MatchError` variants and typed candidate/field diagnostics support
programmatic handling without parsing explanation strings.

For one-to-one matching, optional `Config::global_diagnostics` probes alternative
assignments within explicit solve/work budgets. It reports total-objective gaps,
changed IDs and whether analysis completed. It never changes proposals, and
incomplete analysis cannot establish uniqueness. See
[global diagnostics](https://github.com/limadog9/fieldkin/blob/main/docs/stage4-diagnostics.md) and the
[migration guide](https://github.com/limadog9/fieldkin/blob/main/docs/migration.md).

Enable the optional `json` feature for bounded report export and saved review
decisions. `json::report_to_json` exports a display report; it cannot be imported
as accepted mappings. `json::review_to_json` and `json::review_from_json` preserve
explicit caller decisions with application-owned source/target revisions.
Arbitrary matcher text is omitted by default, and no schema or sample serializer
is provided. See [the JSON boundary](https://github.com/limadog9/fieldkin/blob/main/docs/json.md) and the compiling
[resume-review example](https://github.com/limadog9/fieldkin/blob/main/examples/persisted_review.rs). Adapters remain deferred.

## Validation and development

```text
rustup update stable --no-self-update
cargo +stable fmt --all -- --check
cargo +stable clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo +stable test --locked --workspace --all-features
cargo +stable test --locked -p fieldkin --no-default-features
cargo +stable test --locked --doc
cargo +stable doc --locked --no-deps --all-features
cargo +stable run --locked --example baseline
cargo +stable bench --locked --bench matching
cargo +stable run --locked -p fieldkin-tools -- import-northix --check
cargo +stable run --locked -p fieldkin-tools -- import-t2d --check
```

CI checks latest stable Rust on Linux, Windows and macOS, including tests,
clippy, documentation, consumer examples, packaging and development/regression evaluation.
Formatting uses that same toolchain. Older compiler maintenance has ended.
Workflow permissions remain read-only. Library tests and performance inputs are
original synthetic fixtures.
The separate external evaluation includes Apache-licensed T2D correspondences
and the CC-BY-4.0 Northix benchmark, each with provenance and notices. The baseline example compares name-only matching with the
default engine on small labeled cases; it is a regression illustration, not a
real-world accuracy claim. Benchmarks generate fixed inputs in memory and use
`std::hint::black_box`; report toolchain, hardware, and workload alongside timings.
See [benchmark results](https://github.com/limadog9/fieldkin/blob/main/docs/benchmarks.md), [CONTRIBUTING.md](https://github.com/limadog9/fieldkin/blob/main/CONTRIBUTING.md),
[ROADMAP.md](https://github.com/limadog9/fieldkin/blob/main/ROADMAP.md), and [MAINTAINERS.md](https://github.com/limadog9/fieldkin/blob/main/MAINTAINERS.md).

Stage 1 added a separate development-only evaluation package with 40 synthetic
families, 200 schema pairs and 1,200 labeled source decisions. The
[evaluation report](https://github.com/limadog9/fieldkin/blob/main/docs/evaluation.md) records the initial matcher's results,
including substantial false-proposal rates; the roadmap's precision and coverage
targets are not yet met. The full reproduction guide is in
[evaluation/](https://github.com/limadog9/fieldkin/tree/main/evaluation).

Stage 2 prepares built-in names and samples once per field within each call.
The [performance report](https://github.com/limadog9/fieldkin/blob/main/docs/stage2-performance.md) records five-run before/after
measurements, allocation costs and the slower report-budget rejection path.
Stage 2 preserved all frozen development decisions; those measurements precede
the deliberate scoring changes in Stage 3.

Stage 3 adds distinct-aware sample reliability, exact integers/decimals, verified
semantic hints, inspectable aliases and optional sample profiles. Its
[development evaluation](https://github.com/limadog9/fieldkin/blob/main/docs/stage3-evaluation.md) reports the full tradeoff:
default precision is nearly unchanged and recall is lower on the original
development corpus. Profiles remain opt-in and the 95% precision target is still
unmet. See the [migration guide](https://github.com/limadog9/fieldkin/blob/main/docs/stage3-migration.md) for API changes.

## Release qualification

The October 3, 2026 roadmap implementation and qualification are recorded in
the [candidate scorecard](https://github.com/limadog9/fieldkin/blob/main/docs/release-scorecard.md). One million generated cases
passed, but original held-out precision is **37.5%** and unique-field coverage **40%**:
the planned quality bar is unmet. Fieldkin remains experimental and unpublished.
See the [changelog](https://github.com/limadog9/fieldkin/blob/main/docs/changelog.md) for delivered features and deferred work.

The subsequent historical [corroboration experiment](https://github.com/limadog9/fieldkin/blob/main/docs/corroboration-evaluation.md)
introduced 24 synthetic families. On that experiment's frozen holdout, the opt-in sample gate reduces
false independent proposals from 55 to 32, while the number of correct independent
proposals stays at 20: precision improves from 26.67% to 38.46%, but unique coverage
is only 16.30%. These harder challenge cases are a separate dataset, not a trend
against the earlier 37.5% result. Defaults remain unchanged; all misses and the
stricter name-floor ablation are documented.

The next phases add [caller review constraints](https://github.com/limadog9/fieldkin/blob/main/docs/review-constraints.md) and
[external annotation evidence](https://github.com/limadog9/fieldkin/blob/main/docs/external-evaluation.md). The latter evaluates
549 development tables and leaves 218 tables reserved by whole class. Its labels
are positive-only, so it cannot establish precision or unmatched-field safety.
The [follow-up qualification](https://github.com/limadog9/fieldkin/blob/main/docs/review-qualification.md) records 213 passing
Rust tests, five importer tests, a 1.2-million-case generated campaign and measured
costs. These engineering checks do not close the original accuracy gap.

The [verified external evaluation workflow](https://github.com/limadog9/fieldkin/blob/main/docs/verified-evaluation.md) binds new
development runs to a recorded executable, source inputs and build configuration.
It rejects stale builds and changed inputs before recording a successful run.
This development tooling adds no library dependency and does not score the reserved
holdout or change matching behavior.

The [partial-assignment optimization](https://github.com/limadog9/fieldkin/blob/main/docs/assignment-performance.md) skips
unused target columns and solves with no eligible positive edges.
All pairs are still scored, and diagnostics retain their original work budgets.
The report documents full-report compatibility checks and measured costs.

The historical [remaining build cycle](https://github.com/limadog9/fieldkin/blob/main/docs/continuation-scorecard.md) added optional
review persistence, a bounded 1,000-field scale experiment and a frozen
Northix/Valentine comparison. Active fixture imports and verified evaluation now
use the native Rust `fieldkin-tools` package. Local Python/Valentine COMA execution
has been retired; Rust can still import the archived comparison score files.
Reusing those scores is a reproducibility comparison, not a newly executed
external matcher. Default input limits and weighted matching remain unchanged.

## License

Licensed under either [MIT](https://github.com/limadog9/fieldkin/blob/main/LICENSE-MIT) or [Apache License 2.0](https://github.com/limadog9/fieldkin/blob/main/LICENSE-APACHE),
at your option. Dependency code retains its original copyright and license.
