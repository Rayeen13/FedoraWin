import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';

const script = readFileSync(new URL('./collect-w11-manual-snapshot.ps1', import.meta.url), 'utf8');

test('snapshot is observational and cannot change Explorer or native DWM state', () => {
  assert.match(script, /Get-CimInstance\s+-ClassName\s+Win32_OperatingSystem/);
  assert.match(script, /Get-FileHash\s+-LiteralPath\s+\$exe\s+-Algorithm\s+SHA256/);
  assert.match(script, /EnumWindows\(/);
  assert.match(script, /GetWindowThreadProcessId/);
  assert.match(script, /StartTime\.ToUniversalTime\(\)\.Ticks\.ToString\(/);
  assert.match(script, /hwnd\s*=\s*\$bar\.Hwnd\.ToString\(/);
  assert.doesNotMatch(script, /\b(?:Stop-Process|Start-Process|ShowWindow|SetWindowPos|DwmSetWindowAttribute|taskkill|Restart-Computer)\b/i);
});

test('snapshot rejects non-Windows 11 hosts and mismatched candidate hashes', () => {
  assert.match(script, /Caption\s+-notmatch\s+'\\bWindows 11\\b'/);
  assert.match(script, /build\s+-lt\s+22000/);
  assert.match(script, /Executable SHA-256 does not match the expected CI release candidate/);
  assert.match(script, /Explorer is not running/);
  assert.match(script, /No Explorer taskbar HWND found/);
});

test('captured environment cannot be mistaken for a completed manual gate', () => {
  assert.match(script, /evidenceType\s*=\s*'read-only-environment-snapshot'/);
  assert.match(script, /manualGateStatus\s*=\s*'NOT RUN'/);
  assert.match(script, /betaApproved\s*=\s*\$false/);
  assert.match(script, /ValidateSet\('before', 'after-clean-exit', 'after-force-kill', 'after-explorer-restart'\)/);
});
