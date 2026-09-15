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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowEntry {
    pub handle: String,
    pub title: String,
    pub minimized: bool,
    pub desktop_id: Option<String>,
    pub on_current_workspace: bool,
}

#[link(name = "user32")]
extern "system" {
    fn EnumWindows(callback: extern "system" fn(isize, isize) -> i32, lparam: isize) -> i32;
    fn IsWindow(hwnd: isize) -> i32;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn IsIconic(hwnd: isize) -> i32;
    fn GetWindow(hwnd: isize, command: u32) -> isize;
    fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
    fn GetWindowTextLengthW(hwnd: isize) -> i32;
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

unsafe fn eligible(hwnd: isize) -> bool {
    if hwnd == 0 || IsWindowVisible(hwnd) == 0 || GetWindow(hwnd, GW_OWNER) != 0 {
        return false;
    }
    if GetWindowLongPtrW(hwnd, GWL_EXSTYLE) & WS_EX_TOOLWINDOW != 0 {
        return false;
    }
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, &mut pid);
    if pid == 0 || pid == std::process::id() {
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

extern "system" fn enum_callback(hwnd: isize, lparam: isize) -> i32 {
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
        windows.push(WindowEntry {
            handle: hwnd.to_string(),
            title,
            minimized: IsIconic(hwnd) != 0,
            desktop_id: None,
            on_current_workspace: true,
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

    for window in &mut windows {
        let Ok(hwnd) = window.handle.parse::<isize>() else {
            continue;
        };
        if let Ok(workspace) = super::virtual_desktop::window_info(hwnd) {
            window.desktop_id = Some(workspace.desktop_id);
            window.on_current_workspace = workspace.on_current_workspace;
        }
    }

    Ok(windows)
}

pub fn activate(handle: &str) -> Result<(), String> {
    let hwnd = handle
        .parse::<isize>()
        .map_err(|_| "invalid window handle")?;
    unsafe {
        if hwnd == 0 || IsWindow(hwnd) == 0 {
            return Err("window is no longer available".into());
        }
        if IsIconic(hwnd) != 0 {
            ShowWindow(hwnd, SW_RESTORE);
        }
        if SetForegroundWindow(hwnd) == 0 {
            return Err("Windows refused to foreground the requested window".into());
        }
    }
    Ok(())
}

pub fn close(handle: &str) -> Result<(), String> {
    let hwnd = handle
        .parse::<isize>()
        .map_err(|_| "invalid window handle")?;
    unsafe {
        if hwnd == 0 || IsWindow(hwnd) == 0 || !eligible(hwnd) {
            return Err("window is no longer available".into());
        }
        if PostMessageW(hwnd, WM_CLOSE, 0, 0) == 0 {
            return Err("Windows refused to close the requested window".into());
        }
    }
    Ok(())
}
