# FedoraWin development snapshot

The active development line is the `develop` branch. Repository-visible source on that branch is the authoritative continuation point for daily work.

## 2026-09-13 continuation point

The previously archived working shell has been restored into normal tracked source files. The bootstrap archive is no longer the CI source of truth. The corrupt legacy archive and one-time hydration staging files have now been removed from `develop`, so future work and CI operate only on reviewable tracked source.

Windows CI now tests the tracked tree directly and currently passes all available automated gates:

- Python safety, structure, and regression tests;
- native C# bridge compilation on `windows-latest`;
- installed-app catalog smoke coverage under Windows PowerShell 5.1;
- WPF/XAML and PowerShell 5.1 smoke coverage.

Implementation advanced in this pass:

- `FedoraWin.ps1` consumes the dedicated `shell/AppCatalog.ps1` module rather than maintaining a second monolithic launcher index;
- Activities/app search uses ranked aliases from the shared catalog, including GNOME-style terms such as `terminal`, `files`, `preferences`, and `screenshot`;
- packaged apps launch through `shell:AppsFolder`, while classic Start Menu apps use their resolved targets;
- the application drawer uses GNOME-style page indicators instead of desktop-style previous/next pager chrome;
- Activities now exposes previous, next, and new workspace controls backed by documented Windows virtual-desktop shortcuts, without installing global hooks;
- existing native DWM frame styling, AppBar top-panel reservation, power-mode integration, Quick Settings work, recovery paths, and non-destructive shell policy remain preserved.

The authoritative native bridge is `native/FedoraWinNativeUi.cs`; do not replace it with the older copy that existed in the recovered working snapshot. Likewise, keep the repository's newer `ui/QuickSettings.xaml` and `shell/AppCatalog.ps1` implementations unless a deliberate newer change supersedes them.

## Remaining runtime gate

Automated Windows CI is green, but final release readiness still requires hands-on desktop validation on a real Windows session for native caption-button hit testing, Snap Layouts, resize borders, multi-monitor AppBar work areas, Activities keyboard/click toggling, Quick Settings device effects, and virtual-desktop transitions. These behaviors depend on an interactive Explorer/DWM session that a hosted CI runner cannot faithfully validate.

Do not publish a release ZIP from this state. Continue development on `develop` until the shell is coherent enough to reasonably call a GNOME clone and both automated gates and interactive Windows runtime validation are green.
