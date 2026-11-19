# Activities Overview latency and memory evidence protocol

**Status: measurement protocol only; no performance pass is claimed.** This protocol is for a real FedoraWin executable on Windows 11. It does not change the rendering architecture or relax the manual beta gates.

## Purpose and baseline

The native Win32/GDI panel remains resident while the Tauri/WebView2 Activities surface is created on demand. The Rust DWM thumbnail manager registers live thumbnails against eligible HWNDs and checks process identity. Before adding a hidden pre-warmed WebView2, establish whether initialization, rendering, or thumbnail binding dominates user-perceived latency.

Keep the existing Alt+F1 shortcut, Explorer shell/taskbar, native captions and hit testing, DWM journal/guardian, and the under-300 MB process-tree budget intact. Do not introduce global Windows-key hooks or a compositor rewrite as part of this experiment.

## Required run identity

For each measurement attach: UTC timestamp, exact Git commit SHA, Windows build, executable SHA-256, WebView2 runtime version, CPU/GPU, display refresh rate, display scaling and monitor topology, power mode, warm/cold state, and whether a physical or interactive remote Windows 11 session was used. Record raw samples, not just a screenshot or aggregate.

Use the **same exact executable** and machine for baseline and proposed pre-warm comparisons. Capture an Explorer taskbar screenshot before and after; preserve the genuine executable screenshot and its provenance. Do not use mocked UI captures as runtime evidence.

## Instrumentation points

Use a monotonic clock for within-process intervals. Correlate Rust and renderer events by a unique invocation ID rather than subtracting unrelated clocks.

1. `input_received`: native panel action or Alt+F1 callback begins.
2. `surface_requested`: Activities show/create request begins.
3. `hwnd_visible`: visibility is verified for the actual Activities HWND (not merely requested).
4. `renderer_ready`: WebView2 JS has loaded the intended view and data. This is **not** first paint.
5. `thumbnails_bound`: DWM thumbnail update has succeeded for the current eligible window set; record eligible count, registered count, and failures.
6. `first_visible_frame`: independently corroborated via an actual captured/presented non-empty frame. `NavigationCompleted`, `requestAnimationFrame`, or a JS message alone is insufficient proof of a displayed frame.
7. `overview_closed`: window is hidden/closed and resources are in the intended idle state.

For each invocation report input-to-visible, input-to-first-visible-frame, renderer-ready-to-first-visible-frame, and thumbnail-bind latency. Record p50/p95/max and every timeout or missing milestone. Keep failures in the denominator.

## Reproducible comparison

- Restart FedoraWin and collect at least 20 cold invocations across fresh process launches; record initialization separately from subsequent toggles.
- Collect at least 100 warm open/close cycles with a stable set of eligible windows (including a multi-workspace case). Repeat after an Explorer restart.
- If the baseline is demonstrably slow, test a **single** hidden pre-warmed WebView2 surface, not an additional resident copy for every shell view. Measure the same invocations, on the same machine and candidate commit.
- Record frame pacing with a presentation-aware trace such as ETW/PresentMon. At 120 Hz, 8.33 ms is the refresh interval, **not** an automatically achieved FedoraWin frame time. Include p95 frame interval, missed presents, and the trace configuration; DWM timing alone is not a per-app presented-frame guarantee.
- Sample total FedoraWin process-tree private bytes and working set (including WebView2 child processes and GTK helpers) at idle, immediately after first open, during repeated toggles, and after close. Note peak and steady-state values. Compare against the existing **300 MB ceiling**; a previously observed ~33 MB idle value is not a universal guarantee.
- Include keyboard focus, Alt+F1 re-entry, multi-monitor placement, DPI changes, virtual desktop switching, and Explorer restart in the observation notes. No native Windows shortcut may be stolen.

## Decision and rollback

Do not implement pre-warming based on an unverified intuition. Retain the on-demand design if it meets the measured responsiveness goal, or if pre-warming breaches memory, focus, lifecycle, or Explorer recovery constraints. If a pre-warm experiment is pursued, make it explicitly reversible and require the same Windows CI, native GTK/Adwaita, native frame proof, forced-kill rollback, Explorer recovery, screenshot-provenance, and manual Windows 11 gates as the baseline.

A performance trace or CI screenshot is **not** a substitute for W11-01 through W11-14 human-operated validation. Hardware touchpad gestures, mixed DPI, sleep/resume, physical Quick Settings, real caption interaction, and extended memory stability remain unverified until individually evidenced on a physical or interactive remote Windows 11 machine.

## Evidence record template

```text
candidate_commit:
exe_sha256:
utc_start:
windows_build:
webview2_version:
hardware_and_gpu:
monitor_refresh_scaling_topology:
power_mode:
session_type: physical | interactive-remote
variant: on-demand | single-prewarmed
raw_trace_path_and_sha256:
runtime_capture_path_and_sha256:
invocations_total:
invocations_failed:
input_to_hwnd_visible_ms_p50_p95_max:
input_to_first_visible_frame_ms_p50_p95_max:
thumbnail_bind_ms_p50_p95_max:
present_frame_interval_ms_p95:
missed_presents:
process_tree_private_bytes_idle_peak_after_close:
process_tree_working_set_idle_peak_after_close:
explorer_taskbar_before_after:
manual_gate_ids_verified:
notes:
```

No blank field should be interpreted as a pass.
