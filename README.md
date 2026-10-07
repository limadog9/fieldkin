# Fieldkin

Fieldkin is a small, experimental Rust library for suggesting correspondences
between fields in two flat schemas. It uses field names, declared types, and
optional sample values. Results are heuristic suggestions for review, not verified
mappings. Scores are not probabilities.

## Usage

Add Fieldkin to your `Cargo.toml`:

```toml
[dependencies]
fieldkin = "0.1.0"
```

```rust
use fieldkin::{match_schemas, Config, DataType, Decision, Field, Schema};

let source = Schema {
    fields: vec![Field {
        name: "customer_id".into(),
        data_type: DataType::Integer,
        samples: vec![],
    }],
};
let target = Schema {
    fields: vec![Field {
        name: "customer-id".into(),
        data_type: DataType::Integer,
        samples: vec![],
    }],
};

let report = match_schemas(&source, &target, Config::default());
assert!(matches!(
    &report.fields[0].decision,
    Decision::Match { target, .. } if target == "customer-id"
));
```

Each source field receives one decision:

- `Match`: one candidate meets the threshold and stands apart from the others.
- `Ambiguous`: multiple candidates meet the threshold and fall within the
  ambiguity margin of the best score.
- `NoMatch`: no candidate meets the threshold, or the target schema is empty.

The report also includes ranked candidates and their component scores.
`Config` controls the threshold, ambiguity margin, and number of returned
candidates. Candidate truncation does not affect decisions.

## Evidence and limitations

Name evidence combines RapidFuzz Jaro-Winkler similarity with shared tokens and a
small synonym normalization layer. Declared type compatibility contributes to
the score; differing types do not automatically forbid a match. Optional sample
evidence measures overlap after trimming and lowercasing values. A column with
only one distinct sampled value receives no positive sample credit, and shared
strongest sample evidence across multiple targets is weakened.

Exact names after case and separator normalization receive strong preference
when types are compatible. Fuzzy abbreviation matching with
`fuzzy-matcher` / `SkimMatcherV2` runs only after normal matching returns
`NoMatch`.

Fieldkin matches source fields independently, so several sources may select the
same target. It accepts empty and duplicate names, including empty names that
normalize identically; use nonempty, distinct names for meaningful review.
Reported fields are identified by name, so duplicate names cannot be distinguished
in a decision. Fieldkin does not infer types, handle nested schemas, or transform
values. Review suggestions in your application's context before using them.

## Development evaluation

On the current included development corpora, Fieldkin produces 44/49 correct
decisions on the synthetic set and 171/207 on the real-world-derived set. These
are development measurements, not general accuracy guarantees.

From the [repository](https://github.com/limadog9/fieldkin), run:

```sh
cargo run --example evaluate_all -- eval
cargo run --example evaluate_all -- eval_realworld
```

The development corpora and evaluators are excluded from the published crate.

## License

MIT.
