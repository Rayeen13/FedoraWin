# Windows 11 interactive beta evidence protocol

This protocol supplements automated Windows CI; it **does not** turn a passing CI run into manual approval. Run on a physical or interactive remote Windows 11 desktop with an actual logged-in user, not a headless runner. Record the native development commit SHA, Windows build, GPU, monitors, DPI scales, session type, test operator and timestamp before testing.

## Evidence and result rules

For each applicable W11-01 through W11-14 gate in the repository's manual beta checklist, record **PASS**, **FAIL**, **BLOCKED**, or **NOT RUN**. Never infer PASS from a screenshot, a unit test, a runner process exit code, or an older commit. Attach a short screen recording or before/after captures showing the real executable and OS behavior, the reproduction steps, the observed result, and any relevant Windows Event Viewer logs. Redact usernames, notifications and personal content before sharing. Record SHA-256 hashes of the executable and evidence files so results are attributable to one build. A blocked gate remains unverified.

## High-risk interactive sequence

1. Before starting FedoraWin, capture Explorer.exe PID, taskbar visibility, existing window caption appearance and Windows theme. Verify the ordinary Windows desktop is usable.
2. Start the exact CI-built executable in a standard-user session. Capture the read-only `during` snapshot while the candidate is visibly running, **before** clean exit, forced kill, or Explorer restart; the three-snapshot comparator rejects an unobserved runtime or a replacement process. Confirm the Windows taskbar/Explorer remain owned by Windows; verify Activities, workspace switching and undo with real application windows. Check that shell/taskbar/desktop windows cannot be moved and that closed or recycled windows cannot be undone into a new process lifetime.
3. Open a normal Win32 app and a custom-titlebar app. Verify only eligible standard frames change. Exercise minimize, maximize, close, resize, caption dragging, Snap Layouts, keyboard navigation and mixed-DPI monitor movement. Verify application-owned caption changes made after FedoraWin styling are not overwritten during normal rollback.
4. Exercise Quick Settings against real hardware: audio, wireless radio, battery/AC and brightness where supported. Record unsupported capabilities as unavailable, not passing toggles. Exercise the GNOME-like panel, calendar and overview while switching focus.
5. Kill the FedoraWin process tree without graceful shutdown. Collect a read-only schema-v3 post-kill snapshot only after the runtime and independent guardians exit; the comparator rejects any matching processes still running. Independently inspect DWM attributes of the previously styled app; confirm the original values are restored without logging out or rebooting. Check Explorer taskbar remains responsive. Restart Explorer and verify taskbar/AppBar recovery and no abandoned FedoraWin-owned surfaces.
6. Repeat startup/shutdown, sleep/resume, monitor attach/detach and Explorer restart in the same interactive session. Record process-tree private memory over an extended representative workload, including all FedoraWin child/guardian processes; fail the budget if the total exceeds 300 MB.
7. Run the independent GTK4/libadwaita and native Win32 frame proofs against the matching commit. These are automated/native proof gates, **not** proof that foreign applications render GTK widgets.

## Release decision

Require passing Windows CI on the exact candidate SHA, independent GTK/libadwaita proof, native frame proof, genuine executable screenshot provenance, forced-kill DWM rollback, Explorer recovery, under-300 MB measurement, and all required W11-01..W11-14 **interactive** results. Any FAIL, BLOCKED or NOT RUN means **pre-beta**. Do not publish a beta ZIP or advertise beta readiness. Website changes may show only verified runtime captures and clearly label automated evidence separately from manual evidence. Do not claim cross-process Adwaita injection is implemented.
