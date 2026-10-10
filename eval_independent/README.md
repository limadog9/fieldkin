# Independent accuracy corpus

Fifteen pairs of actual published schemas, acquired and annotated on 2026-10-10,
without running Fieldkin predictions. These inputs have not appeared in
Fieldkin's development evaluation corpora or Valentine parity fixtures.

Run `cargo run --locked --example benchmark_accuracy -- --independent` from the
repository root. Add `--details` for field diagnostics or `--json` for every
metric by algorithm and dataset. No network access or Python is needed.

[Protocol, annotation rules and limitations](../validation/independent_accuracy.md)
explain what "independent" means here. [The frozen manifest](../validation/independent_corpus.json)
contains every source URL, pinned revision, retrieval date, SHA-256, native type
reference, table projection and per-source-field label justification.
Publisher originals are archived under `sources/`; the numbered JSON files are
small schema/sample projections. [Notices](NOTICE.md) cover their separate
licenses. [The measured report](../validation/independent_accuracy_report.md)
is separate from the inputs and annotations.
