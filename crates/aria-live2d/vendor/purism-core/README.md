# Vendored Purism Core

Upstream: https://github.com/SakuraMotion/PurismCore
Revision: `1069334965522df5d0b791e97f01e26b19c45456`
Version: 1.1.0; compatibility ABI: v6 (6.0.1).
License: MIT; see [LICENSE](LICENSE) and the shipped
[notice](../../../../docs/licenses/purism-core.txt).

`PurismCoreBundle.h` is the unmodified amalgamation of that revision generated
with upstream `scripts/bundle.sh` (UTF-8, LF line endings).
SHA-256: `68e57128c18a489d01fb66497738abfeae448b266c302d25cef118012a264603`

Only the library is vendored. Upstream testdata, sample artwork, SDK binaries,
viewer and third-party sample dependencies are excluded. Cargo builds the C
source into ARIA; no download, external library path or proprietary SDK is used.
Updates require review of upstream source/license changes, regeneration of the
bundle and digest, and runtime/packaging tests. Keep this notice and the full MIT
license with source and binary distributions.
