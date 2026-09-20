import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { test } from 'node:test';

const frame = readFileSync(new URL('../src-tauri/src/windows/frame.rs', import.meta.url), 'utf8');
const runtime = readFileSync(new URL('../src-tauri/src/main.rs', import.meta.url), 'utf8');

test('foreign native caption buttons and hit testing remain Windows-owned', () => {
  assert.match(frame, /GetWindowLongPtrW\(hwnd, GWL_STYLE\) & WS_CAPTION != WS_CAPTION/);
  assert.match(frame, /WS_EX_LAYERED \| WS_EX_NOREDIRECTIONBITMAP/);
  assert.doesNotMatch(frame, /SetWindowLongPtrW|SetWindowSubclass|SetWindowPos/);
});

test('DWM original values are journaled and restored only after successful writes', () => {
  for (const property of ['dark', 'corner', 'border', 'caption', 'text']) {
    assert.match(frame, new RegExp(`changed_${property}: bool`));
    assert.match(frame, new RegExp(`original\\.changed_${property} \\|= set_attr`));
  }
  assert.match(frame, /window_pid\(hwnd\) != original\.pid/);
  assert.match(frame, /restore_colors\(hwnd, original\)/);
});

test('reset prevents in-flight and future watcher mutations', () => {
  assert.match(frame, /FRAME_WATCHER_ENABLED\.store\(false, Ordering::SeqCst\)/);
  assert.match(frame, /if !FRAME_WATCHER_ENABLED\.load\(Ordering::SeqCst\)/);
  assert.match(runtime, /RunEvent::Exit/);
  assert.match(runtime, /windows::frame::reset_top_level_windows\(\)/);
});

test('System restores forced caption colors without changing real frames', () => {
  assert.match(frame, /ThemeMode::System => FramePalette/);
  assert.match(frame, /restore_colors\(hwnd, original\)/);
  assert.match(frame, /DWMWA_USE_IMMERSIVE_DARK_MODE/);
});
