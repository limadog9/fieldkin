# Candidate package and documentation checks

The October 5, 2026 readiness pass keeps version `0.1.0`, the empty default
feature set, and the optional `json = ["dep:serde", "dep:serde_json"]` feature.
The [current readiness decision](readiness-pass.md) remains separate from package
correctness. Passing these checks does not qualify matching or authorize publishing.

Cargo metadata declares edition 2021, `rust-version = "1.99"`,
`MIT OR Apache-2.0`, the Fieldkin repository, and `README.md` as the crate readme.
`rust-toolchain.toml` selects current stable Rust; README and CONTRIBUTING state
that older compiler maintenance has ended. This pass uses Rust 1.99.0 and Cargo
1.99.0 on Windows. It makes no claim about compatibility with an older compiler.

The package includes original `LICENSE-MIT`, `LICENSE-APACHE`, and
`THIRD_PARTY.md`. These files were compared byte-for-byte with the extracted
archive. Dependencies are registry dependencies rather than vendored code;
their own notices remain in their upstream packages. The optional JSON graph
includes `unicode-ident` under `(MIT OR Apache-2.0) AND Unicode-3.0`, as already
documented in THIRD_PARTY. No dependency license or notice was removed.
Development tooling, evaluation datasets, qualification/performance workspaces,
and target output are excluded by the package include list.

The normalized packaged manifest has no workspace members or path dependencies.
The isolated default consumer resolves only `strsim`, `unicode-normalization`
and transitive `tinyvec`, in addition to Fieldkin. The JSON consumer additionally
resolves Serde/serde_json and their documented runtime and derive dependencies.
Neither consumer resolves the evaluator, tooling, proptest, or an unpublished
workspace dependency.

## Commands and run status

After runtime, evaluation and Cargo sources were frozen, all four package and
publish commands below succeeded again. The extracted default and JSON consumers
were rebuilt in fresh target directories and passed, and strict rustdoc plus all
37 unique README repository destinations passed. The archive contains 85 files;
its sources, original manifest, README, licenses and notices match the working
tree. Final clean-commit outcomes against the exact distributable are recorded
in the draft PR after committing; the working-tree checks do not imply them.
`--allow-dirty` includes reviewed
working-tree changes; clean-commit checks omit that flag. Every publish invocation
uses `--dry-run`; Cargo explicitly aborted its upload.

```powershell
$env:CARGO_TARGET_DIR='target/readiness-package'
cargo +stable package --locked -p fieldkin --allow-dirty --no-default-features
cargo +stable package --locked -p fieldkin --allow-dirty --no-default-features --features json
cargo +stable publish --locked -p fieldkin --dry-run --allow-dirty --no-default-features
cargo +stable publish --locked -p fieldkin --dry-run --allow-dirty --no-default-features --features json
tar -tf target/readiness-package/package/fieldkin-0.1.0.crate
```

The checks verify the archive with Cargo; `--no-verify` is never used. Inspect
`Cargo.toml` inside the archive, not just `Cargo.toml.orig`. Default and JSON
features produce the same distributable contents but compile different consumer
dependency graphs. The archive contains the sources, examples, integration tests,
benchmarks and documentation selected by the include list, plus Cargo's normalized
manifest, lockfile and VCS record.

## Isolated consumers

Extract the freshly checked archive under `target/readiness-package/unpacked`.
The two synthetic smoke consumers live in `consumer-default` and `consumer-json`
under the same check directory. Each has its own `[workspace]` declaration and
declares only a path dependency to `../unpacked/fieldkin-0.1.0`, with
`default-features = false`; the JSON consumer adds `features = ["json"]`.
They build using independent Cargo target directories and newly generated local
lockfiles. Package metadata confirms that the only workspace member is that
consumer and that Fieldkin's manifest path points to the extracted archive.

For reproduction, `consumer-default/Cargo.toml` is:

```toml
[package]
name = "fieldkin-packaged-default-smoke"
version = "0.0.0"
edition = "2021"
publish = false
[workspace]
[dependencies]
fieldkin = { path = "../unpacked/fieldkin-0.1.0", default-features = false }
```

Its `src/main.rs` is:

```rust
use fieldkin::{Config, DataType, Field, MatchEngine, Schema};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = Schema::new(vec![Field::new("s", "TransDate", DataType::Date)]);
    let target = Schema::new(vec![Field::new("t", "transaction_date", DataType::Date)]);
    let report = MatchEngine::new(Config::default())?.match_schemas(&source, &target)?;
    assert_eq!(report.fields[0].selected.as_ref().unwrap().target.0, "t");
    Ok(())
}
```

For `consumer-json`, change the package name to `fieldkin-packaged-json-smoke`,
add `features = ["json"]` to the Fieldkin dependency, and add this block before
`Ok(())` in the same main function:

```rust
use fieldkin::json::{report_to_json, review_from_json, review_to_json,
    JsonLimits, ReportJsonOptions, ReviewContext};
use fieldkin::{FieldPair, MatchConstraints};
let bytes = report_to_json(&report, &ReportJsonOptions::default())?;
assert!(String::from_utf8(bytes)?.contains("fieldkin.report"));
let context = ReviewContext { source_revision: "source-v1", target_revision: "target-v1" };
let review = MatchConstraints {
    confirmed: vec![FieldPair::new("s", "t")], ..Default::default()
};
let bytes = review_to_json(&review, context, &JsonLimits::default())?;
let resumed = review_from_json(&bytes, context, &JsonLimits::default())?;
assert_eq!(resumed.confirmed, review.confirmed);
```

```powershell
New-Item -ItemType Directory -Path target/readiness-package/unpacked -Force
tar -xf target/readiness-package/package/fieldkin-0.1.0.crate -C target/readiness-package/unpacked
cargo +stable generate-lockfile --offline --manifest-path target/readiness-package/consumer-default/Cargo.toml
cargo +stable generate-lockfile --offline --manifest-path target/readiness-package/consumer-json/Cargo.toml
cargo +stable run --offline --locked --manifest-path target/readiness-package/consumer-default/Cargo.toml --target-dir target/readiness-package/consumer-default-working-tree-target
cargo +stable run --offline --locked --manifest-path target/readiness-package/consumer-json/Cargo.toml --target-dir target/readiness-package/consumer-json-working-tree-target
cargo +stable metadata --offline --locked --manifest-path target/readiness-package/consumer-default/Cargo.toml --format-version 1
cargo +stable metadata --offline --locked --manifest-path target/readiness-package/consumer-json/Cargo.toml --format-version 1
```

Both consumers construct `TransDate` and `transaction_date` Date schemas, run the
default engine and assert selection of the expected target. The JSON consumer
also calls `report_to_json`, checks the report format marker, then exports and
imports an explicit `MatchConstraints` confirmation using `ReviewContext` and
`JsonLimits`, asserting that the confirmation survives. These are package smoke
tests authored for this pass, not independent adoption or business validation.
`--offline` uses cached registry packages; the publish dry runs still contact the
crates.io index. They never upload the crate.

## Rendered documentation

```powershell
$env:CARGO_TARGET_DIR='target/readiness-package'
$env:RUSTDOCFLAGS='-D warnings'
cargo +stable doc --locked -p fieldkin --no-deps --no-default-features --features json
```

`[package.metadata.docs.rs] features = ["json"]` now exposes the optional JSON
API using the [documented docs.rs metadata](https://docs.rs/about/metadata).
The strict local rustdoc build succeeds and produces `fieldkin/json/index.html`.
README repository links use absolute GitHub URLs so including README in rustdoc
does not reinterpret `docs/`, `examples/` or license paths underneath the API
documentation URL. Validation inspects actual `<a href>` values in generated
`fieldkin/index.html`, verifies all README repository targets appear, rejects
remaining relative repository links and checks that local destinations exist.
This verifies rendered link destinations; it does not claim a hosted docs.rs build
or remote HTTP success for newly added branch files before merge.
