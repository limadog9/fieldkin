# Source-only annotation review

Performed before any predictions and without matcher implementation access.

Stripe annotations were independently reviewed by a source-research agent against
the archived component definitions. The recurring-Price migration constraint,
ID-string union variant, renamed nested fields, amount units, and omitted-
counterpart negatives are supported.

A subsequent integration annotation audit identified that the initial unique
storage-preserving amount labels were not justified under a semantic-equivalence
policy. Integer and decimal-string properties measure the same economic amount;
integer-to-decimal conversion may be lossless while decimal-to-integer conversion
can lose fractional precision. With no observed values, neither unconditional
unique labels nor symmetric ambiguity labels are established. Before any
predictions, the decimal base-amount and per-tier unit-amount properties were
therefore removed from both projections. The revised Stripe case contains 21
source and 22 target paths, retaining one representation of each corresponding
quantity. Flat-tier amounts still have no target counterpart in either
representation. The initial freeze remains outside this active corpus for audit.

Treasury annotations were reviewed against the official crosswalk's concept
definitions. The review distinguished code from explanatory description,
transaction change from cumulative award value, loan face value from loan subsidy
cost, and ordering end date from performance end date. An initially selected
generic legal-city target could overlap a foreign-city source concept, so that
target-only distractor was replaced with a distinct county-code field before the
final freeze. This revision was
driven only by source evidence, with no predictions viewed.

All cases have complete source coverage, unique field names per side, valid target
references, empty sample arrays, and publisher-grounded declared types. Resource
integrity and offline deterministic regeneration were verified separately from
annotation review; neither constitutes a matcher evaluation.
