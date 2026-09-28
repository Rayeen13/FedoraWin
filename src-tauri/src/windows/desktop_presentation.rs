//! FedoraWin DE temporarily hides Explorer's presentation, without terminating the Windows shell process.
//! A separate copy of this executable restores the original taskbar HWNDs
//! even when the desktop process is force-killed. No registry or Shell change.
use std::fs::{self, File};
use std::io::Write;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const SW_HIDE: i32 = 0;
const SW_SHOW: i32 = 5;
const PROCESS_SYNCHRONIZE: u32 = 0x0010_0000;
const WAIT_FOREVER: u32 = 0xffff_ffff;
const WAIT_OBJECT_0: u32 = 0;
const MOVEFILE_REPLACE_EXISTING: u32 = 1;
const MOVEFILE_WRITE_THROUGH: u32 = 8;

#[link(name = "user32")]
extern "system" {
    fn FindWindowW(class: *const u16, title: *const u16) -> isize;
    fn EnumWindows(callback: unsafe extern "system" fn(isize, isize) -> i32, data: isize) -> i32;
    fn GetClassNameW(hwnd: isize, text: *mut u16, capacity: i32) -> i32;
    fn IsWindow(hwnd: isize) -> i32;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn ShowWindow(hwnd: isize, command: i32) -> i32;
}

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
    fn WaitForSingleObject(handle: isize, timeout: u32) -> u32;
    fn CloseHandle(handle: isize) -> i32;
    fn MoveFileExW(source: *const u16, destination: *const u16, flags: u32) -> i32;
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

fn valid_taskbar(hwnd: isize) -> bool {
    hwnd != 0
        && unsafe { IsWindow(hwnd) } != 0
        && class_name(hwnd).is_some_and(|name| is_taskbar_class(&name))
}

unsafe extern "system" fn enumerate_taskbars(hwnd: isize, data: isize) -> i32 {
    if valid_taskbar(hwnd) && IsWindowVisible(hwnd) != 0 {
        let windows = &mut *(data as *mut Vec<isize>);
        if !windows.contains(&hwnd) {
            windows.push(hwnd);
        }
    }
    1
}

fn visible_taskbars() -> Vec<isize> {
    let mut windows = Vec::new();
    let primary = unsafe { FindWindowW(wide("Shell_TrayWnd").as_ptr(), std::ptr::null()) };
    if valid_taskbar(primary) && unsafe { IsWindowVisible(primary) } != 0 {
        windows.push(primary);
    }
    unsafe {
        EnumWindows(enumerate_taskbars, &mut windows as *mut Vec<isize> as isize);
    }
    windows
}

fn restore(windows: &[isize]) {
    for &hwnd in windows {
        // Only restore HWNDs that are still genuine Explorer taskbar classes.
        if valid_taskbar(hwnd) {
            unsafe { ShowWindow(hwnd, SW_SHOW) };
        }
    }
}

fn add_new_handles(known: &mut Vec<isize>, observed: &[isize]) -> bool {
    let mut changed = false;
    for &hwnd in observed {
        if hwnd > 0 && !known.contains(&hwnd) {
            known.push(hwnd);
            changed = true;
        }
    }
    changed
}

fn persist_handles(path: &Path, windows: &[isize]) -> Result<(), String> {
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

fn read_handles(path: &Path) -> Vec<isize> {
    fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice::<Vec<isize>>(&bytes).ok())
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

    for &hwnd in &windows {
        if valid_taskbar(hwnd) {
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

                for &hwnd in &known {
                    if valid_taskbar(hwnd) && unsafe { IsWindowVisible(hwnd) } != 0 {
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
    use super::{add_new_handles, is_taskbar_class};

    #[test]
    fn only_explorer_taskbar_classes() {
        assert!(is_taskbar_class("Shell_TrayWnd"));
        assert!(is_taskbar_class("Shell_SecondaryTrayWnd"));
        assert!(!is_taskbar_class("Progman"));
        assert!(!is_taskbar_class("CabinetWClass"));
    }

    #[test]
    fn explorer_restart_adds_only_new_valid_handle_values() {
        let mut known = vec![10, 20];
        assert!(add_new_handles(&mut known, &[20, 30, 0, -4]));
        assert_eq!(known, vec![10, 20, 30]);
        assert!(!add_new_handles(&mut known, &[10, 20, 30]));
    }
}
