# Embedding Fieldkin

Start with stable IDs, coarse declared types and representative samples. IDs
must be unique within each schema; duplicate names are allowed. Preserve IDs
across review and schema reordering. `Field::new` and `Schema::new` construct
ordinary owned Rust data; validation happens when matching.

Use `MatchEngine::new(Config::default())` for independent proposals. Inspect every
field's `decision`, selected candidate, typed issues and alternatives. Scores
are weighted heuristic agreements, not probabilities. Unmatched sources and
targets are part of the result, not failures to silently discard.

## Samples and verified knowledge

Choose samples representative of the field, not just convenient matching rows.
Distinguish unavailable (`None`), observed empty (`Some(vec![])`) and null samples.
The engine does not choose or truncate your samples. At least three non-null
observations are required by default; repeated constants still contribute zero
sample score and two-value columns receive partial support. Sample coincidence
cannot prove shared meaning, and missing evidence retains its missing weight.

Prefer exact `Integer` or `Decimal` values when identity/precision matters. Keep
the source representation before converting: a rounded float cannot recover its
original integer. Numeric sample kinds and text are not implicitly coerced.
Provide `SemanticHints` only from verified application metadata using one shared
case-sensitive vocabulary. Conflicting unit/currency/scope labels exclude a pair;
agreement alone does not prove equivalence. Caller labels are never inferred from
the proposed mapping. Checked aliases are lexical replacements, not semantic
facts. Optional profiles compare shapes and can agree on unrelated fields.

## Review and global assignment

[import_review.rs](https://github.com/limadog9/fieldkin/blob/main/examples/import_review.rs)
builds a review queue for a successful rename, ambiguous amount and unmatched
note. All proposals await application confirmation, and simulated feedback is
kept outside the matcher. A real UI can page through more candidates or increase
`max_candidates`; displayed top-k does not restrict the engine's decisions.

[catalog_review.rs](https://github.com/limadog9/fieldkin/blob/main/examples/catalog_review.rs)
uses partial one-to-one assignment, sees competing email fields and inspects a
global alternative. It retains every unmatched field and stores a curator's
accepted pairs in an application-owned map. Neither example rewrites data.

Use `MatchConstraints` with `match_schemas_with_constraints` to apply explicit
confirmed mappings, forbidden pairs or unmatched-source decisions. A confirmation
records caller review even when its original heuristic score is zero; it does
not become an automatic proposal. See the [review migration guide](migration.md).

For saved review sessions, enable the optional `json` feature and follow the
[JSON boundary guide](json.md) and
[persisted_review.rs](https://github.com/limadog9/fieldkin/blob/main/examples/persisted_review.rs).
The example exports a display report, saves explicit decisions, resumes after
field reordering and rejects a stale application-owned revision. Report exports
cannot be imported as confirmations. Keep trusted revision values outside the
incoming document and pass imported directives through the engine's validation.
Stable IDs and revision strings remain visible; free-form matcher text is omitted
from reports unless explicitly enabled. The application owns file storage,
authorization and any data rewriting.

One-to-one is appropriate only when two sources cannot legitimately share a
target. Local ambiguity and global competition are different: one source can
have a single plausible target that several sources want. Stable tie-breaking
makes the result reproducible; it does not justify acceptance. Enable
`global_diagnostics` only with explicit solve/work budgets and inspect completeness.
An alternative objective gap is a difference of sums of heuristic scores, never
a confidence interval. Review near alternatives even when the solver selected
one deterministically.

When representative overlapping samples are required by your application, enable
`Config::corroboration`. The [corroboration guide](corroboration.md) explains the
positive-name/sample-floor gate, its typed exclusion reasons and coverage costs.
The [example](https://github.com/limadog9/fieldkin/blob/main/examples/corroboration.rs)
retains a supported date rename and withholds an amount proposal with no samples.
The gate does not verify semantic equivalence or remove candidates from review.

## Limits and extensions

Defaults allow 128 fields per side, 16,384 pairs, 65,536 signal evaluations,
256 samples per field, bounded text and 16 MiB of signal explanations. Larger
limits opt into more work; no hidden pruning happens. Global alternatives have
separate solve/work budgets and report exhaustion without changing selections.
The work units bound the assignment algorithm, not wall time or peak memory.

Use typed `MatchError` variants to distinguish invalid input, configuration,
resource limits and extension failure. Samples/hints are redacted from built-in
debug output, but names, IDs and applied aliases are application metadata.
Custom matcher code is trusted: keep it deterministic, bound its work, avoid
panics and I/O, and never place sample values in explanations or matcher names.
Its arbitrary error text is suppressed by the engine; its explanation is not.

Before enabling a custom matcher or changing weights, build labeled development
fixtures with ambiguity and no-match cases. Compare name-only and ablated models
at useful precision/coverage targets, retaining an independent final test set.
The [tuning recipe](stage3-evaluation.md#application-specific-tuning-recipe) explains
the protocol. The current synthetic results do not establish deployment accuracy.
No background worker, database, network connection or API key is needed after
the library and its dependencies are built.
