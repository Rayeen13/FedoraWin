# FedoraWin development snapshot

The active development line is the `develop` branch. Repository-visible source on that branch is the authoritative continuation point for daily work.

The legacy `.ci/FedoraWin-worktree.tar.gz` is retained unchanged as a source-snapshot artifact from the earlier bootstrap. Its recorded SHA256 is:

`579c78aec047d04909c6f0f46b22d18dce97cc3b98f37ae69b536a908c4d7c46`

The hash still matches in Windows CI, but the archive currently fails gzip extraction on the GitHub Windows runner. It must therefore **not** be treated as a green full-runtime/regression gate until it is regenerated from a verified working tree.

Current repo-native Windows gates compile `native/FedoraWinNativeUi.cs` under Windows PowerShell 5.1 and exercise `shell/AppCatalog.ps1`, including GNOME-style aliases such as `terminal -> Windows Terminal` and `files -> File Explorer`.

Current implementation delta after the archived snapshot:

- real DWM/native frame manager remains compiled and guarded by CI;
- top-panel AppBar and native power integration remain compiled and guarded by CI;
- installed-app discovery now combines `Get-StartApps` packaged entries with classic Start Menu `.lnk` discovery;
- launcher search has normalized aliases and deterministic ranking for GNOME-style names;
- packaged applications launch through `shell:AppsFolder`, while classic shortcuts launch their resolved target;
- Windows CI keeps the archived full-suite gate visible as a warning instead of silently claiming it passed.

Do not publish a ZIP from this state. Continue development on `develop` until the shell is coherent and the full regenerated snapshot/runtime regression gate is green.
