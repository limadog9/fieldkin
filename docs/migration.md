# Migrating from the initial checkout

Fieldkin remains an unpublished experimental library. The maintainer delegated
implementation of the roadmap; the implementation decision is to accept the
source changes below before initial publication, with explicit migration notes.
This is not a stable 1.0 API commitment or a publication decision. @limadog9
retains final authority over release version and timing.

Adding public struct fields, adding variants to exhaustive enums and replacing
a tuple struct can break source compatibility under the
[Cargo compatibility guidance](https://doc.rust-lang.org/cargo/reference/semver.html).
Consumers following Git revisions should review these changes rather than
assuming the unchanged unpublished `0.1.0` manifest version means compatibility.

## Errors and reasons

`MatchError(String)` is now a non-exhaustive enum. Replace `error.0` with
`error.to_string()` for display. For handling, match the error category instead
of parsing an English message:

```rust
use fieldkin::{BudgetKind, MatchError};

fn resource_failure(error: &MatchError) -> bool {
    matches!(error, MatchError::BudgetExceeded(BudgetKind::Pairs | BudgetKind::Fields))
}
```

Nested configuration, input, budget and overflow enums identify the exact
category. Matcher failures preserve the registered matcher name while suppressing
arbitrary error details. Keep fallback match arms: error/reason enums are
non-exhaustive. English messages are for people and are not a serialization API.
Custom `Matcher::evaluate` still returns `Result<Evidence, String>`; no new
required trait methods or preparation lifecycle is imposed on extensions.

Reports add `Candidate::issues`, `FieldMatch::diagnostics`,
`MatchReport::target_competition` and `MatchReport::assignment_diagnostics`.
Candidate issues identify insufficient scores, unavailable evidence, type
incompatibility and semantic categories. Field reasons distinguish no eligible
target, local ambiguity, competition, displacement from the first ranked eligible
target and being left unmatched by global constraints. Existing `Decision` and
human-readable warnings remain. Prefer these typed fields to parsing warnings.
Code constructing report structs for tests must provide the new members.

`Config` adds `global_diagnostics`. Prefer `Config { one_to_one: true,
..Config::default() }` over exhaustive struct literals. The diagnostics default
is disabled and enabling it does not change scores or selections:

```rust
use fieldkin::{Config, GlobalDiagnosticsConfig, MatchEngine};
let engine = MatchEngine::new(Config {
    one_to_one: true,
    global_diagnostics: GlobalDiagnosticsConfig {
        max_solves: 16,
        max_work: 8_388_608,
        objective_margin: 0.08,
    },
    ..Config::default()
})?;
# Ok::<(), fieldkin::MatchError>(())
```

Exhausted diagnostic budgets return a valid match report with
`AssignmentDiagnosticStatus::BudgetExhausted`, not a matching error. Read the
status before drawing conclusions from an empty alternative list. `Complete`
applies only to the eligible graph after local abstention/type/hint constraints;
it is not a semantic guarantee. See [global diagnostics](stage4-diagnostics.md).

## Samples and fields

The [Stage 3 guide](stage3-migration.md) records the earlier changes:
`Field::hints`, `SampleMatcher::reliability`, exact `Integer`/`Decimal` sample
variants, checked aliases, and the deliberate distinct-support default. Construct
fields with `Field::new` and attach samples/hints through builders. Handle new
sample variants explicitly in custom matchers; do not silently cast exact values
to floats. Stage 4/5 does not change those weights, thresholds or scoring rules.

## Supported builds and deferred API work

The corrective cycle adds `Config::corroboration: Option<Corroboration>`.
Full `Config` literals must add `corroboration: None` to preserve existing behavior;
construction with `..Config::default()` needs no change. `Some(Corroboration::default())`
opts into positive name evidence and a raw distinct-aware sample floor of 0.5.
New non-exhaustive reason variants distinguish insufficient name and sample support,
and `ConfigurationError::Corroboration` reports invalid floors. Existing wildcard
error/issue arms remain valid. No default weights, scores, thresholds or selections
change. See [the gate's limits](corroboration.md) before enabling it.

New development and qualification require latest stable Rust, with Rust 1.99.0
used for the October 4, 2026 validation. The repository selects `stable`; older
compiler maintenance has ended. Edition 2021 is unchanged. CI exercises stable
across Linux, Windows and macOS. The default and `--no-default-features` library
builds retain the existing dependency graph. The optional `json` feature adds the bounded JSON
module and its Serde dependencies; `--all-features` exercises it. The separate
performance tool's `allocations` feature is not a library feature.

The optional [JSON boundary](json.md) supports the persisted-review reference
consumer. Its version-1 report format is export-only and its separate review
format contains explicit human decisions tied to trusted application revisions.
It does not derive serialization over schemas or sample values. Applications must
advance their revisions when prior decisions need reconsideration; these strings
are not authentication or automatic schema fingerprints. `Debug` is not a
stable wire format. Public reusable prepared schemas and richer transformations
remain deferred. The in-memory API has
no network, database, LLM, credential or runtime service requirement.

Fixture reproduction and verified build/run recording now use the unpublished
native Rust `fieldkin-tools` package. Replace Python importer/runner commands with
the commands in [verified evaluation](verified-evaluation.md) and
[evaluation](../evaluation/README.md). Imports check the vendored pinned archives
offline. Local Python/Valentine COMA execution is retired; archived score import
remains available in Rust. Historical artifacts retain their dated compiler and
runner provenance.

## Contextual evidence

`Config::contextual_evidence: Option<ContextualEvidence>` defaults to `None`.
Exhaustive configuration literals must add `contextual_evidence: None` to preserve
weighted matching; literals using `..Config::default()` need no change.
`Some(ContextualEvidence::default())` enables the experimental strict-identifier,
scoped-support policy. Its two options are `strict_identifier_samples` and
`scoped_support`, both true in the opt-in preset.

Unlike corroboration and name-conflict checks, this option can raise the overall
heuristic score: it takes the maximum of the original weighted sum, 0.90 when
informative lexical agreement has required support, and 0.95 when samples are
adequate. Original `SignalReport` evidence and weights remain unchanged and can
reconstruct the weighted base score. `ContextualScoreAdjustment` identifies the
transformation; no probability calibration is claimed.

`InsufficientContextSupport` explains a contextual exclusion and can produce
`Decision::InsufficientEvidence` when other automatic requirements pass. Caller
confirmations override this soft gate while retaining hard semantic constraints
and an enabled declared-type veto. Context is derived before review directives,
ambiguity, assignment and top-k. Only active concrete built-in name and default
distinct-aware sample signals qualify; otherwise construction returns
`ConfigurationError::ContextualEvidence`. Custom signal names cannot substitute.
JSON adds these issue codes; the saved-review format is unchanged. See
[development evidence and release limits](release-readiness.md).

## Caller review constraints

The precision improvement adds `Config::name_conflicts: Vec<NameConflictRule>`.
Full configuration literals must add `name_conflicts: Vec::new()` to retain
ordinary weighted matching; `..Config::default()` needs no change. Each rule
declares qualifier or unit alternatives with synonymous normalized phrases.
No rule is enabled by default. See [configuration and limits](precision.md).

Exhaustive `Decision` matches must now handle `InsufficientEvidence`. Enabled
corroboration returns it when an otherwise viable edge fails required support;
it previously returned `BelowThreshold`. This is a reporting change to that
opt-in policy; scores and selected proposals are unchanged. New non-exhaustive
candidate/field reasons and `ConfigurationError::NameConflicts` support callers
with wildcard handling. JSON reports add `insufficient_evidence` and a
`name_conflict` issue with `kind: qualifier|unit`; review JSON is unchanged.

`match_schemas_with_constraints` adds per-call `MatchConstraints` without changing
`Config` or existing report fields. `match_schemas` and empty constraints retain
their previous full reports. `FieldPair` addresses source and target IDs explicitly.

Exhaustive `Decision` matches must handle the new `Confirmed` and
`ExcludedByCaller` variants. A selected candidate under `Confirmed` can have
`eligible == false`, including a zero score: the caller has overridden automatic
evidence requirements, not supplied stronger signal evidence. Consumers requiring
automatic proposals should explicitly check `Decision::Proposed`. Both types of
selection count toward unmatched-list complements.

`MatchError::InvalidConstraints(ConstraintError)` and new non-exhaustive
candidate/field/budget reasons identify review conflicts and exclusions. Existing
wildcard handling continues to compile. Diagnostic objectives and alternative
witnesses cover the remaining automatic graph, excluding fixed confirmations.
See [review constraints](review-constraints.md) for validation, bounds and examples.
