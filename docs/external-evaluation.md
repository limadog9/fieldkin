# External correspondence evidence

The next evaluation phase adds independently authored schema correspondences from
the original [T2D Gold Standard](https://webdatacommons.org/webtables/goldstandard.html),
by Dominique Ritze, Oliver Lehmberg and Christian Bizer. Its License section
explicitly grants Apache-2.0 for correspondences, separately from the terms for
raw web tables and DBpedia values. Only correspondence files are redistributed;
no row values, Common Crawl tables or DBpedia instance data are bundled.

The original annotation snapshots, license, attribution/modification notice,
source URLs and SHA256 hashes live in
[`evaluation/external/t2d-v1`](../evaluation/external/t2d-v1/).
The importer and derived task were committed at `bc39722` before any model scoring.
Library packaging excludes this development-only data. The in-memory Rust library
gains no runtime dependency, network access or data adapter.

## What the task measures

This is an **induced annotation-only task**, not the native T2D benchmark. For each
table, source fields are only the columns present in its correspondence file.
Their original headers and column indices are preserved, including empty headers.
The target vocabulary is the sorted union of annotated property URIs for that
table's class, with URI fragments used as display names. Thus the target vocabulary
is constructed using labels; it is not an independently supplied consumer schema.
The class itself is assumed known rather than inferred by Fieldkin.

Types are `Unknown`, samples are absent, and no metadata hints, aliases beyond
the existing defaults, caller confirmations or forbidden pairs are supplied.
Only explicit correspondences are known positive. Every other pairing has an
unknown outcome. Missing annotation is not a false-match or no-match label.
Multiple positive targets for one source index would form an acceptable set;
the pinned included data have one correspondence per source field.
Different source fields can share a property: 41 development tables contain this
situation. One-to-one assignment cannot retain every such positive edge, so its
recall must be reported separately rather than treated as universally appropriate.
Its label-based field-recall ceiling is 1,400/1,451 (96.49%), even for a perfect
matcher under this task's known-positive labels.

The downloaded snapshots produce 767 table schemas, 90 classes and 2,136 positive
correspondences. They exclude 971 empty attribute files and ten nonempty files
without class metadata (12 correspondence rows). All excluded table IDs and
reasons are retained. These are observed snapshot counts; historical narrative
counts on the source page differ. Do not silently reconcile them by deleting data.

## Split and fixed policy

The first SHA256 byte of the UTF-8 class URI, modulo five, reserves the whole class
when zero. The other classes are development. This rule gives:

| Partition | Classes | Table schemas | Annotated fields / positive edges |
| --- | ---: | ---: | ---: |
| Development | 71 | 549 | 1,451 |
| Reserved holdout | 19 | 218 | 685 |

No class crosses partitions, but ontology properties can recur across classes and
tables within a class are correlated. The public corpus is not concealed. The
holdout has **not been scored**; this evaluator deliberately has no holdout entry
point. A later accuracy experiment must freeze its policy and implementation
before adding an explicit verified final-scoring path.

The [frozen protocol](../evaluation/external-protocol.json) compares the unchanged
combined default and a true name-only engine, each independently and one-to-one.
Threshold 0.70, ambiguity margin 0.08, top-k five and all ordinary budgets remain
fixed. No threshold sweep or tuning is performed. Oversized or invalid inputs
are reported as rejections, with their annotated fields retained in denominators;
names are never silently shortened or inferred into richer types.

The combined default gives names weight 0.65. With types unknown and samples
unavailable, its maximum possible score is below 0.70, so it cannot propose a
mapping in this task. That follows from the existing missing-evidence policy;
it is not fixed by supplying gold labels as semantic hints or lowering thresholds
after examining results. Name-only remains a separate comparison with weaker
evidence requirements, not a newly recommended production default.

## Interpretation and reproduction

Report known-positive proposal recall, known-positive candidate-edge recall@5,
unknown proposals, abstentions and input rejections. Candidate recall includes
ineligible ranked pairs. **Precision and unmatched-field safety cannot be measured
from these positive-only annotations.** Counts of unknown proposals must remain
visible and must not be renamed false positives or silently excluded to imply
overall accuracy. Nothing here establishes calibrated scores or production safety.

```text
python evaluation/import_t2d.py --check
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --external --output target/t2d-development
cargo +1.85.0 run --locked --release -p fieldkin-eval -- --external --check --output evaluation/results/t2d-v1
```

Fixture reproduction is offline by default and verifies pinned upstream bytes
before conversion. It reads archive members in memory without extracting paths.
CI runs development only and verifies deterministic results. Original synthetic
results and the missed 95% precision / 60% coverage release targets remain in
force; these limited external annotations cannot certify those targets. Genuine
consumer validation still needs independently provided source/target schemas,
representative values when available, and explicit negative/unmatched labels.
