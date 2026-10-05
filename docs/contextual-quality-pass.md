# Contextual matching quality pass — October 5, 2026

This is a working branch candidate, with the original release requirements intact:
95% proposal precision, 60% aggregate unique-field coverage, 90% candidate
recall@5, nonempty output, both assignments and every required variant. Main
integration requires fresh qualification and all engineering/platform checks.
Development improvements alone cannot qualify the candidate.

## Public policy and evidence

`Config::contextual_quality()` explicitly selects the experimental candidate.
Ordinary `Config::default()` and `ContextualEvidence::default()` keep their
existing behavior. The preset includes the same inspectable qualifier/unit
vocabulary used by the precision protocol. It does not perform conversions.

```rust
use fieldkin::{Config, MatchEngine};
let engine = MatchEngine::new(Config::contextual_quality())?;
# Ok::<(), fieldkin::MatchError>(())
```

The sampling assumption is explicit: populations can differ. Disjoint samples
are therefore inconclusive, rather than automatically incompatible. Informative
role evidence, compatible representations and separation from plausible choices
must support matching. Generic names, opaque codes, copied constants and sample
profiles do not independently establish meaning.

Relationships distinguish supported equivalence, contradiction and unresolved
wording. Only actual contradictions are excluded before calculating competition;
unresolved choices remain, including choices that fail current support thresholds.
Competition uses every compatible original pair before top-k and caller review.
Events, entities, polarity, direction, qualifiers, units/currencies and declared
Date/Timestamp representation remain safeguards. Scores are heuristics.

Structured `ContextualReason` values expose overlapping support and relationship
reasons without sample values. The evaluator attributes unselected unique matches
to the correct candidate, and separately records score/ambiguity/assignment
effects. Labels and slicing metadata reach only the scorer after matching.

## Controlled evidence

The starting remote main is `8a8568033269b5f15fba6cc3958760f24fd9b8ff`; the working
tree was clean. Stable Rust was checked with `rustup check`: Rust 1.99.0 and Cargo
1.99.0. The original development, hint-free extension, corrective development and
previously examined qualification are kept separate. Historical files remain
unchanged. Fresh qualification data stay outside policy development.

The baseline reproduction agrees with the recorded October 5 results. Individual
experiments include the existing relaxed identifier option, relationship repair,
independent populations and score ranking. Each experiment records configuration,
source/input/dependency identities, actual decisions, slices and changes. Failed
experiments are retained; proposed policy repairs were not assumed to work.

## Qualification and engineering discipline

`development-acceptance` checks numerical counts and required corpora/modes/
variants during iteration. `quality-freeze` binds the implementation, evaluator,
configuration, development evidence and opaque fresh inputs to a fresh executable.
`quality-qualify` requires an explicit reserved-run acknowledgement and preserves
the first-attempt start record even if execution fails. `release-acceptance`
verifies that frozen authorized record and its unrounded count-based gates.
Snapshot equality remains a separate reproducibility check.

The fresh corpus is independently instructed synthetic authorship, isolated by
instructions from implementation, results and existing answers. Agents share a
filesystem; this is not enforced sandbox isolation or production adjudication.
The independent author and reviewer must record their actual limitations.

The native performance comparison measures the complete selected public policy
against the original contextual policy using five alternating processes per
revision and latency/allocation mode, including success, ambiguity, independent
populations and four budget rejections in both assignment modes. Input/engine
construction is excluded; matching and report destruction are included. Allocator
requests are separate from timing and do not measure peak memory. The roadmap's
2x speed target remains an aspiration, not an achieved result.

No crates.io publishing, release, tag, credential changes, force push or protection
bypass is authorized by this work. A failed or unverifiable required gate leaves
main unchanged. Final measurements and validation records are added here when
the candidate is selected and checked.
