# Landscape and name check

Checked on 2026-10-03, before creating `limadog9/fieldkin`. Search results are a dated discovery record, not a guarantee that a name is reserved or that every related project was found.

## Name

- The [crates.io exact endpoint](https://crates.io/api/v1/crates/fieldkin) returned HTTP 404. The [crates.io search endpoint](https://crates.io/api/v1/crates?q=fieldkin&per_page=100) returned zero crates.
- The [GitHub repository-name search](https://api.github.com/search/repositories?q=fieldkin+in:name&per_page=100) returned three substring matches: `mohdamir404327-a11y/fieldking_backend`, `Darshilgor/FieldKing`, and `Darshilgor/FieldKingAdmin`. None was named Fieldkin; their available metadata did not identify a schema-matching library. A direct collision was not found.
- Broader searches for `fieldmatch` and `schemamatch` found existing names. [FieldMatching](https://github.com/justkolesov/FieldMatching) implements an electrostatic model for generating and transferring data. [SchemaMatch](https://github.com/chitralabs/schemamatch) describes Java tabular comparison and diffing. [Lonko/SchemaMatcher](https://github.com/Lonko/SchemaMatcher) describes Java attribute correspondence. These names are already used and were not adopted.
- A [GitHub search for schema matching in Rust](https://api.github.com/search/repositories?q=%22schema+matching%22+language:Rust&per_page=100) returned zero results. That limited query does **not** establish that no Rust implementation exists. The crates.io query `schema match` was broad and mostly found schema validation and unrelated crates.

Fieldkin is an independent project maintained solely by [@limadog9](https://github.com/limadog9). No affiliation with the projects above is implied.

## Prior work

[Valentine](https://github.com/delftdata/valentine) is a Python schema-matching package and experimental suite. Its current documentation covers COMA, Cupid, distribution-based matching, value-set Jaccard matching, and Similarity Flooding. It accepts tabular data through DataFrames, supports evaluation against ground truth, and offers Hungarian, greedy, and mutual-top one-to-one selectors. Its [research documentation](https://github.com/delftdata/valentine/blob/master/docs/research.md) points to the original experimental suite at tag `v1.1`; current package behavior must not be confused with the historical benchmark.

The original paper defines dataset-discovery scenarios and a fabrication/evaluation methodology: Koutras et al., *Valentine: Evaluating Matching Techniques for Dataset Discovery*, ICDE 2021, pp. 468–479 ([paper](https://arxiv.org/abs/2010.07386)). Fieldkin's synthetic cases use the general lesson that renamed, overlapping, unrelated, and misleading schemas need separate evaluation. They do not reproduce Valentine's benchmark or establish comparative performance against Valentine.

Valentine's [license](https://github.com/delftdata/valentine/blob/master/LICENSE) is Apache-2.0, with copyright 2021–2026 Delft University of Technology at inspection. No Valentine code, fixtures, or paper text was copied into Fieldkin. General schema-matching ideas and the research are credited here; Fieldkin is an original implementation, not a port of COMA, Cupid, or Valentine.

## What Fieldkin adds

The first release packages conservative field correspondence as an embeddable Rust library: stable field IDs, optional typed samples, per-signal explanations, explicit missing evidence, ranked alternatives, ambiguity, and unmatched fields. An optional bounded global assignment chooses among eligible pairs. The core requires only memory and CPU, with no DataFrame engine, service, model, or credentials.

This is a scope and API distinction, not a claim of novel matching theory, state-of-the-art accuracy, or superior speed. Scores are heuristic and require application-specific validation. The synthetic name-only baseline documents behavior on small controlled cases; it is not a general benchmark ranking.

## Dependency decisions

The inspected options below retain the October 3 provenance. Current development
supports only latest stable Rust, and the development-only `proptest` dependency
has since moved from 1.6.0 to 1.11.0. The older compiler floor is retired; the
internal solver decision still rests on its narrow API, bounded dimensions and
independent oracle checks. See the [updated dependency review](dependency-review.md).

| Component | Inspected option | Decision |
| --- | --- | --- |
| String similarity | [`strsim` 0.11.1](https://docs.rs/strsim/0.11.1/strsim/), MIT | Reuse established similarity primitives instead of writing edit-distance code. |
| Unicode normalization | [`unicode-normalization` 0.1.25](https://docs.rs/unicode-normalization/0.1.25/unicode_normalization/), MIT OR Apache-2.0 | Reuse normalization tables and algorithms. `Cargo.toml` permits compatible 0.1 releases; `Cargo.lock` records the reviewed build. Normalization does not supply semantic translation. |
| Global assignment | [`pathfinding` 4.16.0 Kuhn–Munkres](https://docs.rs/pathfinding/4.16.0/pathfinding/kuhn_munkres/fn.kuhn_munkres.html), Apache-2.0/MIT | Considered but not added. Its API assigns every row and documents dimension/overflow preconditions; the inspected crate required Rust 1.88. Fieldkin keeps a narrow internal rectangular solver with dummy unmatched choices, bounded dimensions, and independent brute-force checks on small cases. This avoids a general graph dependency; its former Rust 1.85 compatibility rationale is historical. |
| Property testing | [`proptest` 1.6.0](https://docs.rs/proptest/1.6.0/proptest/), MIT OR Apache-2.0 | Development-only dependency with default features disabled and `std` enabled. Small generated cases test invariants and assignment optimality. |

Registry metadata was inspected through the crates.io API for each crate, including license and Rust-version fields. [THIRD_PARTY.md](../THIRD_PARTY.md) records direct dependencies and runtime attribution. Future dependency changes need another review; these observations are not a promise about future releases.
