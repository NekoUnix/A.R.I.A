# Contributing to A.R.I.A.

Start with the [Windows build guide](docs/windows.md) and
[architecture map](docs/architecture.md). The supported development target is
Windows x64. The built-in Mica puppet runs without proprietary SDKs or avatar files.

## From issue to pull request

1. Use a bug, feature or compatibility issue to explain the problem and expected
   behavior. Small, clear fixes can go straight to a pull request.
2. Update `main`, then create a short-lived branch such as `fix/head-pitch`,
   `feature/camera-controls` or `docs/setup`. Coding agents use `codex/` branches.
   Public contributors without write access should fork the repository and open
   a pull request from their fork. A maintainer must approve external-fork workflow
   runs; that permission is separate from reviewing the proposed code.
3. Keep the change focused. Include profile migration when saved settings change;
   preserve authored model ranges and keep avatar settings scoped to that avatar.
4. Run the checks below and open a draft PR early if feedback would help.
5. Complete the PR template. Request a collaborator other than the author, address
   feedback and resolve each conversation after the correction is verified.
6. Obtain one approving review and green checks for the latest revision, then
   squash merge. The merged branch is deleted automatically by GitHub.

The public repository enforces pull requests, one independent approval, resolved
review conversations and both CI checks on `main`. Only NekoUnix has the explicit
owner bypass described in [the development guide](docs/development.md); there is
no blanket administrator, bot or collaborator bypass.
See the [review policy](docs/development.md). CODEOWNERS requests the default
maintainer on eligible PRs; the owner's PRs need another write-access reviewer.
Green checks alone do not constitute review. Maintainers may explicitly delegate
review to a coding agent, which must identify that delegation in its review;
GitHub's separate-author and latest-push approval requirements still apply.

## Local checks

Install the Rust toolchain from `rust-toolchain.toml`, C++ Build Tools and CMake as
described in the Windows guide. Python 3.12 is used for camera conversion and
repository checks; those checks do not require the webcam runtime or a camera.

```powershell
cargo fmt --all --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo test --locked --workspace --all-features
python -m unittest discover -s tracking -v
python scripts/check-repository.py
```

Run the tests affected by your change while developing. Before merge, let the
Windows workflow complete formatting, Clippy, tests and portable packaging.
UI, rendering and tracking changes also need a manual check with a relevant avatar.
Report the Windows/GPU/input hardware used and what could not be tested. Never
claim physical webcam, controller or NVIDIA inference coverage from simulated data.
Use `./scripts/build-windows.ps1` when a portable artifact needs local verification.
For camera dependency changes, use Python 3.12 x64 to run
`python -m pip install --dry-run --ignore-installed --only-binary=:all: -r tracking/requirements-lock.txt`
before installing into a clean test environment. CI also checks this resolution.

## Code and review expectations

- Keep rendering and UI work responsive. Bound queues and allocations; handle
  tracking loss, device disconnects and worker shutdown explicitly.
- Validate file sizes, external values, FFI contracts and API commands before
  touching native resources or persisted settings.
- Add regression tests for meaningful behavior changes, especially input axes,
  range conversion, profile migration, API authorization and worker ownership.
- Update the relevant guide, offline help and templates when a user-facing
  behavior or protocol changes. Include screenshots for visible UI changes.
- Pin reproducible dependencies. For camera updates, update both requirements
  files, rebuild a clean Python 3.12 x64 environment and repeat inference checks.
- Keep third-party notices current. Do not commit SDK binaries, private avatars,
  access tokens, personal profiles or unapproved artwork. Documentation screenshot
  permission is scoped as described in [image provenance](docs/images/README.md).

## Dependencies and releases

Dependabot opens grouped weekly update PRs for Cargo, GitHub Actions and the
camera Python environment. Updates require the same review and validation as
other code. A maintainer can authorize auto-merge for an individually reviewed PR
after checking compatibility; GitHub still requires passing checks and approval.
Major upgrades stay separate.

For releases, follow the [release checklist](docs/development.md#release-checklist).
Use the artifact from a successful Windows run for the exact release commit.
Do not distribute Cubism or NVIDIA binaries inside ARIA packages.

See [security reporting](SECURITY.md) and [community expectations](CODE_OF_CONDUCT.md).
