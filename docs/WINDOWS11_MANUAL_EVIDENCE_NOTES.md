# Windows 11 manual evidence: reviewer notes

Use [the 14-gate Windows 11 beta matrix](BETA_WINDOWS_VALIDATION.md)
and [the evidence record contract](W11_EVIDENCE_MANIFEST.md) together.
Passing GitHub Actions cannot turn a manual `NOT RUN` into `PASS`.

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
