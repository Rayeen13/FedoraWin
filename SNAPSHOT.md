# FedoraWin development snapshot

Current authoritative working snapshot for the `develop` branch is `.ci/FedoraWin-worktree.tar.gz`.

The archive is a source-snapshot artifact for CI and continued development, not a user-facing release package. It includes the current PowerShell, native C# bridge, XAML, tests, recovery tooling, documentation and SHA256 manifest.

SHA256: `0a0755d3541f1048bc783ea930aa6b6c2e418b53424682e461e407dacc9bb4dc`

The root files in this branch are a bootstrap subset from earlier iterations. Windows CI intentionally verifies and expands the authoritative snapshot before running the full test suite.
