//! FedoraWin DE temporarily hides Explorer's presentation, never Explorer.exe.
//! A separate copy of this executable restores the original taskbar HWNDs
//! even when the desktop process is force-killed. No registry or Shell change.
use std::mem::zeroed;
use std::process::Command;
use std::thread;
use std::time::Duration;

const SW_HIDE: i32 = 0;
const SW_SHOW: i32 = 5;
const PROCESS_SYNCHRONIZE: u32 = 0x0010_0000;
const WAIT_FOREVER: u32 = 0xffff_ffff;
const WAIT_OBJECT_0: u32 = 0;

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
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
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
        EnumWindows(
            enumerate_taskbars,
            &mut windows as *mut Vec<isize> as isize,
        );
    }
    windows
}

fn restore(windows: &[isize]) {
    for &hwnd in windows {
        // Never reveal a newly-created, unrelated window after Explorer restarts.
        if valid_taskbar(hwnd) {
            unsafe { ShowWindow(hwnd, SW_SHOW) };
        }
    }
}

fn parse_handles(arg: &str) -> Vec<isize> {
    arg.split(',')
        .filter_map(|value| value.parse::<isize>().ok())
        .filter(|&value| value > 0)
        .collect()
}

/// A watchdog executable only restores windows that FedoraWin itself hid.
/// It does not create a panel, patch Explorer, or own desktop UI.
pub fn maybe_run_guardian() -> bool {
    let mut args = std::env::args();
    let _exe = args.next();
    if args.next().as_deref() != Some("--restore-taskbar-after") {
        return false;
    }
    let pid = args.next().and_then(|arg| arg.parse::<u32>().ok());
    let windows = args
        .next()
        .map(|arg| parse_handles(&arg))
        .unwrap_or_default();
    if let Some(pid) = pid {
        let process = unsafe { OpenProcess(PROCESS_SYNCHRONIZE, 0, pid) };
        if process != 0 {
            let finished = unsafe { WaitForSingleObject(process, WAIT_FOREVER) };
            unsafe { CloseHandle(process) };
            if finished == WAIT_OBJECT_0 {
                restore(&windows);
            }
        } else {
            // Parent already exited before this helper could obtain a handle.
            restore(&windows);
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
    let handles = windows
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(",");
    // Do not hide any taskbar if crash recovery cannot be started.
    Command::new(current_exe)
        .arg("--restore-taskbar-after")
        .arg(parent_pid.to_string())
        .arg(handles)
        .spawn()
        .map_err(|error| format!("taskbar recovery could not start: {error}"))?;
    for &hwnd in &windows {
        if valid_taskbar(hwnd) {
            unsafe { ShowWindow(hwnd, SW_HIDE) };
        }
    }
    // Explorer may re-show its taskbar when the work area or display changes.
    // Only re-hide the same original HWNDs; the guardian owns eventual restore.
    thread::Builder::new()
        .name("fedorawin-desktop-presentation".into())
        .spawn(move || loop {
            thread::sleep(Duration::from_millis(950));
            for &hwnd in &windows {
                if valid_taskbar(hwnd) && unsafe { IsWindowVisible(hwnd) } != 0 {
                    unsafe { ShowWindow(hwnd, SW_HIDE) };
                }
            }
        })
        .map_err(|error| format!("taskbar presentation monitor: {error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{is_taskbar_class, parse_handles};

    #[test]
    fn only_explorer_taskbar_classes() {
        assert!(is_taskbar_class("Shell_TrayWnd"));
        assert!(is_taskbar_class("Shell_SecondaryTrayWnd"));
        assert!(!is_taskbar_class("Progman"));
        assert!(!is_taskbar_class("CabinetWClass"));
    }

    #[test]
    fn malformed_watchdog_arguments_cannot_restore_arbitrary_handles() {
        assert_eq!(parse_handles("10,bad,0,-4,20"), vec![10, 20]);
    }
}
