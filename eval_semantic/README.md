# Semantic reliability trial

These four publisher-schema migrations are separate from the development and
earlier independent corpora. The financial cases and VISIT_OCCURRENCE migration
contain **55 fully held-out source fields**. The 13-field PROCEDURE_COST case was
independently annotated but partially exposed to the implementation author before
the candidate freeze; it must be reported separately.

Ground truth describes corresponding field roles, not guaranteed value equality
or a complete ETL transformation. Samples are publisher illustrations, not
production observations. Names, declared types and labels come from pinned CFPB,
Adyen and OHDSI sources. Sources, licenses, per-field evidence, extraction scripts
and source-only review records are under `eval_reliability/sources/semantic_*`.
The adapter verifies their bytes and the complete original case selection before
running a matcher, using `validation/semantic_corpus.json`.

```sh
cargo run --locked --example evaluate_all -- --check-semantic
cargo run --locked --example benchmark_accuracy -- --semantic --details
```

The original runtime's results are frozen in
`validation/semantic_quality_baseline.json`; existing baselines are unchanged.
See [the protocol and measured report](../validation/semantic_reliability_report.md)
for the rejected candidate, independence limitations and reproduction commands.
These data and tools are excluded from the published crate.
