# Development and code review

ARIA uses short-lived branches, pull requests, independent review and Windows CI.
Start with [CONTRIBUTING.md](../CONTRIBUTING.md) for the normal development loop.

## Current repository setup

The About panel describes ARIA and links to the documentation index. Issues use
bug, feature and model/device compatibility forms. Labels separate concerns such
as tracking, rendering, API, UI, security, performance and CI. Use Discussions for
open-ended design conversations; keep reproducible defects and accepted work in
Issues. Squash merging produces one commit per change and merged branches are
deleted automatically.

The default maintainer is **NekoUnix**, recorded in
[CODEOWNERS](https://github.com/NekoUnix/A.R.I.A/blob/main/.github/CODEOWNERS). Request a different write-access collaborator
for the owner's PRs. The current team also includes **Silvani-In-The-Net** and
**BennyBusinessButton**; repository access is managed in GitHub settings.

### Active protections

The repository is public. Its existing
[Protection ruleset](https://github.com/NekoUnix/A.R.I.A/rules/23132231) targets
`refs/heads/main` and is active. It requires a pull request, one independent
approval, approval of the latest reviewable push, resolved review conversations,
an up-to-date branch and the two CI checks below. New reviewable changes dismiss
stale approvals. Only squash merges are permitted; linear history is required.
Force pushes and deletion of `main` are blocked for normal contributors. At the
owner's explicit request, **only NekoUnix (user ID 215495180)** has `always` bypass
access. No administrator role, team, bot or other user is on the bypass list.
NekoUnix authorized bypass for the first release; ordinary contributions should
continue through review and CI. Keep this individual exception narrow when editing
the ruleset; granting an admin-role bypass would also grant access to future admins.

CODEOWNERS now routes eligible PRs to the default maintainer. Code-owner approval
is optional so the owner can receive review from another write-access collaborator.
The [ruleset configuration](https://github.com/NekoUnix/A.R.I.A/blob/main/.github/main-ruleset.json)
records the applied settings. An administrator can inspect the live configuration
and update this same ruleset with GitHub CLI:

```powershell
gh api repos/NekoUnix/A.R.I.A/rulesets/23132231
gh api --method PUT repos/NekoUnix/A.R.I.A/rulesets/23132231 --input .github/main-ruleset.json
gh api repos/NekoUnix/A.R.I.A/rules/branches/main
```

Inspect the live rules first and preserve any stronger settings added since this
file was written. Verify the branch-level readback after updates; an active ruleset
with an empty target list protects no branches. GitHub documents
[rulesets](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-rulesets/about-rulesets)
and [CODEOWNERS](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners).

## Checks and review

| Check / activity | What it verifies |
| --- | --- |
| `test-and-package` (Windows) | Pinned Rust toolchain, formatting, strict Clippy, workspace tests, Python 3.12 x64 dependency resolution/conversion tests and a portable Windows package. |
| `repository-checks` (Repository) | Local documentation links and headings, JSON syntax, pinned Actions, version consistency, camera dependency pins and conversion regression tests. |
| Independent review | Intended behavior, architecture, correctness, security, migration, performance and what automated tests cannot establish. |
| Relevant hardware check | Real avatar rendering, camera/controller behavior, SDK interoperability and output behavior for the changed feature. |

Both automated workflows run on every PR, even for documentation changes, so a
required check is never absent because of a path filter. Workflow tokens are
read-only, checkout does not persist credentials, Actions use full commit SHAs,
and PR jobs do not need proprietary models, SDK downloads or repository secrets.
Hardware-dependent Rust tests stay explicitly ignored unless a developer supplies
the required fixtures. The package job uploads reviewable build artifacts.
External-fork workflow runs require maintainer approval before they execute.
This approval is separate from approving the code for merge. Inspect fork changes
before authorizing a run, and retain the read-only token and no-secrets PR design.

A reviewer should inspect the diff and validation evidence, run relevant checks
when needed, and request changes for unresolved correctness or compatibility
problems. Focus especially on per-avatar settings, authored parameter ranges,
input axes, tracking loss, bounded work, native resource lifetimes and API access.
Never approve your own change or convert automated lint results into a review.
A maintainer can explicitly delegate review to a coding agent; identify that
delegation in the review and still inspect the diff and validation evidence.
GitHub's separate-author and latest-push approval requirements continue to apply.

After a new push, review the new diff and approve the current revision. Resolve
conversations only after the concern is addressed. Merge when the independent
approval and both checks are green. User-authorized automatic merging follows the
same gates for contributors. Self-authored PRs normally need another eligible
collaborator's approval. NekoUnix may explicitly use the owner-only exception;
record that decision and validation in the release/PR rather than claiming an
independent approval that did not occur.

## Dependencies and security

Dependabot checks Cargo, Actions and `/tracking` Python requirements weekly.
Compatible updates are grouped; major updates remain separate. At most three
version-update PRs per ecosystem are open at once. Security alerts and security
update PRs are enabled in repository settings. Dependabot does not approve its own
changes. An authorized maintainer can enable auto-merge for an individually
reviewed PR; GitHub waits for the same approval and CI requirements. A clean alert
list does not cover every proprietary SDK or runtime.

Camera dependency PRs must update both requirements files consistently, install in
a clean Python 3.12 x64 environment and run real inference before merging. Review
model hash/source changes and third-party notices separately from version bumps.
CI resolves the complete camera lock with binary wheels for Python 3.12 x64 before
packaging. A dependency upgrade that conflicts with MediaPipe's constraints fails
this check; a successful dry run still needs the real inference check during review.
Report vulnerabilities according to [SECURITY.md](../SECURITY.md).
Private vulnerability reporting, secret scanning and secret push protection are
enabled. Ordinary issues and discussions remain public regardless of their labels.

## Release checklist

1. Confirm the intended fixes and compatibility notes have been reviewed.
2. Update the Cargo workspace version, regenerate `Cargo.lock`, update the version
   in `scripts/build-windows.ps1`, the Windows workflow ZIP path, the app banner
   and user-facing installation instructions. The repository check catches ZIP
   version mismatches; it does not replace reviewing all displayed version text.
3. Update documentation, offline help, examples, screenshots and validation notes
   for the changed behavior. Record untested hardware/SDK boundaries explicitly.
4. Run all required CI workflows and the four-target **Alpha packages** matrix on the final revision. Extract the resulting Windows ZIP
   into a clean folder, launch it and exercise representative avatar/output paths.
5. Check licenses and package contents: no private avatars, SDK binaries, API keys,
   OAuth tokens or personal profiles. Confirm screenshots have permission.
6. Publish/tag only the reviewed release commit. Include a checksum, notable
   changes, installation instructions and known limitations with the artifact.
   GitHub Actions artifacts expire; a release asset requires a deliberate upload.

Repository administration changes should be reviewed like code. Keep this page,
the ruleset configuration and actual GitHub settings consistent when policy changes.

### Native Alpha packaging

The Alpha matrix builds Windows x64, Linux x64, macOS arm64 and macOS x64. Linux
also produces Fedora RPM and Arch packages for the app and its optional OBS plugin.
Native integration checks cover the Linux shared-memory protocol/OBS receiver and
macOS Syphon/Metal transfer. All artifact names and application branding identify
Alpha. Packages include their source revision, documentation and dependency notices;
macOS receives a local ad-hoc signature, not a Developer ID/notarized signature.

The workflow has read-only permissions and uploads expiring CI artifacts. Publishing
uses a deliberate tagged prerelease after successful matrix checks and package
inspection. Release assets must all come from the same source revision. A failed
platform job must be repaired and rerun before publishing; do not substitute an
older binary under the new version. Include SHA256SUMS.txt and state native hardware
validation limits. Main-branch review protections remain separate from prerelease
publication; a prerelease branch must not be described as independently approved.
