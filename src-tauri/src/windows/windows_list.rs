use serde::Serialize;
use std::ffi::c_void;
use std::mem::size_of;

const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOOLWINDOW: isize = 0x00000080;
const GW_OWNER: u32 = 4;
const DWMWA_CLOAKED: u32 = 14;
const DWM_CLOAKED_SHELL: i32 = 0x2;
const SW_RESTORE: i32 = 9;
const WM_CLOSE: u32 = 0x0010;

// Shell-owned surfaces must never appear as movable Activities app windows.
// Ordinary File Explorer windows (CabinetWClass) remain eligible.
fn is_shell_surface_class(class_name: &str) -> bool {
    [
        "Shell_TrayWnd",
        "Shell_SecondaryTrayWnd",
        "Progman",
        "WorkerW",
        "NotifyIconOverflowWindow",
    ]
    .iter()
    .any(|protected| class_name.eq_ignore_ascii_case(protected))
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowEntry {
    pub handle: String,
    // Identity captured during enumeration, never reconstructed at click time.
    pub action_identity: String,
    pub title: String,
    pub minimized: bool,
    pub desktop_id: Option<String>,
    pub on_current_workspace: bool,
    pub frame_excluded: bool,
    // Read-only classification hint, never an injection authorization.
    pub frame_candidate: String,
}

#[link(name = "user32")]
extern "system" {
    fn EnumWindows(callback: unsafe extern "system" fn(isize, isize) -> i32, lparam: isize) -> i32;
    fn IsWindow(hwnd: isize) -> i32;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn IsIconic(hwnd: isize) -> i32;
    fn GetWindow(hwnd: isize, command: u32) -> isize;
    fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
    fn GetWindowTextLengthW(hwnd: isize) -> i32;
    fn GetClassNameW(hwnd: isize, class_name: *mut u16, capacity: i32) -> i32;
    fn GetWindowTextW(hwnd: isize, text: *mut u16, max_count: i32) -> i32;
    fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
    fn ShowWindow(hwnd: isize, command: i32) -> i32;
    fn SetForegroundWindow(hwnd: isize) -> i32;
    fn PostMessageW(hwnd: isize, message: u32, wparam: usize, lparam: isize) -> i32;
}

#[link(name = "dwmapi")]
extern "system" {
    fn DwmGetWindowAttribute(hwnd: isize, attribute: u32, value: *mut c_void, size: u32) -> i32;
}

// Workspace undo must still find an app window after Windows cloaks it on
// another virtual desktop. Reject shell/tool/owned windows without requiring
// visibility or a current-desktop title.
pub(crate) fn is_workspace_safe_window(hwnd: isize) -> bool {
    unsafe {
        if hwnd == 0
            || IsWindow(hwnd) == 0
            || GetWindow(hwnd, GW_OWNER) != 0
            || GetWindowLongPtrW(hwnd, GWL_EXSTYLE) & WS_EX_TOOLWINDOW != 0
        {
            return false;
        }
        let mut pid = 0u32;
        if GetWindowThreadProcessId(hwnd, &mut pid) == 0 || pid == 0 || pid == std::process::id() {
            return false;
        }
        let mut class_name = [0u16; 256];
        let length = GetClassNameW(hwnd, class_name.as_mut_ptr(), class_name.len() as i32);
        length > 0
            && !is_shell_surface_class(&String::from_utf16_lossy(&class_name[..length as usize]))
    }
}

unsafe fn eligible(hwnd: isize) -> bool {
    if !is_workspace_safe_window(hwnd) || IsWindowVisible(hwnd) == 0 {
        return false;
    }
    let mut cloaked = 0i32;
    if DwmGetWindowAttribute(
        hwnd,
        DWMWA_CLOAKED,
        &mut cloaked as *mut i32 as *mut c_void,
        size_of::<i32>() as u32,
    ) == 0
        && cloaked != 0
        && cloaked & DWM_CLOAKED_SHELL == 0
    {
        return false;
    }
    GetWindowTextLengthW(hwnd) > 0
}

// Shared guard for Activities actions and workspace moves. Re-evaluate the
// current HWND instead of trusting a stale handle supplied by the UI.
pub(crate) fn is_actionable_window(hwnd: isize) -> bool {
    unsafe { hwnd != 0 && IsWindow(hwnd) != 0 && eligible(hwnd) }
}

unsafe extern "system" fn enum_callback(hwnd: isize, lparam: isize) -> i32 {
    let windows = unsafe { &mut *(lparam as *mut Vec<WindowEntry>) };
    unsafe {
        if !eligible(hwnd) {
            return 1;
        }
        let len = GetWindowTextLengthW(hwnd);
        if len <= 0 {
            return 1;
        }
        let mut buffer = vec![0u16; len as usize + 1];
        let written = GetWindowTextW(hwnd, buffer.as_mut_ptr(), buffer.len() as i32);
        if written <= 0 {
            return 1;
        }
        let title = String::from_utf16_lossy(&buffer[..written as usize])
            .trim()
            .to_string();
        if title.is_empty() {
            return 1;
        }
        // Reject windows whose process lifetime is unreadable or changes mid-enumeration.
        let mut pid = 0u32;
        if GetWindowThreadProcessId(hwnd, &mut pid) == 0 || pid == 0 {
            return 1;
        }
        let Some(created) = super::frame_recovery::process_creation_time(pid) else {
            return 1;
        };
        if !same_action_target(hwnd, pid, created) {
            return 1;
        }
        windows.push(WindowEntry {
            handle: hwnd.to_string(),
            action_identity: format!("{pid}:{created}"),
            title,
            minimized: IsIconic(hwnd) != 0,
            desktop_id: None,
            on_current_workspace: true,
            frame_excluded: false,
            frame_candidate: "unknown".into(),
        });
    }
    1
}

pub fn list() -> Result<Vec<WindowEntry>, String> {
    let mut windows: Vec<WindowEntry> = Vec::new();
    let ok = unsafe {
        EnumWindows(
            enum_callback,
            &mut windows as *mut Vec<WindowEntry> as isize,
        )
    };
    if ok == 0 {
        return Err("EnumWindows failed".into());
    }

    // Avoid displaying/enriching a handle recycled after EnumWindows.
    windows.retain(|window| {
        let (Some((pid, created)), Ok(hwnd)) = (
            parse_action_identity(&window.action_identity),
            window.handle.parse::<isize>(),
        ) else {
            return false;
        };
        same_action_target(hwnd, pid, created)
    });

    for window in &mut windows {
        let Ok(hwnd) = window.handle.parse::<isize>() else {
            continue;
        };
        if let Ok(workspace) = super::virtual_desktop::window_info(hwnd) {
            window.desktop_id = Some(workspace.desktop_id);
            window.on_current_workspace = workspace.on_current_workspace;
        }
        window.frame_excluded = super::frame::is_app_excluded(&window.handle).unwrap_or(true);
        window.frame_candidate = super::frame::frame_candidate_status(&window.handle).into();
    }

    Ok(windows)
}

// The UI must return the enumerated process lifetime, not one discovered
// at click time. Decimal text avoids JavaScript's 53-bit integer limit.
// This is a best-effort pre-dispatch guard, not an atomic Win32 HWND lease.
fn parse_action_identity(identity: &str) -> Option<(u32, u64)> {
    let (pid, created) = identity.split_once(':')?;
    let pid = pid.parse::<u32>().ok()?;
    let created = created.parse::<u64>().ok()?;
    (pid != 0 && created != 0).then_some((pid, created))
}

fn action_identity_matches(
    expected_pid: u32,
    expected_created: u64,
    observed_pid: u32,
    observed_created: Option<u64>,
) -> bool {
    expected_pid != 0
        && expected_created != 0
        && expected_pid == observed_pid
        && observed_created == Some(expected_created)
}

fn same_action_target(hwnd: isize, pid: u32, created: u64) -> bool {
    unsafe {
        if !is_actionable_window(hwnd) {
            return false;
        }
        let mut current_pid = 0u32;
        if GetWindowThreadProcessId(hwnd, &mut current_pid) == 0 {
            return false;
        }
        action_identity_matches(
            pid,
            created,
            current_pid,
            super::frame_recovery::process_creation_time(current_pid),
        )
    }
}

fn action_target_identity(hwnd: isize, identity: &str) -> Result<(u32, u64), String> {
    let (pid, created) =
        parse_action_identity(identity).ok_or("invalid Activities window identity")?;
    if !same_action_target(hwnd, pid, created) {
        return Err("window changed since Activities snapshot".into());
    }
    Ok((pid, created))
}

pub fn activate(handle: &str, identity: &str) -> Result<(), String> {
    let hwnd = handle
        .parse::<isize>()
        .map_err(|_| "invalid window handle")?;
    let (pid, created) = action_target_identity(hwnd, identity)?;
    unsafe {
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        if !same_action_target(hwnd, pid, created) {
            return Err("window changed before activation".into());
        }
        if SetForegroundWindow(hwnd) == 0 {
            return Err("Windows refused to foreground the requested window".into());
        }
    }
    Ok(())
}

pub fn close(handle: &str, identity: &str) -> Result<(), String> {
    let hwnd = handle
        .parse::<isize>()
        .map_err(|_| "invalid window handle")?;
    let (pid, created) = action_target_identity(hwnd, identity)?;
    unsafe {
        if !same_action_target(hwnd, pid, created) {
            return Err("window changed before closing".into());
        }
        if PostMessageW(hwnd, WM_CLOSE, 0, 0) == 0 {
            return Err("Windows refused to close the requested window".into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{action_identity_matches, is_shell_surface_class, parse_action_identity};

    #[test]
    fn shell_taskbars_and_desktop_surfaces_are_not_activities_app_windows() {
        for class_name in [
            "Shell_TrayWnd",
            "Shell_SecondaryTrayWnd",
            "Progman",
            "WorkerW",
            "NotifyIconOverflowWindow",
            "shell_traywnd",
        ] {
            assert!(is_shell_surface_class(class_name), "{class_name}");
        }
        for class_name in ["CabinetWClass", "Chrome_WidgetWin_1", "Notepad", ""] {
            assert!(!is_shell_surface_class(class_name), "{class_name}");
        }
    }

    #[test]
    fn activities_snapshot_identity_rejects_missing_or_malformed_lifetimes() {
        assert_eq!(
            parse_action_identity("42:18446744073709551615"),
            Some((42, u64::MAX))
        );
        for invalid in [
            "", "42", "0:100", "42:0", "x:100", "42:x", "42:100:1", "-1:100",
        ] {
            assert_eq!(parse_action_identity(invalid), None, "accepted {invalid}");
        }
    }

    #[test]
    fn activities_actions_reject_recycled_hwnd_process_identity() {
        assert!(action_identity_matches(42, 100, 42, Some(100)));
        assert!(!action_identity_matches(42, 100, 43, Some(100)));
        assert!(!action_identity_matches(42, 100, 42, Some(101)));
        assert!(!action_identity_matches(42, 100, 42, None));
        assert!(!action_identity_matches(0, 100, 0, Some(100)));
        assert!(!action_identity_matches(42, 0, 42, Some(0)));
    }
}
