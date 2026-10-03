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

The minimum supported compiler remains Rust 1.85.0, edition 2021. Qualification
also pins Rust 1.99.0, the current stable verified on October 3, 2026, across Linux,
Windows and macOS. There are no optional library features; default,
`--no-default-features` and `--all-features` expose the same API. The separate
performance tool's `allocations` feature is not a library feature.

Optional serialization is deferred deliberately. Both consumer examples use
Rust-owned review state and need no wire format. Adding derives over input
schemas could accidentally export sample values; a future versioned report-only
format needs a concrete consumer and independent validation. `Debug` is not a
stable wire format. Public reusable prepared schemas, confirmed/forbidden pair
constraints and richer transformations remain deferred. The in-memory API has
no network, database, LLM, credential or runtime service requirement.
