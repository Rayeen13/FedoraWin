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
| `testDate` | Actual manual test date in `YYYY-MM-DD` format |
| `windows11Build` | Windows 11 edition and build as observed on the test machine |
| `executableSha256` | Exact 64-digit hex SHA-256 of the **executed** CI-built binary |
| `evidence` | Nonempty array of references to original screenshots, recordings, or logs |

**W11-10, W11-11, W11-12 require a physical Windows 11 device.**
For the other gates, an unmodified Windows 11 VM can provide exploratory/manual
coverage under the matrix's criteria. A fabricated filename or SHA-shaped string
is not proof: the reviewer must open each original, check timestamp/build and
match the hash against the actual CI executable and run provenance.

All 14 manual gate records must identify **the same executed release-candidate
binary SHA-256**. Digest hex casing is ignored; a different hash means the
manifest mixes builds and cannot establish release-candidate validation.
Re-test any affected gates against one candidate before reviewer sign-off.

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
