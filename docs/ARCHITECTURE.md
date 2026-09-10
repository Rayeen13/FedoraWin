# FedoraWin architecture

## Direction

FedoraWin 0.4 abandons the PowerShell/WPF fake-frame prototype as the primary shell. The production direction is a small Rust/Tauri host with HTML/CSS/JS shell surfaces and native Windows bridges.

The architecture intentionally mirrors the *shape* of successful Windows desktop-environment projects without copying their code: web UI for rapid visual fidelity, a Rust backend for Win32/COM/WinRT integration, and event-driven native hooks for windows/workspaces.

## Shell surfaces

- **panel** — 32 logical-pixel top bar, registered as a Windows AppBar so maximized/snapped windows stop below it.
- **activities** — full work-area overview, hidden until toggled.
- **date-menu** — calendar, notifications, appointments.
- **quick-settings** — direct-action system controls.

All surfaces use the same `ui/index.html` bundle with a `?view=` selector so styling and state stay consistent.

## Window frames

FedoraWin never paints a second fake caption over an app. It applies reversible DWM attributes to real top-level HWNDs:

- immersive dark/light mode
- native caption color
- native caption text color
- native border/accent color
- native rounded-corner preference

Windows continues to own minimize/maximize/close, hit testing, resizing, Snap Layouts, keyboard accessibility, DPI scaling, and non-client behavior.

## Quick Settings

Direct public APIs are preferred. Wi-Fi uses `WlanSetInterface(... wlan_intf_opcode_radio_state ...)`, which Microsoft documents for changing the software radio state from desktop apps. Features without a stable/public direct control path are reported unavailable rather than pretending a Settings deep-link is a toggle.

## Safety

No Explorer shell replacement, no System32/uxtheme patching, no injected DLL hooks, no driver/service installation, and no machine-wide registry takeover. The shell should be killable without leaving foreign window styles patched.
