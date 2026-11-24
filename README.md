# FedoraWin DE

**FedoraWin DE** is a desktop environment for Windows that targets the current Fedora Workstation / GNOME interaction model, not a generic Linux theme. **FedoraWin Shell** is its shell layer: panel, Activities, app grid, workspaces, calendar, Quick Settings and desktop presentation. Windows/DWM/device APIs remain the underlying platform.

## Development status

`0.4.0-dev` is an architecture rewrite. The old WPF prototype proved the concept but also proved that fake caption overlays and script-scoped UI state are too fragile for a desktop shell. The new host uses Rust + Tauri/WebView2 + HTML/CSS/JS and keeps native Windows APIs behind a small bridge.

The project is **not shippable yet**. Development builds are intentionally withheld until the Windows CI/runtime gates and GNOME-fidelity gates are green.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).
