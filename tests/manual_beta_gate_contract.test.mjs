import test from 'node:test';
import assert from 'node:assert/strict';
import { checkManualGateCoverage, GATE_IDS } from '../tools/check-w11-manual-gates.mjs';

// Synthetic fixtures only. None is a claim of actual Windows 11 manual testing.
const complete = () => GATE_IDS.map((id) => ({
  id, status: 'PASS', environment: 'physical',
  tester: 'test-fixture', testDate: '2026-10-19',
  windows11Build: 'Windows 11 test fixture, build 26100',
  executableSha256: 'a'.repeat(64), evidence: [id + '.mp4']
}));

const check = (gates) => checkManualGateCoverage(gates);

test('synthetic 14-gate coverage is structurally complete, never beta approved', () => {
  const result = check(complete());
  assert.equal(result.complete, true);
  assert.equal(result.betaApproved, false);
});

test('missing and duplicate gates fail closed', () => {
  assert.equal(check(complete().slice(1)).complete, false);
  assert.equal(check([...complete(), complete()[0]]).complete, false);
});

test('CI checks and missing evidence do not replace manual proof', () => {
  const gates = complete();
  gates[0].status = 'NOT RUN';
  gates[1].evidence = [];
  const result = check(gates);
  assert.equal(result.complete, false);
  assert.match(result.errors.join(' '), /W11-01/);
  assert.match(result.errors.join(' '), /W11-02/);
});

test('hardware gates reject remote VMs and local VMs', () => {
  for (const id of ['W11-10', 'W11-11', 'W11-12']) {
    for (const environment of ['remote-vm', 'vm']) {
      const gates = complete();
      gates.find((gate) => gate.id === id).environment = environment;
      assert.equal(check(gates).complete, false, `${id} on ${environment}`);
    }
  }
});

test('a VM is structurally acceptable for a non-hardware gate', () => {
  const gates = complete();
  gates[0].environment = 'vm';
  assert.equal(check(gates).complete, true);
});

test('missing tester or Windows 11 build fails closed', () => {
  for (const property of ['tester', 'windows11Build']) {
    const gates = complete();
    gates[0][property] = '  ';
    assert.equal(check(gates).complete, false, property);
  }
});

test('test date must be a real ISO calendar date', () => {
  for (const testDate of ['', 'yesterday', '2026-02-30', '2026-2-3']) {
    const gates = complete();
    gates[0].testDate = testDate;
    assert.equal(check(gates).complete, false, testDate);
  }
});

test('manual test date cannot be in the future', () => {
  const gates = complete();
  gates[0].testDate = '2099-01-01';
  const result = check(gates);
  assert.equal(result.complete, false);
  assert.match(result.errors.join(' '), /W11-01/);
});

test('Windows 11 OS edition and plausible build are required', () => {
  for (const windows11Build of [
    'Windows 10 Pro build 19045', 'Windows 11 Pro build 19045',
    'Windows 11 Pro build unknown', 'Windows 11', 'Linux build 26100'
  ]) {
    const gates = complete();
    gates[0].windows11Build = windows11Build;
    assert.equal(check(gates).complete, false, windows11Build);
  }
  const gates = complete();
  gates[0].windows11Build = 'Windows 11 Pro 25H2 (OS Build 26200.8875)';
  assert.equal(check(gates).complete, true);
});

test('executable SHA-256 is mandatory and must be 64 hex characters', () => {
  for (const executableSha256 of [undefined, '', 'unknown', 'a'.repeat(63), 'g'.repeat(64)]) {
    const gates = complete();
    gates[0].executableSha256 = executableSha256;
    assert.equal(check(gates).complete, false, String(executableSha256));
  }
});

test('blank, null and non-string evidence references fail closed', () => {
  for (const evidence of [[], [' '], [null], ['capture.mp4', '']]) {
    const gates = complete();
    gates[0].evidence = evidence;
    assert.equal(check(gates).complete, false);
  }
});

test('unknown environments and gate IDs fail closed', () => {
  const gates = complete();
  gates[0].environment = 'CI';
  assert.equal(check(gates).complete, false);
  assert.equal(check([{ id: 'W11-99', status: 'PASS' }]).complete, false);
});

test('malformed manifests always report betaApproved false', () => {
  for (const gates of [null, {}, undefined, 'PASS']) {
    const result = check(gates);
    assert.equal(result.complete, false);
    assert.equal(result.betaApproved, false);
  }
});

test('manual gates may not mix different executable SHA-256 digests', () => {
  const gates = complete();
  gates[13].executableSha256 = 'b'.repeat(64);
  const result = check(gates);
  assert.equal(result.complete, false);
  assert.equal(result.betaApproved, false);
  assert.match(result.errors.join(' '), /different executable SHA-256/);
});

test('case differences in hexadecimal digest do not imply different builds', () => {
  const gates = complete();
  gates[0].executableSha256 = 'A'.repeat(64);
  const result = check(gates);
  assert.equal(result.complete, true);
  assert.equal(result.betaApproved, false);
});
