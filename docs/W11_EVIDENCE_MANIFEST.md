# Windows 11 evidence manifest contract

This is a **record format**, not a completed test report. Current manual gate
results remain **NOT RUN** in [the authoritative validation matrix](BETA_WINDOWS_VALIDATION.md).
The validator checks field structure only. It never approves a beta.

For **each** `W11-01` through `W11-14`, a PASS record must include:

| Field | Required value |
| --- | --- |
| `id` | One unique ID from W11-01 through W11-14 |
| `status` | `PASS` only after manual testing (otherwise FAIL/BLOCKED/NOT RUN) |
| `environment` | `physical`, `vm`, or `remote-vm` |
| `tester` | Named human tester, not a CI job |
| `testDate` | Actual manual test date in `YYYY-MM-DD` format; a future date is rejected (one UTC day of timezone tolerance) |
| `windows11Build` | Observed Windows 11 edition and numeric OS build 22000 or later, e.g. `Windows 11 Pro 25H2 (OS Build 26200.8875)` |
| `executableSha256` | Exact 64-digit hex SHA-256 of the **executed** CI-built binary |
| `evidence` | Nonempty array of references to original screenshots, recordings, or logs |

**W11-10, W11-11, W11-12 require a physical Windows 11 device.**
For the other gates, an unmodified Windows 11 VM can provide exploratory/manual
coverage under the matrix's criteria. The structural validator rejects a Windows 10/unknown build or an impossible
future test date, but these checks do **not** authenticate the tester or OS.
A fabricated filename or SHA-shaped string
is not proof: the reviewer must open each original, check timestamp/build and
match the hash against the actual CI executable and run provenance.

All 14 manual gate records must identify **the same executed release-candidate
binary SHA-256**. Digest hex casing is ignored; a different hash means the
manifest mixes builds and cannot establish release-candidate validation.
Re-test any affected gates against one candidate before reviewer sign-off.

## Read-only Windows 11 environment snapshots

On an **actual Windows 11** test machine, use the new collector before and after
the human-operated procedure. Run from the repository root, pointing at the
**exact CI-built executable being tested** (do not substitute a debug build).
The optional expected hash should be copied from the trusted CI artifact:

```powershell
pwsh ./tests/collect-w11-manual-snapshot.ps1 -Executable './fedorawin.exe' -Stage before -ExpectedExecutableSha256 '<64-character CI artifact SHA-256>'
# Perform the W11 manual procedure and record the original screen video.
pwsh ./tests/collect-w11-manual-snapshot.ps1 -Executable './fedorawin.exe' -Stage after-force-kill -ExpectedExecutableSha256 '<same SHA-256>'
```

Other supported post-test stages are `after-clean-exit` and
`after-explorer-restart`. Each invocation writes a separate timestamped JSON
under `artifacts/manual-w11`, recording OS edition/build, executable hash,
Explorer PID + process creation lifetime, and the real primary/secondary
taskbar HWND visibility and owning process lifetime. The collector does **not**
launch or terminate applications, change DWM attributes, hide taskbars, or
restart Explorer. It rejects non-Windows 11 hosts, absent Explorer/taskbars,
and a mismatched expected executable hash.

Compare the **before** and **after** records with the original human-recorded
video, noting that Explorer restart intentionally changes process identity.
The JSON snapshots alone are **not** forced-kill proof, screenshot
authentication, human sign-off, or a PASS record. The collector always emits
`manualGateStatus: "NOT RUN"` and `betaApproved: false`. Review original
recordings and CI provenance before adding separate signed manual gate results.
Do not publish snapshots without checking for machine-specific metadata.

Example of a **NOT RUN placeholder**, not test evidence:

```json
{
  "id": "W11-01",
  "status": "NOT RUN",
  "environment": "vm",
  "tester": "",
  "testDate": "",
  "windows11Build": "",
  "executableSha256": "",
  "evidence": []
}
```

Run `node --test tests/manual_beta_gate_contract.test.mjs` to verify the
validator's regression contract. The fixtures use synthetic PASS data solely
to exercise error handling; **no Windows 11 testing is implied**.

Even when `checkManualGateCoverage(...).complete === true`,
`betaApproved` remains `false`: a release owner must separately review
actual artifacts and approve every gate before any beta-ready claim or ZIP.
