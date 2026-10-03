# Stage 3 API and behavior changes

Fieldkin is still unpublished and experimental. This stage introduces the small
source changes needed for exact samples and verified semantics; it does not
declare a stable 1.0 API or publish a crate. Existing `Field::new` and
`with_samples` calls continue to work.

## Samples and reliability

The default `SampleMatcher` now multiplies exact overlap and null coverage by
`min((lower distinct count - 1) / 2, 1)`. Constants contribute zero sample score;
two-value columns contribute at most half; three or more distinct values can
receive full weight. Non-null observations must still meet `min_non_null`.
This distinguishes repeated observations from distinct support; it does not
establish semantic equivalence or a calibrated reliability probability.

There is no threshold or weight change: defaults remain 0.70 eligibility,
0.08 ambiguity margin and name/type/sample weights 0.65/0.20/0.15. A same-name,
same-type pair can still exceed the threshold without samples. Cardinality
attenuation alone cannot resolve hidden gross/net or identifier-scope differences.

Code constructing `SampleMatcher` with a struct literal must add its policy:

```rust
use fieldkin::{SampleMatcher, SampleReliability};

let conservative = SampleMatcher { min_non_null: 5, ..SampleMatcher::default() };
let historical = SampleMatcher { min_non_null: 3, reliability: SampleReliability::Legacy };
```

`Legacy` reproduces the original overlap/coverage heuristic. Use it deliberately
for comparisons or compatibility, recognizing that a repeated constant can then
produce full sample agreement. Explanation text changes with the selected policy.

Exhaustive matches over `SampleValue` must handle `Integer(i128)` and
`Decimal(ExactDecimal)`. See [exact numeric samples](exact-numbers.md) for range,
scale and representation rules. Floats, exact integers, exact decimals and text
remain separate sample kinds: no implicit cast or parsing happens. Floating point
non-finite values are rejected and signed zero remains equal. Convert from
original values before precision has been lost.

## Verified hints

`Field` has a new `hints: SemanticHints` member. Prefer its constructors; callers
using a struct literal must add `hints: SemanticHints::default()` or explicit
verified metadata. `with_hints` attaches optional `unit`, `currency` and
`identifier_scope` labels. Each label must be nonempty, trimmed and at most 128
UTF-8 bytes. Matching validates these bounds even if the other schema is empty.

Labels compare exactly and case-sensitively using the vocabulary chosen by the
caller. A conflict on any jointly supplied axis makes the candidate ineligible,
even when names/samples agree or the declared-type veto is disabled. This applies
independently of configured matchers. Remove a hint explicitly if it should no
longer constrain matching; there is no silent conversion or conflict override.
One-sided or absent labels do not assert agreement. Agreement does not boost the
score or prove equivalence. Reports explain conflict, agreement and one-sided
absence by category, without copying label values. `Debug` redacts hint payloads.

See the compiling
[verified-semantics example](https://github.com/limadog9/fieldkin/blob/main/examples/verified_semantics.rs).
Hints must come from an authoritative application source. Inventing hints from
the matcher’s proposed mapping would make the check circular.

## Aliases and optional profiles

`NameMatcher::with_alias(from, to)` validates both sides as single normalized
tokens of at most 256 bytes. It rejects rather than guesses normalization.
Aliases remain one-pass and apply to every occurrence; a domain-specific token
replacement is not a unit, currency or scope guarantee. The existing public map
remains available. Used map targets are validated at evaluation as before.
Name explanations list applied aliases in stable order, with a bounded detail
list and an explicit omission notice if necessary.

`SampleProfileMatcher` is optional and absent from `MatchEngine::new`. It compares
runtime-kind proportions and Unicode character-length bins, attenuated by null
coverage, observation count and distinct count. Identical shapes can belong to
unrelated fields; the matcher deliberately does not compare numeric magnitudes,
parse text or infer semantics. It uses the same private per-call preparation
path as other built-ins. It has additional fixed caps of 65,536 observations per
field and 1,024 bytes per text sample, including when engine limits are raised.
Engine pair, signal and report budgets still apply; adding a fourth signal can
exhaust a budget that was sufficient for three.

No runtime dependency, external service, public prepared-schema lifecycle or
automatic data rewriting is introduced. Custom matchers keep their existing
pairwise evaluation contract. New sample kinds require their explicit handling
when applicable, and the engine enforces supplied hint conflicts for them too.
