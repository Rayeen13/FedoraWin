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

FedoraWin never paints a second fake caption over an app. It applies reversible DWM attributes to eligible real top-level HWNDs. Borderless, layered, no-redirection-bitmap and tool windows are excluded; custom-drawn windows can still require an additional app-specific exclusion:

- immersive dark/light mode
- native caption color
- native caption text color
- native border/accent color
- native rounded-corner preference

Windows continues to own minimize/maximize/close, hit testing, resizing, Snap Layouts, keyboard accessibility, DPI scaling, and non-client behavior. The per-HWND journal tracks successful writes and restores the observed original attributes on ordinary shutdown or when a window becomes ineligible. Switching back to System restores previously forced caption colors.

## Quick Settings

Direct public APIs are preferred. Wi-Fi uses `WlanSetInterface(... wlan_intf_opcode_radio_state ...)`. Master volume uses the default Core Audio endpoint through `IAudioEndpointVolume`. Battery/AC/charging state comes from `GetSystemPowerStatus`. Screenshot opens the native Snipping overlay through the documented Win+Shift+S input chord. Bluetooth is planned around `Windows.Devices.Radios` with permission and effective-state checks. Brightness will be capability-detected across internal WMI and supported physical-monitor paths. Features without a stable/public direct control path are reported unavailable rather than pretending a Settings deep-link is a toggle.

## Safety

No Explorer shell replacement, no System32/uxtheme patching, no injected DLL hooks, no driver/service installation, and no machine-wide registry takeover. The independent taskbar guardian restores Explorer even after a force kill. Native DWM attributes currently restore on normal process shutdown, not on a force kill: the frame journal is process-local. Do not mark native-frame recovery or the beta release gate complete until an independent forced-termination recovery path and a physical Windows test verify it. This limitation must not be hidden by the screenshot or taskbar-recovery CI gates.
