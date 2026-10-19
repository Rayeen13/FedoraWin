import test from 'node:test';
import assert from 'node:assert/strict';
import { checkManualGateCoverage, GATE_IDS } from '../tools/check-w11-manual-gates.mjs';

const complete = () => GATE_IDS.map((id) => ({
  id, status: 'PASS', environment: 'physical', evidence: [id + '.mp4']
}));

test('all 14 gates with evidence and physical coverage are structurally complete, not approved', () => {
  const result = checkManualGateCoverage(complete());
  assert.equal(result.complete, true);
  assert.equal(result.betaApproved, false);
});

test('missing or duplicate manual gates cannot pass', () => {
  assert.equal(checkManualGateCoverage(complete().slice(1)).complete, false);
  assert.equal(checkManualGateCoverage([...complete(), complete()[0]]).complete, false);
});

test('CI results and missing evidence do not replace manual proof', () => {
  const gates = complete();
  gates[0].status = 'NOT RUN';
  gates[1].evidence = [];
  const result = checkManualGateCoverage(gates);
  assert.equal(result.complete, false);
  assert.match(result.errors.join(' '), /W11-01/);
  assert.match(result.errors.join(' '), /W11-02/);
});

test('hardware gates cannot be marked complete using VM evidence', () => {
  for (const id of ['W11-10', 'W11-11', 'W11-12']) {
    const gates = complete();
    gates.find((gate) => gate.id === id).environment = 'remote-vm';
    assert.equal(checkManualGateCoverage(gates).complete, false);
  }
});

test('malformed evidence manifests fail closed', () => {
  assert.equal(checkManualGateCoverage(null).complete, false);
  assert.equal(checkManualGateCoverage([{ id: 'W11-99', status: 'PASS' }]).complete, false);
});
