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

test('normal rollback respects a target application overriding its DWM border', () => {
  assert.match(frame, /if original\.changed_border \{/);
  assert.match(frame, /read_attr::<u32>\(hwnd, DWMWA_BORDER_COLOR\) == Some\(DWMWA_COLOR_NONE\)/);
  assert.match(frame, /if read_attr::<u32>\(hwnd, DWMWA_BORDER_COLOR\) == Some\(DWMWA_COLOR_NONE\) \{\s*set_attr\(hwnd, DWMWA_BORDER_COLOR, &value\)/);
});

test('System restores forced caption colors without changing real frames', () => {
  assert.match(frame, /ThemeMode::System => FramePalette/);
  assert.match(frame, /restore_colors\(hwnd, original\)/);
  assert.match(frame, /DWMWA_USE_IMMERSIVE_DARK_MODE/);
});

const recovery = readFileSync(new URL('../src-tauri/src/windows/frame_recovery.rs', import.meta.url), 'utf8');
const framePolicy = readFileSync(new URL('../src-tauri/src/windows/frame_policy.rs', import.meta.url), 'utf8');
const activitiesUi = readFileSync(new URL('../ui/app.js', import.meta.url), 'utf8');

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

const appbar = readFileSync(new URL('../src-tauri/src/windows/appbar.rs', import.meta.url), 'utf8');

test('AppBar registration failures never proceed with a stale desktop reservation', () => {
  assert.match(appbar, /if SHAppBarMessage\(ABM_NEW, &mut data\) == 0 \{\s*return Err/);
  for (const message of ['QUERYPOS', 'SETPOS']) {
    const failure = appbar.match(new RegExp(
      `if SHAppBarMessage\\(ABM_${message}, &mut data\\) == 0 \\{([\\s\\S]*?)\\n        \\}`
    ));
    assert.ok(failure, `missing failure handling for ABM_${message}`);
    assert.match(failure[1], /SHAppBarMessage\(ABM_REMOVE, &mut data\) != 0/);
    assert.match(failure[1], /AppBar rollback also failed/);
  }
});


test('per-window frame exclusions are lifetime-bound and immediately reversible', () => {
  assert.match(frame, /struct WindowIdentity \{/);
  assert.match(frame, /pid: u32,/);
  assert.match(frame, /created: u64,/);
  assert.match(frame, /FRAME_EXCLUSIONS/);
  assert.match(frame, /identity_is_excluded\(identity\)/);
  assert.match(frame, /journal\.remove\(&hwnd\);\s*unsafe \{ restore_frame\(hwnd, original\) \}/);
  assert.match(runtime, /fn exclude_window_frame\(handle: String\)/);
  assert.match(runtime, /fn include_window_frame\(handle: String\)/);
  assert.match(runtime, /fn is_window_frame_excluded\(handle: String\)/);
  assert.match(runtime, /exclude_window_frame,\s*include_window_frame,\s*is_window_frame_excluded,/);
});

const windowsList = readFileSync(new URL('../src-tauri/src/windows/windows_list.rs', import.meta.url), 'utf8');
const desktopPresentation = readFileSync(new URL('../src-tauri/src/windows/desktop_presentation.rs', import.meta.url), 'utf8');
const appIcons = readFileSync(new URL('../src-tauri/src/windows/app_icons.rs', import.meta.url), 'utf8');
const virtualDesktop = readFileSync(new URL('../src-tauri/src/windows/virtual_desktop.rs', import.meta.url), 'utf8');

test('shared Win32 FFI declarations use one ABI-compatible signature', () => {
  for (const source of [frame, windowsList, desktopPresentation]) {
    assert.match(source, /fn EnumWindows\(callback: unsafe extern "system" fn\(isize, isize\) -> i32, (?:lparam|data): isize\) -> i32;/);
  }
  assert.match(frame, /unsafe extern "system" fn apply_callback/);
  assert.match(windowsList, /unsafe extern "system" fn enum_callback/);
  for (const source of [appIcons, virtualDesktop]) {
    assert.match(source, /fn CoInitializeEx\(reserved: \*mut c_void, (?:model|apartment): u32\) -> i32;/);
  }
  assert.match(appIcons, /CoInitializeEx\(null_mut\(\), COINIT_APARTMENTTHREADED\)/);
});


test('persistent app exclusions are local, atomic, fail-closed, and user reversible', () => {
  assert.match(framePolicy, /LOCALAPPDATA/);
  assert.match(framePolicy, /frame-exclusions\.json/);
  assert.match(framePolicy, /QueryFullProcessImageNameW/);
  assert.match(framePolicy, /MOVEFILE_REPLACE_EXISTING \| MOVEFILE_WRITE_THROUGH/);
  assert.match(frame, /let Ok\(process_key\) = frame_policy::process_key\(pid\) else \{\s*return false;/);
  assert.match(frame, /let Ok\(app_excluded\) = frame_policy::is_process_key_excluded\(&process_key\) else \{\s*return false;/);
  assert.match(frame, /pub fn set_app_excluded\(handle: &str, excluded: bool\)/);
  assert.match(frame, /restore_frame\(candidate_hwnd, original\)/);
  assert.match(windowsList, /pub frame_excluded: bool/);
  assert.match(windowsList, /is_app_excluded\(&window\.handle\)\.unwrap_or\(true\)/);
  assert.match(runtime, /fn set_app_frame_excluded[\s\S]*excluded: bool/);
  assert.match(activitiesUi, /data-frame-policy-window=/);
  assert.match(activitiesUi, /set_app_frame_excluded/);
  assert.match(activitiesUi, /Never style this app/);
});


test('DWM fallback rejects protected Windows shell and security process identities', () => {
  assert.match(frame, /PROTECTED_PROCESS_NAMES/);
  for (const process of [
    'shellexperiencehost\\.exe',
    'startmenuexperiencehost\\.exe',
    'searchhost\\.exe',
    'lockapp\\.exe',
    'logonui\\.exe',
    'consent\\.exe',
    'securityhealthservice\\.exe'
  ]) {
    assert.match(frame, new RegExp(process));
  }
  assert.match(frame, /frame_policy::process_key\(pid\)/);
  assert.match(frame, /protected_process_family\(&process_key\)\.is_some\(\)/);
  assert.match(framePolicy, /pub fn is_process_key_excluded\(process_key: &str\)/);
  assert.doesNotMatch(frame, /\("explorer\.exe",/);
});

const taskbarRecovery = readFileSync(new URL('./check-taskbar-recovery.ps1', import.meta.url), 'utf8');

test('taskbar recovery cleanup is bound to the original Explorer process lifetime', () => {
  assert.match(taskbarRecovery, /function Test-BaselineExplorerTaskbar/);
  assert.match(taskbarRecovery, /GetWindowThreadProcessId\(\$Hwnd, \[ref\]\$ownerPid\)/);
  assert.match(taskbarRecovery, /if \(\$threadId -eq 0\) \{ return 0 \}/);
  assert.match(taskbarRecovery, /\$script:explorerStartTicks\[\[int\]\$process\.Id\]/);
  assert.match(taskbarRecovery, /\$owner\.ProcessName -ne 'explorer'/);
  assert.match(taskbarRecovery, /\$owner\.StartTime\.ToUniversalTime\(\)\.Ticks -eq \$script:explorerStartTicks/);

  const cleanup = taskbarRecovery.slice(taskbarRecovery.lastIndexOf('} finally {'));
  assert.match(cleanup, /if \(\(Test-BaselineExplorerTaskbar -Hwnd \$hwnd\) -and/);
  assert.match(cleanup, /\[FedoraWinTaskbarRecoveryProbe\]::ShowWindow\(\$hwnd, 5\)/);
  assert.doesNotMatch(cleanup, /if \(\(Test-ExplorerTaskbar -Hwnd \$hwnd\) -and/);
});
