import test from 'node:test';
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { mkdtemp, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { GATE_IDS, checkManualGateCoverage } from '../tools/check-w11-manual-gates.mjs';
import { verifyManualEvidence } from '../tools/verify-w11-manual-evidence.mjs';

// Entirely synthetic binary and evidence; not a Windows 11 manual test.
const executableBytes = Buffer.from('MZ synthetic fixture only');
const executableSha256 = createHash('sha256').update(executableBytes).digest('hex');
const complete = (digest = executableSha256) => GATE_IDS.map((id) => ({
  id, status: 'PASS', environment: 'physical', tester: 'test-fixture',
  testDate: '2026-10-19', windows11Build: 'Windows 11 fixture build 26100',
  executableSha256: digest, evidence: [id + '-synthetic.mp4']
}));

async function fixture(t, gates) {
  const dir = await mkdtemp(join(tmpdir(), 'fedorawin-evidence-'));
  t.after(() => rm(dir, { recursive: true, force: true }));
  const manifest = join(dir, 'manifest.json');
  const executable = join(dir, 'fedorawin.exe');
  await Promise.all([
    writeFile(manifest, JSON.stringify({ gates })),
    writeFile(executable, executableBytes)
  ]);
  return { manifest, executable };
}

test('all synthetic gate hashes match the actual fixture binary bytes', async (t) => {
  const { manifest, executable } = await fixture(t, complete());
  const result = await verifyManualEvidence(manifest, executable);
  assert.equal(result.executableSha256, executableSha256);
  assert.equal(result.complete, true);
  assert.equal(result.betaApproved, false);
});

test('uniform but incorrect gate hashes cannot masquerade as the candidate', async (t) => {
  const { manifest, executable } = await fixture(t, complete('a'.repeat(64)));
  const result = await verifyManualEvidence(manifest, executable);
  assert.equal(result.complete, false);
  assert.equal(result.betaApproved, false);
  assert.match(result.errors.join(' '), /release-candidate executable SHA-256/);
});

test('digest comparisons are case-insensitive and invalid expected hashes fail closed', () => {
  const upper = complete(executableSha256.toUpperCase());
  assert.equal(checkManualGateCoverage(upper, { expectedExecutableSha256: executableSha256 }).complete, true);
  assert.equal(checkManualGateCoverage(complete(), { expectedExecutableSha256: 'bad' }).complete, false);
});

test('missing candidate executable is an error, never a synthetic PASS', async (t) => {
  const { manifest, executable } = await fixture(t, complete());
  await rm(executable);
  await assert.rejects(verifyManualEvidence(manifest, executable), /ENOENT/);
});

test('CLI exits nonzero for a mismatched candidate and never claims beta approval', async (t) => {
  const { manifest, executable } = await fixture(t, complete('b'.repeat(64)));
  const script = fileURLToPath(new URL('../tools/verify-w11-manual-evidence.mjs', import.meta.url));
  const result = spawnSync(process.execPath, [script, manifest, executable], { encoding: 'utf8' });
  assert.equal(result.status, 1);
  assert.match(result.stdout, /"betaApproved": false/);
  assert.match(result.stdout, /STRUCTURAL CHECK ONLY/);
});

test('CLI rejects missing arguments instead of reporting complete', () => {
  const script = fileURLToPath(new URL('../tools/verify-w11-manual-evidence.mjs', import.meta.url));
  const result = spawnSync(process.execPath, [script], { encoding: 'utf8' });
  assert.equal(result.status, 2);
  assert.match(result.stderr, /Usage:/);
});
