# Northix: independently labeled no-match evidence

This protocol evaluates the unchanged library on
[Northix](https://archive.ics.uci.edu/dataset/237/northix), by Farid Bourennani
(2012), UCI Machine Learning Repository, DOI
[10.24432/C5M60J](https://doi.org/10.24432/C5M60J). UCI explicitly licenses the
dataset under CC BY 4.0. The unmodified archive, full license, attribution,
license-source record and provenance are in
[`evaluation/external/northix-v1`](../evaluation/external/northix-v1/).
These development-only data retain their own license and are excluded from the
Rust library package.

Northix groups 115 column entities from Northwind and Sakila demonstration
databases into 33 manually authored correspondence classes and one `UNCLASSED`
group. The latter explicitly identifies fields with no similar counterpart.
The source describes injected tuples and coarse semantic classes. It is an
independently authored diagnostic, not independently validated consumer data or
a guarantee of business-compatible mappings.

## Task and input construction

The [offline importer](../evaluation/import_northix.py) reads the pinned ZIP in
memory. It bounds member counts, declared and read sizes, and total expansion;
rejects unsafe/noncanonical paths, duplicate IDs, links and unsupported members;
and never extracts paths or executes files. The original archive is 1,240,631
bytes with SHA256
`2b158f99e1a041311177e9519c0d93a3821400121b809cbf7cb4614c6635a838`.

Only `InputData/` supplies names and values. `Classes/` supplies labels. Fifteen
duplicated value files under `Classes/` differ from their corresponding input
copies; every discrepancy and both hashes remain in the fixture. No class name,
class-derived type, alias, semantic hint or caller confirmation enters a matcher.

Original filenames identify an attribute, native table and database. The full
filename is the stable field ID; the original attribute is its display name.
We evaluate every original database-1 table against every database-2 table:
12 times seven, or 84 table pairs. Table names and IDs are not matching evidence.
Duplicate display names across native tables remain distinct fields. The importer
rejects duplicate display names within a table rather than silently renaming
columns for a DataFrame adapter.

All declared types are `Unknown`. A names-only input view supplies no samples.
The sampled view supplies at most 64 line records per column, including blanks:
for `n` rows and `k=min(64,n)`, choose index `floor(i*(n-1)/(k-1))` for each
`i=0..k-1`. Empty and singleton columns use `[]` and `[0]`. Indices are unique,
include both endpoints and depend on neither labels nor scores. This regular
subsample is reproducible, not a statistical guarantee against ordering bias.

The input-byte audit found 104 ASCII-only files and 11 containing non-ASCII
bytes. None uses bytes `0x80..0x9F`; ISO-8859-1 and Windows-1252 agree for every
input byte. The importer explicitly decodes ISO-8859-1 and rejects newly ambiguous
control bytes. It retains nonblank lines exactly as `SampleValue::Text` and
represents whitespace-only lines as `Null`. No number/date inference or value
truncation is performed. It retains full row counts, sample indices, blank counts
and original file hashes. Independently sampled columns do not assert aligned
rows; pandas padding is only a column container for the comparison.

## Labels and denominators

The fixed task defines a positive pair as two fields sharing a published class
other than `UNCLASSED`. Every other pair is a task-defined negative. This
closed-world interpretation follows the published grouping, not a new judgment
that different names or samples establish different semantics.

| Inventory | Count |
| --- | ---: |
| Unique source / target fields | 67 / 48 |
| Table pairs with / without a positive edge | 16 / 68 |
| Source-field occurrences across all 84 tasks | 469 |
| Positive edges / positive-field occurrences | 28 / 28 |
| Complement no-match occurrences | 441 |
| Explicitly `UNCLASSED` source-field occurrences | 70 |

A complement no-match means that a field has no class-equivalent target in this
particular table pair. It is broader than the upstream `UNCLASSED` assertion;
both are reported separately. Ten unique source fields and nine unique target
fields are `UNCLASSED`. Repeated appearances of a field in seven tasks are
correlated observations, not seven independently labeled fields.

Classes can contain several acceptable targets across tables. Within these
particular 84 tasks each matchable source occurrence has one acceptable target,
but two source fields can compete for the same target. The label-derived
one-to-one positive-recall ceiling is 27/28 (96.43%). It must not be interpreted
as an error by a perfect matcher when that assignment policy excludes a valid
many-to-one correspondence. Independent assignment is the primary task view.

## Frozen comparison policy

The [protocol](../evaluation/northix-protocol.json) fixes threshold 0.70,
ambiguity margin 0.08, ambiguity abstention, top five candidates, ordinary input
budgets, no corroboration requirement and no global ambiguity probes. There are
eight native configurations: existing combined defaults and the true name-only
baseline, each with independent or one-to-one assignment, and with samples absent
or present. Name-only ignores samples; its repeated views serve as a consistency
check. Defaults, weights and aliases are unchanged.

The optional comparison runs Apache-2.0
[Valentine 1.0.0](https://github.com/delftdata/valentine) locally in a separate
evaluation environment. Its COMA matcher uses `max_n=0`, `delta=1.0`,
`threshold=0.0`, `use_schema=True` and `instance_weight=1.0`, once with instances
disabled and once enabled. Both see the same original names and preselected
values. Neutral table names `aaa` and `bbb` avoid supplying structural names that
Fieldkin does not consume. No models, credentials, services or LLM calls are
required by this comparison or added to the Rust library.

The producer exports all finite returned pair scores, including explicitly
returned zeros. An omitted pair remains missing evidence. The Rust evaluator
validates exact corpus/protocol hashes, all 84 task IDs, known stable field IDs,
unique pairs and scores in `[0,1]`. One custom matcher with weight one applies
Fieldkin's fixed threshold/ambiguity policy to external scores, independently or
one-to-one. These are **Valentine scores with Fieldkin selection**, not a claim
to reproduce Valentine's native decisions or historical paper results. No
Valentine convenience metric, dynamically chosen cutoff or post-result tuning
is used.

Report task-defined precision, positive field/edge recall, candidate edge
recall@5, proposal coverage, abstentions, complement no-match false proposals,
the explicit `UNCLASSED` subset and every input rejection. Recall denominators
retain rejected inputs, which are not successful abstentions. Candidate retrieval
includes ineligible candidates, but omitted external-score pairs cannot earn
retrieval credit merely by appearing as zero-score display fillers. A zero
proposal count has undefined precision, rendered `null`/`n/a`, not 100%.

## Reproduction and evidence handling

```text
python evaluation/import_northix.py --check
python -m unittest discover -s evaluation -p test_import_northix.py
cargo +1.85.0 test --locked -p fieldkin-eval northix::tests
```

Those checks reproduce metadata and exercise toy cases without scoring Northix.
After committing the policy and implementation freeze, a recorded run can use:

```text
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --northix --output target/northix-first-run
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --northix --output target/northix-with-valentine --scores path/to/schema-only.json --scores path/to/schema-and-samples.json
```

Outputs are `results.json`, `predictions.jsonl` and `results.md`. A write run
requires a fresh output directory. `--check` recomputes and strictly compares
existing artifacts, including all recorded source/input hashes; it does not
replace them. Score files are bounded to 16 MiB each. Input inventories are
checked before and after execution. These source hashes alone do not attest to
the executed binary; use the verified build/run workflow for recorded evidence.

This corpus is a fixed external diagnostic with no tuning split or unseen-domain
claim. Its 28 positive edges cannot establish the library's 95% precision / 60%
coverage release target. The separate 218-table T2D holdout remains reserved,
unloaded and unscored. Results and the exact implementation-freeze revision will
be recorded after qualification; this initial document specifies the experiment.
