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

### Enforcement status

On 2026-09-12, GitHub refused branch protection for this private repository with:
"Upgrade to GitHub Pro or make this repository public to enable this feature."
The repository remains private. The review rules below are the working agreement;
GitHub does **not yet block** a collaborator from bypassing them. Automatic
CODEOWNERS routing is also plan-dependent, so request reviewers manually.

The [ready-to-apply protection configuration](https://github.com/NekoUnix/A.R.I.A/blob/main/.github/main-protection.json)
requires an up-to-date branch, the two check names below, one independent approval,
reapproval after new changes, resolved conversations and linear history. It
includes administrators and prohibits force pushes and deletion of `main`.
Code-owner approval is optional so the owner can receive review from another
collaborator. This file is a configuration template, not evidence of active rules.

Once the owner enables a supporting plan, an authenticated repository administrator
can apply it from the checkout with GitHub CLI:

```powershell
gh api --method PUT repos/NekoUnix/A.R.I.A/branches/main/protection --input .github/main-protection.json
gh api repos/NekoUnix/A.R.I.A/branches/main/protection
```

Inspect existing protections first and preserve any stronger rules added since
this template was written. Verify both CI jobs have run successfully and the
readback has the intended settings before claiming enforcement is active.
GitHub documents [branch protection](https://docs.github.com/en/repositories/configuring-branches-and-merges-in-your-repository/managing-protected-branches/about-protected-branches)
and [CODEOWNERS availability](https://docs.github.com/en/repositories/managing-your-repositorys-settings-and-features/customizing-your-repository/about-code-owners).

## Checks and review

| Check / activity | What it verifies |
| --- | --- |
| `test-and-package` (Windows) | Pinned Rust toolchain, formatting, strict Clippy, workspace tests, Python conversion tests and a portable Windows package. |
| `repository-checks` (Repository) | Local documentation links and headings, JSON syntax, pinned Actions, version consistency, camera dependency pins and conversion regression tests. |
| Independent review | Intended behavior, architecture, correctness, security, migration, performance and what automated tests cannot establish. |
| Relevant hardware check | Real avatar rendering, camera/controller behavior, SDK interoperability and output behavior for the changed feature. |

Both automated workflows run on every PR, even for documentation changes, so a
required check is never absent because of a path filter. Workflow tokens are
read-only, checkout does not persist credentials, Actions use full commit SHAs,
and PR jobs do not need proprietary models, SDK downloads or repository secrets.
Hardware-dependent Rust tests stay explicitly ignored unless a developer supplies
the required fixtures. The package job uploads reviewable build artifacts.

A reviewer should inspect the diff and validation evidence, run relevant checks
when needed, and request changes for unresolved correctness or compatibility
problems. Focus especially on per-avatar settings, authored parameter ranges,
input axes, tracking loss, bounded work, native resource lifetimes and API access.
Never approve your own change or convert automated lint results into a human review.

After a new push, review the new diff and approve the current revision. Resolve
conversations only after the concern is addressed. Merge when the independent
approval and both checks are green. For an urgent exception, the owner records
the reason and follow-up review in the PR; do not silently bypass the process.

## Dependencies and security

Dependabot checks Cargo, Actions and `/tracking` Python requirements weekly.
Compatible updates are grouped; major updates remain separate. At most three
version-update PRs per ecosystem are open at once. Security alerts and security
update PRs are enabled in repository settings. No dependency PR is auto-approved
or auto-merged. A clean alert list does not cover every proprietary SDK or runtime.

Camera dependency PRs must update both requirements files consistently, install in
a clean Python 3.12 x64 environment and run real inference before merging. Review
model hash/source changes and third-party notices separately from version bumps.
Report vulnerabilities according to [SECURITY.md](../SECURITY.md).

## Release checklist

1. Confirm the intended fixes and compatibility notes have been reviewed.
2. Update the Cargo workspace version, regenerate `Cargo.lock`, update the version
   in `scripts/build-windows.ps1`, the Windows workflow ZIP path, the app banner
   and user-facing installation instructions. The repository check catches ZIP
   version mismatches; it does not replace reviewing all displayed version text.
3. Update documentation, offline help, examples, screenshots and validation notes
   for the changed behavior. Record untested hardware/SDK boundaries explicitly.
4. Run both CI workflows on the final revision. Extract the resulting Windows ZIP
   into a clean folder, launch it and exercise representative avatar/output paths.
5. Check licenses and package contents: no private avatars, SDK binaries, API keys,
   OAuth tokens or personal profiles. Confirm screenshots have permission.
6. Publish/tag only the reviewed release commit. Include a checksum, notable
   changes, installation instructions and known limitations with the artifact.
   GitHub Actions artifacts expire; a release asset requires a deliberate upload.

Repository administration changes should be reviewed like code. Keep this page,
the protection template and actual GitHub settings consistent when policy changes.
