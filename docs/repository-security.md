# Repository protection verification

Verified through the GitHub REST API on 2026-10-03, after creation of the public
[limadog9/fieldkin repository](https://github.com/limadog9/fieldkin).

The authenticated identity was `limadog9` (personal account), with repository
admin access. The API did not disclose a paid plan name. All requested protection
settings were accepted and independently read back, establishing their availability
on this public repository rather than assuming it from the account plan.

## Bootstrap ordering

Main was created with only `.gitignore`, `Cargo.toml`, `Cargo.lock`, a placeholder
`src/lib.rs`, `.github/CODEOWNERS`, and `.github/workflows/ci.yml`, at
[`23132e9`](https://github.com/limadog9/fieldkin/commit/23132e9c3394edecf8d4429bdd9256b10ef81ef9).
PR protections were enabled and read back before the implementation branch was
created. The [bootstrap CI run](https://github.com/limadog9/fieldkin/actions/runs/37153871210)
passed; its actual check names and GitHub Actions app ID were then required before
any substantive implementation push.

## Effective settings read back

| Setting | Value |
| --- | --- |
| Main protected | `true` |
| Required approving PR reviews | `1` |
| Required code-owner reviews | `true` |
| CODEOWNERS on main | `* @limadog9`; API reports no errors |
| Dismiss stale approvals | `true` |
| Required conversation resolution | `true` |
| Enforce administrators | `true` |
| Allow force pushes | `false` |
| Allow branch deletion | `false` |
| Required checks | `format`, `clippy`, `test`, `docs` |
| Check source | GitHub Actions app ID `15368` for all four |
| Require branch up to date | `true` |
| Bypass actors | None configured; no bypass allowances returned |
| Additional rulesets | None (`[]`); classic branch protection is used |
| Repository collaborators | Only `limadog9` |
| Default workflow token permissions | `read` |
| Workflows can approve PR reviews | `false` |

CI explicitly sets `contents: read`, pins checkout by commit SHA, and disables
persisting checkout credentials. It uses `pull_request`, never
`pull_request_target` or a privileged follow-up workflow. No secrets or write
credentials are supplied to contributor code. No external service integration,
new credential, collaborator or publishing owner was added.

## Identity limitation

This verification does **not** mean a PR authored as `limadog9` can receive the
required human approval. GitHub cannot distinguish agent actions authenticated
as that account from the account holder, and
[authors cannot approve their own PRs](https://docs.github.com/en/pull-requests/how-tos/review-pull-requests/approving-a-pull-request-with-required-reviews).
The implementation therefore remains in a draft PR. A separately authorized
contributor identity, for example submitting from its own fork, is needed before
@limadog9 can provide a qualifying personal review. Creating such an identity or
granting persistent access requires the user's authorization.

No approval, merge, bypass, weakened rule or synthetic approval check was used.
Administrative enforcement covers branch operations, but the account owner can
still edit repository settings; it cannot distinguish two people using the same
credentials. Protection should be re-read before future maintenance or release
claims because these settings are mutable.
