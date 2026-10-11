# Independent real-world accuracy protocol

## Scope and independence

This corpus was sourced and fully annotated on 2026-10-10 before any Fieldkin
prediction on its inputs. Fifteen cases use public observations and actual
publisher field names from Palmer LTER penguin studies; US BTS/FAA transportation
records and IEM weather; historic economic, industrial, road-casualty, crime,
hydrological, solar and oceanographic studies. It contains neither existing
`eval/` / `eval_realworld/` cases nor Valentine reference fixtures. The matcher
implementation is unchanged from the development benchmark.

The inputs, per-field evidence and fingerprints are committed separately before
the first benchmark run. The later measured report records that freeze commit.
`validation/independent_corpus.json` freezes both parsed inputs/labels and the
exact bytes of every acquired resource. Membership, labels and provenance are
validated before predictions. A changed freeze is a new evaluation edition,
not a way to repair scores. Git history supplies the manifest's immutable anchor.

"Independent" means held out from this repository's previous development and
annotation independent of its predictions. These well-known public statistical
series are not guaranteed unseen by the original Valentine researchers. There
was one annotator (the implementation agent), with a second evidence audit but
no independent human adjudication. This is a reproducible, evidence-backed
held-out measurement, not an estimate representative of all production schemas.

## Label rules and evidence

The manifest provides a label, explanation and archived publisher references
for **every retained source field**. References identify dictionaries, explicit
renames, published join keys, and original study definitions. Values were used
to verify extraction, never as sufficient evidence for semantic correspondence.

A `match` denotes the same attribute or documented lookup coordinate, including
publisher-renamed fields, different storage types, category-label encoding or
units. It does not assert that differently dated observations are equal.
Calendar-year coordinates can match across domains; a manufacture year cannot
match a departure year. A full date is not equivalent to its year component.
A code and its expanded name are separate attributes. Derived ratios,
aggregations, narrower populations and changed event definitions are not direct
equivalents. These rules were fixed before running the benchmark.

A `no_match` requires the full retained target dictionary to lack that attribute.
Examples include aircraft manufacture year versus flight year; driver deaths
alone versus drivers killed **or seriously injured**; and murder arrests in 1973
versus murders/non-negligent manslaughter in 1976. Three cases intentionally pair
unrelated or partially related **real** schemas as negative controls; they are
identified in the manifest and are not claimed to be published migrations.

An `ambiguous` label requires multiple target alternatives supported by the
observable publisher evidence. No such case was established confidently here;
there are **zero ambiguity labels**, and ambiguity accuracy remains unmeasured.
We excluded uncertain correspondences rather than manufacturing that category.
The existing development benchmark and its ambiguity metric tests remain intact.

## Input construction

### 2026-10-11 extension

Case 16 adds the complete published `datasets/global-temp` annual and monthly
schemas: three source fields, one verified match and two hard negatives. The
unchanged policy distinguishes annual averages from monthly averages and a
calendar year from a year-month coordinate, despite identical column names.
The publisher dictionary maps `string` to text, `number` to float, `year` (YYYY)
to integer, and `date` (YYYY-MM) to date. Samples use the same fixed 16-row rule.
All original 15 inputs, labels, resources and fingerprints remain unchanged.

The manifest records this separate annotation edition, frozen before its first
predictions. The initial 15 cases have already been evaluated; the combined
16-case measurement is not a new blind trial. Report case 16 separately. Its
mirror provenance and inconsistent historical publisher descriptions are
documented in the manifest and notices. This small addition tests aggregation
traps; it does not resolve the absence of independently supported ambiguity.

### Extraction

Publisher snapshots, revisions, retrieval dates, acquisition URLs and SHA-256
are committed under `eval_independent/sources/`. Each view separately records
original provider/documentation URLs, type evidence, row count and projection.
A GitHub mirror URL is never presented as a download from the original agency.
The R manuals' bibliographic citations are the authority for historic sources
without a surviving public data-download URL. Data/document licenses and upstream
limitations are described in `eval_independent/NOTICE.md`.

Every retained schema is complete for that published view. Export-generated
numeric row indices are omitted, including Longley's duplicated year row label;
meaningful state-name row labels are retained under the **published** `rownames`
field. Time-series `time` and `value` are actual publisher CSV export names.
The Seatbelts CSV does not expose its R time-series index as a field; we do not
invent one. R/statsmodels editions and the small published lookup tables make
this corpus considerably easier and narrower than unknown enterprise schemas.

Each table independently contributes at most **16 evenly spaced original rows**:
`k = min(16, n)` and zero-based index `floor(i * (n - 1) / (k - 1))`, including
both endpoints. This policy was selected without predictions and applies to all
algorithms. Samples preserve row ordering and CSV text. Published missing `NA`
and empty CSV cells, or native NA/NaN, become empty strings and receive the
existing missing-sample treatment. Native integers are rendered exactly;
native doubles use shortest round-trip text. Native POSIXct instants are rendered
as UTC ISO 8601, retaining timestamp semantics rather than timezone display.
No values are imputed or adjusted to improve overlap.

Types come from publisher-native R storage/constructors, publisher manuals and
explicit statsmodels loader declarations: factor/character -> text, R integer ->
integer, double/numeric -> float, Date -> date, POSIXct -> timestamp. Untyped CSV
fields whose loaders do not declare a type remain `unknown` (Engel numeric and
Grunfeld numeric fields except explicitly cast year). Types are not guessed from
answers. Source references and this mapping document representation differences.

Normal benchmark rerun (Rust only, offline after dependency installation):

```sh
cargo run --locked --example benchmark_accuracy -- --independent
cargo run --locked --example benchmark_accuracy -- --independent --details
cargo run --locked --example benchmark_accuracy -- --independent --json
cargo run --locked --example evaluate_all -- --independent
```

Optional reconstruction from archived inputs is separate from evaluation. Python
3, pandas, and `rdata==1.1.0` decode the original R files; they are acquisition
utilities, not Rust/library/benchmark dependencies:

```sh
python validation/independent/build_cases.py
```

This verifies extraction and hashes without writing or predicting. The explicit
`--write` mode intentionally reconstructs a new corpus edition; it must not be
used to accommodate predictions. The file manifest can also be checked without
those utilities by running the Rust benchmark or its integrity tests.

## Metrics and selection

Reuse `validation/accuracy_protocol.md`'s **unchanged** five fixed configurations,
positive-score per-source rankings, lexicographic tie-breaks, and `>= 0.5` top-1
automatic selection. All algorithms see identical declared types and sample
inputs. No one-to-one filtering or algorithm-specific answer-driven tuning.

Automatic precision = correct unique matches / all automatic selections;
recall = correct unique matches / unique-match labels; F1 is their harmonic
mean. A wrong/no-match/ambiguous automatic choice is a false positive, and a
wrong/abstained unique match is a false negative. No-match abstentions never
increase match precision. Coverage = automatic selections / all source fields.
Undefined denominators are `n/a` / JSON `null`.

**Ranking** top-1 accuracy and recall@5 use unique-match labels only, before the
cutoff. They measure candidate retrieval, not automatic match correctness.
The report includes automatic errors and missed matches, full per-dataset
metrics, and name/type condition slices. All-negative cases have undefined
ranking recall and still contribute their false positives and coverage.

Scores are algorithm-specific similarities, not calibrated probabilities. The
common 0.5 threshold is inherited unchanged for reproducibility. It can cause
one algorithm to abstain much more than another; it is not a fair assertion of
identical confidence. Compare raw ranking separately, and do not interpret high
coverage as accuracy or zero coverage as proof of bad ranking. This task does
not calibrate or optimize thresholds.

The corpus is clustered: NYC lookup cases share target records, parallel software
editions reuse the same underlying studies, and negative controls share views.
Micro-averages weight source fields, not independent dataset families. Only
16 records support distribution/instance methods, sometimes with few observed
values or all-missing fields. Native/R CSV representations differ in text
formatting. Publisher transcription/revision caveats (including USArrests) are
retained. No confidence interval or statistically significant winner is claimed.
Results measure this exact corpus/configuration, not universal generalization.

The original suggestion API uses `Config::default()` and explicit decisions,
without imposing the Valentine benchmark's 0.5 cutoff. Only a correct `Match`
is an automatic true positive; `NoMatch` and `Ambiguous` both abstain from
automatic selection. Decision accuracy additionally requires the correct kind
and, for ambiguity, exactly the labeled target set. An erroneous ambiguity is
reported separately from an incorrect automatic match. Precision excludes true
negatives. Per-case metrics and incorrect fields are printed by `evaluate_all`.

Valentine reference parity and the development suggestion-API quality gate
remain separate and unchanged. Independent runs report measurements, not a
new minimum-quality gate; input integrity and metric tests protect reproducibility.
