#!/usr/bin/env python3
"""Fail-closed structural validation of FedoraWin Activities benchmark evidence.

This checks evidence completeness and arithmetic only. It cannot authenticate
captures, traces, hardware, or interactive Windows 11 manual gates.
"""
import json
import math
import re
import sys
import unittest
from pathlib import Path

SHA256 = re.compile(r"^[0-9a-fA-F]{64}$")
COMMIT = re.compile(r"^[0-9a-fA-F]{40}$")
REQUIRED_TEXT = (
    "utc_start", "windows_build", "webview2_version", "hardware_and_gpu",
    "monitor_refresh_scaling_topology", "power_mode",
    "raw_trace_path", "runtime_capture_path",
    "explorer_taskbar_before", "explorer_taskbar_after",
)
MILESTONES = ("input_to_hwnd_visible_ms", "input_to_first_visible_frame_ms",
              "thumbnail_bind_ms")
MEMORY_LIMIT_BYTES = 300 * 1024 * 1024


def validate(record):
    errors = []
    if not isinstance(record, dict):
        return ["evidence must be a JSON object"]
    for key, pattern in (("candidate_commit", COMMIT),
                         ("exe_sha256", SHA256),
                         ("raw_trace_sha256", SHA256),
                         ("runtime_capture_sha256", SHA256)):
        if not isinstance(record.get(key), str) or not pattern.fullmatch(record[key]):
            errors.append(f"{key}: missing or invalid hash")
    for key in REQUIRED_TEXT:
        if not isinstance(record.get(key), str) or not record[key].strip():
            errors.append(f"{key}: required nonempty string")
    if record.get("session_type") not in ("physical", "interactive-remote"):
        errors.append("session_type: must identify a physical or interactive-remote session")
    if record.get("variant") not in ("on-demand", "single-prewarmed"):
        errors.append("variant: unsupported value")
    if record.get("invocation_type") not in ("cold", "warm"):
        errors.append("invocation_type: must be cold or warm")
    total, failed = record.get("invocations_total"), record.get("invocations_failed")
    if type(total) is not int or total <= 0:
        errors.append("invocations_total: must be a positive integer")
    if type(failed) is not int or failed < 0 or (type(total) is int and failed > total):
        errors.append("invocations_failed: must be between zero and total")
    if type(total) is int and total > 0:
        minimum = 20 if record.get("invocation_type") == "cold" else 100
        if total < minimum:
            errors.append(f"invocations_total: requires at least {minimum} samples")
    for key in MILESTONES:
        values = record.get(key)
        if not isinstance(values, dict):
            errors.append(f"{key}: requires p50, p95, max")
            continue
        numbers = [values.get(k) for k in ("p50", "p95", "max")]
        if any(type(n) not in (int, float) or not math.isfinite(n) or n < 0 for n in numbers):
            errors.append(f"{key}: finite nonnegative p50/p95/max required")
        elif not numbers[0] <= numbers[1] <= numbers[2]:
            errors.append(f"{key}: percentile order invalid")
    for key in ("present_frame_interval_ms_p95",):
        v = record.get(key)
        if type(v) not in (int, float) or not math.isfinite(v) or v < 0:
            errors.append(f"{key}: finite nonnegative number required")
    missed = record.get("missed_presents")
    if type(missed) is not int or missed < 0:
        errors.append("missed_presents: nonnegative integer required")
    for key in ("process_tree_private_bytes", "process_tree_working_set_bytes"):
        values = record.get(key)
        if not isinstance(values, dict):
            errors.append(f"{key}: idle/peak/after_close required")
            continue
        nums = [values.get(k) for k in ("idle", "peak", "after_close")]
        if any(type(n) is not int or n < 0 for n in nums):
            errors.append(f"{key}: nonnegative integer byte counts required")
        elif nums[1] < max(nums[0], nums[2]):
            errors.append(f"{key}: peak must include idle and after_close")
        if key == "process_tree_private_bytes" and all(type(n) is int for n in nums):
            if nums[1] >= MEMORY_LIMIT_BYTES:
                errors.append("process_tree_private_bytes: peak breaches conservative 300 MiB limit")
    gates = record.get("manual_gate_ids_verified")
    if not isinstance(gates, list) or any(
        not isinstance(g, str) or not re.fullmatch(r"W11-(0[1-9]|1[0-4])", g)
        for g in gates
    ) or len(gates) != len(set(gates)):
        errors.append("manual_gate_ids_verified: invalid or duplicate gate IDs")
    return errors


class EvidenceTests(unittest.TestCase):
    def sample(self):
        result = {k: "observed" for k in REQUIRED_TEXT}
        result.update(
            candidate_commit="a" * 40, exe_sha256="b" * 64,
            raw_trace_sha256="c" * 64, runtime_capture_sha256="d" * 64,
            session_type="physical", variant="on-demand", invocation_type="cold",
            invocations_total=20, invocations_failed=0, missed_presents=0,
            present_frame_interval_ms_p95=8.3,
            process_tree_private_bytes=dict(idle=10, peak=20, after_close=10),
            process_tree_working_set_bytes=dict(idle=10, peak=20, after_close=10),
            manual_gate_ids_verified=[],
        )
        for name in MILESTONES:
            result[name] = dict(p50=1, p95=2, max=3)
        return result

    def test_valid_structure(self):
        self.assertEqual(validate(self.sample()), [])

    def test_missing_evidence_fails_closed(self):
        self.assertTrue(validate({}))
        sample = self.sample()
        sample["runtime_capture_sha256"] = ""
        self.assertTrue(validate(sample))

    def test_memory_and_percentiles(self):
        sample = self.sample()
        sample["process_tree_private_bytes"]["peak"] = MEMORY_LIMIT_BYTES
        sample["input_to_first_visible_frame_ms"]["p95"] = float("nan")
        errors = validate(sample)
        self.assertTrue(any("breaches" in e for e in errors))
        self.assertTrue(any("finite" in e for e in errors))

    def test_invalid_counts_and_gate_ids(self):
        sample = self.sample()
        sample["invocations_failed"] = 21
        sample["manual_gate_ids_verified"] = ["W11-01", "W11-01"]
        self.assertGreaterEqual(len(validate(sample)), 2)


if __name__ == "__main__":
    if len(sys.argv) == 2 and sys.argv[1] == "--self-test":
        unittest.main(argv=[sys.argv[0]])
    elif len(sys.argv) == 2:
        try:
            errors = validate(json.loads(Path(sys.argv[1]).read_text(encoding="utf-8")))
        except (OSError, ValueError) as exc:
            errors = [f"cannot read evidence: {exc}"]
        if errors:
            for error in errors:
                print(f"FAIL: {error}", file=sys.stderr)
            sys.exit(1)
        print("Evidence structure valid; runtime authenticity and manual gates NOT verified")
    else:
        print("usage: python scripts/validate_activities_evidence.py FILE.json | --self-test", file=sys.stderr)
        sys.exit(2)
