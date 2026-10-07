# Fieldkin

A deliberately small, explainable schema matcher.

Fieldkin compares every source field with every target field using three signals:

1. field-name similarity
2. declared-type compatibility
3. optional sample-value overlap

Those signals are combined into one score.

- below the threshold -> `NoMatch`
- multiple candidates within the ambiguity margin -> `Ambiguous`
- otherwise -> `Match`

That is the whole algorithm.

## Run the answer-key evaluator

```powershell
cargo run --example evaluate -- eval/customers.json
```

## Run tests

```powershell
cargo test
```

## Where to tweak the matcher

- `src/signals.rs` — how names, types, and samples are scored
- `src/engine.rs` — weights, threshold, and ambiguity logic
- `eval/customers.json` — source schema, target schema, and answer key

The development loop is intentionally simple:

1. inspect the source and target schemas
2. inspect the answer key
3. run the evaluator
4. look at failures and their component scores
5. change the matching logic
6. run it again
