# FedoraWin optional Adwaita-style native frame engine — design contract

**Status: STAGE 1 PROVEN; CROSS-PROCESS INJECTION NOT IMPLEMENTED.**
This document defines safety constraints for a future opt-in app-process engine.
A disposable Win32 window now passes a *same-process, same-GUI-thread*
attach/detach/reattach test using actual window-procedure replacement;
its original and restored screenshots were identical in the initial
[Windows CI run](https://github.com/Rayeen13/FedoraWin/actions/runs/35592649463).
This does not validate injection into another application, Snap hover, physical
DPI, accessibility or complete Adwaita visual fidelity. The separate real
GTK4/libadwaita executable is a *FedoraWin-owned GTK window*.
No third-party application is currently injected or given genuine `AdwHeaderBar`.

## Goal

Give consenting, compatible applications with *standard Win32 non-client frames* a
faithful Adwaita-style headerbar, including window-control circles, proper spacing,
GNOME-aligned light/dark colours and iconography. Keep ordinary move, minimize,
maximize/restore, resize, keyboard/system menu, high-DPI, accessibility, snap and
multimonitor behavior. FedoraWin itself uses **real GTK4/libadwaita** where practical;
the foreign-window renderer is a **separate native Win32 implementation** of the
visual/interaction specification. Sharing design tokens does not turn a foreign HWND
into a GTK/Adwaita widget.

The existing reversible DWM-colour/corner engine remains the compatible fallback.

## Why in-process code may be necessary

DWM can recolour captions and request corners, but cannot draw a replacement
Adwaita headerbar or its circular controls inside another app's own non-client area.
A conventional custom frame needs the owning window's message processing for
`WM_NCCALCSIZE`, `WM_NCHITTEST`, non-client painting, system actions and resize
behavior. Microsoft's custom-frame reference describes precisely this requirement.
A **narrowly scoped opt-in helper inside the app's process** is therefore a possible
implementation, not an excuse to patch the OS or pretend colour changes are enough.

Research references:
- https://learn.microsoft.com/en-us/windows/win32/dwm/customframe
- https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowshookexw
- https://learn.microsoft.com/en-us/windows/win32/api/commctrl/nf-commctrl-setwindowsubclass
- https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-unhookwindowshookex

## Scope and consent

- **Off by default; separate explicit opt-in.** No silent injection into all processes.
- Start with one disposable test app, then an allowlist of user-selected conventional
  apps only after successful compatibility tests. A safe reset/disable switch works
  before enabling a second app.
- Reject custom/borderless/titlebar-rendering apps by window behavior and class;
  Opera GX/Chromium, Firefox, GTK, Qt, WinUI custom chrome, games and launchers
  are initially excluded. The class veto is conservative, not a proof that all
  remaining apps are safe. Per-window user exclusions take precedence.
- Never attach to Explorer, other OS shell components, elevated/privileged targets,
  protected processes, security software, anti-cheat/DRM, UWP/package isolation
  boundaries or apps whose integrity level differs. No privilege elevation to force it.
- A 64-bit helper cannot attach to 32-bit apps. Any 32-bit support needs its own
  tested helper; do not claim universal app coverage.
- No new driver, service, boot/startup patch, uxtheme/System32 change,
  undocumented kernel hook, registry takeover or cross-process memory scan.

## Proposed lifecycle (requires implementation and tests)

1. Discover candidate HWNDs out-of-process, using the existing lifecycle tracker.
   Capture PID + process creation time + thread ID and verify standard-frame eligibility.
2. Ask for per-app permission and attempt **thread-scoped** rather than
   desktop-global attachment; fail closed if Windows refuses.
3. On that same GUI thread, save the exact prior window/message-handler state and
   DWM attributes *before* changing anything. Do not subclass a foreign GUI thread
   from the FedoraWin process: the Windows subclass helper disallows cross-thread
   use. No hook is installed in the current build.
4. Render and hit-test the replacement non-client controls in that app's owning
   context. Preserve border sizing, caption drag, system menu, right-click,
   keyboard actions, minimize, restore, maximize, Snap Layouts and DPI changes.
   Avoid overlays with input interception and do not modify application content.
5. Disable styling and restore the original handler + window state **in the owning
   GUI thread**, without restarting Explorer or Windows. Remove scoped hooks after
   all in-flight callbacks are quiescent; never unload code executing in a callback.
   UnhookWindowsHookEx explicitly warns a callback may still be running afterward.
6. On app exit, window destruction, PID reuse, loss of consent, failed health check
   or incompatibility, stop further attachment and restore where possible.
   Maintain a bounded crash/failure exclusion journal.

## Risk and rollback contract — no false guarantees

- Intended normal-path result: toggle OFF → original frame returns without
  restarting Windows or Explorer; app relaunch should not normally be needed.
- **Not a universal guarantee:** a crashed, hung, or non-cooperating target might
  need *that application* restarted. Uninstall must never claim a DLL was unloaded
  while another process still has active callbacks.
- In-process helpers share the target's failure domain. An injected-frame bug can
  crash an app, interfere with its input, or cause unsaved work to be lost. We
  cannot truthfully promise zero risk to OS, app data, or all third-party apps.
- FedoraWin does not intentionally read/modify documents, credentials, settings,
  app databases or networking. Nevertheless a DLL inside an app has that app's
  privileges, so signed/reviewable binaries, narrow IPC messages, no telemetry,
  and permission restrictions are necessary. Make no absolute privacy guarantee.
- If the loader fails, use existing DWM styling or leave the window untouched.
  If frame recovery is uncertain, choose the original Windows frame.

## Release gates before any user-facing switch

- First same-process prototype in a disposable Windows app: automated attach/detach/reattach passed; appearance and cross-app integration remain unfinished.
- Attach, disable, detach, reattach and force-kill cases on 64-bit and, separately,
  32-bit where supported; no OS reboot, and document when app relaunch is needed.
- Prove Win+arrow Snap, Windows Snap Layouts, border/corner resizing, caption drag,
  double-click, Alt+Space, keyboard/accessibility, per-monitor mixed DPI,
  dark/light changes, minimized/maximized and device/monitor changes.
- Confirm self-styled apps and protected targets receive **zero injection**;
  audit helper loaded-module list and process identity before/after.
- Run multi-hour app stress, memory/handle-leak tests and per-app compatibility
  matrix on physical Windows. Do not ship while only CI test apps are passing.

**Beta status:** Native *cross-process* injected frame engine remains unimplemented and untested. The disposable same-process proof, existing native shell CI, and separate real-libadwaita captures do not validate injection-specific release gates.
