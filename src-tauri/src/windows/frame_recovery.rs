//! Independent, write-ahead recovery for foreign native window DWM attributes.
//! This process never modifies Explorer, window styles, caption hit testing, or the registry.
use serde::{Deserialize, Serialize};
use std::ffi::c_void;
use std::fs::{self, File};
use std::io::Write;
use std::mem::size_of;
use std::os::windows::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::{SystemTime, UNIX_EPOCH};

const PROCESS_SYNCHRONIZE: u32 = 0x0010_0000;
const PROCESS_QUERY_LIMITED_INFORMATION: u32 = 0x1000;
const WAIT_OBJECT_0: u32 = 0;
const ERROR_INVALID_PARAMETER: i32 = 87;
const WAIT_FOREVER: u32 = 0xffff_ffff;
const MOVEFILE_REPLACE_EXISTING: u32 = 1;
const MOVEFILE_WRITE_THROUGH: u32 = 8;
const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
const DWMWA_BORDER_COLOR: u32 = 34;
const DWMWA_CAPTION_COLOR: u32 = 35;
const DWMWA_TEXT_COLOR: u32 = 36;
const DWMWCP_ROUND: i32 = 2;
const DWMWA_COLOR_NONE: u32 = 0xffff_fffe;

static JOURNAL_PATH: OnceLock<PathBuf> = OnceLock::new();
// OnceLock tracks a selected journal path, not a successfully running guardian.
// If persistence or spawn fails, this process must never treat a retry as ready.
static GUARDIAN_READY: AtomicBool = AtomicBool::new(false);

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct FileTime {
    low: u32,
    high: u32,
}

#[link(name = "kernel32")]
extern "system" {
    fn OpenProcess(access: u32, inherit: i32, pid: u32) -> isize;
    fn WaitForSingleObject(handle: isize, timeout: u32) -> u32;
    fn CloseHandle(handle: isize) -> i32;
    fn GetProcessTimes(
        handle: isize,
        created: *mut FileTime,
        exited: *mut FileTime,
        kernel: *mut FileTime,
        user: *mut FileTime,
    ) -> i32;
    fn MoveFileExW(source: *const u16, destination: *const u16, flags: u32) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn IsWindow(hwnd: isize) -> i32;
    fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
}

#[link(name = "dwmapi")]
extern "system" {
    fn DwmGetWindowAttribute(hwnd: isize, attr: u32, value: *mut c_void, size: u32) -> i32;
    fn DwmSetWindowAttribute(hwnd: isize, attr: u32, value: *const c_void, size: u32) -> i32;
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub hwnd: isize,
    pub pid: u32,
    pub created: u64,
    pub dark: Option<i32>,
    pub corner: Option<i32>,
    pub border: Option<u32>,
    pub caption: Option<u32>,
    pub text: Option<u32>,
}

fn process_creation_time_from_handle(process: isize) -> Option<u64> {
    let mut created = FileTime::default();
    let mut exited = FileTime::default();
    let mut kernel = FileTime::default();
    let mut user = FileTime::default();
    let ok = unsafe { GetProcessTimes(process, &mut created, &mut exited, &mut kernel, &mut user) };
    (ok != 0).then_some(((created.high as u64) << 32) | created.low as u64)
}

pub fn process_creation_time(pid: u32) -> Option<u64> {
    if pid == 0 {
        return None;
    }
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid) };
    if process == 0 {
        return None;
    }
    let created = process_creation_time_from_handle(process);
    unsafe { CloseHandle(process) };
    created
}

#[derive(Debug, PartialEq, Eq)]
enum ParentLifetime {
    Running,
    Stopped,
    Unknown,
}

// An absent/recycled PID proves the original parent's exit. If process access
// fails for any other reason, recovery cannot safely infer termination.
fn parent_lifetime(
    expected_created: u64,
    observed_created: Option<u64>,
    open_error: Option<i32>,
) -> ParentLifetime {
    if expected_created == 0 {
        return ParentLifetime::Unknown;
    }
    match observed_created {
        Some(created) if created == expected_created => ParentLifetime::Running,
        Some(_) => ParentLifetime::Stopped,
        None if open_error == Some(ERROR_INVALID_PARAMETER) => ParentLifetime::Stopped,
        None => ParentLifetime::Unknown,
    }
}

fn wide(path: &Path) -> Vec<u16> {
    path.as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect()
}

/// Commit a complete journal BEFORE the first DWM write to any newly observed HWND.
/// A crash can leave an extra snapshot, but cannot leave an unjournaled mutation.
pub fn persist(snapshots: &[Snapshot]) -> Result<(), String> {
    let path = JOURNAL_PATH.get().ok_or("frame guardian is unavailable")?;
    let temporary = path.with_extension("pending");
    let data = serde_json::to_vec(snapshots).map_err(|e| e.to_string())?;
    let mut file = File::create(&temporary).map_err(|e| e.to_string())?;
    file.write_all(&data).map_err(|e| e.to_string())?;
    file.sync_all().map_err(|e| e.to_string())?;
    drop(file);
    let moved = unsafe {
        MoveFileExW(
            wide(&temporary).as_ptr(),
            wide(path).as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if moved == 0 {
        return Err(format!(
            "atomic frame journal replace failed: {}",
            std::io::Error::last_os_error()
        ));
    }
    Ok(())
}

pub fn remove_journal() {
    if let Some(path) = JOURNAL_PATH.get() {
        let _ = fs::remove_file(path);
        let _ = fs::remove_file(path.with_extension("pending"));
    }
}

pub fn start_guardian() -> Result<(), String> {
    if JOURNAL_PATH.get().is_some() {
        return if GUARDIAN_READY.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err("frame guardian initialization failed; refusing to style windows".into())
        };
    }
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| e.to_string())?
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "fedorawin-frames-{}-{nonce}.json",
        std::process::id()
    ));
    JOURNAL_PATH
        .set(path.clone())
        .map_err(|_| "frame journal already initialized".to_string())?;
    persist(&[])?;
    let parent_created = process_creation_time(std::process::id())
        .ok_or("cannot identify frame guardian parent process lifetime")?;
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    Command::new(exe)
        .arg("--restore-frames-after")
        .arg(std::process::id().to_string())
        .arg(parent_created.to_string())
        .arg(&path)
        .spawn()
        .map_err(|e| format!("independent frame guardian did not start: {e}"))?;
    GUARDIAN_READY.store(true, Ordering::SeqCst);
    Ok(())
}

unsafe fn read_attr<T: Copy + Default>(hwnd: isize, attr: u32) -> Option<T> {
    let mut value = T::default();
    (DwmGetWindowAttribute(
        hwnd,
        attr,
        (&mut value as *mut T).cast(),
        size_of::<T>() as u32,
    ) == 0)
        .then_some(value)
}

unsafe fn restore_if<T: Copy + Default + PartialEq>(
    hwnd: isize,
    attr: u32,
    original: Option<T>,
    matches_our_style: impl Fn(T) -> bool,
) {
    if let (Some(original), Some(current)) = (original, read_attr::<T>(hwnd, attr)) {
        // Do not overwrite a new, non-FedoraWin value set by the owning application.
        if matches_our_style(current) && current != original {
            let _ = DwmSetWindowAttribute(
                hwnd,
                attr,
                (&original as *const T).cast(),
                size_of::<T>() as u32,
            );
        }
    }
}

fn colorref(r: u8, g: u8, b: u8) -> u32 {
    r as u32 | ((g as u32) << 8) | ((b as u32) << 16)
}

fn restore_snapshot(snapshot: Snapshot) {
    if snapshot.hwnd == 0 || unsafe { IsWindow(snapshot.hwnd) } == 0 {
        return;
    }
    let mut current_pid = 0u32;
    unsafe { GetWindowThreadProcessId(snapshot.hwnd, &mut current_pid) };
    if current_pid != snapshot.pid || process_creation_time(current_pid) != Some(snapshot.created) {
        return;
    }
    unsafe {
        restore_if(
            snapshot.hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            snapshot.dark,
            |v| v == 0 || v == 1,
        );
        restore_if(
            snapshot.hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            snapshot.corner,
            |v| v == DWMWCP_ROUND,
        );
        restore_if(snapshot.hwnd, DWMWA_BORDER_COLOR, snapshot.border, |v| {
            v == DWMWA_COLOR_NONE
        });
        restore_if(snapshot.hwnd, DWMWA_CAPTION_COLOR, snapshot.caption, |v| {
            v == colorref(46, 46, 50) || v == colorref(255, 255, 255)
        });
        restore_if(snapshot.hwnd, DWMWA_TEXT_COLOR, snapshot.text, |v| {
            v == colorref(255, 255, 255) || v == colorref(32, 32, 34)
        });
    }
}

pub fn maybe_run_guardian() -> bool {
    let mut args = std::env::args_os();
    args.next();
    if args.next().as_deref() != Some(std::ffi::OsStr::new("--restore-frames-after")) {
        return false;
    }
    let pid = args
        .next()
        .and_then(|s| s.to_string_lossy().parse::<u32>().ok());
    let expected_created = args
        .next()
        .and_then(|s| s.to_string_lossy().parse::<u64>().ok());
    let path = args.next().map(PathBuf::from);
    if let (Some(pid), Some(expected_created), Some(path)) = (pid, expected_created, path) {
        if pid == 0 || expected_created == 0 || args.next().is_some() {
            return true;
        }
        let process = unsafe {
            OpenProcess(PROCESS_SYNCHRONIZE | PROCESS_QUERY_LIMITED_INFORMATION, 0, pid)
        };
        let stopped = if process == 0 {
            parent_lifetime(
                expected_created,
                None,
                std::io::Error::last_os_error().raw_os_error(),
            ) == ParentLifetime::Stopped
        } else {
            // Query the opened handle, avoiding a second PID lookup/reuse race.
            let state = parent_lifetime(
                expected_created,
                process_creation_time_from_handle(process),
                None,
            );
            let stopped = match state {
                ParentLifetime::Running => {
                    unsafe { WaitForSingleObject(process, WAIT_FOREVER) } == WAIT_OBJECT_0
                }
                ParentLifetime::Stopped => true,
                ParentLifetime::Unknown => false,
            };
            unsafe { CloseHandle(process) };
            stopped
        };
        if stopped {
            if let Ok(bytes) = fs::read(&path) {
                if let Ok(snapshots) = serde_json::from_slice::<Vec<Snapshot>>(&bytes) {
                    for snapshot in snapshots {
                        restore_snapshot(snapshot);
                    }
                }
            }
            let _ = fs::remove_file(&path);
            let _ = fs::remove_file(path.with_extension("pending"));
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::{parent_lifetime, ParentLifetime, ERROR_INVALID_PARAMETER};

    #[test]
    fn guardian_waits_only_for_the_original_parent_lifetime() {
        assert_eq!(
            parent_lifetime(100, Some(100), None),
            ParentLifetime::Running
        );
        assert_eq!(
            parent_lifetime(100, Some(101), None),
            ParentLifetime::Stopped
        );
    }

    #[test]
    fn guardian_fails_closed_when_parent_identity_is_unverifiable() {
        assert_eq!(
            parent_lifetime(100, None, Some(ERROR_INVALID_PARAMETER)),
            ParentLifetime::Stopped
        );
        assert_eq!(parent_lifetime(100, None, Some(5)), ParentLifetime::Unknown);
        assert_eq!(parent_lifetime(100, None, None), ParentLifetime::Unknown);
        assert_eq!(parent_lifetime(0, Some(100), None), ParentLifetime::Unknown);
    }
}
