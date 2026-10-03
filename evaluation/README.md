# Fieldkin evaluation protocol v1

This development-only workspace package measures the unchanged initial matcher
before Stage 2 optimization or Stage 3 scoring changes. It is not a runtime
dependency of `fieldkin` and is not published to crates.io. All fixtures are
original synthetic data under MIT OR Apache-2.0, copyright 2026 Fieldkin
contributors. No external dataset, personal record or Valentine fixture is used.

## Reproduce

From the repository root, one command regenerates the complete corpus and both
baseline reports with the locked dependencies:

```text
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --split all --acknowledge-holdout --output target/evaluation-full
```

Routine development deliberately defaults to the development partition:

```text
cargo +1.85.0 run --locked --release -p fieldkin-eval
```

Verify the committed development snapshot without writing files or evaluating
holdout outcomes:

```text
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --check --output evaluation/results/baseline-v1
```

`--split holdout` and `--split all` require `--acknowledge-holdout`. The flag is a
procedural safeguard, not secrecy: this is a public synthetic corpus. CI runs
only development predictions; structural validation covers the complete corpus.
The historical seven cases in `examples/baseline.rs` remain regression examples
and are not counted as new holdout observations.

## Corpus and ground truth

Four JSON files contain ten separately specified families each: product/import
data, financial records, CRM/contact data, and operational telemetry. Each family
has six source fields and 5-9 targets. Every source has one label and a rationale:

- `match`: exactly one true target represents the same fact in the same existing
  value representation. An erroneous declared type need not change ground truth.
- `no_match`: no target provides that direct correspondence. Unit/currency
  conversion, derivation, changed scope and related-but-different facts do not
  count as direct matches in this protocol.
- `ambiguous`: at least two targets remain plausible from the supplied evidence.
  Ground truth requires abstention. Selecting even a plausible target counts as
  an unsafe proposal; retrieving the alternatives is scored separately.

`concept`, family titles, tags and label rationales are evaluator metadata. They
are **never passed to a matcher**. Matching receives only IDs, names, declared
types and optional samples. Semantic metadata intentionally contains distinctions
that may be absent from observable evidence; these measure honest failure modes,
not an expectation that lexical methods infer hidden information.

Families are independently specified domain scenarios, not claims of independently
sampled real-world observations. A separate structural/semantic label audit was
performed before the first corpus scoring run. No matcher results were used to
select labels, thresholds or the partition. Reusable stress patterns such as
duplicate names occur across domains, so even distinct families may share design
assumptions. Counts are descriptive and do not support population accuracy claims.

Five fixed variants of each family yield **200 schema pairs and 1,200 labeled
source decisions**: base, seeded field reordering, all samples unavailable,
null-heavy samples, and separator noise. The last changes punctuation only; it
does not use the library's normalization implementation to generate expected
inputs. Null-heavy variants append three missing observations per supplied sample,
preserving existing emptiness/unavailability. They never add matching values.
Variants preserve ground truth and remain in their family's partition.

The [frozen protocol](protocol.json) assigns families numbered `04` and `09` in
each domain to holdout: **32 development families / 160 pairs / 960 decisions**,
and **8 holdout families / 40 pairs / 240 decisions**. No family crosses the split.
The seed is 20261003, with documented per-case seeds and a fixed wrapping LCG
Fisher-Yates permutation. Generating five variants does not create five independent
samples: scenario tables expose correlated and overlapping slices.

## Metrics, frozen before evaluation

Counts aggregate first, then ratios are computed (micro averages). Undefined
ratios are JSON `null` and Markdown `n/a`, never zero or a perfect score.

| Metric | Numerator / denominator |
| --- | --- |
| Proposal precision | Correct unique-target selections / all selections, including errors on unmatched and ambiguous labels |
| Unique-match recall | Correct unique-target selections / uniquely labeled sources |
| Unique-match coverage | Any selection on uniquely labeled sources / uniquely labeled sources; wrong targets still count as coverage |
| Overall proposal coverage | All selections / all labeled sources |
| Candidate recall@5 | Gold targets present among the first five ranked candidates / all gold targets, across unique and ambiguous labels |
| Unique/ambiguous candidate recall@5 | The same ratio restricted to that label kind |
| Candidate-any@5 | Sources with at least one gold target in their first five ranks / sources with nonempty gold target sets |
| No-match false-proposal rate | Selections on no-match labels / no-match labels |
| Ambiguous unsafe-proposal rate | Any selection on ambiguous labels / ambiguous labels |
| Expected-abstention accuracy | No selection on no-match or ambiguous labels / all labels of those two kinds |
| Feasible unique recall | Correct unique selections / maximum number of unique facts assignable under the chosen policy |

A wrong target on a unique label reduces both precision and recall. Abstaining on
a unique label reduces recall but creates no false proposal. Candidate retrieval
includes **ineligible** ranks: it measures whether useful alternatives are visible,
not whether the engine would select them. With only 5-9 targets, top-five retrieval
is a relatively easy task; it must not be confused with correct mapping.

In independent mode, feasible unique recall has the ordinary unique-field
denominator. In one-to-one mode it uses the number of distinct uniquely labeled
gold targets per case. This is an exact cardinality ceiling for this protocol's
single-target unique labels. Two redundant source facts may correctly share one
target; only one can be selected under a one-to-one constraint. We retain raw
field-level recall alongside the adjusted ceiling. Ambiguous labels require
abstention and are not used to increase this ceiling.

Results are separated by partition, assignment policy, domain, variant and
overlapping scenario tags. The JSON includes raw counts for every row. Do not
sum overlapping scenario rows or average their percentages. No significance
tests or confidence intervals assuming 1,200 independent samples are claimed.

## Frozen models and configuration

Each model runs both independent selection and optional one-to-one assignment
on identical inputs. Threshold 0.70, ambiguity margin 0.08, top-k 5, conservative
ambiguity abstention, aliases, sample minimum and resource budgets are explicitly
recorded in `protocol.json`; no thresholds were tuned for this baseline.

| Model | Evidence and veto |
| --- | --- |
| `combined` | Name 0.65, type 0.20, samples 0.15, incompatible-type veto; matches current defaults |
| `name_only` | Name weight 1.0; no type/sample evidence and no type veto |
| `no_name` | Name weight remains 0.65 but its evidence is absent; type/sample signals and veto remain |
| `no_samples` | Sample weight remains 0.15 but its evidence is absent; names/types and veto remain |
| `no_type` | Type weight remains 0.20 but its evidence is absent; type veto is also disabled |
| `no_type_veto` | All original signals and weights; only the incompatible-type veto is disabled |

Absent-evidence placeholders retain the default score scale for ablations; they
do not redistribute the removed signal's weight. This isolates a policy's effect
at fixed settings, not each subset's best achievable performance. In particular,
`no_name` has a maximum score of 0.35 and cannot pass the frozen 0.70 threshold.
Its perfect abstention cannot be interpreted as high matching accuracy. The
name-only comparator intentionally uses its own unit weight. Later threshold
sweeps belong on development data and must be reported separately.

The quarter's frozen aspirational quality targets remain precision >=95%,
unique-match coverage >=60%, and candidate recall@5 >=90%, without caller hints.
All false proposals on no-match and ambiguous labels count against precision.
This evaluation records how far the initial matcher is from those targets; it
does not redefine targets after observing the baseline.

## Artifacts and provenance

The committed [baseline directory](results/baseline-v1) contains:

- `corpus.json`: all 200 expanded cases, labels, seeds, license and provenance.
- `development.json` / `holdout.json`: configuration, baseline commit, complete
  dependency lockfile, source/manifest/corpus SHA-256 hashes, inventory, raw counts
  and derived metrics for every slice.
- `development.md` / `holdout.md`: readable aggregate tables.
- `development-predictions.jsonl`: per-case/model decisions, ranks and scores for
  diagnosis. It does not include sample values. No holdout prediction file is emitted.
- `environment.json`: compiler, architecture and OS used to run the evaluator.
  It is intentionally excluded from cross-platform snapshot comparison.

Hashes normalize CRLF to LF. Deterministic artifacts omit wall-clock timestamps,
absolute paths and timings. Compiler/environment identity is recorded separately
because it may differ across reproductions. The baseline engine revision is
`7c4da25b12998a3a92f7a36c9b84477ffcbe03ae`; its matching source and runtime dependencies
remain unchanged for Stage 1. The evaluator has its own source hashes so a scoring
bug fix cannot silently masquerade as an algorithm improvement.

The runner reads the workspace sources to record their hashes. Run it through
Cargo from this checkout; it is not a standalone distributed benchmark binary.
`--check` verifies corpus, summary and development prediction snapshots without
rewriting them. Generation refuses to overwrite an existing deterministic
artifact with different contents; choose a new output directory. Future deliberate algorithm or protocol changes should create a
new versioned result directory, preserve this baseline, and explain differences.

## Holdout discipline

Freeze fixture families, partitions, metric definitions and configuration before
measuring an algorithm change. The initial holdout is measured once for aggregate
baseline reporting. Routine CI does not score it. Do not inspect its individual
failures to tune thresholds, aliases or feature weights. If a held-out family is
used that way, move it into development in a new corpus version and replace it
with a separately authored family before making a new held-out claim.

This public corpus is not a concealed test set or independent third-party
validation. A future real-world licensed corpus and fair external matcher
comparison remain separate work; Stage 1 does not claim superiority to Valentine.
