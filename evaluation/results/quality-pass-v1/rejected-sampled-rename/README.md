# Rejected sampled identifier rename prototype

This new artifact preserves the runtime patch and source digests of an unsafe
development experiment. The patch is relative to commit
`8a8568033269b5f15fba6cc3958760f24fd9b8ff`; it is archived evidence, not a supported
runtime policy. Existing historical artifacts were not rewritten.

The prototype accepted an unresolved name relationship when both fields had
informative wording and identifier markers, compatible observed representation,
three distinct values, at least 25% non-null coverage, at least 90% sample
Jaccard overlap, and a 0.10 advantage over noncontradictory competitors on both
axes. It still reported the relationship as unresolved.

The close negative `warehouse_code` → `paint_id` with the unique shared integer
samples `[10,20,30]` exposes the defect: identifier shape and exclusive sampled
values cannot establish the meaning of unrelated nouns. The rule proposed this
pair in the failing independent-mode regression. One-to-one selection was
inferred from the same single eligible positive edge, rather than separately
executed before removal. The final safe regression actually checks abstention
under both assignment modes. The prototype was rejected as a hard matching contract
failure, regardless of aggregate development metrics. The final public policy
must not expose this broad option. Development experiment counts and executable
provenance are preserved separately by the evaluation runner.

No reserved or fresh qualification labels were read to diagnose this failure.
