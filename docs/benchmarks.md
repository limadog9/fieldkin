# Reproducible validation and benchmark record

Run on 2026-10-03 on an AMD Ryzen 5 5625U (6 cores / 12 logical processors),
Windows 11 Home 10.0.26200, `x86_64-pc-windows-msvc`, Rust 1.85.0
(`4d91de4e4`, 2025-02-17). Timings use Cargo's optimized bench profile and the
committed lockfile. The machine was not isolated; these are one-run wall-clock
observations, not statistical estimates or performance claims.

## Commands and checks

```text
cargo +1.85.0 fmt --all -- --check
cargo +1.85.0 clippy --locked --all-targets --all-features -- -D warnings
cargo +1.85.0 test --locked --all-features
cargo +1.85.0 test --locked --doc
cargo +1.85.0 doc --locked --no-deps --all-features
cargo +1.85.0 run --locked --example basic
cargo +1.85.0 run --locked --example domain_aliases
cargo +1.85.0 run --locked --example baseline
cargo +1.85.0 bench --locked --bench matching
```

Documentation was built with `RUSTDOCFLAGS=-D warnings`. Full tests passed:
16 unit tests, 20 integration/property tests, and 2 README doctests. The property
test generates 64 schema-pair cases, checks repeated-call and input-reordering
invariance, score bounds, candidate ordering and assignment uniqueness. The
assignment tests separately compare exhaustive matrices up to 3×3 over
`{forbidden, 0.5, 1.0}` and 500 deterministically generated matrices up to 5×5
against a brute-force optimum. Property-test failures retain a reproducible seed;
the benchmark inputs contain no randomness.

## Synthetic name-only comparison

`examples/baseline.rs` contains seven explicitly labeled cases: separator/case
renaming, a documented abbreviation, a misleading exact name with incompatible
type, duplicate-name ambiguity, unrelated fields, an opaque rename, and a hidden
gross/net mismatch. Both engines use threshold 0.70, margin 0.08 and the same name
normalization/aliases. The name-only engine has name weight 1, no other signals,
and its type veto disabled. Both abstain on local ambiguity.

| Engine | Correct proposals | Incorrect proposals | Missed labeled matches | Precision | Recall | Abstentions |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Combined | 3 | 1 | 1 | 0.750 | 0.750 | 3/7 |
| Name-only | 2 | 2 | 2 | 0.500 | 0.500 | 3/7 |

Precision is correct proposals / all proposals. Recall is correct proposals /
labeled correspondences; a wrong-target proposal counts as both an error and a
missed correspondence. Abstentions include ambiguous and below-threshold fields.
Both engines miss the opaque rename and falsely propose the hidden gross/net
pair. The combined engine resolves the misleading exact name using type and
sample evidence. This deliberately tiny, hand-selected set illustrates behavior;
it is not a representative accuracy estimate, tuning corpus or comparison with
Valentine.

## Timing inputs and results

The benchmark constructs deterministic schemas with names `Metric0000Count` and
`metric_0000_count` and reverses target order. Types are Integer. Sample-enabled
cases contain 16 values per field (four nulls and twelve deterministic numbers).
Every run has one warm-up. Input/engine construction is excluded, but validation,
pair evaluation, report allocation, assignment when enabled, and result drop are
included. Results pass through `std::hint::black_box`. Candidate limits and other
settings use defaults. Sample values are validated even by name-only matching.

| Fields per side | Samples | One-to-one | Iterations | Combined ms/call | Name-only ms/call |
| ---: | --- | --- | ---: | ---: | ---: |
| 16 | no | no | 30 | 1.241 | 0.906 |
| 16 | no | yes | 30 | 1.243 | 0.961 |
| 16 | yes | no | 30 | 2.262 | 1.314 |
| 16 | yes | yes | 30 | 2.105 | 1.009 |
| 64 | no | no | 10 | 19.026 | 14.944 |
| 64 | no | yes | 10 | 18.745 | 15.002 |
| 64 | yes | no | 10 | 27.377 | 16.311 |
| 64 | yes | yes | 10 | 26.928 | 19.379 |
| 128 | no | no | 5 | 128.159 | 54.932 |
| 128 | no | yes | 5 | 77.186 | 54.088 |
| 128 | yes | no | 5 | 99.252 | 53.847 |
| 128 | yes | yes | 5 | 101.509 | 61.155 |

These include all 24 engine/workload combinations. The noisier 128-field result
and apparent reversals between assignment modes demonstrate why this run cannot
justify fine-grained comparisons. Re-run on your deployment hardware with
representative sizes, longer samples and contention patterns. No assertion of
being the fastest matcher is made. Samples are currently prepared per pair,
which is a clear future profiling target.
