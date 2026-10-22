# Windows 11 manual evidence: candidate executable binding

**Pre-beta only.** The authoritative 14 manual gates remain **NOT RUN**
until original Windows 11 interactive and physical-device testing is reviewed.
A green CI run cannot turn those gates into PASS.

After manually testing one CI-built release candidate, save the gate records
as a JSON array or as an object containing a `gates` array. For each record,
include the fields defined in [the manual evidence contract](W11_EVIDENCE_MANIFEST.md).
Run the following from the repository root, supplying the exact binary tested:

```powershell
node tools/verify-w11-manual-evidence.mjs .\manual-evidence.json .\fedorawin.exe
```

This helper streams the candidate binary into SHA-256 and compares its actual
digest to every gate's `executableSha256`. A missing executable, invalid
manifest, or mismatch fails. A uniform but incorrect digest is not enough.

**Structural success is not beta approval.** The tool always reports
`betaApproved: false`. It does not verify that a named human ran the tests,
that the images/videos are genuine, or that the binary matches the CI artifact.
A release reviewer must independently compare CI provenance and inspect
original evidence for all 14 gates, including physical hardware for W11-10,
W11-11 and W11-12. Do not publish a beta ZIP on this tool's result alone.

The regression tests use synthetic evidence and a synthetic binary, not real
Windows captures.
