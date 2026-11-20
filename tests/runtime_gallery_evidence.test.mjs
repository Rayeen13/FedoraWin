// Verify actual Windows executable captures against recorded provenance.
// Automated evidence is not Windows 11 manual beta sign-off.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, statSync, writeFileSync } from 'node:fs';
import { basename, join, resolve } from 'node:path';
import { test } from 'node:test';

const root = resolve(import.meta.dirname, '..');
const gallery = join(root, 'artifacts', 'site-preview');
const metadataPath = join(gallery, 'preview-metadata.json');
const executablePath = join(root, 'src-tauri', 'target', 'release', 'fedorawin.exe');
const required = [
  'desktop', 'panel', 'activities', 'apps', 'search_terminal',
  'quick_settings_dark', 'quick_settings_light', 'appearance',
  'power_mode', 'wifi', 'bluetooth', 'radio_pause', 'date_menu',
  'native_frame', 'native_frame_light',
];
const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');

// Verify PNG chunk framing and checksums rather than trusting only its signature.
function crc32(bytes) {
  let crc = 0xffffffff;
  for (const byte of bytes) {
    crc ^= byte;
    for (let bit = 0; bit < 8; bit++) {
      crc = (crc >>> 1) ^ ((crc & 1) ? 0xedb88320 : 0);
    }
  }
  return (crc ^ 0xffffffff) >>> 0;
}

function pngDimensions(bytes) {
  assert.ok(bytes.length >= 45, 'truncated PNG');
  assert.equal(bytes.subarray(0, 8).toString('hex'), '89504e470d0a1a0a', 'not PNG');
  let offset = 8;
  let width = 0;
  let height = 0;
  let chunks = 0;
  let hasImageData = false;
  let ended = false;
  while (offset < bytes.length) {
    assert.ok(offset + 12 <= bytes.length, 'truncated PNG chunk header');
    const length = bytes.readUInt32BE(offset);
    assert.ok(length <= bytes.length - offset - 12, 'truncated PNG chunk data');
    const type = bytes.toString('ascii', offset + 4, offset + 8);
    assert.match(type, /^[A-Za-z]{4}$/, 'invalid PNG chunk type');
    const end = offset + 12 + length;
    const expectedCrc = bytes.readUInt32BE(end - 4);
    assert.equal(crc32(bytes.subarray(offset + 4, end - 4)), expectedCrc, 'PNG ' + type + ' CRC mismatch');
    if (chunks++ === 0) {
      assert.equal(type, 'IHDR', 'PNG must start with IHDR');
      assert.equal(length, 13, 'invalid PNG IHDR length');
      width = bytes.readUInt32BE(offset + 8);
      height = bytes.readUInt32BE(offset + 12);
      assert.ok(width >= 24 && height >= 24, 'invalid PNG dimensions');
    }
    if (type === 'IDAT') hasImageData = true;
    if (type === 'IEND') {
      assert.equal(length, 0, 'invalid PNG end chunk');
      assert.equal(end, bytes.length, 'unexpected trailing PNG data');
      ended = true;
      break;
    }
    offset = end;
  }
  assert.ok(hasImageData && ended, 'incomplete PNG image');
  return { width, height };
}

function verifyCapture(key, filename, expectedHash) {
  assert.equal(filename, key + '.png', 'unexpected filename for ' + key);
  assert.equal(basename(filename), filename, 'unsafe path for ' + key);
  const file = join(gallery, filename);
  assert.ok(existsSync(file), 'missing capture: ' + filename);
  const bytes = readFileSync(file);
  assert.ok(bytes.length >= 512, 'capture unexpectedly small: ' + filename);
  const { width, height } = pngDimensions(bytes);
  assert.match(expectedHash, /^[0-9a-f]{64}$/, 'invalid SHA-256 for ' + key);
  const hash = sha256(bytes);
  assert.equal(expectedHash, hash, 'capture SHA-256 mismatch: ' + key);
  return { filename, width, height, sha256: hash, bytes: bytes.length };
}

test('Windows runtime gallery is tied to its CI source, release executable and valid PNG captures', () => {
  assert.ok(existsSync(metadataPath), 'runtime gallery metadata missing');
  assert.ok(existsSync(executablePath), 'release executable missing');
  const metadataBytes = readFileSync(metadataPath);
  const metadata = JSON.parse(metadataBytes.toString('utf8'));
  const commit = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root, encoding: 'utf8' }).trim();
  assert.match(commit, /^[0-9a-f]{40}$/);
  assert.equal(metadata.source_sha, commit, 'capture metadata belongs to a different revision');
  if (process.env.GITHUB_SHA) {
    assert.equal(metadata.source_sha, process.env.GITHUB_SHA, 'GitHub checkout SHA mismatch');
  }
  if (process.env.GITHUB_RUN_ID) {
    assert.equal(String(metadata.github_run_id), process.env.GITHUB_RUN_ID, 'CI run identity mismatch');
  }
  assert.ok(typeof metadata.source_branch === 'string' && metadata.source_branch.length > 0,
    'capture branch missing');
  assert.equal(metadata.launch_executable, 'fedorawin.exe');
  assert.equal(metadata.runner_os, 'Windows', 'captures must originate from Windows CI');
  assert.ok(Number.isFinite(Date.parse(metadata.captured_utc)), 'missing capture timestamp');
  assert.ok(metadata.screenshots && typeof metadata.screenshots === 'object');
  assert.ok(metadata.sha256 && typeof metadata.sha256 === 'object');
  assert.deepEqual(Object.keys(metadata.screenshots).sort(), [...required].sort());
  assert.deepEqual(Object.keys(metadata.sha256).sort(), [...required].sort());

  const captures = {};
  for (const key of required) {
    captures[key] = verifyCapture(key, metadata.screenshots[key], metadata.sha256[key]);
  }
  assert.ok(new Set(Object.values(captures).map(c => c.sha256)).size >= 13,
    'gallery is not sufficiently distinct');
  assert.notEqual(captures.quick_settings_dark.sha256, captures.quick_settings_light.sha256);
  assert.notEqual(captures.native_frame.sha256, captures.native_frame_light.sha256);

  const exeBytes = readFileSync(executablePath);
  assert.ok(statSync(executablePath).size > 100_000, 'release executable unexpectedly small');
  assert.equal(exeBytes.subarray(0, 2).toString('ascii'), 'MZ', 'release binary is not a Windows PE executable');
  const report = {
    source_sha: commit,
    source_branch: metadata.source_branch,
    github_run_id: metadata.github_run_id,
    captured_utc: metadata.captured_utc,
    runner_image: metadata.runner_image,
    executable_sha256: sha256(exeBytes),
    executable_bytes: exeBytes.length,
    gallery_metadata_sha256: sha256(metadataBytes),
    screenshots: captures,
    validation: 'automated-windows-ci-only; manual-W11-01-through-W11-14-not-run',
  };
  writeFileSync(join(gallery, 'evidence-integrity.json'), JSON.stringify(report, null, 2) + '\n');
  console.log('Verified ' + required.length + ' Windows captures; executable SHA-256 ' + report.executable_sha256);
});

test('PNG evidence parser fails closed on invalid signatures and truncated chunks', () => {
  assert.throws(() => pngDimensions(Buffer.alloc(45)), /not PNG/);
  const signatureOnly = Buffer.alloc(45);
  Buffer.from('89504e470d0a1a0a', 'hex').copy(signatureOnly);
  signatureOnly.writeUInt32BE(0xfffffff0, 8);
  assert.throws(() => pngDimensions(signatureOnly), /truncated PNG chunk data/);
});

test('PNG parser rejects corrupted CRCs, absent IEND and trailing data', () => {
  function chunk(type, payload) {
    const bytes = Buffer.alloc(12 + payload.length);
    bytes.writeUInt32BE(payload.length, 0);
    bytes.write(type, 4, 4, 'ascii');
    payload.copy(bytes, 8);
    bytes.writeUInt32BE(crc32(bytes.subarray(4, bytes.length - 4)), bytes.length - 4);
    return bytes;
  }
  const header = Buffer.alloc(13);
  header.writeUInt32BE(24, 0);
  header.writeUInt32BE(24, 4);
  header[8] = 8;
  header[9] = 2;
  const signature = Buffer.from('89504e470d0a1a0a', 'hex');
  const ihdr = chunk('IHDR', header);
  const idat = chunk('IDAT', Buffer.alloc(12, 0x61));
  const iend = chunk('IEND', Buffer.alloc(0));
  const valid = Buffer.concat([signature, ihdr, idat, iend]);
  assert.deepEqual(pngDimensions(valid), { width: 24, height: 24 });
  const tampered = Buffer.from(valid);
  tampered[signature.length + ihdr.length + 10] ^= 0x80;
  assert.throws(() => pngDimensions(tampered), /CRC mismatch/);
  assert.throws(() => pngDimensions(valid.subarray(0, -12)), /incomplete PNG/);
  assert.throws(() => pngDimensions(Buffer.concat([valid, Buffer.from('junk')])),
    /unexpected trailing PNG data/);
});
