// Structural evidence validation only; a release owner must authenticate the
// originals and approve the beta independently of this check.
export const GATE_IDS = Array.from({ length: 14 }, (_, index) =>
  'W11-' + String(index + 1).padStart(2, '0'));

const HARDWARE_GATES = new Set(['W11-10', 'W11-11', 'W11-12']);
const ENVIRONMENTS = new Set(['physical', 'vm', 'remote-vm']);
const nonEmpty = (value) => typeof value === 'string' && value.trim().length > 0;

function validDate(value) {
  if (typeof value !== 'string' || !/^\d{4}-\d{2}-\d{2}$/.test(value)) return false;
  const parsed = new Date(value + 'T00:00:00Z');
  // A test date cannot be in the future. Allow the next UTC calendar day
  // because a legitimate Windows 11 tester may be in UTC+14.
  return !Number.isNaN(parsed.valueOf()) &&
    parsed.toISOString().slice(0, 10) === value &&
    parsed.getTime() <= Date.now() + 24 * 60 * 60 * 1000;
}

function validWindows11Build(value) {
  if (!nonEmpty(value)) return false;
  // Require an observed Windows 11 OS build, not an arbitrary nonempty label.
  // Windows 11 started at build 22000; allow revision suffixes such as 26100.1.
  const match = /\bWindows\s+11\b[\s\S]*?\b(?:OS\s+)?build\s+(\d{5,6})(?:\.\d+)?\b/i.exec(value);
  return match !== null && Number(match[1]) >= 22000;
}

export function checkManualGateCoverage(gates) {
  const errors = [];
  if (!Array.isArray(gates)) {
    return { complete: false, betaApproved: false, errors: ['Manual gates must be an array.'] };
  }

  const seen = new Set();
  const executableHashes = new Set();
  for (const gate of gates) {
    if (!gate || typeof gate !== 'object' || !GATE_IDS.includes(gate.id)) {
      errors.push('Unknown manual gate.');
      continue;
    }
    if (seen.has(gate.id)) errors.push('Duplicate gate ' + gate.id);
    seen.add(gate.id);
    if (gate.status !== 'PASS') errors.push(gate.id + ' has not passed.');
    if (!ENVIRONMENTS.has(gate.environment)) {
      errors.push(gate.id + ' has no valid environment.');
    }
    if (HARDWARE_GATES.has(gate.id) && gate.environment !== 'physical') {
      errors.push(gate.id + ' needs physical Windows 11 testing.');
    }
    if (!nonEmpty(gate.tester)) errors.push(gate.id + ' has no named tester.');
    if (!validDate(gate.testDate)) errors.push(gate.id + ' has no valid YYYY-MM-DD test date.');
    if (!validWindows11Build(gate.windows11Build)) {
      errors.push(gate.id + ' has no valid Windows 11 OS build (22000 or later).');
    }
    if (typeof gate.executableSha256 !== 'string' || !/^[0-9a-f]{64}$/i.test(gate.executableSha256)) {
      errors.push(gate.id + ' has no valid executable SHA-256.');
    } else {
      // One exact release candidate must be exercised by every manual gate.
      // SHA-256 hex casing is immaterial; mixing binaries fails closed.
      executableHashes.add(gate.executableSha256.toLowerCase());
    }
    if (!Array.isArray(gate.evidence) || gate.evidence.length === 0 ||
        !gate.evidence.every(nonEmpty)) {
      errors.push(gate.id + ' has no valid original evidence references.');
    }
  }
  if (executableHashes.size > 1) {
    errors.push('Manual gates refer to different executable SHA-256 hashes.');
  }
  for (const id of GATE_IDS) if (!seen.has(id)) errors.push('Missing ' + id);

  return { complete: errors.length === 0, betaApproved: false, errors };
}
