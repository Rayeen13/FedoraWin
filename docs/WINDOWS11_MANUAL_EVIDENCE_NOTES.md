# Windows 11 manual evidence: reviewer notes

Use [the 14-gate Windows 11 beta matrix](BETA_WINDOWS_VALIDATION.md)
and [the evidence record contract](W11_EVIDENCE_MANIFEST.md) together.
Passing GitHub Actions cannot turn a manual `NOT RUN` into `PASS`.

The read-only before/after comparator preserves the collector's seven-digit UTC
fractions (100 ns resolution) when checking snapshot order, and rejects
impossible calendar dates instead of accepting JavaScript date rollover.
This is a structural consistency check, not proof that an operator actually
performed a Windows 11 test or that native DWM rollback succeeded.

Explorer start-time ticks must not be later than their own capture. For an
intentional Explorer restart, the new primary taskbar owner's process lifetime
must begin **after** the baseline snapshot and no later than the post-restart
snapshot; merely switching to a pre-existing Explorer process is insufficient.
This check still cannot establish that a restart command was executed or that
all secondary-monitor taskbars and native caption controls behave correctly.

For a candidate CI-built executable, keep the immutable run URL, commit SHA,
downloaded artifact identifier, and SHA-256 measured on the binary actually
executed. Note Windows 11 edition/build, tester, date, and VM or physical
device. Capture real, time-correlated output and original screenshots/logs
for each relevant gate. Keep failed attempts and forced-kill rollback evidence.

Hardware Quick Settings, DPI/displays, and sleep/lock lifecycle gates
(`W11-10`, `W11-11`, `W11-12`) require physical testing. For forced-kill
and Explorer continuity, verify process identity, native caption restoration
and taskbar behavior before/after—not merely a visually similar screenshot.

The Node checker validates that references and metadata are present. It does
**not** inspect capture bytes, attest the Windows build, authenticate the
tester, verify binary execution, or make a release decision. Have a release
owner independently review the originals and cross-check binary provenance.

**Current approval: withheld.** Do not publish a beta ZIP or change the
public website to claim completion until automated and Windows 11 manual
gates are independently verified.
