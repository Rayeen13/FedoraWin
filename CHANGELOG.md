# FedoraWin changelog

## 0.4.0-dev - native-frame GNOME shell rebuild

- Replaced the legacy fake caption overlay direction with native DWM styling on real top-level HWNDs.
- Preserved Windows-owned caption buttons, hit testing, Snap, resize, DPI behavior and accessibility.
- Kept real Windows AppBar top-panel reservation with reversible cleanup.
- Added safer `SetWinEventHook` tracking for newly created/moved windows without DLL injection.
- Improved installed-app discovery with Start menu + `shell:AppsFolder` sources and launcher aliases such as Terminal.
- Added GNOME-style Activities overview, dash, search, app drawer and calendar surfaces.
- Changed Show Applications from a vertical scrolling launcher to a paged app grid with mouse wheel, arrow and Page Up/Page Down navigation.
- Added compact in-place Quick Settings for supported Windows controls: audio, brightness, Wi-Fi/Bluetooth radios, airplane mode, power mode and screenshot.
- Added Light / Dark / System appearance switching and libadwaita-inspired accent propagation across shell controls.
- Added in-shell power actions and retained explicit recovery tooling.
- Expanded safety, structure and regression tests; Windows CI now runs PowerShell 5.1/WPF smoke checks.

## 0.3.0 - early shell / window integration prototype

- Fixed Activities search state lifetime and dispatcher exception containment.
- Added initial AppBar integration and top-level-window tracking.
- Added the first appearance controls and application launcher.
- This line still used an experimental overlay-frame approach and is superseded by 0.4.0-dev.

## 0.2.x

- Reworked the initial WPF prototype into a modern GNOME-inspired shell layout.
- Replaced stock WPF calendar and Unicode glyph UI with custom vector/layout controls.
