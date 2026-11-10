import test from 'node:test';
import assert from 'node:assert/strict';
import { compareManualSnapshots } from '../tools/compare-w11-manual-snapshots.mjs';

// Synthetic fixtures only: not actual Windows 11 testing or beta approval.
const oldTicks = '638971111111111111';
// .NET DateTime ticks for an Explorer restart two minutes after baseline.
const dotnetEpoch = 621355968000000000n;
const newTicks = String(dotnetEpoch + BigInt(Date.parse('2026-10-26T08:02:00Z')) * 10000n);
const runtimeTicks = String(dotnetEpoch + BigInt(Date.parse('2026-10-26T08:01:00Z')) * 10000n);
const sha = 'a'.repeat(64);
const bootTicks = String(dotnetEpoch + BigInt(Date.parse('2025-10-01T08:00:00Z')) * 10000n);
const before = () => ({
  schemaVersion: 3, evidenceType: 'read-only-environment-snapshot',
  stage: 'before', capturedAtUtc: '2026-10-26T08:00:00.0000000Z',
  windows11Build: 'Microsoft Windows 11 Pro (OS Build 26200)',
  sessionId: 2, bootTimeUtcTicks: bootTicks,
  executableSha256: sha, executablePath: 'C:\\FedoraWin\\fedorawin.exe',
  explorerProcesses: [{ pid: 111, startTimeUtcTicks: oldTicks }],
  candidateProcesses: [],
  taskbars: [
    { hwnd: '123456789012345678', className: 'Shell_TrayWnd', visible: true, ownerPid: 111, ownerStartTimeUtcTicks: oldTicks },
    { hwnd: '987654321098765432', className: 'Shell_SecondaryTrayWnd', visible: false, ownerPid: 111, ownerStartTimeUtcTicks: oldTicks }
  ],
  manualGateStatus: 'NOT RUN', betaApproved: false
});
const after = (stage = 'after-force-kill') => ({
  ...structuredClone(before()), stage, capturedAtUtc: '2026-10-26T08:05:00.0000000Z',
  candidateProcesses: stage === 'after-explorer-restart'
    ? [{ pid: 999, startTimeUtcTicks: runtimeTicks, role: 'runtime' }] : []
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


test('Explorer restart must actually start after the baseline capture', () => {
  const stale = after('after-explorer-restart');
  stale.explorerProcesses = [{ pid: 222, startTimeUtcTicks: oldTicks }];
  for (const bar of stale.taskbars) {
    bar.ownerPid = 222;
    bar.ownerStartTimeUtcTicks = oldTicks;
  }
  assert.match(check(before(), stale).errors.join(' '), /between snapshots/);
});

test('rejects Explorer process start times later than their own capture', () => {
  const future = after();
  const futureTicks = String(dotnetEpoch + BigInt(Date.parse('2026-10-26T08:06:00Z')) * 10000n);
  future.explorerProcesses[0].startTimeUtcTicks = futureTicks;
  for (const bar of future.taskbars) bar.ownerStartTimeUtcTicks = futureTicks;
  assert.match(check(before(), future).errors.join(' '), /future lifetime/);
});

test('Explorer restart rejects secondary taskbars left with the previous Explorer lifetime', () => {
  const changed = after('after-explorer-restart');
  // The new primary taskbar is valid, but an old Explorer process still owns
  // the secondary bar. This must not masquerade as a complete restart.
  changed.explorerProcesses.push({ pid: 222, startTimeUtcTicks: newTicks });
  changed.taskbars[0].ownerPid = 222;
  changed.taskbars[0].ownerStartTimeUtcTicks = newTicks;
  const result = check(before(), changed);
  assert.equal(result.structurallyConsistent, false);
  assert.match(result.errors.join(' '), /secondary taskbar belongs to a different Explorer lifetime/);
  assert.equal(result.betaApproved, false);
});

test('rejects mixed Explorer taskbar ownership even without an explicit restart', () => {
  const mixed = after('after-clean-exit');
  mixed.explorerProcesses.push({ pid: 222, startTimeUtcTicks: newTicks });
  mixed.taskbars[1].ownerPid = 222;
  mixed.taskbars[1].ownerStartTimeUtcTicks = newTicks;
  assert.match(check(before(), mixed).errors.join(' '), /secondary taskbar belongs to a different Explorer lifetime/);
});

test('rejects decimal aliases of the same HWND', () => {
  const duplicate = after();
  duplicate.taskbars.push({
    ...duplicate.taskbars[0], hwnd: '0' + duplicate.taskbars[0].hwnd
  });
  assert.match(check(before(), duplicate).errors.join(' '), /invalid taskbar HWND/);
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

test('compares 100 ns capture ordering without millisecond truncation', () => {
  const baseline = before();
  baseline.capturedAtUtc = '2026-10-26T08:00:00.1234567Z';
  const later = after();
  later.capturedAtUtc = '2026-10-26T08:00:00.1234568Z';
  assert.equal(check(baseline, later).structurallyConsistent, true);
  later.capturedAtUtc = baseline.capturedAtUtc;
  assert.match(check(baseline, later).errors.join(' '), /captured after baseline/);
  later.capturedAtUtc = '2026-10-26T08:00:00.1234566Z';
  assert.match(check(baseline, later).errors.join(' '), /captured after baseline/);
});

test('rejects normalized impossible UTC dates instead of accepting Date.parse rollover', () => {
  for (const timestamp of [
    '2026-02-30T08:05:00.0000000Z',
    '2026-02-29T08:05:00.0000000Z',
    '2026-04-31T08:05:00.0000000Z'
  ]) {
    const invalid = after();
    invalid.capturedAtUtc = timestamp;
    assert.match(check(before(), invalid).errors.join(' '), /invalid UTC capture timestamp/);
  }
});

test('post-exit and force-kill snapshots reject lingering runtime or guardian processes', () => {
  for (const stage of ['after-clean-exit', 'after-force-kill']) {
    for (const role of ['runtime', 'guardian']) {
      const observed = after(stage);
      observed.candidateProcesses = [{ pid: 999, startTimeUtcTicks: newTicks, role }];
      const result = check(before(), observed);
      assert.equal(result.structurallyConsistent, false);
      assert.match(result.errors.join(' '), /still running/);
      assert.equal(result.betaApproved, false);
    }
  }
});

test('before snapshot must be captured before FedoraWin starts', () => {
  const started = before();
  started.candidateProcesses = [{ pid: 999, startTimeUtcTicks: oldTicks, role: 'runtime' }];
  assert.match(check(started, after()).errors.join(' '), /already running/);
});

test('Explorer-restart snapshot requires the live runtime, not just a guardian', () => {
  for (const processes of [
    [],
    [{ pid: 999, startTimeUtcTicks: newTicks, role: 'guardian' }]
  ]) {
    const observed = after('after-explorer-restart');
    observed.candidateProcesses = processes;
    const result = check(before(), observed);
    assert.equal(result.structurallyConsistent, false);
    assert.match(result.errors.join(' '), /no running FedoraWin runtime/);
  }
});

test('candidate process records must have lossless, plausible and unique lifetimes', () => {
  for (const candidateProcesses of [
    null,
    [{ pid: 999, startTimeUtcTicks: Number(newTicks), role: 'runtime' }],
    [{ pid: 999, startTimeUtcTicks: '0', role: 'runtime' }],
    [{ pid: 999, startTimeUtcTicks: newTicks, role: 'unknown' }],
    [{ pid: 999, startTimeUtcTicks: newTicks, role: 'runtime' },
      { pid: 999, startTimeUtcTicks: newTicks, role: 'guardian' }]
  ]) {
    const observed = after('after-explorer-restart');
    observed.candidateProcesses = candidateProcesses;
    const result = check(before(), observed);
    assert.equal(result.structurallyConsistent, false);
    assert.equal(result.betaApproved, false);
  }
});

test('older snapshot schemas cannot silently bypass session or process observations', () => {
  for (const version of [1, 2]) {
    const old = before();
    old.schemaVersion = version;
    delete old.sessionId;
    delete old.bootTimeUtcTicks;
    if (version === 1) delete old.candidateProcesses;
    const result = check(old, after());
    assert.equal(result.structurallyConsistent, false);
    assert.equal(result.betaApproved, false);
  }
});

test('rejects post-test evidence from a different interactive logon session', () => {
  const observed = after();
  observed.sessionId = 3;
  const result = check(before(), observed);
  assert.equal(result.structurallyConsistent, false);
  assert.match(result.errors.join(' '), /session changed/);
  assert.equal(result.betaApproved, false);
});

test('rejects rebooted Windows even if Explorer taskbar appears unchanged', () => {
  const observed = after();
  observed.bootTimeUtcTicks = String(BigInt(bootTicks) + 10000000n);
  const result = check(before(), observed);
  assert.equal(result.structurallyConsistent, false);
  assert.match(result.errors.join(' '), /boot lifetime changed/);
  assert.equal(result.betaApproved, false);
});

test('rejects missing, session-zero, numeric and future boot identity fields', () => {
  for (const sessionId of [undefined, null, 0, '2', 1.5]) {
    const observed = after();
    observed.sessionId = sessionId;
    assert.match(check(before(), observed).errors.join(' '), /invalid interactive Windows session ID/);
  }
  for (const bootTimeUtcTicks of [undefined, Number(bootTicks), '0',
    String(dotnetEpoch + BigInt(Date.parse('2026-10-27T08:00:00Z')) * 10000n)]) {
    const observed = after();
    observed.bootTimeUtcTicks = bootTimeUtcTicks;
    assert.match(check(before(), observed).errors.join(' '), /invalid or future Windows boot lifetime/);
  }
});


test('Explorer-restart rejects a runtime that predates the clean baseline', () => {
  const observed = after('after-explorer-restart');
  observed.candidateProcesses[0].startTimeUtcTicks = oldTicks;
  observed.explorerProcesses = [{ pid: 222, startTimeUtcTicks: newTicks }];
  for (const bar of observed.taskbars) {
    bar.ownerPid = 222;
    bar.ownerStartTimeUtcTicks = newTicks;
  }
  const result = check(before(), observed);
  assert.equal(result.structurallyConsistent, false);
  assert.match(result.errors.join(' '), /runtime predates the baseline/);
  assert.equal(result.betaApproved, false);
});


test('Explorer restart rejects multiple simultaneous runtimes with valid lifetimes', () => {
  const observed = after('after-explorer-restart');
  observed.explorerProcesses = [{ pid: 222, startTimeUtcTicks: newTicks }];
  for (const bar of observed.taskbars) {
    bar.ownerPid = 222;
    bar.ownerStartTimeUtcTicks = newTicks;
  }
  observed.candidateProcesses.push({ pid: 998, startTimeUtcTicks: newTicks, role: 'runtime' });
  const result = check(before(), observed);
  assert.equal(result.structurallyConsistent, false);
  assert.match(result.errors.join(' '), /exactly one FedoraWin runtime/);
  assert.equal(result.betaApproved, false);
});

test('Explorer restart rejects a guardian that predates the clean baseline', () => {
  const observed = after('after-explorer-restart');
  observed.explorerProcesses = [{ pid: 222, startTimeUtcTicks: newTicks }];
  for (const bar of observed.taskbars) {
    bar.ownerPid = 222;
    bar.ownerStartTimeUtcTicks = newTicks;
  }
  observed.candidateProcesses.push({ pid: 998, startTimeUtcTicks: oldTicks, role: 'guardian' });
  const result = check(before(), observed);
  assert.equal(result.structurallyConsistent, false);
  assert.match(result.errors.join(' '), /guardian predates the baseline/);
  assert.equal(result.betaApproved, false);
});

test('Explorer restart allows one new runtime and a new recovery guardian without approving beta', () => {
  const observed = after('after-explorer-restart');
  observed.explorerProcesses = [{ pid: 222, startTimeUtcTicks: newTicks }];
  for (const bar of observed.taskbars) {
    bar.ownerPid = 222;
    bar.ownerStartTimeUtcTicks = newTicks;
  }
  observed.candidateProcesses.push({ pid: 998, startTimeUtcTicks: newTicks, role: 'guardian' });
  const result = check(before(), observed);
  assert.equal(result.structurallyConsistent, true);
  assert.equal(result.manualGateStatus, 'NOT RUN');
  assert.equal(result.betaApproved, false);
});

test('Explorer restart rejects FedoraWin launched only after Explorer recovered', () => {
  const observed = after('after-explorer-restart');
  observed.explorerProcesses = [{ pid: 222, startTimeUtcTicks: newTicks }];
  for (const bar of observed.taskbars) {
    bar.ownerPid = 222;
    bar.ownerStartTimeUtcTicks = newTicks;
  }
  for (const lateStart of [newTicks, String(BigInt(newTicks) + 1n)]) {
    observed.candidateProcesses[0].startTimeUtcTicks = lateStart;
    const result = check(before(), observed);
    assert.equal(result.structurallyConsistent, false);
    assert.match(result.errors.join(' '), /runtime must start before the replacement Explorer lifetime/);
    assert.equal(result.betaApproved, false);
  }
});

test('rejects candidate and Explorer lifetimes from before the observed boot', () => {
  const staleTicks = String(BigInt(bootTicks) - 1n);
  const staleExplorer = after();
  staleExplorer.explorerProcesses[0].startTimeUtcTicks = staleTicks;
  for (const bar of staleExplorer.taskbars) bar.ownerStartTimeUtcTicks = staleTicks;
  assert.match(check(before(), staleExplorer).errors.join(' '), /Explorer process lifetime predates Windows boot/);

  const staleCandidate = after('after-explorer-restart');
  staleCandidate.candidateProcesses[0].startTimeUtcTicks = staleTicks;
  assert.match(check(before(), staleCandidate).errors.join(' '), /candidate process lifetime predates Windows boot/);
});
