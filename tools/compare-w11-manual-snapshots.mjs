// Compare two read-only Windows 11 environment snapshots. This checks for
// obvious rollback inconsistencies; it cannot attest execution or approve a gate.
import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const AFTER_STAGES = new Set([
  'after-clean-exit', 'after-force-kill', 'after-explorer-restart'
]);
const DECIMAL_TICKS = /^\d{15,20}$/;
const DECIMAL_HWND = /^-?\d{1,20}$/;
const SHA256 = /^[a-f\d]{64}$/i;
const ISO_UTC = /^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,7})?Z$/;
// DateTime.Ticks uses year 0001; captureUtcTicks uses the Unix epoch.
const DOTNET_UNIX_EPOCH_TICKS = 621355968000000000n;

/**
 * Compare .NET's seven-digit UTC fractions without losing 100 ns precision.
 * Date.parse rounds to milliseconds and normalizes impossible calendar dates.
 * A strict round trip rejects such dates; BigInt preserves the remaining ticks.
 */
function captureUtcTicks(value) {
  if (typeof value !== 'string' || !ISO_UTC.test(value)) return null;
  const seconds = value.slice(0, 19);
  const milliseconds = Date.parse(`${seconds}Z`);
  if (!Number.isFinite(milliseconds) ||
      new Date(milliseconds).toISOString().slice(0, 19) !== seconds) return null;
  const fraction = /\.(\d{1,7})Z$/.exec(value)?.[1] ?? '';
  return BigInt(milliseconds) * 10000n + BigInt(fraction.padEnd(7, '0') || '0');
}


function inspect(snapshot, label, errors) {
  if (!snapshot || typeof snapshot !== 'object' || Array.isArray(snapshot)) {
    errors.push(`${label}: expected a snapshot object.`);
    return null;
  }
  if (snapshot.schemaVersion !== 1 ||
      snapshot.evidenceType !== 'read-only-environment-snapshot' ||
      snapshot.manualGateStatus !== 'NOT RUN' || snapshot.betaApproved !== false) {
    errors.push(`${label}: invalid snapshot contract; never use a manual PASS here.`);
  }
  if (captureUtcTicks(snapshot.capturedAtUtc) === null) {
    errors.push(`${label}: invalid UTC capture timestamp.`);
  }
  const build = /\bWindows\s+11\b[\s\S]*?\b(?:OS\s+)?build\s+(\d{5,6})(?:\.\d+)?\b/i.exec(snapshot.windows11Build ?? '');
  if (!build || Number(build[1]) < 22000) {
    errors.push(`${label}: observed Windows 11 build 22000+ is required.`);
  }
  if (typeof snapshot.executableSha256 !== 'string' || !SHA256.test(snapshot.executableSha256)) {
    errors.push(`${label}: invalid executable SHA-256.`);
  }
  if (typeof snapshot.executablePath !== 'string' || !snapshot.executablePath.trim()) {
    errors.push(`${label}: missing executable path.`);
  }

  const capturedTicks = captureUtcTicks(snapshot.capturedAtUtc);
  const lifetimes = new Map();
  if (!Array.isArray(snapshot.explorerProcesses) || !snapshot.explorerProcesses.length) {
    errors.push(`${label}: missing Explorer processes.`);
  } else {
    for (const row of snapshot.explorerProcesses) {
      const validTicks = typeof row?.startTimeUtcTicks === 'string' && DECIMAL_TICKS.test(row.startTimeUtcTicks);
      const startTicks = validTicks ? BigInt(row.startTimeUtcTicks) - DOTNET_UNIX_EPOCH_TICKS : null;
      if (!row || !Number.isSafeInteger(row.pid) || row.pid <= 0 ||
          !validTicks || startTicks < 0n || (capturedTicks !== null && startTicks > capturedTicks) ||
          lifetimes.has(row.pid)) {
        errors.push(`${label}: invalid/duplicate Explorer PID, lossless start-time ticks, or future lifetime.`);
      } else {
        lifetimes.set(row.pid, row.startTimeUtcTicks);
      }
    }
  }

  const primary = [];
  const secondary = [];
  const seenHwnds = new Set();
  if (!Array.isArray(snapshot.taskbars) || !snapshot.taskbars.length) {
    errors.push(`${label}: missing taskbar HWNDs.`);
  } else {
    for (const bar of snapshot.taskbars) {
      if (!bar || typeof bar.hwnd !== 'string' || !DECIMAL_HWND.test(bar.hwnd) ||
          BigInt(bar.hwnd) === 0n || seenHwnds.has(bar.hwnd) ||
          !['Shell_TrayWnd', 'Shell_SecondaryTrayWnd'].includes(bar.className) ||
          typeof bar.visible !== 'boolean' || !Number.isSafeInteger(bar.ownerPid) ||
          bar.ownerPid <= 0 || typeof bar.ownerStartTimeUtcTicks !== 'string' ||
          !DECIMAL_TICKS.test(bar.ownerStartTimeUtcTicks) ||
          lifetimes.get(bar.ownerPid) !== bar.ownerStartTimeUtcTicks) {
        errors.push(`${label}: invalid taskbar HWND, ownership, or lossless lifetime.`);
        continue;
      }
      seenHwnds.add(bar.hwnd);
      (bar.className === 'Shell_TrayWnd' ? primary : secondary).push(bar);
    }
  }
  if (primary.length !== 1) {
    errors.push(`${label}: expected exactly one Explorer-owned primary taskbar.`);
  }
  return { snapshot, primary: primary[0], secondary };
}

export function compareManualSnapshots(before, after) {
  const errors = [];
  const baseline = inspect(before, 'before', errors);
  const observed = inspect(after, 'after', errors);
  if (!baseline || !observed) return outcome(errors);
  if (before.stage !== 'before') errors.push('Baseline stage must be before.');
  if (!AFTER_STAGES.has(after.stage)) errors.push('Unknown post-test stage.');
  const start = captureUtcTicks(before.capturedAtUtc);
  const end = captureUtcTicks(after.capturedAtUtc);
  if (start !== null && end !== null && end <= start) {
    errors.push('Post-test snapshot must be captured after baseline.');
  }
  if (before.windows11Build !== after.windows11Build) {
    errors.push('Windows build changed between snapshots.');
  }
  if (typeof before.executableSha256 === 'string' && typeof after.executableSha256 === 'string' &&
      before.executableSha256.toLowerCase() !== after.executableSha256.toLowerCase()) {
    errors.push('Executable SHA-256 differs between snapshots.');
  }
  if (typeof before.executablePath === 'string' && typeof after.executablePath === 'string' &&
      before.executablePath.toLowerCase() !== after.executablePath.toLowerCase()) {
    errors.push('Executable path differs between snapshots.');
  }

  if (baseline.primary && observed.primary) {
    const oldOwner = `${baseline.primary.ownerPid}:${baseline.primary.ownerStartTimeUtcTicks}`;
    const newOwner = `${observed.primary.ownerPid}:${observed.primary.ownerStartTimeUtcTicks}`;
    if (after.stage === 'after-explorer-restart') {
      if (oldOwner === newOwner) {
        errors.push('Explorer restart did not change primary taskbar owner lifetime.');
      } else if (start !== null && end !== null) {
        // A different already-running Explorer is not proof of a restart after baseline.
        const ownerStarted = BigInt(observed.primary.ownerStartTimeUtcTicks) - DOTNET_UNIX_EPOCH_TICKS;
        if (ownerStarted <= start || ownerStarted > end) {
          errors.push('Explorer restart owner lifetime must begin between snapshots.');
        }
      }
    } else if (AFTER_STAGES.has(after.stage) && oldOwner !== newOwner) {
      errors.push('Explorer primary taskbar owner changed during exit/force-kill recovery.');
    }
    if (baseline.primary.visible !== observed.primary.visible) {
      errors.push('Primary taskbar visibility differs from baseline.');
    }
  }
  const visibility = (bars) => bars.map((bar) => String(bar.visible)).sort().join(',');
  if (baseline.secondary.length !== observed.secondary.length ||
      visibility(baseline.secondary) !== visibility(observed.secondary)) {
    errors.push('Secondary taskbar count or visibility differs from baseline.');
  }
  return outcome(errors);
}

function outcome(errors) {
  return {
    structurallyConsistent: errors.length === 0,
    manualGateStatus: 'NOT RUN',
    betaApproved: false,
    errors
  };
}

// Explicit CLI use only; importing this module in Node tests never reads files.
if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  const paths = process.argv.slice(2);
  if (paths.length !== 2) {
    console.error('Usage: node tools/compare-w11-manual-snapshots.mjs BEFORE.json AFTER.json');
    process.exitCode = 2;
  } else {
    try {
      const [before, after] = paths.map((path) => JSON.parse(readFileSync(path, 'utf8')));
      const result = compareManualSnapshots(before, after);
      console.log(result.structurallyConsistent
        ? 'STRUCTURALLY CONSISTENT (manual gates remain NOT RUN)'
        : 'INCONSISTENT (review required; manual gates remain NOT RUN)');
      for (const error of result.errors) console.error(`- ${error}`);
      console.log('Does not verify DWM rollback, actual execution, original video, or human sign-off.');
      if (!result.structurallyConsistent) process.exitCode = 1;
    } catch (error) {
      console.error(`Could not compare snapshots: ${error.message}`);
      process.exitCode = 2;
    }
  }
}
