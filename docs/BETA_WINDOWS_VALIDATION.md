# FedoraWin pre-beta Windows 11 validation gates

**Status: NOT YET APPROVED FOR BETA.** CI success proves only the checks that
actually ran. Every manual gate below must be recorded as PASS on an
unmodified Windows 11 environment; FAIL, BLOCKED, or NOT RUN is not a pass.

This is a release checklist, not evidence of completed testing. Do not
publish a beta archive or label FedoraWin beta until the release owner
reviews real executable captures, logs, and this matrix.

## Environments and evidence

Use an isolated Windows 11 VM or spare PC first; take a snapshot before running
the executable. Test at least one **unmodified Windows 11** installation for
release sign-off. Tiny11/Nano11 may be useful for exploratory smoke tests but
cannot establish compatibility with ordinary Windows 11. A remote VM is suitable
for window, Explorer, and shell checks; physical-device results are required
for hardware Bluetooth, Wi-Fi, brightness, GPU/DPI, sleep/resume and monitors.

Record for each session:

- Commit SHA, CI run URL, executable SHA-256, Windows build and edition
- Test environment (VM or physical hardware), monitor resolutions/scales and GPU
- Baseline Explorer PIDs, whether the Windows taskbar is initially visible,
  FedoraWin's process-tree memory measurements, and cleanup outcome
- A PASS / FAIL / BLOCKED / NOT RUN result for each case, with timestamped
  real screenshots or screen recordings where they demonstrate the behavior
- Reproduction steps and logs for failures; never substitute UI mockups
  or Linux/Wine/OnWorks captures for genuine Windows 11 runtime evidence

CI evidence to review before the manual run: Windows CI (unit/contract/compile/
release build, force-kill frame recovery, taskbar recovery, Explorer restart,
memory budget and executable captures), native Adwaita proof, and disposable
native frame attach/detach proof. CI artifacts are not release approval.

## Manual gate matrix

| ID | Gate | Required result | Result |
| --- | --- | --- | --- |
| W11-01 | Startup and clean shutdown | Explorer stays running; panel starts, exits and returns Windows taskbar visibility to baseline | NOT RUN |
| W11-02 | Forced termination | End FedoraWin in Task Manager; independent guardians restore real taskbar and original DWM attributes without restarting Explorer | NOT RUN |
| W11-03 | Explorer restart | While FedoraWin runs, restart Explorer once; new taskbar HWNDs are managed and restored on FedoraWin exit; no shell replacement | NOT RUN |
| W11-04 | Windows-owned caption | Standard Win32 apps retain native minimize/maximize/close, caption drag, double-click, Alt+Space, resizing and keyboard accessibility | NOT RUN |
| W11-05 | Snap behavior | Win+Arrow, Snap Layouts, snapped maximized geometry, and taskbar/panel reserved work area behave correctly | NOT RUN |
| W11-06 | Exclusion and rollback | A user exclusion immediately restores an affected app; it stays excluded on restart; unreadable exclusion policies fail closed | NOT RUN |
| W11-07 | Custom and protected apps | Chromium/Firefox, GTK/Qt custom frames, system/security windows and Explorer shell surfaces are not incorrectly restyled | NOT RUN |
| W11-08 | Activities and workspaces | Open/close Activities, switch actual desktops, move/focus windows and use the launcher; state remains synchronized across focus failures | NOT RUN |
| W11-09 | Panel and popovers | Activities/date menu/Quick Settings are interactive and mutually consistent, including clock/calendar/notification updates | NOT RUN |
| W11-10 | Hardware Quick Settings | Audio, Wi-Fi, Bluetooth and brightness are backed by real device state where supported; unsupported controls show unavailable rather than a fake success | NOT RUN |
| W11-11 | DPI and displays | 100%/150%/200% scale, secondary monitor, display disconnect/reconnect and mixed DPI do not strand overlays or taskbar restoration | NOT RUN |
| W11-12 | Theme and lifecycle | Dark/light/System switches, sleep/resume, lock/unlock and app relaunch preserve native Windows behavior and leave no stale styles | NOT RUN |
| W11-13 | Memory and stress | Whole FedoraWin process tree stays under 300 MB sampled working-set ceiling, with no growing handle/process leak during extended interaction | NOT RUN |
| W11-14 | Portable GTK/libadwaita | Fresh Windows test environment launches packaged FedoraWin-owned native preferences without assuming MSYS2 is installed | NOT RUN |

## Procedure and safety invariants

1. Capture baseline taskbar visibility and Explorer process IDs:
   `Get-Process explorer | Select-Object Id, ProcessName, StartTime`.
   Do not run tests on the main working desktop without a snapshot or restore plan.
2. Launch the **actual CI-built executable** with its corresponding captured
   commit SHA. Do not install kernel hooks, replace Explorer, patch System32/
   uxtheme, disable Windows Update, or add a driver/service for these tests.
3. Use a disposable standard-frame Win32 window for frame interaction before
   testing more applications. Confirm native Windows still owns caption input
   and system actions. Compare the same HWND's DWM state before/after OFF.
4. Record a real video of Task Manager force termination and recovery, and
   verify the *same baseline Explorer process* remained alive. On failure,
   restore a VM snapshot; do not claim recovery based on a screenshot alone.
5. For independent window/desktop testing, avoid making the test depend on
   working third-party cross-process injection: that frame engine is only an
   RFC, not a current implemented FedoraWin feature.
6. Run the hardware controls on an actual laptop when possible. Remote Desktop
   redirection may not expose Wi-Fi, Bluetooth, brightness or display behavior
   representative of local hardware.
7. Switch FedoraWin OFF and close it; verify original taskbar visibility,
   caption behavior, application content and window-state continuity. Preserve
   logs of any stranded HWND and block release until fixed.

## Release decision

A reviewer may mark a gate PASS only with reproducible evidence from the
specified platform. Record evidence links in the PR/release record rather
than editing this template into a misleading permanent all-PASS matrix.

**No public beta ZIP** until the automated Windows/native gates are green,
W11-01 through W11-14 have the required manual/physical coverage, critical
regressions have been resolved, and the intended release build has been tested.
Do not update the website with synthetic desktop pictures or unevaluated builds.
