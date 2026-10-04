# Corrective evidence experiment

The new opt-in sample-support gate reduces false proposals on this synthetic
challenge corpus, but **does not meet the release quality bar**. The ordinary
weighted default remains unchanged. No crate or release tag was published.

Use `Config { corroboration: Some(Corroboration::default()), ..Default::default() }`
to require positive built-in name evidence and a raw distinct-aware sample score
of at least 0.5. Scores and candidate ranking are preserved; eligibility,
ambiguity and assignment are recomputed. See the [API guide](corroboration.md)
and [compiling example](../examples/corroboration.rs).

## Protocol and provenance

- Experiment settings and family partitions: `090b100`.
- Fresh fixture data and labels committed before any evaluation: `fb975cd`.
  A redundant compile-time assertion was subsequently removed for clippy; no
  fixture data, labels or partitions changed.
- Feature/evaluator/tool freeze: `db47b17b45097b82f34d54af8071a5b4cc02955b`.
- Protocol-only commit recording that SHA before fresh holdout scoring: `f702377`.

The four prespecified models use threshold 0.70 and ambiguity margin 0.08:
combined (unchanged weighted default), sample_gate (the opt-in preset),
corroborated (a stricter 0.80 name-floor ablation), and true name_only. No threshold
grid was fitted, and no defaults or settings changed after examining fresh results.
Analysis of previously published proposals informed the opt-in preset before the
protocol freeze; those data are explicitly regression evidence now.

The new corpus has 24 separately authored synthetic families across fulfillment,
subscription billing, research labs and civic records. Each family has six source
fields and three correlated variants: base, reordered and null-heavy. Odd families
01/03/05 in each domain are development; even families 02/04/06 are holdout.
Each partition contains 12 families, 36 pairs and 216 labels. Original fixtures
remain intact as a separate 200-pair, 1,200-label regression set.

Fixture authorship was separate from scoring implementation and used no matching
outputs to assign labels. This is original MIT OR Apache-2.0 project data, **not
independent consumer or third-party validation**. The cases deliberately stress
vocabulary changes, hidden semantic distinctions, coincident samples, missingness
and duplicate scopes. They are not a representative production distribution.
The evaluator passes no gold concepts, labels, family IDs or rationales to matching.

The final holdout was scored once after the evaluator verified the Git commit's
actual source/fixture/configuration/dependency contents. Only the recorded freeze
SHA itself may differ from that commit. Reports retain source and dependency hashes,
per-field outcomes and full-report digests, without sample values. CI scores only
fresh development and explicitly designated original regression data.

## Fresh held-out results

The holdout contains 135 unique-match labels, 60 no-match labels and 21 ambiguous
labels. Unique coverage counts proposals on uniquely labeled fields, including
wrong targets; correct recall counts only correct targets.

| Model | Assignment | Correct/proposed | Precision | Correct recall | Unique coverage | False proposals |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Combined | Independent | 20/75 | 26.67% | 14.81% | 17.04% | 55 |
| Sample gate | Independent | 20/52 | 38.46% | 14.81% | 16.30% | 32 |
| Strict name ablation | Independent | 4/32 | 12.50% | 2.96% | 4.44% | 28 |
| Name-only | Independent | 0/54 | 0% | 0% | 2.22% | 54 |
| Combined | One-to-one | 18/73 | 24.66% | 13.33% | 15.56% | 55 |
| Sample gate | One-to-one | 18/50 | 36.00% | 13.33% | 14.81% | 32 |
| Strict name ablation | One-to-one | 4/32 | 12.50% | 2.96% | 4.44% | 28 |
| Name-only | One-to-one | 0/54 | 0% | 0% | 2.22% | 54 |

The sample gate retains exactly the same 20 correct independent pairs and 18
correct one-to-one pairs. Its 32 false proposals include 30 no-match proposals and two wrong
unique targets. Every model abstains on all 21 ambiguous labels. Candidate recall@5
is 100% for the three combined models and 88.14% for name-only. The metric includes
ineligible candidates in small target schemas; it does not establish proposal
quality. None of the models meets the combined 95% precision / 60% unique-coverage
release goal. Stronger lexical thresholds can remove useful renames while retaining
false exact-name matches, as the strict ablation demonstrates.

On fresh development, combined and sample gate both make 20 correct proposals;
false proposals decrease from 54 to 32, precision increases from 27.03% to 38.46%,
and unique coverage stays 14.18%. On original regression, independent precision
increases from 66.67% (372/558) to 75.76% (225/297), but unique coverage falls from
43.91% to 26.21%. These partitions are reported separately, never pooled into an
apparently general accuracy figure. The gate's coverage cost is real.

Some false proposals already have name, type and sample scores all equal to one.
Requiring those same signals cannot verify the missing semantic distinction.
The gate also rejects valid disjoint, absent, constant or null-heavy samples.
Fully overlapping two-value columns can meet the 0.5 floor. Caller verification
and truthful semantic metadata remain necessary.

Read [held-out scenario slices](../evaluation/results/corrective-v1/fresh_holdout.md),
[development results](../evaluation/results/corrective-v1/fresh_development.md),
[original regression](../evaluation/results/corrective-v1/original_regression.md)
and their adjacent JSON/JSONL files. `analyze_support.py` is a separate diagnostic
over old selected proposals; it does not recompute assignment. The actual gate
corrects three formerly wrong selected targets in each assignment mode on original
regression, so those diagnostic filter counts differ from real engine results.

## Reliability and cost

Validation covers **184 passing tests**, including seven doctests and 17 gate tests
with a 256-case property, plus one intentionally ignored preparation microbenchmark.
Rust 1.85/1.99 formatting, strict clippy and warning-free documentation pass.
The extended generated campaign passed **1,000,000 cases**, seed `20261004`, in
19.28 seconds in the Rust process. Each of five categories contributes 200,000
cases; report cases randomly exercise disabled, sample-gated and strict modes.
Repeated calls and assertions do not inflate case counts. See the
[campaign record](../qualification/results/corrective-v1/run.json).

Historical default decisions, scores and full reports remain unchanged. The
explicit historical `--check-behavior` gates permit only current engine/evaluator
source hashes to differ; protocol, fixture, dependency, metric and prediction
changes remain failures. Exact `--check` remains available for the historical
version. No runtime dependencies, integrations or publishing access were added.

Measurements use Rust 1.85 on the recorded Windows x86-64 AMD Ryzen 5 5625U machine
(six cores, twelve logical processors), five separate processes, without concurrent
builds/tests/campaigns, CPU affinity or fixed CPU frequency. Values below are medians
of process means; brackets show their observed min/max, not confidence intervals.

The existing 50-workload protocol compares unchanged-default behavior at `3b8d65a`
against the frozen candidate. All allocation/deallocation/reallocation counts and
byte totals match across all 50 workloads. Sampled 128-field combined timings are
40.81 ms [35.51–62.50] before versus 38.24 [36.43–42.39] after independently, and
58.34 [50.78–61.09] versus 53.65 [50.46–69.07] with assignment. There are also slower
medians: missing-sample 128-field name-only assignment rises 15.86%, from 25.01 to
28.97 ms, with overlapping ranges [24.09–37.19] and [24.36–32.57]. Empty-target
assignment rises 13.31%, from 2.68 to 3.04 ms; its observed process ranges do not
overlap. Five processes do not establish statistical certainty. These results
do not support a general speedup or a zero-overhead guarantee. The
[complete table](../performance/results/corrective-v1/default-regression/summary.md)
includes every workload; its title retains the Stage 2 protocol name, while
`build.json` identifies the actual revisions.

A separate 36-workload benchmark compares gate disabled/enabled on the same frozen
candidate. For sampled 128-field schemas, independent calls rise from 34.03 ms
[33.73–35.22] to 41.12 [37.77–43.43], and assignment calls from 48.64 ms
[45.21–49.99] to 62.16 [59.23–67.11]. Eligibility and extra explanatory warnings can
change cost; the gate performs no extra sample scans or matcher callbacks.
Timing includes returned-report destruction, excludes input/engine construction,
and uses fixed workload order. See [all measurements](../performance/results/corrective-v1/gate-cost/summary.json).

## Reproduction and next work

```text
cargo +1.85.0 run --locked --example corroboration
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --corrective --check --output evaluation/results/corrective-v1
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --stage3 --check-behavior --output evaluation/results/rc-v1/stage3-development
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --release --check-behavior --output evaluation/results/rc-v1/release
python qualification/run.py build --output target/corrective-qualification
python qualification/run.py run --output target/corrective-qualification --cases 1000000 --seed 20261004
python performance/corroboration.py build --output target/corroboration-cost
python performance/corroboration.py run --output target/corroboration-cost
git worktree add --detach .local/corroboration-baseline 3b8d65a
python performance/run.py build --baseline .local/corroboration-baseline --candidate . --output target/default-cost
python performance/run.py run --output target/default-cost
```

Use fresh output directories and the frozen checkout for exact provenance; skip
the worktree command if that baseline already exists. Normal tests, clippy and
formatting follow CONTRIBUTING and CI for both compiler versions. The final run
used `--corrective --output evaluation/results/corrective-v1 --acknowledge-holdout`
after recording and verifying the freeze. Do not treat these now-examined families
as untouched validation in the next iteration.

The next accuracy work needs independently labeled, appropriately licensed consumer
schemas and application-verified semantic distinctions, with a new reserved test
partition. Richer evidence must address meaning and vocabulary without guessing
hidden facts. Preserve the failures here as regression cases and evaluate useful
coverage alongside precision. The optional gate is an application control, not
a completed solution to that research problem. Fieldkin stays experimental and
unpublished; publication remains @limadog9's decision.
