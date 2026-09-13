# Security reporting

Security fixes target the latest development branch and the most recent packaged
Windows build. Older preview versions are not maintained separately.

This repository is currently private. Collaborators can open an issue with the
`security` label; it is visible to everyone with repository access. Include the
affected commit/version, attack prerequisites, minimal reproduction, impact and
a proposed mitigation if known. Use synthetic data and redact credentials,
personal profiles, camera images and private avatar paths.

For details that should not be shared with all collaborators, ask repository
owner **NekoUnix** for a private reporting channel before posting the details.
Do not put a live exploit or secret in an ordinary public issue. If the repository
becomes public, the owner must establish private vulnerability reporting and
update this policy first. No response-time guarantee is currently offered.

Review sensitive changes carefully: native Cubism/NVIDIA interfaces, avatar and
image parsers, HTTP authentication, OAuth credentials, camera subprocesses,
dependency installation and Windows shared-texture resources. Vulnerability
alerts and Dependabot security PRs supplement code review; their absence does not
prove a dependency or runtime is safe.
