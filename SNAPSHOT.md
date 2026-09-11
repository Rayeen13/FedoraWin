# FedoraWin development snapshot

Current authoritative working snapshot for the `develop` branch is `.ci/FedoraWin-worktree.tar.gz`.

The archive is a source-snapshot artifact for CI and continued development, not a user-facing release package. It includes the current PowerShell, native C# bridge, XAML, tests, recovery tooling, documentation and SHA256 manifest.

SHA256: `579c78aec047d04909c6f0f46b22d18dce97cc3b98f37ae69b536a908c4d7c46`

The root files in this branch are a bootstrap subset from earlier iterations. Windows CI intentionally verifies and expands the authoritative snapshot before running the full test suite.
