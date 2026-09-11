# FedoraWin

> A Fedora Workstation / GNOME-inspired desktop experience for Windows 11 — built as a **safe, reversible shell layer**, not a destructive Windows replacement.

[Documentation](https://rayeen13.github.io/FedoraWin/) · [Alpha status](https://rayeen13.github.io/FedoraWin/status.html) · [Architecture](https://rayeen13.github.io/FedoraWin/architecture.html) · [Issues](https://github.com/Rayeen13/FedoraWin/issues)

![FedoraWin Quick Settings running on Windows](docs/assets/screenshots/quick-settings-dark.webp)

## Current status

**0.4.0-dev · Alpha preview**

FedoraWin is ready to **showcase as a developer alpha / technical preview**. It is not yet positioned as a daily-driver replacement or a general public release.

The project currently includes GNOME-inspired Activities, application discovery and paging, virtual-workspace controls, dash/search/calendar concepts, Quick Settings, top-panel work-area reservation, light/dark/accent propagation, and native DWM styling on real application HWNDs.

## What makes FedoraWin different

- **Real Windows windows stay real.** Native caption buttons, hit-testing, resizing and Snap are preserved.
- **No fake frame overlays.** Window styling is applied through the actual HWND/DWM path.
- **Windows stays underneath.** Explorer and DWM remain available.
- **Safe system integration.** Direct Windows APIs are used when they are reliable and reversible.
- **Recovery is a feature.** The project maintains restore/startup tooling and regression gates.
- **No system DLL patching, Explorer replacement, kernel drivers or background services.**

## Alpha priorities

1. Harden Activities and workspace behavior across repeated runtime use.
2. Improve live window/workspace presentation toward current GNOME.
3. Expand Quick Settings direct actions and hardware fallbacks.
4. Refine GNOME 50 spacing, proportions, animation and multi-monitor behavior.
5. Keep Windows CI, safety regression tests and recovery checks green.
6. Build a repeatable package only when the shell is coherent enough to ship.

## Documentation

The project website lives under docs/ and is designed to deploy through GitHub Pages. It includes a modern project landing page with real development screenshots, getting-started and recovery guidance, architecture and safety boundaries, an explicit alpha-status page, and static documentation validation in CI.

## Development policy

Releases are intentionally withheld until FedoraWin is coherent enough to reasonably call a GNOME clone rather than a cosmetic overlay. A successful build alone is **not** considered a release.

FedoraWin is an independent open-source project and is not affiliated with the Fedora Project, Red Hat, the GNOME Foundation, or Microsoft.
