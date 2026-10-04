# Design and known limits

Fieldkin 0.1 matches flat fields, not rows or values. The caller supplies stable
IDs, declared logical types, and optional representative samples. Inputs are
validated on every call; there is no registry, database, worker pool, or I/O.

## Scoring and abstention

The engine evaluates all bounded source-target pairs through a small `Matcher`
trait. The default build uses two direct runtime dependencies: `strsim` for the established
Jaro-Winkler implementation and `unicode-normalization` for NFKC. Standard-library
collections give stable iteration order. `proptest` is development-only.

Weights are validated and normalized at construction. A signal can return no
score. Missing evidence contributes zero against the original total weight;
renormalizing only the available signals would let an isolated exact name look
fully supported. The consequence is intentional: with unknown types and no
samples, even an exact name scores only 0.65 and abstains by default. Tune weights
and thresholds using representative labeled data for your domain.

Declared types are coarse. Different numeric types score 0.85, identical known types score 1,
date/timestamp pairs score 0.70, unknown types provide no evidence, and other
known pairs score zero. The default type veto applies independently of configured
signals, so a custom name-only baseline must explicitly disable it. No conversion
is performed or promised.

Sample agreement is exact typed distinct-value Jaccard, multiplied by the lower
non-null fraction and `min((lower distinct count - 1) / 2, 1)`. Three non-null
observations are required by default. Repeated values count toward that minimum
but cannot increase distinct support; constants score zero and two-value sets
receive half support. This is a heuristic, not a statistical sufficiency test.
Unrelated three-value code lists can still agree fully. `SampleReliability::Legacy`
retains the historical observation/coverage-only behavior for comparisons.
Exact integer and decimal samples preserve equality without floating-point casts;
all numeric sample kinds remain distinct. See [exact numeric samples](exact-numbers.md).

Optional `SemanticHints` supplies verified units, currency and identifier scope.
Any jointly supplied conflict excludes a pair independently of configured signals;
agreement does not add score, and missing labels do not assert agreement. Labels
compare exactly in a caller-chosen vocabulary. Warnings identify the affected
category without printing hint values. This relies on supplied facts; it cannot
infer a hidden gross/net distinction. Optional sample profiles compare kind/length
distributions, with reliability attenuation, but are not default evidence.

Candidates that pass the threshold, type veto and hint constraints are eligible. If two or more
eligible targets are within the configured absolute margin of the best, the
source is locally ambiguous. By default it gets no selection. Alternatives are
computed before top-k truncation and returned in stable target-ID order.

## Assignment and reproducibility

Independent mode selects the best eligible candidate for each unambiguous
source, so several sources may propose the same target. Optional one-to-one mode
uses a rectangular Hungarian solver with zero-weight dummy columns. It maximizes
the sum of eligible raw heuristic scores, not the number of matches; forbidden,
zero-score and ambiguous-source edges are excluded by default. Every source can
remain unmatched. The objective is not a likelihood or a mapping confidence.

With `n` source and `m` target fields, assignment takes `O(n²(m+n))` time and
`O(m+n)` kernel workspace beyond the candidate matrix. Omitting unused targets
can additionally allocate an `O(nm)` projected matrix; source rows and dummy
ordering are retained to preserve fractional ties. Pair evaluation and reports
take `O(nm)` signal evaluations; individual signal cost depends on bounded name
and sample lengths. Within each match call, concrete built-in name and sample
matchers prepare each field once. The private cache holds normalized, expanded
name tokens, joined strings and distinct sample sets; sample text is borrowed,
not copied. It is discarded when the call returns. No cross-call cache or public
prepared-schema abstraction is introduced.

The existing custom `Matcher::evaluate` path remains pair-by-pair, in the same
sorted order. A defaulted, hidden `as_any` hook identifies concrete built-ins by
Rust type, never by a signal's name. Custom implementations need not change and
should leave that hook at its default. Wrapping a built-in in a custom matcher
uses the ordinary path. Preparation retains errors until that signal would have
been evaluated, preserving error precedence and disabled-signal behavior.

Fields and targets are sorted by their stable IDs before evaluation. Candidates
are sorted by descending score then target ID. Hungarian rows/columns use that
same order; equal reduced costs visit the first column and retain the first
predecessor. Equal optimum assignments therefore receive reproducible choices
without perturbing scores. We do not enumerate all global optima or promise a
lexicographically minimal assignment. Global target contention gets a warning
and typed source/target diagnostics. Optional bounded alternative analysis probes
each selected edge and reports representative mappings, objective gaps and
completeness; it never changes selections. See [global diagnostics](stage4-diagnostics.md).
Floating-point arithmetic and dependency upgrades can change borderline scores;
record your configuration and lock dependencies for reproducible deployments.

## Failure cases

- `GrossAmt` and `amount` lack evidence about gross versus net. With no samples,
  defaults abstain. Similar names plus coincident values can still create a false
  positive; this is not a gross/net semantics classifier.
- `SKU_Code` and `product_id` can be a valid renamed pair that defaults miss.
  Add verified domain aliases or a custom matcher; do not infer synonymy from the
  fact that both contain identifier-shaped strings.
- Names such as `id`, `value` and `amount` are underspecified. Duplicate names with
  separate IDs can produce genuine ambiguity. Deterministic tie-breaking is not
  evidence of correctness.
- Unrelated status or boolean columns often share most possible values. Sample
  overlap does not establish shared meaning, units, currency or scope.
- Disjoint samples from two related columns yield low overlap. Biased, null-heavy
  or tiny samples provide weak evidence. No extrapolation or distribution fit is
  performed.
- Wrong type declarations may veto a valid proposal; text-to-number/date parsing
  is deliberately absent. Coarse numeric compatibility ignores scale, overflow,
  precision and signedness.
- NFKC merges some compatibility characters; lowercase is not locale-specific
  Unicode case folding. Accent removal, transliteration, word segmentation for
  unseparated scripts, stemming and general synonym recognition are absent.
- One-to-one constraints are inappropriate for field splits/merges and can
  suppress real correspondences. Nested paths, transforms and composite keys are
  deferred.

## Resource and privacy boundaries

`Config::limits` bounds field count, pair count, raw name/ID length, samples per
field, text sample length and aggregate text bytes. All input is checked before
any matcher is called, including when the opposite schema is empty. Invalid or
oversize input yields `MatchError`; samples are never silently truncated.
Name matching additionally bounds normalized/expanded names to 1,024 bytes.
Semantic hint labels have a fixed 128-byte limit. The optional profile matcher
also has fixed caps of 65,536 samples per field and 1,024 bytes per text sample;
raising engine limits does not raise these caps.
The public normalization utility alone is a pure string utility and has no budget.

Global assignment retains every evaluated candidate until final selection, then
truncates rankings. Independent mode finishes each source's decision and drops
undisplayed candidates before processing the next source; it allocates no
assignment matrix. Both modes evaluate all pairs and charge their full explanation
bytes, including discarded reports. This makes decisions independent of display
settings, with bounded `O(nm)` worst-case report storage. At most 64 signals and 4,096 bytes per
signal explanation are accepted. Defaults also cap total signal evaluations at
65,536 and aggregate explanation text at 16 MiB, including candidates later
truncated. Raising budgets is an explicit caller choice.

Built-in matchers do not log samples. Sample `Debug` representations are redacted;
reports contain counts, scores, reasons and applied token aliases. Field names,
IDs and applied aliases are not redacted; semantic hint values are redacted.
Custom matcher errors are suppressed by the engine, but custom
explanations are returned verbatim. Custom code must enforce its own work bounds,
determinism and privacy: Fieldkin does not catch its panics or sandbox it.
