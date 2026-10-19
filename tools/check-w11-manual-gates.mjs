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
  return !Number.isNaN(parsed.valueOf()) && parsed.toISOString().slice(0, 10) === value;
}

export function checkManualGateCoverage(gates) {
  const errors = [];
  if (!Array.isArray(gates)) {
    return { complete: false, betaApproved: false, errors: ['Manual gates must be an array.'] };
  }

  const seen = new Set();
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
    if (!nonEmpty(gate.windows11Build)) errors.push(gate.id + ' has no Windows 11 build.');
    if (typeof gate.executableSha256 !== 'string' || !/^[0-9a-f]{64}$/i.test(gate.executableSha256)) {
      errors.push(gate.id + ' has no valid executable SHA-256.');
    }
    if (!Array.isArray(gate.evidence) || gate.evidence.length === 0 ||
        !gate.evidence.every(nonEmpty)) {
      errors.push(gate.id + ' has no valid original evidence references.');
    }
  }
  for (const id of GATE_IDS) if (!seen.has(id)) errors.push('Missing ' + id);

  return { complete: errors.length === 0, betaApproved: false, errors };
}
