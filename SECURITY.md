# Security reporting

Security fixes target the latest development branch and the most recent packaged
Windows build. Older preview versions are not maintained separately.

This repository is public. Use GitHub's
[private vulnerability reporting form](https://github.com/NekoUnix/A.R.I.A/security/advisories/new)
to send security findings to the maintainers. You can also open the repository's
Security tab, choose Advisories and select **Report a vulnerability**.
Ordinary issues, pull requests and discussions are public; a `security` label
does not make their contents private.

Include the affected commit/version, attack prerequisites, minimal reproduction,
impact and a proposed mitigation if known. Use synthetic data and redact
credentials, personal profiles, camera images and private avatar paths.
Coordinate disclosure with the maintainers before publishing exploit details.
If GitHub's reporting form is unavailable, ask repository owner **NekoUnix** for
a private channel before sharing sensitive details. No response-time guarantee
is currently offered.

Review sensitive changes carefully: native Cubism/NVIDIA interfaces, avatar and
image parsers, HTTP authentication, OAuth credentials, camera subprocesses,
dependency installation and Windows shared-texture resources. Vulnerability
alerts and Dependabot security PRs supplement code review; their absence does not
prove a dependency or runtime is safe.
