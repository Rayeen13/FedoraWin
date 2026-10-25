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

test('foreign window frame candidates are read-only hints, never attachment authority', () => {
  assert.match(frame, /fn classify_frame_traits\(/);
  assert.match(frame, /pub fn frame_candidate_status\(handle: &str\)/);
  assert.match(frame, /"standard_candidate_unverified"/);
  assert.match(frame, /"custom_chrome"/);
  assert.match(frame, /"system_excluded"/);
  assert.match(frame, /"policy_excluded"/);
  assert.match(frame, /"nonstandard_window"/);
  assert.match(frame, /"unknown"/);
  assert.match(frame, /WS_THICKFRAME|WS_SYSMENU/);
  assert.match(frame, /window_identity\(hwnd\) != Some\(identity\)/);
  assert.match(frame, /passive_frame_candidate_preserves_custom_chrome_explorer_and_unknowns/);
  assert.doesNotMatch(frame, /SetWindowLongPtrW|SetWindowSubclass|SetWindowPos/);
  assert.match(readFileSync(new URL('../src-tauri/src/windows/windows_list.rs', import.meta.url), 'utf8'), /frame_candidate: String/);
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

test('frame guardian binds parent wait to process creation time and refuses ambiguous identity', () => {
  assert.match(recovery, /let parent_created = process_creation_time\(std::process::id\(\)\)/);
  assert.match(recovery, /\.arg\(parent_created\.to_string\(\)\)/);
  assert.match(recovery, /PROCESS_SYNCHRONIZE \| PROCESS_QUERY_LIMITED_INFORMATION/);
  assert.match(recovery, /process_creation_time_from_handle\(process\)/);
  assert.match(recovery, /ParentLifetime::Unknown => false/);
  assert.match(recovery, /None if open_error == Some\(ERROR_INVALID_PARAMETER\) => ParentLifetime::Stopped/);
  assert.doesNotMatch(recovery, /OpenProcess\(PROCESS_SYNCHRONIZE, 0, pid\)/);
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

const explorerRestartProbe = readFileSync(new URL('./check-explorer-restart-recovery.ps1', import.meta.url), 'utf8');

test('Explorer restart probe only manipulates genuine Windows Explorer taskbars', () => {
  assert.match(explorerRestartProbe, /function Test-RealExplorerProcess/);
  assert.match(explorerRestartProbe, /function Test-RealExplorerTaskbar/);
  assert.match(explorerRestartProbe, /function Test-ReplacementTaskbar/);
  assert.match(explorerRestartProbe, /\$script:windowsExplorer = \[IO\.Path\]::GetFullPath/);
  assert.match(explorerRestartProbe, /\$Process\.Path/);
  assert.match(explorerRestartProbe, /\$current\.StartTime\.ToUniversalTime\(\)\.Ticks -eq \$created/);
  assert.match(explorerRestartProbe, /if \(-not \(Test-RealExplorerProcess -Process \$owner\) -or/);
  assert.match(explorerRestartProbe, /Refusing to terminate a recycled or non-Explorer PID/);
  assert.match(explorerRestartProbe, /identify the new taskbar by[\s\S]*Explorer process lifetime/);

  const cleanup = explorerRestartProbe.slice(explorerRestartProbe.lastIndexOf('} finally {'));
  assert.match(cleanup, /if \(\(Test-RealExplorerTaskbar -Hwnd \$hwnd\) -and/);
  assert.match(cleanup, /\[FedoraWinExplorerRestartProbe\]::ShowWindow\(\$hwnd, 5\)/);
  assert.doesNotMatch(cleanup, /if \(\(Test-Taskbar -Hwnd \$hwnd\) -and/);
});

test('taskbar watchdog journals Explorer process lifetime, not bare HWNDs', () => {
  assert.match(desktopPresentation, /struct TaskbarSnapshot \{/);
  for (const field of ['hwnd: isize', 'pid: u32', 'created: u64']) {
    assert.ok(desktopPresentation.includes(field), `missing journal identity field: ${field}`);
  }
  assert.match(desktopPresentation, /serde_json::from_slice::<Vec<TaskbarSnapshot>>/);
  assert.match(desktopPresentation, /fn same_taskbar_lifetime\(snapshot: &TaskbarSnapshot\) -> bool/);
  assert.match(desktopPresentation, /pid == snapshot\.pid/);
  assert.match(desktopPresentation, /process_creation_time\(pid\) == Some\(snapshot\.created\)/);
  assert.match(desktopPresentation, /fn restore\(windows: &\[TaskbarSnapshot\]\)/);
  assert.match(desktopPresentation, /if same_taskbar_lifetime\(snapshot\) \{/);
  assert.match(desktopPresentation, /same_taskbar_lifetime\(snapshot\) && unsafe \{ IsWindowVisible\(hwnd\) \}/);
  assert.match(desktopPresentation, /taskbar_journal_round_trips_identity_and_rejects_legacy_handle_arrays/);
  assert.match(desktopPresentation, /explorer_restart_rebinds_a_recycled_hwnd_to_its_new_process_lifetime/);
  assert.doesNotMatch(desktopPresentation, /from_slice::<Vec<isize>>/);
});

test('taskbar journal compaction retains unknown lifetimes and writes before hiding', () => {
  assert.match(desktopPresentation, /fn snapshot_provably_stale\(snapshot: &TaskbarSnapshot\) -> bool/);
  assert.match(desktopPresentation, /if unsafe \{ IsWindow\(snapshot\.hwnd\) \} == 0/);
  assert.match(desktopPresentation, /thread_id != 0 && current_pid != snapshot\.pid/);
  assert.match(desktopPresentation, /process_creation_time\(snapshot\.pid\)[\s\S]*?\.is_some_and/);
  assert.match(desktopPresentation, /fn next_journal\(/);
  assert.match(desktopPresentation, /\.filter\(\|snapshot\| !is_provably_stale\(snapshot\)\)/);
  assert.match(desktopPresentation, /let next = next_journal\(&known, &observed, snapshot_provably_stale\)/);
  assert.match(desktopPresentation, /match persist_handles\(&journal, &next\) \{\s*Ok\(\(\)\) => known = next/);
  assert.match(desktopPresentation, /journal_compaction_removes_only_proven_stale_lifetimes/);
  assert.match(desktopPresentation, /repeated_explorer_restarts_do_not_accumulate_dead_recovery_records/);
});

test('Activities focus and close actions reject ineligible and recycled HWND targets', () => {
  assert.match(windowsList, /fn action_target_identity\(hwnd: isize, identity: &str\)/);
  assert.match(windowsList, /fn same_action_target\(hwnd: isize, pid: u32, created: u64\)/);
  assert.match(windowsList, /fn action_identity_matches\(/);
  assert.match(windowsList, /fn parse_action_identity\(identity: &str\)/);
  assert.match(windowsList, /action_identity: format!\("\{pid\}:\{created\}"\)/);
  assert.match(windowsList, /fn activities_snapshot_identity_rejects_missing_or_malformed_lifetimes\(/);
  assert.match(windowsList, /windows\.retain\(\|window\|/);
  assert.match(windowsList, /pub fn activate\(handle: &str, identity: &str\)/);
  assert.match(windowsList, /pub fn close\(handle: &str, identity: &str\)/);
  assert.match(runtime, /fn activate_window\(handle: String, identity: String\)/);
  assert.match(runtime, /fn close_window\(handle: String, identity: String\)/);
  assert.match(activitiesUi, /data-action-identity=/);
  assert.match(activitiesUi, /activate_window', \{ handle: button\.dataset\.window, identity: button\.dataset\.actionIdentity \}/);
  assert.match(activitiesUi, /close_window', \{ handle: button\.dataset\.closeWindow, identity: button\.dataset\.actionIdentity \}/);
  assert.match(windowsList, /super::frame_recovery::process_creation_time\(current_pid\)/);
  assert.match(windowsList, /fn activities_actions_reject_recycled_hwnd_process_identity\(/);
  assert.match(windowsList, /pub fn activate\(handle: &str, identity: &str\)[\s\S]*?action_target_identity\(hwnd, identity\)\?/);
  assert.match(windowsList, /pub fn close\(handle: &str, identity: &str\)[\s\S]*?action_target_identity\(hwnd, identity\)\?/);
  assert.match(windowsList, /if !same_action_target\(hwnd, pid, created\) \{[\s\S]*?window changed before activation/);
  assert.match(windowsList, /if !same_action_target\(hwnd, pid, created\) \{[\s\S]*?window changed before closing/);
});
