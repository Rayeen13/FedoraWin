import test from 'node:test';
import assert from 'node:assert/strict';
import { compareManualSnapshots } from '../tools/compare-w11-manual-snapshots.mjs';

// Synthetic fixtures only: not actual Windows 11 testing or beta approval.
const oldTicks = '638971111111111111';
const newTicks = '638971111111111112';
const sha = 'a'.repeat(64);
const before = () => ({
  schemaVersion: 1, evidenceType: 'read-only-environment-snapshot',
  stage: 'before', capturedAtUtc: '2026-10-26T08:00:00.0000000Z',
  windows11Build: 'Microsoft Windows 11 Pro (OS Build 26200)',
  executableSha256: sha, executablePath: 'C:\\FedoraWin\\fedorawin.exe',
  explorerProcesses: [{ pid: 111, startTimeUtcTicks: oldTicks }],
  taskbars: [
    { hwnd: '123456789012345678', className: 'Shell_TrayWnd', visible: true, ownerPid: 111, ownerStartTimeUtcTicks: oldTicks },
    { hwnd: '987654321098765432', className: 'Shell_SecondaryTrayWnd', visible: false, ownerPid: 111, ownerStartTimeUtcTicks: oldTicks }
  ],
  manualGateStatus: 'NOT RUN', betaApproved: false
});
const after = (stage = 'after-force-kill') => ({
  ...structuredClone(before()), stage, capturedAtUtc: '2026-10-26T08:05:00.0000000Z'
});
const check = (a, b) => compareManualSnapshots(a, b);

test('clean exit and force-kill snapshots may be consistent but never pass a manual gate', () => {
  for (const stage of ['after-clean-exit', 'after-force-kill']) {
    const result = check(before(), after(stage));
    assert.equal(result.structurallyConsistent, true);
    assert.equal(result.manualGateStatus, 'NOT RUN');
    assert.equal(result.betaApproved, false);
  }
});

test('Explorer restart requires a new primary owner lifetime', () => {
  const same = check(before(), after('after-explorer-restart'));
  assert.equal(same.structurallyConsistent, false);
  assert.match(same.errors.join(' '), /did not change/);
  const changed = after('after-explorer-restart');
  changed.explorerProcesses = [{ pid: 222, startTimeUtcTicks: newTicks }];
  for (const bar of changed.taskbars) {
    bar.ownerPid = 222;
    bar.ownerStartTimeUtcTicks = newTicks;
  }
  assert.equal(check(before(), changed).structurallyConsistent, true);
  assert.equal(check(before(), changed).betaApproved, false);
});

test('forced termination cannot silently restart Explorer', () => {
  const changed = after();
  changed.explorerProcesses = [{ pid: 222, startTimeUtcTicks: newTicks }];
  for (const bar of changed.taskbars) {
    bar.ownerPid = 222;
    bar.ownerStartTimeUtcTicks = newTicks;
  }
  assert.match(check(before(), changed).errors.join(' '), /owner changed/);
});

test('baseline taskbar visibility and monitor count must be restored', () => {
  const hidden = after();
  hidden.taskbars[0].visible = false;
  assert.match(check(before(), hidden).errors.join(' '), /Primary taskbar visibility/);
  const lostMonitor = after();
  lostMonitor.taskbars.pop();
  assert.match(check(before(), lostMonitor).errors.join(' '), /Secondary taskbar count/);
});

test('rejects swapped binaries, OS builds, paths, stages, and nonmonotonic timestamps', () => {
  for (const [field, value] of [
    ['executableSha256', 'b'.repeat(64)], ['windows11Build', 'Windows 11 Pro (OS Build 26100)'],
    ['executablePath', 'C:\\other.exe'], ['stage', 'before'],
    ['capturedAtUtc', '2026-10-26T07:59:59.0000000Z']
  ]) {
    const modified = after();
    modified[field] = value;
    assert.equal(check(before(), modified).structurallyConsistent, false, field);
  }
});

test('rejects numeric .NET ticks/HWNDs that lose precision in JavaScript', () => {
  const invalid = after();
  invalid.explorerProcesses[0].startTimeUtcTicks = Number(oldTicks);
  assert.match(check(before(), invalid).errors.join(' '), /lossless start-time ticks/);
  const invalidHwnd = after();
  invalidHwnd.taskbars[0].hwnd = Number('123456789012345678');
  assert.match(check(before(), invalidHwnd).errors.join(' '), /invalid taskbar HWND/);
});

test('rejects mismatched taskbar ownership, missing primary, duplicates and fake PASS', () => {
  const invalidOwner = after();
  invalidOwner.taskbars[0].ownerStartTimeUtcTicks = newTicks;
  assert.equal(check(before(), invalidOwner).structurallyConsistent, false);
  const noPrimary = after();
  noPrimary.taskbars = noPrimary.taskbars.slice(1);
  assert.equal(check(before(), noPrimary).structurallyConsistent, false);
  const duplicate = after();
  duplicate.taskbars.push(structuredClone(duplicate.taskbars[0]));
  assert.equal(check(before(), duplicate).structurallyConsistent, false);
  const fakePass = after();
  fakePass.manualGateStatus = 'PASS';
  fakePass.betaApproved = true;
  assert.equal(check(before(), fakePass).structurallyConsistent, false);
  assert.equal(check(before(), fakePass).betaApproved, false);
});

test('rejects malformed inputs without claiming beta approval', () => {
  for (const invalid of [null, [], {}, { ...before(), taskbars: null }]) {
    const result = check(invalid, after());
    assert.equal(result.structurallyConsistent, false);
    assert.equal(result.betaApproved, false);
  }
});
