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
observable publisher evidence. The initial 16 cases contain no ambiguity labels.
Case 17 adds two documented equivalent epoch representations; this narrow case
does not establish accuracy on ambiguous business roles or uncertain semantics.
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

### Ceres epoch-representation extension

Case 17 adds complete NASA/JPL Horizons state-vector and orbital-element schemas
for Ceres, archived by Astroquery: four records, two ambiguity labels and nine
negatives. The publisher's TIME sections and headers identify `JDTDB` and
`Calendar Date (TDB)` as full encodings of the same TDB epoch. Both are valid
targets under the unchanged encoding-equivalence policy. A unique selection of
either representation fails the strict ambiguity decision label but does **not**
select an incorrect fact; report these separately from semantically wrong
automatic correspondences. `Tp` denotes periapsis time, a different event.

The dictionaries distinguish instantaneous state components, range and light-time
from orbital elements. Joint coordinate transformations are not direct matches.
There are no unique-match labels, so positive recall and ranking recall for this
case are undefined. It is one astronomical object, not broad ambiguity evidence.
All previous 16 cases and their evidence remain unchanged; report this new case
separately after committing its inputs and labels before any predictions.

The original ASCII responses declare units, not scalar storage types: all fields
remain `unknown`. Headers retain publisher names with padding removed; sample
cells retain their original whitespace and numeric/date text. The trailing
unnamed empty CSV column is formatting and omitted. TDB is not converted to UTC.
The `$$SOE`/`$$EOE` delimiters identify the original CSV records; no duplicate
converted files are committed. See the notices for archive and licensing limits.

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

## Independent regression gate

CI runs both checks on every push and pull request:

```sh
cargo run --locked --example evaluate_all -- --check-independent
cargo run --locked --example benchmark_accuracy -- --independent --check
```

`validation/independent_quality_baseline.json` freezes quality counts measured at
`df5f8cc0ea295ebac756200f7839a1040a7f1123`, before the accompanying suggestion
decision changes. It records actual performance, including errors, not a claim
that current quality is sufficient. Suggestions had 90/105 correct decisions,
35/41 correct automatic matches and 8/43 incorrect automatic selections.
The five algorithm baselines retain their fixed configurations and selection
rules above. Original labels, corpus fingerprints and development baseline are
unchanged.

The suggestion gate reuses the development gate: correct decisions cannot fall,
and incorrect automatic or ambiguity decisions cannot increase, overall and
within every dataset and expected decision category. The Valentine gate protects
correct automatic matches, top-1 and recall@5 counts in each dataset, plus
ambiguity alternative retrieval. Incorrect automatic selections cannot increase
within any dataset's match, no-match or ambiguity labels. These per-dataset
floors and ceilings also protect aggregate precision and recall. Coverage can
decrease when false automatic selections disappear; an abstain-everywhere result
fails the correct-match floors. No exact prediction or score equality is required.

Both checks reject missing/changed corpus membership, inputs or labels and print
affected dataset/field diagnostics and metric changes on failure. The existing
`--independent` reports remain available without gating. Reference parity and
the development suggestion quality gate remain separate checks.

These cases have now been inspected during development. Their labels remain
independently sourced, but future improvements measured here are regression
evidence, not another blind generalization trial. New held-out evaluation still
requires separately sourced labels frozen before predictions.

### Missing-sample decision check (2026-10-12)

The accompanying suggestion change keeps the ranking formula, but
requires the best candidate's unnormalized name/type contribution to meet the
existing minimum when sample evidence is unavailable. Before the change,
`net_amount` versus `gross_amount` (both Decimal) abstained with disjoint samples
(score 0.659), but selected a match after removing those samples (score 0.775).
It now abstains in both cases. An additional abstention can trigger the existing
abbreviation fallback and change the returned ranking. These are constructed
regression examples, not independent annotations.

| Suggestion metric | Before | After |
| --- | ---: | ---: |
| Development correct decisions | 215/256 | 215/256 |
| Development automatic precision | 170/179 (94.97%) | 170/178 (95.51%) |
| Development automatic recall | 170/204 (83.33%) | 170/204 (83.33%) |
| Development incorrect ambiguities | 4 | 4 |
| Independent correct decisions | 90/105 | 91/105 |
| Independent automatic precision | 35/43 (81.40%) | 35/43 (81.40%) |
| Independent automatic recall | 35/41 (85.37%) | 35/41 (85.37%) |
| Independent incorrect ambiguities | 1 | 0 |

The baseline above remains the pre-change measurement. Broader containment and
score-normalization changes were rejected because they failed the unchanged
development gate. Name containment, exact-name preference and coincidental
value overlap can still produce incorrect matches; this change does not infer
semantic equivalence. Both independent ambiguity labels still fail. This is a
small decision-safety improvement, not evidence that unattended mapping is safe.
