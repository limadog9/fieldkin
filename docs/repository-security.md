# Repository protection verification

Verified through the GitHub REST API on 2026-10-03, after creation of the public
[limadog9/fieldkin repository](https://github.com/limadog9/fieldkin).

The authenticated identity was `limadog9` (personal account), with repository
admin access. The API did not disclose a paid plan name. All requested protection
settings were accepted and independently read back, establishing their availability
on this public repository rather than assuming it from the account plan.

The owner subsequently requested that their own account be allowed to merge
while outside contributions still require their approval. Only administrator
enforcement was disabled for that policy; the other protection requirements
remain configured. @limadog9 is the sole administrator.

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
| Enforce administrators | `false`, owner-requested administrator exception |
| Allow force pushes | `false` |
| Allow branch deletion | `false` |
| Required checks | `format`, `clippy`, `test`, `docs` |
| Check source | GitHub Actions app ID `15368` for all four |
| Require branch up to date | `true` |
| Bypass actors | Administrator exception; only `limadog9` is an administrator. No explicit PR bypass allowances configured |
| Additional rulesets | None (`[]`); classic branch protection is used |
| Repository collaborators | Only `limadog9` |
| Default workflow token permissions | `read` |
| Workflows can approve PR reviews | `false` |

CI explicitly sets `contents: read`, pins checkout by commit SHA, and disables
persisting checkout credentials. It uses `pull_request`, never
`pull_request_target` or a privileged follow-up workflow. No secrets or write
credentials are supplied to contributor code. No external service integration,
new credential, collaborator or publishing owner was added.

## Owner exception and identity boundary

Outside contributions still require a code-owner review from @limadog9. The
owner can merge their own work using the administrator exception. This is not a
self-approval: GitHub cannot distinguish agent actions authenticated as that
account from the account holder, and
[authors cannot approve their own PRs](https://docs.github.com/en/pull-requests/how-tos/review-pull-requests/approving-a-pull-request-with-required-reviews).

The administrator exception is role-based, not a special allowance tied to a
particular username. Granting another account administrator access would extend
it; no such access was granted. No self-approval or synthetic approval check was
used. Creating an identity or granting persistent access requires the owner's
authorization. Protection should be re-read before future maintenance or release
claims because these settings are mutable.
