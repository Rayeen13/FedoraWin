use crate::shell::{AppearanceState, ThemeMode};
use std::ffi::c_void;
use std::mem::size_of;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

const GWL_EXSTYLE: i32 = -20;
const WS_EX_TOOLWINDOW: isize = 0x00000080;
const WS_EX_NOACTIVATE: isize = 0x08000000;
const GW_OWNER: u32 = 4;
const DWMWA_CLOAKED: u32 = 14;
const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
const DWMWA_BORDER_COLOR: u32 = 34;
const DWMWA_CAPTION_COLOR: u32 = 35;
const DWMWA_TEXT_COLOR: u32 = 36;
const DWMWCP_ROUND: i32 = 2;
const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;

#[link(name = "user32")]
extern "system" {
    fn EnumWindows(callback: extern "system" fn(isize, isize) -> i32, lparam: isize) -> i32;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn IsIconic(hwnd: isize) -> i32;
    fn GetWindow(hwnd: isize, command: u32) -> isize;
    fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
    fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
}

#[link(name = "dwmapi")]
extern "system" {
    fn DwmSetWindowAttribute(hwnd: isize, attribute: u32, value: *const c_void, size: u32) -> i32;
    fn DwmGetWindowAttribute(hwnd: isize, attribute: u32, value: *mut c_void, size: u32) -> i32;
}

#[derive(Clone, Copy)]
struct FramePalette {
    dark: i32,
    caption: u32,
    text: u32,
}

fn colorref(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

fn palette(appearance: &AppearanceState) -> FramePalette {
    let dark = !matches!(appearance.theme, ThemeMode::Light);
    // Current libadwaita header bar roles: #2e2e32 in dark style and
    // white in light style. Windows owns the real caption buttons, so we
    // theme the supported non-client surface without replacing hit-testing.
    let caption = if dark {
        colorref(46, 46, 50)
    } else {
        colorref(255, 255, 255)
    };
    let text = if dark {
        colorref(255, 255, 255)
    } else {
        colorref(32, 32, 34)
    };
    FramePalette {
        dark: if dark { 1 } else { 0 },
        caption,
        text,
    }
}

unsafe fn set_attr<T>(hwnd: isize, attribute: u32, value: &T) {
    let _ = DwmSetWindowAttribute(
        hwnd,
        attribute,
        value as *const T as *const c_void,
        size_of::<T>() as u32,
    );
}

unsafe fn eligible(hwnd: isize) -> bool {
    if hwnd == 0
        || IsWindowVisible(hwnd) == 0
        || IsIconic(hwnd) != 0
        || GetWindow(hwnd, GW_OWNER) != 0
    {
        return false;
    }
    let exstyle = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
    if exstyle & WS_EX_TOOLWINDOW != 0 || exstyle & WS_EX_NOACTIVATE != 0 {
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
    {
        return false;
    }
    true
}

struct EnumContext {
    palette: FramePalette,
    count: usize,
}

extern "system" fn apply_callback(hwnd: isize, lparam: isize) -> i32 {
    let ctx = unsafe { &mut *(lparam as *mut EnumContext) };
    unsafe {
        if eligible(hwnd) {
            let corner = DWMWCP_ROUND;
            set_attr(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &ctx.palette.dark);
            set_attr(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &corner);
            set_attr(hwnd, DWMWA_CAPTION_COLOR, &ctx.palette.caption);
            set_attr(hwnd, DWMWA_TEXT_COLOR, &ctx.palette.text);
            // libadwaita does not paint an accent outline around every application window.
            // Keep the real Windows non-client frame, but suppress its colored border.
            set_attr(hwnd, DWMWA_BORDER_COLOR, &DWMWA_COLOR_NONE);
            ctx.count += 1;
        }
    }
    1
}

pub fn apply_to_top_level_windows(appearance: &AppearanceState) -> Result<usize, String> {
    let mut ctx = EnumContext {
        palette: palette(appearance),
        count: 0,
    };
    let ok = unsafe { EnumWindows(apply_callback, &mut ctx as *mut EnumContext as isize) };
    if ok == 0 {
        return Err("EnumWindows failed".into());
    }
    Ok(ctx.count)
}

pub fn reset_top_level_windows() {
    struct ResetContext;
    extern "system" fn reset_callback(hwnd: isize, _: isize) -> i32 {
        unsafe {
            if eligible(hwnd) {
                let default = 0xFFFF_FFFFu32;
                let dark = 0i32;
                let corner = 0i32;
                set_attr(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark);
                set_attr(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &corner);
                set_attr(hwnd, DWMWA_BORDER_COLOR, &default);
                set_attr(hwnd, DWMWA_CAPTION_COLOR, &default);
                set_attr(hwnd, DWMWA_TEXT_COLOR, &default);
            }
        }
        1
    }
    unsafe {
        EnumWindows(reset_callback, 0);
    }
    let _ = ResetContext;
    let _ = DWMWA_COLOR_NONE;
}

pub fn start_frame_watcher(state: Arc<crate::shell::ShellState>) {
    thread::spawn(move || loop {
        let appearance = state.snapshot().appearance;
        let _ = apply_to_top_level_windows(&appearance);
        thread::sleep(Duration::from_millis(750));
    });
}
