# Contributing

@limadog9 is the sole maintainer and final decision-maker. Open an issue for a
substantial change, then send a pull request from your own fork/identity. Small
fixes can go straight to a PR. Keep examples and fixtures synthetic or identify
their redistribution license and attribution.

Run the commands in the README, including `RUSTDOCFLAGS="-D warnings" cargo doc
--locked --no-deps --all-features` (PowerShell:
`$env:RUSTDOCFLAGS='-D warnings'`). CI covers Rust 1.85.0, the supported minimum,
and pinned current stable 1.99.0 on Linux, Windows and macOS. Run the standalone
`performance/` and `qualification/` checks when changing matching or budgets.
Use development-only evaluation for routine work; never score holdout data for
tuning. Historical artifacts are immutable; publish new versioned artifacts
and document deliberate report/configuration changes.
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
