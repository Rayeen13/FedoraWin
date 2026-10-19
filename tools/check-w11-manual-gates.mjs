// Structural validation only: never interpret this as beta approval.
export const GATE_IDS = Array.from({ length: 14 }, (_, index) =>
  'W11-' + String(index + 1).padStart(2, '0'));

export function checkManualGateCoverage(gates) {
  const errors = [];
  if (!Array.isArray(gates)) {
    return { complete: false, errors: ['Manual gates must be an array.'] };
  }
  const seen = new Set();
  for (const gate of gates) {
    if (!gate || !GATE_IDS.includes(gate.id)) {
      errors.push('Unknown manual gate.');
      continue;
    }
    if (seen.has(gate.id)) errors.push('Duplicate gate ' + gate.id);
    seen.add(gate.id);
    if (gate.status !== 'PASS') errors.push(gate.id + ' has not passed.');
    if (!gate.evidence || !Array.isArray(gate.evidence) || gate.evidence.length === 0) {
      errors.push(gate.id + ' has no original evidence reference.');
    }
    if (['W11-10', 'W11-11', 'W11-12'].includes(gate.id) && gate.environment !== 'physical') {
      errors.push(gate.id + ' needs physical Windows 11 testing.');
    }
  }
  for (const id of GATE_IDS) if (!seen.has(id)) errors.push('Missing ' + id);
  return { complete: errors.length === 0, betaApproved: false, errors };
}
