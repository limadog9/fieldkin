# Configurable precision checks

Fieldkin can exclude a candidate when its field names contain conflicting
caller-configured qualifiers or units, and can require corroborating evidence
before proposing a pair. These controls preserve the weighted scores and ranked
candidates. They change automatic eligibility, ambiguity and assignment.

The ordinary weighted policy remains the default: `Config::name_conflicts` is
empty, `Config::corroboration` and `Config::contextual_evidence` are `None`. Default weights remain
name/type/sample = 0.65/0.20/0.15, the threshold is 0.70 and the ambiguity margin
is 0.08. The [development evaluation](precision-evaluation.md) reports precision
and useful-match coverage together.

## Name conflicts

Populate `Config::name_conflicts` with distinctions meaningful to your
application. A qualifier rule can distinguish gross from net, billing from
shipping, or actual from forecast. A unit rule can distinguish kilograms from
pounds or seconds from milliseconds. Each alternative may contain multiple
equivalent spellings, so `kg` and `kilograms` can identify the same unit without
being treated as a contradiction.

```rust
use fieldkin::{Config, MatchEngine, NameConflictKind, NameConflictRule};

let engine = MatchEngine::new(Config {
    name_conflicts: vec![
        NameConflictRule {
            kind: NameConflictKind::Qualifier,
            alternatives: vec![vec!["gross".into()], vec!["net".into()]],
        },
        NameConflictRule {
            kind: NameConflictKind::Unit,
            alternatives: vec![
                vec!["k pa".into(), "kpa".into(), "kilopascal".into()],
                vec!["pa".into(), "pascal".into()],
            ],
        },
    ],
    ..Config::default()
})?;
# Ok::<(), fieldkin::MatchError>(())
```

Rule phrases use Fieldkin's name normalization: separator and camel-case
boundaries become tokens, and comparisons use normalized token phrases.
Names are inspected before name-score aliases are applied. Phrases must already
be in canonical normalized form: lowercase tokens separated by a single space,
with no leading or trailing whitespace. In the example, `k pa` covers the
camel-case name fragment `kPa`; `kpa` covers its lowercase spelling.

Phrases contain one to four tokens and at most 64 UTF-8 bytes. A rule has two to
16 alternatives, each containing one to 16 equivalent spellings, without
duplicate phrases anywhere in that rule. Configure at most 32 rules. Invalid
rules cause a configuration error when building the engine.

Longer overlapping phrases take precedence, allowing a
rule to recognize a phrase such as `tax exclusive` before a shorter overlapping
phrase. A phrase must match complete tokens; a short unit such as `m` does not
match an arbitrary substring in a field name.

Conflict checks compare the sets of alternatives observed in each name. When
both sets are nonempty and have no alternative in common, the pair is excluded.
Equivalent spellings in the same alternative do not conflict. Missing evidence
or overlapping sets preserve eligibility. For example, a rule distinguishing
gross from net can reject `gross_amount` against `net_amount`, but cannot infer
what an unqualified `amount` means. A name mentioning both alternatives is not
treated as an unambiguous contradictory label.

`CandidateIssue::NameConflict(NameConflictKind::Qualifier)` or `Unit` records
the exclusion without copying field names or configured phrases into warnings.

No unit conversion or field transformation is inferred. A same-unit result does
not increase confidence: `gross_amount_usd` and `net_amount_usd` can still refer
to different facts. Rule selection is caller-owned configuration, rather than
semantic truth inferred from fixture concepts, labels or explanations.

## Evidence sufficiency

Enable the existing support gate when sampled evidence is required:

```rust
use fieldkin::{Config, Corroboration, MatchEngine};

let engine = MatchEngine::new(Config {
    corroboration: Some(Corroboration::default()),
    ..Config::default()
})?;
# Ok::<(), fieldkin::MatchError>(())
```

`Corroboration::default()` requires a positive active built-in name score and a
distinct-aware built-in sample score of at least 0.50, before weighting. Custom
signals merely named `name` or `samples`, disabled signals, historical `Legacy`
sample overlap and profiles cannot meet these requirements. Matching still
uses the same threshold and hard constraints. See the
[corroboration guide](corroboration.md) for floor validation and sample
reliability details.

When no candidate can be selected because the support gate excludes an
otherwise viable edge, `Decision::InsufficientEvidence` explicitly records
abstention. This differs from `BelowThreshold`, where no candidate passes the
ordinary scoring and other exclusions. The viable edge must pass the score,
type, supplied-hint, configured name-conflict and caller restrictions before
corroboration is considered the reason for abstention.
`FieldDiagnostic::InsufficientEvidence` and candidate issues identify the
missing support. An insufficient-evidence decision does not claim the fields
are unrelated; the available inputs do not meet the configured requirement.

This requirement can reject valid matches with unavailable, disjoint,
constant or null-heavy samples. Identical names and sampled values can also
describe different facts and still meet the gate. More abstention alone does
not establish a useful precision improvement.

## Review and constraints

The separate optional contextual policy can also recover useful lexical and
sample-supported matches while rejecting unsupported generic or identifier
agreement. Enable it with `contextual_evidence: Some(ContextualEvidence::default())`
and select appropriate name-conflict rules for your vocabulary. Both strict
identifier support and scoped support default to true within this preset.
Its adequacy and mutual-comparison checks examine all hard-compatible pairs
before caller restrictions, ambiguity, assignment and display truncation.

This contextual option **does adjust the overall score**: it uses the maximum
of the weighted score, a 0.90 informative-lexical floor and a 0.95 adequate-sample
floor. Original signal scores remain visible, and `ContextualScoreAdjustment`
records the transformation. These floors are heuristics, not probabilities.
`InsufficientContextSupport` and `InsufficientEvidence` explain rejected support.
The score-preserving behavior described above applies to name-conflict and
ordinary corroboration checks. See the [release evaluation](release-readiness.md)
for useful-match losses, recovery, development iterations and qualification limits.

Inspect candidate eligibility and the source decision as well as score. A
candidate remains visible at its original rank after a conflict or support
exclusion. Eligibility is recomputed before local ambiguity and one-to-one
assignment, across every candidate rather than only the displayed top-k.
Removing an edge can therefore change which other pair is proposed.

An explicit application confirmation can override these name and support
heuristics through `match_schemas_with_constraints`. It retains the candidate's
original score and issues and returns `Decision::Confirmed`. Such decisions are
application review, rather than automatic matching successes.

Caller-verified `SemanticHints` remain hard constraints. Conflicting supplied
unit, currency or identifier-scope hints cannot be bypassed with a confirmation;
the application must correct the input metadata first. The enabled incompatible
type veto also remains a confirmation constraint. Agreeing hints do not supply
the sampled support required by corroboration.

With the optional `json` feature, reports serialize a name conflict as
`{"code":"name_conflict","kind":"qualifier"}` or `"unit"`, and the
explicit abstention decision as `"insufficient_evidence"`.

Name rules and support floors are heuristics. They help reject visible
contradictions and state when evidence is insufficient, while preserving useful
alternatives for review. They do not verify semantic distinctions absent from
the supplied schemas and samples.
