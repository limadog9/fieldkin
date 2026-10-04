# Bounded scale experiment

Recorded results for the retained `5bfaa29` runtime are in
[continuation-v2](../performance/results/continuation-v2/scale/summary.json).
All 18 cases completed in five processes. Builtin 1,000-field medians were
1,794.606 ms independently and 3,102.716 ms one-to-one on the recorded Windows
machine. The [scorecard](continuation-scorecard.md) includes ranges, smaller sizes,
failed before/after latency gates and the rejected optimization. Earlier
`continuation-v1` results remain separate evidence for the rejected candidate.

The scale experiment characterizes exact all-pairs matching at 128, 512 and
1,000 fields per schema. It raises caller budgets for these particular fixtures;
the library's default 128-field and 16,384-pair limits remain unchanged. It adds
neither candidate pruning nor a new prepared-schema API.

## Fixed inputs and scope

The [public-API harness](../examples/scale_cost.rs) defines 18 workloads: three
sizes, three fixture families and independent/one-to-one selection for each.
Source and target IDs are stable; target input order is reversed. No input or
sample values are logged.

| Family | Evidence | Expected outcome |
| --- | --- | --- |
| `diagonal` | One synthetic matcher returns 0.9 on matching source/target indices and zero elsewhere | Every source selects its matching target |
| `partial` | The same matcher removes every eighth diagonal edge | 16, 64 or 125 sources and targets remain unmatched, respectively |
| `builtin` | Default name, type and distinct-sample matchers; integer fields named `MetricNNNNCount` / `metric_NNNN_count`; 16 distinct exact integer samples per field, shared only with its corresponding target | Every source selects its matching target |

All field pairs are evaluated, including synthetic zero edges. Each call retains
one displayed candidate per source; this does not limit the assignment graph.
The ordinary 0.70 score threshold, 0.08 ambiguity margin and ambiguity abstention
policy remain enabled. Global diagnostic probes are disabled. The partial fixture
has a bounded number of unmatched rows; it is not a worst-case dense-tie graph.

Caller limits are explicit:

| Fields per schema | Pair budget | Synthetic signal budget | Built-in signal budget |
| ---: | ---: | ---: | ---: |
| 128 | 16,384 | 16,384 | 49,152 |
| 512 | 262,144 | 262,144 | 786,432 |
| 1,000 | 1,000,000 | 1,000,000 | 3,000,000 |

The field budget equals the fixture size, and the aggregate explanation budget
is 512 MiB for every case. Other limits remain at their defaults. These are
ceilings, not claimed resident-memory measurements. Global matching retains
full pair evidence until assignment, so large calls can consume substantial
memory even when only one candidate is displayed.

## Measurement protocol

The [driver](../performance/scale.py) pins Rust 1.85.0, creates a fresh isolated
release executable, and records recursive library input, harness, helper-script,
manifest, lockfile, Cargo-configuration and executable hashes. Compiler, recorded
inputs and configuration must match before and after measurement. A build does
not run any measurements. Existing output destinations are rejected.

After all builds and tests stop, five fresh processes run the same fixed workload
order. Each workload has one warmup call whose complete selections and unmatched
counts are checked outside timing. The 128-field cases time three calls; the
512- and 1,000-field cases time one call. Timing includes matching and destruction
of each returned report, and excludes fixture/engine creation and warmup report
validation. Reported medians and ranges summarize the five process means. There
is no CPU affinity, frequency control or confidence-interval claim.

The driver checks all 18 output records, pair/signal-product counts, selected and
unmatched counts, and zero diagnostic work. Pair/signal counts describe the
configured full product rather than instrumented callback counts. Raw JSONL uses
explicit LF newlines so its recorded byte hashes survive archival normalization.

```sh
python performance/scale.py build --output performance/results/scale-v1
# Stop concurrent builds and tests before running the measurement command.
python performance/scale.py run --output performance/results/scale-v1
```

The small fixture unit tests and parser tests do not measure large cases:

```sh
cargo +1.85.0 test --locked -p fieldkin --example scale_cost
python -m unittest discover -s performance -p test_scale.py
```

## Interpretation and limits

This is one frozen candidate's scale characterization, not a comparison with
other libraries or a claim of production suitability at 1,000 fields. The
synthetic matcher makes graph shapes deliberate but supplies no real matching
evidence. The built-in controls exercise actual preparation and matching on
ideal disjoint sample sets; they do not establish quality on arbitrary schemas.
No held-out evaluation data is involved.

This experiment does not measure allocations or peak memory. The separate
50-workload before/after comparison uses existing allocator instrumentation to
check solver scratch reuse at the ordinary sizes. Neither allocation totals nor
the configured byte budgets are peak-memory measurements. Successful scale
calls do not justify increasing default limits, exposing reusable prepared
schemas, or enabling candidate pruning without a concrete consumer requirement.

Recorded hashes detect accidental local drift. They are not a signed attestation
or a hermetic-build guarantee, and assume the wrapper, compiler, dependency cache
and local processes are trusted.
