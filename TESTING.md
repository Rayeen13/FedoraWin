# FedoraWin 0.4.0-dev testing notes

## Automated gates

- Python safety, structure and regression suite.
- PowerShell delimiter/recovery-order checks.
- XAML XML validity and control/reference consistency.
- Activities click + Alt+F1 toggle regression guard.
- Application discovery and Terminal alias regression guard.
- GNOME-style paged app drawer regression guard (no vertical scrolling fallback).
- Compact Quick Settings/direct API regression guard.
- Light/Dark/System and accent-propagation regression guard.
- Native DWM frame manager presence and legacy fake-overlay absence.
- AppBar registration/removal and native frame disposal guards.
- No target-window style mutation, `SetWindowsHookEx`, explorer replacement, driver/service install, System32/UXTheme/Winlogon or boot modification patterns.
- GitHub Actions Windows runner: Windows PowerShell 5.1 + WPF/native compile smoke.

## Manual Windows fidelity gate before any release

1. `Preflight.cmd` must pass, then `Safe-Preview.cmd` must show the panel without changing startup state.
2. Activities must open/close repeatedly from the panel and Alt+F1 without terminating the shell.
3. Search for `Terminal` must surface Windows Terminal when installed and launch it correctly.
4. Show Applications must use page navigation (buttons, wheel, arrows, Page Up/Page Down) rather than a long vertical launcher.
5. Calendar, Quick Settings, Appearance and Power popovers must align correctly and dismiss predictably.
6. Volume, brightness, supported radio controls, airplane mode, power mode and screenshot must act in place where Windows exposes a safe API.
7. Light/Dark/System and every accent must update all open shell surfaces coherently.
8. Standard captioned applications must retain real Windows-owned minimize/maximize/close, hit testing, Snap and resize behavior while DWM colors/corners follow the FedoraWin theme.
9. Maximized/snapped windows must respect the top AppBar work area.
10. Exiting FedoraWin must dispose frame tracking, remove the AppBar and restore taskbar/desktop/work-area state.
11. `Restore-Windows.cmd` must return the machine to normal even after a failed shell session.

A passing build is not a release gate by itself. FedoraWin should only be packaged when the Activities/workspace/dash/search/app-grid/calendar/quick-settings experience is coherent enough to reasonably feel like current GNOME rather than a collection of Windows-themed overlays.
