//! FedoraWin DE temporarily hides Explorer's presentation, without terminating the Windows shell process.
//! A separate copy of this executable restores the original taskbar HWNDs
//! even when the desktop process is force-killed. No registry or Shell change.
use crate::windows::{frame_policy, frame_recovery};
use std::fs::{self, File};
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SW_HIDE: i32 = 0;
const SW_SHOW: i32 = 5;
const PROCESS_SYNCHRONIZE: u32 = 0x0010_0000;
const WAIT_FOREVER: u32 = 0xffff_ffff;
const WAIT_OBJECT_0: u32 = 0;
const MOVEFILE_REPLACE_EXISTING: u32 = 1;
const MOVEFILE_WRITE_THROUGH: u32 = 8;
static WINDOWS_DIRECTORY: OnceLock<Option<String>> = OnceLock::new();

// The independent guardian must never restore an HWND merely because its
// numeric value now names another Explorer taskbar. Bind every record to the
// original Explorer process lifetime, and fail closed on legacy HWND-only data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
struct TaskbarSnapshot {
    hwnd: isize,
    pid: u32,
    created: u64,
}

#[link(name = "user32")]
extern "system" {
    fn FindWindowW(class: *const u16, title: *const u16) -> isize;
    fn EnumWindows(callback: unsafe extern "system" fn(isize, isize) -> i32, data: isize) -> i32;
    fn GetClassNameW(hwnd: isize, text: *mut u16, capacity: i32) -> i32;
    fn IsWindow(hwnd: isize) -> i32;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn ShowWindow(hwnd: isize, command: i32) -> i32;
    fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
}

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
    fn WaitForSingleObject(handle: isize, timeout: u32) -> u32;
    fn CloseHandle(handle: isize) -> i32;
    fn MoveFileExW(source: *const u16, destination: *const u16, flags: u32) -> i32;
    fn GetWindowsDirectoryW(buffer: *mut u16, capacity: u32) -> u32;
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

fn is_taskbar_class(class: &str) -> bool {
    matches!(class, "Shell_TrayWnd" | "Shell_SecondaryTrayWnd")
}

fn class_name(hwnd: isize) -> Option<String> {
    let mut buffer = [0u16; 128];
    let len = unsafe { GetClassNameW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32) };
    (len > 0).then(|| String::from_utf16_lossy(&buffer[..len.max(0) as usize]))
}

fn is_windows_explorer_executable(executable: &str, windows_directory: &str) -> bool {
    let root = windows_directory
        .replace('/', "\\")
        .trim_end_matches('\\')
        .to_lowercase();
    !root.is_empty()
        && executable.replace('/', "\\").to_lowercase() == format!(r"{root}\explorer.exe")
}

fn windows_directory() -> Option<&'static str> {
    WINDOWS_DIRECTORY
        .get_or_init(|| {
            let mut buffer = [0u16; 32_768];
            let len = unsafe { GetWindowsDirectoryW(buffer.as_mut_ptr(), buffer.len() as u32) };
            if len == 0 || len as usize >= buffer.len() {
                return None;
            }
            Some(String::from_utf16_lossy(&buffer[..len as usize]))
        })
        .as_deref()
}

fn explorer_owns_taskbar(hwnd: isize) -> bool {
    let Some(windows_dir) = windows_directory() else {
        return false;
    };
    let mut pid = 0u32;
    if unsafe { GetWindowThreadProcessId(hwnd, &mut pid) } == 0 || pid == 0 {
        return false;
    }
    let Some(created) = frame_recovery::process_creation_time(pid) else {
        return false;
    };
    let Ok(executable) = frame_policy::process_key(pid) else {
        return false;
    };
    // A class name alone can be registered by an unrelated process. Only the
    // real %SystemRoot%\explorer.exe process may have its taskbar HWND hidden.
    // Recheck ownership and process lifetime after inspecting the executable.
    if !is_windows_explorer_executable(&executable, windows_dir)
        || frame_recovery::process_creation_time(pid) != Some(created)
    {
        return false;
    }
    let mut current_pid = 0u32;
    let thread_id = unsafe { GetWindowThreadProcessId(hwnd, &mut current_pid) };
    thread_id != 0 && current_pid == pid
}

fn valid_taskbar(hwnd: isize) -> bool {
    hwnd != 0
        && unsafe { IsWindow(hwnd) } != 0
        && class_name(hwnd).is_some_and(|name| is_taskbar_class(&name))
        && explorer_owns_taskbar(hwnd)
}

fn capture_taskbar(hwnd: isize) -> Option<TaskbarSnapshot> {
    if !valid_taskbar(hwnd) {
        return None;
    }
    let mut pid = 0u32;
    if unsafe { GetWindowThreadProcessId(hwnd, &mut pid) } == 0 || pid == 0 {
        return None;
    }
    let created = frame_recovery::process_creation_time(pid)?;
    let mut current_pid = 0u32;
    if !valid_taskbar(hwnd)
        || unsafe { GetWindowThreadProcessId(hwnd, &mut current_pid) } == 0
        || current_pid != pid
        || frame_recovery::process_creation_time(pid) != Some(created)
    {
        return None;
    }
    Some(TaskbarSnapshot { hwnd, pid, created })
}

fn same_taskbar_lifetime(snapshot: &TaskbarSnapshot) -> bool {
    if snapshot.hwnd == 0 || snapshot.pid == 0 || snapshot.created == 0 {
        return false;
    }
    if !valid_taskbar(snapshot.hwnd) {
        return false;
    }
    let mut pid = 0u32;
    let thread_id = unsafe { GetWindowThreadProcessId(snapshot.hwnd, &mut pid) };
    thread_id != 0
        && pid == snapshot.pid
        && frame_recovery::process_creation_time(pid) == Some(snapshot.created)
}

unsafe extern "system" fn enumerate_taskbars(hwnd: isize, data: isize) -> i32 {
    if IsWindowVisible(hwnd) != 0 {
        if let Some(snapshot) = capture_taskbar(hwnd) {
            let windows = &mut *(data as *mut Vec<TaskbarSnapshot>);
            if !windows.contains(&snapshot) {
                windows.push(snapshot);
            }
        }
    }
    1
}

fn visible_taskbars() -> Vec<TaskbarSnapshot> {
    let mut windows = Vec::new();
    let primary = unsafe { FindWindowW(wide("Shell_TrayWnd").as_ptr(), std::ptr::null()) };
    if unsafe { IsWindowVisible(primary) } != 0 {
        if let Some(snapshot) = capture_taskbar(primary) {
            windows.push(snapshot);
        }
    }
    unsafe {
        EnumWindows(
            enumerate_taskbars,
            &mut windows as *mut Vec<TaskbarSnapshot> as isize,
        );
    }
    windows
}

fn restore(windows: &[TaskbarSnapshot]) {
    for snapshot in windows {
        let hwnd = snapshot.hwnd;
        // Refuse stale/recycled HWNDs even when a new Explorer owns that class.
        if same_taskbar_lifetime(snapshot) {
            unsafe { ShowWindow(hwnd, SW_SHOW) };
        }
    }
}

fn add_new_handles(known: &mut Vec<TaskbarSnapshot>, observed: &[TaskbarSnapshot]) -> bool {
    let mut changed = false;
    for snapshot in observed {
        if snapshot.hwnd > 0
            && snapshot.pid != 0
            && snapshot.created != 0
            && !known.contains(snapshot)
        {
            known.push(*snapshot);
            changed = true;
        }
    }
    changed
}

fn persist_handles(path: &Path, windows: &[TaskbarSnapshot]) -> Result<(), String> {
    let temporary = path.with_extension("pending");
    let data = serde_json::to_vec(windows).map_err(|error| error.to_string())?;
    let mut file = File::create(&temporary).map_err(|error| error.to_string())?;
    file.write_all(&data).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    drop(file);

    let moved = unsafe {
        MoveFileExW(
            wide_path(&temporary).as_ptr(),
            wide_path(path).as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        return Err(format!(
            "atomic taskbar recovery journal replace failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

fn read_handles(path: &Path) -> Vec<TaskbarSnapshot> {
    // Corrupt or legacy HWND-only journals cannot authorize a restore.
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Vec<TaskbarSnapshot>>(&bytes).ok())
        .unwrap_or_default()
}

/// A watchdog executable restores every taskbar HWND FedoraWin journaled before
/// hiding it. The journal can grow when Explorer restarts while FedoraWin stays
/// alive, so one guardian is enough for the whole FedoraWin session.
pub fn maybe_run_guardian() -> bool {
    let mut args = std::env::args_os();
    let _exe = args.next();
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--restore-taskbar-after")) {
        return false;
    }
    let pid = args
        .next()
        .and_then(|arg| arg.to_string_lossy().parse::<u32>().ok());
    let path = args.next().map(PathBuf::from);
    if let (Some(pid), Some(path)) = (pid, path) {
        let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
        let stopped = if process == 0 {
            true
        } else {
            let result = unsafe { WaitForSingleObject(process, WAIT_FOREVER) };
            unsafe { CloseHandle(process) };
            result == WAIT_OBJECT_0
        };
        if stopped {
            restore(&read_handles(&path));
            let _ = fs::remove_file(&path);
            let _ = fs::remove_file(path.with_extension("pending"));
        }
    }
    true
}

/// On by default in FedoraWin DE. FEDORAWIN_KEEP_WINDOWS_TASKBAR=1 opts out.
/// The watchdog also covers Task Manager -> End task and capture process kills.
pub fn start() -> Result<(), String> {
    if std::env::var("FEDORAWIN_KEEP_WINDOWS_TASKBAR").as_deref() == Ok("1") {
        return Ok(());
    }
    let windows = visible_taskbars();
    if windows.is_empty() {
        return Ok(());
    }
    let current_exe =
        std::env::current_exe().map_err(|error| format!("taskbar recovery executable: {error}"))?;
    let parent_pid = std::process::id();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let journal =
        std::env::temp_dir().join(format!("fedorawin-taskbars-{parent_pid}-{nonce}.json"));
    persist_handles(&journal, &windows)?;

    // Do not hide any taskbar if crash recovery cannot be started.
    Command::new(current_exe)
        .arg("--restore-taskbar-after")
        .arg(parent_pid.to_string())
        .arg(&journal)
        .spawn()
        .map_err(|error| {
            let _ = fs::remove_file(&journal);
            let _ = fs::remove_file(journal.with_extension("pending"));
            format!("taskbar recovery could not start: {error}")
        })?;

    for snapshot in &windows {
        let hwnd = snapshot.hwnd;
        if same_taskbar_lifetime(snapshot) {
            unsafe { ShowWindow(hwnd, SW_HIDE) };
        }
    }

    // Explorer can create replacement taskbar HWNDs after a crash/restart.
    // Journal every new genuine taskbar BEFORE hiding it, so the independent
    // guardian can restore the replacement HWND if FedoraWin exits afterwards.
    thread::Builder::new()
        .name("fedorawin-desktop-presentation".into())
        .spawn(move || {
            let mut known = windows;
            loop {
                thread::sleep(Duration::from_millis(950));
                let observed = visible_taskbars();
                let previous_len = known.len();
                add_new_handles(&mut known, &observed);
                if known.len() != previous_len {
                    if let Err(error) = persist_handles(&journal, &known) {
                        eprintln!(
                            "FedoraWin left newly-created Explorer taskbars visible because recovery journaling failed: {error}"
                        );
                        known.truncate(previous_len);
                    }
                }

                for snapshot in &known {
                    let hwnd = snapshot.hwnd;
                    if same_taskbar_lifetime(snapshot) && unsafe { IsWindowVisible(hwnd) } != 0 {
                        unsafe { ShowWindow(hwnd, SW_HIDE) };
                    }
                }
            }
        })
        .map_err(|error| format!("taskbar presentation monitor: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        add_new_handles, is_taskbar_class, is_windows_explorer_executable, TaskbarSnapshot,
    };

    #[test]
    fn only_explorer_taskbar_classes() {
        assert!(is_taskbar_class("Shell_TrayWnd"));
        assert!(is_taskbar_class("Shell_SecondaryTrayWnd"));
        assert!(!is_taskbar_class("Progman"));
        assert!(!is_taskbar_class("CabinetWClass"));
    }

    #[test]
    fn taskbar_owner_requires_real_windows_explorer_path() {
        assert!(is_windows_explorer_executable(
            r"C:\Windows\explorer.exe",
            r"C:\Windows"
        ));
        assert!(is_windows_explorer_executable(
            "c:/windows/EXPLORER.EXE",
            "C:\\WINDOWS\\"
        ));
        assert!(!is_windows_explorer_executable(
            r"C:\Users\Public\explorer.exe",
            r"C:\Windows"
        ));
        assert!(!is_windows_explorer_executable(
            r"C:\Windows\System32\explorer.exe",
            r"C:\Windows"
        ));
        assert!(!is_windows_explorer_executable(
            r"C:\Windows\explorer.exe",
            ""
        ));
    }

    fn record(hwnd: isize, pid: u32, created: u64) -> TaskbarSnapshot {
        TaskbarSnapshot { hwnd, pid, created }
    }

    #[test]
    fn explorer_restart_rebinds_a_recycled_hwnd_to_its_new_process_lifetime() {
        let before = record(10, 111, 1_000);
        let unchanged = record(20, 111, 1_000);
        let after = record(10, 222, 2_000);
        let reused_pid = record(20, 111, 3_000);
        let mut known = vec![before, unchanged];

        assert!(add_new_handles(
            &mut known,
            &[
                unchanged,
                after,
                reused_pid,
                record(0, 222, 2_000),
                record(-4, 222, 2_000),
                record(30, 0, 2_000),
                record(30, 222, 0),
            ]
        ));
        assert_eq!(known, vec![before, unchanged, after, reused_pid]);
        assert!(!add_new_handles(&mut known, &[before, after, reused_pid]));
    }

    #[test]
    fn taskbar_journal_round_trips_identity_and_rejects_legacy_handle_arrays() {
        let snapshots = vec![record(10, 111, 1_000), record(20, 222, 2_000)];
        let serialized = serde_json::to_vec(&snapshots).unwrap();
        assert_eq!(
            serde_json::from_slice::<Vec<TaskbarSnapshot>>(&serialized).unwrap(),
            snapshots
        );
        assert!(serde_json::from_slice::<Vec<TaskbarSnapshot>>(br"[10,20]").is_err());
        assert!(serde_json::from_slice::<Vec<TaskbarSnapshot>>(
            br#"[{"hwnd":10,"pid":111}]"#
        )
        .is_err());
    }
}
