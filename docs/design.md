# Design and known limits

Fieldkin 0.1 matches flat fields, not rows or values. The caller supplies stable
IDs, declared logical types, and optional representative samples. Inputs are
validated on every call; there is no registry, database, worker pool, or I/O.

## Scoring and abstention

The engine evaluates all bounded source-target pairs through a small `Matcher`
trait. We deliberately use two runtime dependencies: `strsim` for the established
Jaro-Winkler implementation and `unicode-normalization` for NFKC. Standard-library
collections give stable iteration order. `proptest` is development-only.

Weights are validated and normalized at construction. A signal can return no
score. Missing evidence contributes zero against the original total weight;
renormalizing only the available signals would let an isolated exact name look
fully supported. The consequence is intentional: with unknown types and no
samples, even an exact name scores only 0.65 and abstains by default. Tune weights
and thresholds using representative labeled data for your domain.

Declared types are coarse. All numeric pairs score 0.85, exact types score 1,
date/timestamp pairs score 0.70, unknown types provide no evidence, and other
known pairs score zero. The default type veto applies independently of configured
signals, so a custom name-only baseline must explicitly disable it. No conversion
is performed or promised.

Sample agreement is exact typed distinct-value Jaccard, multiplied by the lower
non-null fraction. Three non-null observations are required by default. Repeated
values count toward that minimum but do not increase distinct overlap. This is a
small heuristic, not a statistical sufficiency test. It avoids treating shared
nulls as matching evidence; it cannot distinguish unrelated boolean flags or
code lists that happen to share values. `f64` numeric samples cannot preserve
every large integer or exact decimal. Supply canonical text when exactness matters.

Candidates that pass the threshold and type veto are eligible. If two or more
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
`O(m+n)` auxiliary space beyond the candidate matrix. Pair evaluation and reports
take `O(nm)` signal calls/storage; individual signal cost depends on bounded name
and sample lengths. Built-in sample sets are currently recomputed per pair. This
favors a small extension API over preprocessing machinery; benchmarks make this
cost visible. A future prepared-signal API should be justified by measured need.

Fields and targets are sorted by their stable IDs before evaluation. Candidates
are sorted by descending score then target ID. Hungarian rows/columns use that
same order; equal reduced costs visit the first column and retain the first
predecessor. Equal optimum assignments therefore receive reproducible choices
without perturbing scores. We do not enumerate all global optima or promise a
lexicographically minimal assignment. Global target contention gets a warning.
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
The public normalization utility alone is a pure string utility and has no budget.

Reports retain every evaluated candidate until final selection, then truncate
rankings. This makes global decisions independent of display settings, at the
cost of bounded `O(nm)` report construction. At most 64 signals and 4,096 bytes per
signal explanation are accepted. Defaults also cap total signal evaluations at
65,536 and aggregate explanation text at 16 MiB, including candidates later
truncated. Raising budgets is an explicit caller choice.

Built-in matchers do not log samples. Sample `Debug` representations are redacted;
reports contain counts, scores and generic reasons. Field names and IDs are not
redacted. Custom matcher errors are suppressed by the engine, but custom
explanations are returned verbatim. Custom code must enforce its own work bounds,
determinism and privacy: Fieldkin does not catch its panics or sandbox it.
