# Contributing

@limadog9 is the sole maintainer and final decision-maker. Open an issue for a
substantial change, then send a pull request from your own fork/identity. Small
fixes can go straight to a PR. Keep examples and fixtures synthetic or identify
their redistribution license and attribution.

Run the commands in the README, including `RUSTDOCFLAGS="-D warnings" cargo doc
--locked --no-deps --all-features` (PowerShell:
`$env:RUSTDOCFLAGS='-D warnings'`). Use latest stable Rust; the October 4, 2026
validation uses Rust 1.99.0. CI covers stable on Linux, Windows and macOS;
older compiler maintenance has ended. Run the standalone
`performance/` and `qualification/` checks when changing matching or budgets.
Use development and explicitly designated regression data for routine work;
never score a reserved holdout for tuning. The corrective evaluator treats the
original examined families as regression data. Its separately published holdout
results are historical evidence and cannot be retuned and described as fresh.
The external T2D evaluator scores development classes only; its 218 reserved tables
have no scoring entry point. Do not turn unannotated pairs into negative labels
or count caller-confirmed decisions as automatic matching successes. Run
`cargo +stable run --locked -p fieldkin-tools -- import-t2d --check`,
`cargo +stable run --locked -p fieldkin-tools -- import-northix --check`, and
`cargo +stable test --locked -p fieldkin-tools` when changing the external data
tooling, preserving source licenses and offline pinned snapshots.
Historical artifacts are immutable; publish new versioned artifacts
and document deliberate report/configuration changes.
Use the [verified external workflow](docs/verified-evaluation.md) for new external
evidence records. Direct evaluator commands retain their historical behavior;
their source hashes alone do not prove which executable ran. Run
`cargo +stable test --locked -p fieldkin-tools` when changing the verified workflow.
Its fresh-build and fresh-output requirements are intentional. Python helpers and
local Valentine COMA execution are retired; the Rust evaluator can import the
archived score files without rerunning that external matcher.
Tests should exercise observable behavior or meaningful invariants. Include a
reproduction for bug fixes and a before/after benchmark for performance claims.
Avoid adding runtime services or dependencies without a clear core-library need.

All files are owned by @limadog9, including CODEOWNERS and workflows. Outside
contributions require a PR, one code-owner approval, resolved conversations and
passing required checks; stale approvals are dismissed on reviewable changes.
The owner has final authority over API design, scope and releases. Contribution
does not imply maintainer status, write access or crate publishing ownership.

The owner explicitly permits an administrator exception for their own work.
@limadog9 is the only repository administrator and can merge without a separate
reviewer. GitHub still prohibits authors from approving their own PRs; the owner
uses the administrator exception, not a self-approval. Outside contributors have
no bypass. Granting another account administrator access would extend this
exception and requires the owner's explicit decision.

Unless you explicitly state otherwise, contributions intentionally submitted for
inclusion are licensed under MIT OR Apache-2.0, without additional terms. Preserve
upstream notices if introducing third-party code; explain its provenance in
THIRD_PARTY.md. Be constructive, specific and respectful in discussions.
