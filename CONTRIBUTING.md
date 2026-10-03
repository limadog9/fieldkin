# Contributing

@limadog9 is the sole maintainer and final decision-maker. Open an issue for a
substantial change, then send a pull request from your own fork/identity. Small
fixes can go straight to a PR. Keep examples and fixtures synthetic or identify
their redistribution license and attribution.

Run the commands in the README, including `RUSTDOCFLAGS="-D warnings" cargo doc
--locked --no-deps --all-features` (PowerShell:
`$env:RUSTDOCFLAGS='-D warnings'`). CI uses Rust 1.85.0, the supported minimum.
Tests should exercise observable behavior or meaningful invariants. Include a
reproduction for bug fixes and a before/after benchmark for performance claims.
Avoid adding runtime services or dependencies without a clear core-library need.

All files are owned by @limadog9, including CODEOWNERS and workflows. Main requires
a PR, one code-owner approval, resolved conversations and passing required checks;
stale approvals are dismissed on reviewable changes. Do not request bypasses.
The owner has final authority over API design, scope and releases. Contribution
does not imply maintainer status, write access or crate publishing ownership.

GitHub does not allow authors to approve their own PRs. A PR authored as
`limadog9` cannot satisfy the sole-code-owner review rule. Such work must remain
unmerged until submitted through a separately authorized contributor identity;
do not disable protection or manufacture an approval check to route around this.

Unless you explicitly state otherwise, contributions intentionally submitted for
inclusion are licensed under MIT OR Apache-2.0, without additional terms. Preserve
upstream notices if introducing third-party code; explain its provenance in
THIRD_PARTY.md. Be constructive, specific and respectful in discussions.
