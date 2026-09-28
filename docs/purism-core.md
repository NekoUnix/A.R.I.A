# Purism Core runtime and distribution

ARIA uses [Sakura Motion's Purism Core](https://github.com/SakuraMotion/PurismCore),
an independent MIT-licensed implementation of the Cubism-compatible native API.
The C implementation is statically linked. There is no proprietary Core download,
dynamic Core loader, SDK discovery or fallback. Existing Core paths are ignored.
The native worker protocol is version 4; keep the application and worker together.

An [independent Rust ARIA Model Core](aria-model-core.md) is under development.
It is not yet the production evaluator. The Purism attribution and release
notice remain required until the worker has been switched and the derived C
runtime removed from shipped builds.

## Source and notices

- Revision: `1069334965522df5d0b791e97f01e26b19c45456` (Purism Core 1.1.0).
- ABI: v6, compatibility version 6.0.1; C11 compilation via Cargo's `cc` build dependency.
- [Vendored source and checksum](../crates/aria-live2d/vendor/purism-core/README.md).
- [Full MIT copyright and permission notice](licenses/purism-core.txt).

Both packaging scripts include `docs/licenses/purism-core.txt` and check the
release tree for proprietary Core binaries and raw model files before archiving.
The release check is a guard against accidental inclusion, not a legal audit.
Do not add upstream testdata, sample art, private avatars or SDK downloads to a
release. Retain the MIT notice when redistributing the compiled runtime.

## Publication scope

This change removes ARIA's dependency on Live2D's proprietary Core implementation.
Purism Core's MIT license permits redistribution subject to preserving its notice.
Live2D's [SDK publication terms](https://www.live2d.com/en/sdk/license/) address
products using their SDK, including expandable applications. This build contains
neither that Core nor the Cubism Framework. Swapping Core alone would not remove
terms on other SDK components if they were added later.

This is not a guarantee of legal clearance. Existing agreements, rights in models,
editor exports, artwork, fonts, audio and trademarks are separate. Old ARIA
releases are not changed or retroactively cleared. Distribution plans involving
those materials or an existing Live2D agreement need their own review. Live2D and
Cubism are Live2D Inc. trademarks, used here to describe format compatibility;
ARIA is not affiliated with or endorsed by Live2D Inc.

## Build and validation

When upgrading from v0.36 or earlier, extract the complete new package into its
own folder and launch its application. Keep the matching worker with it: protocol
v4 is incompatible with an old worker executable. Remove a custom
`ARIA_CUBISM_HOST` override if it still points at an older ARIA installation.
Saved external Core paths are ignored. Existing avatar physics stays in Authored;
select Bouncy or Natural in Avatar → Physics and save to adopt the new solver.

Use the pinned Rust toolchain and a C11 compiler (MSVC, Clang or GCC), then run:

```sh
cargo test --locked --workspace
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
python scripts/check-runtime-distribution.py
```

The renderer still rejects offscreen parts and advanced blend modes. Runtime
compatibility does not imply support for every authored rendering feature. Model
deformation and host equivalence tests can run on locally supplied assets using
`ARIA_TEST_MOC` and `ARIA_TEST_HOST`; no Core path is required. See
[validation](validation.md) for the checks actually performed.
