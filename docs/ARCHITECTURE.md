# FedoraWin architecture

## Direction

FedoraWin 0.4 abandons the PowerShell/WPF fake-frame prototype as the primary shell. The production direction is a small Rust/Tauri host with HTML/CSS/JS shell surfaces and native Windows bridges.

The architecture intentionally mirrors the *shape* of successful Windows desktop-environment projects without copying their code: web UI for rapid visual fidelity, a Rust backend for Win32/COM/WinRT integration, and event-driven native hooks for windows/workspaces.

## Shell surfaces

- **panel** — 32 logical-pixel native Win32/GDI top bar, registered as a Windows AppBar so maximized/snapped windows stop below it. It follows GNOME's workspace-indicator / centered-clock / system-status structure without keeping WebView2 resident.
- **activities** — full work-area overview, hidden until toggled.
- **date-menu** — calendar, notifications, appointments.
- **quick-settings** — direct-action system controls.

All surfaces use the same `ui/index.html` bundle with a `?view=` selector so styling and state stay consistent.

## Window frames

FedoraWin never paints a second fake caption over an app. It applies reversible DWM attributes to eligible real top-level HWNDs. Borderless, layered, no-redirection-bitmap and tool windows are excluded. WS_CAPTION is not sufficient by itself: Chromium/Opera-style, Firefox, GTK, Qt, SDL, GLFW and other known self-drawn window classes are skipped even if they retain the caption style bit. Windows that explicitly request DWM DO_NOT_ROUND are also skipped. This is a conservative guard, **not perfect automatic detection of every custom frame**; broader class/owner validation and a per-window exclusion mechanism remain beta work. Third-party Win32 windows do not become GTK/libadwaita windows:

- immersive dark/light mode
- native caption color
- native caption text color
- native border/accent color
- native rounded-corner preference

Windows continues to own minimize/maximize/close, hit testing, resizing, Snap Layouts, keyboard accessibility, DPI scaling, and non-client behavior. The per-HWND journal tracks successful writes and restores the observed original attributes on ordinary shutdown or when a window becomes ineligible. Switching back to System restores previously forced caption colors.

## Optional in-process frame engine (design only)

A faithful Adwaita-style frame for standard third-party Win32 apps may require
opt-in, in-process **user-mode** non-client rendering; DWM alone is insufficient.
See [FRAME_ENGINE_RFC.md](FRAME_ENGINE_RFC.md) for the exact eligibility, consent,
privacy, rollback, no-reboot goal, failure modes and release gates. It is not
implemented. FedoraWin-owned GTK windows already have a separate real-libadwaita
proof. Do not call the proposed foreign-window renderer real GTK widgets.

## Quick Settings

Direct public APIs are preferred. Wi-Fi uses `WlanSetInterface(... wlan_intf_opcode_radio_state ...)`. Master volume uses the default Core Audio endpoint through `IAudioEndpointVolume`. Battery/AC/charging state comes from `GetSystemPowerStatus`. Screenshot opens the native Snipping overlay through the documented Win+Shift+S input chord. Bluetooth is planned around `Windows.Devices.Radios` with permission and effective-state checks. Brightness will be capability-detected across internal WMI and supported physical-monitor paths. Features without a stable/public direct control path are reported unavailable rather than pretending a Settings deep-link is a toggle.

## Safety

No Explorer shell replacement, no System32/uxtheme patching, no driver/service installation, and no machine-wide registry takeover. The *currently implemented* runtime has no injected DLL hooks. The proposed optional per-app user-mode frame engine is a future feature and must obey the separate FRAME_ENGINE_RFC.md safeguards. The independent taskbar guardian restores Explorer after a force kill. A separate frame guardian now reads a write-ahead DWM journal after FedoraWin exits, including on forced termination. New HWNDs are not styled unless their original attributes are committed to disk first; recovery checks both PID and process creation time and avoids overwriting values that no longer match FedoraWin's styling. If the guardian cannot launch or persistence fails, frame styling is disabled for the affected scope. A Windows CI integration test force-kills the actual FedoraWin executable while a native WinForms window is styled and checks restoration. This implementation is not a beta guarantee until that CI gate passes and physical Windows testing covers Explorer restarts, custom titlebars, Snap and multiple monitors. DWM restoration after OS crash or power loss is not covered by the process guardian.
