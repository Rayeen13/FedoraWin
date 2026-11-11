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
# Start the exact candidate, confirm it is running, and record original screen video.
pwsh ./tests/collect-w11-manual-snapshot.ps1 -Executable './fedorawin.exe' -Stage during -ExpectedExecutableSha256 '<same SHA-256>'
# Perform the human-operated exit/kill/restart procedure; capture the post-test state.
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

The snapshot collector writes Explorer process creation ticks and HWNDs as
**decimal strings**, not JSON numbers: these Windows 64-bit identifiers can
exceed JavaScript's exact integer range. Older numeric snapshots must be
recaptured rather than rounded or reinterpreted.

To compare two collected records without changing Windows state:

```powershell
node tools/compare-w11-manual-snapshots.mjs './artifacts/manual-w11/w11-before-....json' './artifacts/manual-w11/w11-during-....json' './artifacts/manual-w11/w11-after-force-kill-....json'
```

Snapshot schema v3 binds both snapshots to the same interactive Windows
session ID and Windows boot lifetime (lossless .NET UTC ticks from the observed
OS last-boot time). This rejects a reboot or different logon session as
evidence of forced-kill/Explorer recovery. A session ID is not a host identity
or cryptographic attestation; original recordings and operator review remain
mandatory. Schema v1/v2 snapshots must be recaptured.

Snapshot schema v3 also records `candidateProcesses`: the PIDs, lossless
process creation ticks and runtime/guardian roles of **currently running processes
using the selected executable path**. It observes both the main runtime and its
independently launched frame/taskbar guardians without terminating them.
The comparator rejects a baseline taken after FedoraWin started, an
`after-clean-exit` or `after-force-kill` snapshot with any candidate or guardian
still running, and an `after-explorer-restart` snapshot lacking a live runtime.
For Explorer-restart evidence, require **exactly one** observed runtime and reject
runtime or guardian lifetimes that predate the clean baseline. The runtime
must also predate the replacement Explorer process: launching FedoraWin only
after Explorer has recovered is not a valid W11-03 continuity observation.
Explorer and candidate process creation lifetimes must fall within the
observed Windows boot lifetime. Multiple runtime instances make ownership of
the manually exercised UI ambiguous. A guardian started after the baseline is
permitted but is not proof of recovery or DWM rollback.
Wait for guardian recovery to finish before collecting post-exit evidence.
Snapshots collected with schema v1/v2 must be recaptured; they cannot establish
the absence of lingering processes. If a candidate process's path, command line
or creation lifetime cannot be read, collection fails closed.

The comparator requires **three** snapshots: a clean `before`, a `during` capture observing exactly one live FedoraWin runtime before the manual action, and an `after-*` capture. It rejects a missing, pre-baseline, or ambiguous live runtime; a guardian older than the observed runtime; changes in binary, session, boot, or Explorer owner during startup; and invalid ordering. For Explorer restart, the same runtime PID **and creation lifetime** must survive, and the replacement Explorer must start after the `during` capture. This prevents an empty baseline and empty post-kill snapshot from masquerading as evidence that FedoraWin ever ran. Three structurally consistent JSON files still do not prove a human performed the kill, or that DWM rollback succeeded.

The comparator checks the snapshots' contracts, UTC ordering, Windows build,
binary hash and path, interactive session and boot lifetime, Explorer taskbar PID + **process lifetime**, and baseline
primary/secondary taskbar visibility. All taskbar HWNDs in each snapshot must
share the primary taskbar's Explorer PID and creation lifetime; mixed-generation
secondary taskbars and duplicate numeric HWND aliases are rejected. Clean exit and forced-kill observations
must retain the primary Explorer owner's identity; an Explorer restart must
change it. A missing taskbar, changed display count, or mismatch returns a
nonzero exit code for investigation. The process check is observational and cannot authenticate that a person actually
performed the interaction; a separate original video is still required. The tool does **not** test DWM attributes,
caption hit testing, actual user interaction, or original video provenance.
Even a structurally consistent pair always reports `manualGateStatus: "NOT RUN"`
and `betaApproved: false`; a human must review original evidence.

Compare the **before**, **during**, and **after** records with the original human-recorded
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
