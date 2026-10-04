# Recomputing proposals after review

`MatchEngine::match_schemas_with_constraints` accepts application-owned review
decisions for one matching call. Use stable field IDs to confirm a correspondence,
forbid a pair, or keep a source unmatched. This is useful when an importer or data
catalog already has a reviewer and needs the remaining proposals to respect that
review. The library stores no review state and rewrites no data.

```rust
use fieldkin::{Config, FieldPair, MatchConstraints, MatchEngine};

let engine = MatchEngine::new(Config { one_to_one: true, ..Config::default() })?;
let review = MatchConstraints {
    confirmed: vec![FieldPair::new("source-total", "target-gross")],
    forbidden: vec![FieldPair::new("source-total", "target-net")],
    unmatched_sources: vec!["source-note".into()],
};
// engine.match_schemas_with_constraints(&source, &target, &review)?;
# let _ = (engine, review);
# Ok::<(), fieldkin::MatchError>(())
```

The [complete importer example](../examples/reviewed_import.rs) starts with an
ambiguous amount, records reviewer decisions, and recomputes proposals. Reserving
a confirmed target can resolve ambiguity for other sources in one-to-one mode.
Independent mode permits several sources to share a target; a confirmation there
does not reserve it for just one source.

## Human decisions are separate from evidence

A confirmed pair receives `Decision::Confirmed`. Its actual heuristic score and
signal evidence remain unchanged, even when the score is zero. Its
`Candidate::eligible` flag still describes automatic eligibility and can be false.
Confirmation overrides automatic score/support requirements and local abstention;
it does not manufacture stronger evidence or a probability. Eligible alternatives
remaining after caller exclusions stay visible on confirmed sources. Use the decision variant to distinguish a confirmed
mapping from `Decision::Proposed`, which still needs application review.

Confirmations cannot override conflicting supplied semantic hints or the enabled
incompatible-type veto. Correct erroneous metadata or explicitly configure the
type policy if the application intends a different matching contract. Fieldkin
does not infer that a conversion is safe merely because a reviewer selected a pair.

Forbidden pairs retain ranked scores and explanations but become ineligible.
Explicitly unmatched sources receive `Decision::ExcludedByCaller` and no selected
candidate. In one-to-one mode, other sources cannot use a confirmed target.
Structured candidate issues and field diagnostics explain these exclusions.
The returned unmatched lists account for both confirmed and proposed mappings.

Global assignment and alternative diagnostics run on the **remaining automatic
assignment graph**, with confirmed sources, reserved targets and caller-excluded
edges removed from eligibility. Diagnostic objectives exclude fixed confirmations;
they sum only automatic selections. Alternative witnesses never undo review
decisions. Completeness is conditional on supplied constraints and the usual local
abstention policy; it does not establish semantic correctness.
The report matrix retains its original dimensions with excluded entries masked.
The private solver can omit target columns with no valid positive edge, while
retaining every source row and the original dummy-column order, then expand the
result back to original IDs. Diagnostic work and floating-point
tolerance still use the original dimensions; fixed rows are not probed.

## Validation, limits and persistence

Unknown IDs and contradictory decisions are typed `InvalidConstraints` errors.
The engine rejects two targets confirmed for one source; in one-to-one mode it
also rejects two sources confirmed for the same target. A confirmed pair cannot
also be forbidden, and a confirmed source cannot also be explicitly unmatched.
Identical repeated directives are idempotent. Reordering schemas or directives
does not change a valid report.

Raw confirmed and unmatched lists are each bounded by `limits.max_fields`;
the forbidden list is bounded by `limits.max_pairs`. IDs use the existing byte
limit. Entire schemas and pair/signal budgets are validated before matching,
including fields a reviewer excludes. Valid calls still evaluate every pair and
charge all signal explanations: constraints do not bypass input validation or
custom matcher callbacks. Displayed top-k never restricts review decisions.

An empty `MatchConstraints::default()` produces the same full report as
`match_schemas`. No engine defaults, runtime dependencies or persisted formats
change. Store review decisions with schema versions in application-owned state;
Fieldkin detects absent IDs but cannot detect an ID reused for a different meaning.
Caller decisions are trusted input, not extra automatic accuracy. Evaluation must
report confirmed decisions separately and must not count gold labels supplied as
constraints as autonomous matching successes.
