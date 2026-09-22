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

test('custom application chrome and DWM opt-outs are excluded even with WS_CAPTION', () => {
  assert.match(frame, /GetClassNameW\(hwnd, class_name\.as_mut_ptr\(\)/);
  assert.match(frame, /if !owns_standard_caption\(hwnd\)/);
  assert.match(frame, /chrome_widgetwin/);
  assert.match(frame, /mozillawindowclass/);
  assert.match(frame, /DWMWCP_DONOTROUND/);
  assert.match(frame, /== Some\(DWMWCP_ROUND\)/);
});

test('DWM original values are journaled and restored only after successful writes', () => {
  for (const property of ['dark', 'corner', 'border', 'caption', 'text']) {
    assert.match(frame, new RegExp(`changed_${property}: bool`));
    assert.match(frame, new RegExp(`original\\.changed_${property} \\|=\\s*set_attr`));
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

const recovery = readFileSync(new URL('../src-tauri/src/windows/frame_recovery.rs', import.meta.url), 'utf8');

test('frame guardian is initialized before the watcher can mutate foreign HWNDs', () => {
  assert.match(frame, /AtomicBool::new\(false\)/);
  assert.match(frame, /frame_recovery::start_guardian\(\)\?/);
  assert.match(frame, /FRAME_WATCHER_ENABLED\.store\(true, Ordering::SeqCst\)/);
  assert.match(runtime, /frame_recovery::maybe_run_guardian\(\)/);
});

test('a failed guardian startup cannot be mistaken for an initialized guardian on retry', () => {
  assert.match(recovery, /static GUARDIAN_READY: AtomicBool = AtomicBool::new\(false\)/);
  assert.match(recovery, /if JOURNAL_PATH\.get\(\)\.is_some\(\) \{\s*return if GUARDIAN_READY\.load\(Ordering::SeqCst\)/);
  assert.match(recovery, /frame guardian initialization failed; refusing to style windows/);
  assert.match(recovery, /\.spawn\(\)\s*\.map_err[\s\S]*?\?;\s*GUARDIAN_READY\.store\(true, Ordering::SeqCst\)/);
});

test('original DWM values are committed to disk before first styling', () => {
  assert.match(frame, /journal\.insert\(hwnd, snapshot_frame\(hwnd, pid, created\)\)/);
  assert.match(frame, /if persist_originals\(&journal\)\.is_err\(\)/);
  assert.match(frame, /journal\.remove\(&hwnd\);\s*return 1;/);
  assert.match(recovery, /file\.sync_all\(\)/);
  assert.match(recovery, /MOVEFILE_REPLACE_EXISTING \| MOVEFILE_WRITE_THROUGH/);
});

test('force-kill guardian checks HWND process lifetime and respects external style changes', () => {
  assert.match(recovery, /WaitForSingleObject\(process, WAIT_FOREVER\)/);
  assert.match(recovery, /process_creation_time\(current_pid\) != Some\(snapshot\.created\)/);
  assert.match(recovery, /matches_our_style\(current\) && current != original/);
  assert.match(recovery, /restore_snapshot\(snapshot\)/);
});
