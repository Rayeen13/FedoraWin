use std::{ffi::c_void, mem::size_of};

const ABM_NEW: u32 = 0x00000000;
const ABM_REMOVE: u32 = 0x00000001;
const ABM_QUERYPOS: u32 = 0x00000002;
const ABM_SETPOS: u32 = 0x00000003;
const ABE_TOP: u32 = 1;
const MONITOR_DEFAULTTONEAREST: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct MonitorInfo {
    cb_size: u32,
    monitor: Rect,
    work: Rect,
    flags: u32,
}

#[repr(C)]
struct AppBarData {
    cb_size: u32,
    hwnd: isize,
    callback_message: u32,
    edge: u32,
    rect: Rect,
    lparam: isize,
}

#[link(name = "shell32")]
extern "system" {
    fn SHAppBarMessage(message: u32, data: *mut AppBarData) -> usize;
}

#[link(name = "user32")]
extern "system" {
    fn GetWindowRect(hwnd: isize, rect: *mut Rect) -> i32;
    fn MonitorFromWindow(hwnd: isize, flags: u32) -> isize;
    fn GetMonitorInfoW(monitor: isize, info: *mut c_void) -> i32;
}

pub fn reserve_top(hwnd: isize) -> Result<(), String> {
    if hwnd == 0 {
        return Err("invalid panel HWND".into());
    }

    let mut panel_rect = Rect::default();
    if unsafe { GetWindowRect(hwnd, &mut panel_rect) } == 0 {
        return Err("GetWindowRect failed for panel".into());
    }

    let monitor = unsafe { MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST) };
    if monitor == 0 {
        return Err("MonitorFromWindow failed for panel".into());
    }

    let mut monitor_info = MonitorInfo {
        cb_size: size_of::<MonitorInfo>() as u32,
        monitor: Rect::default(),
        work: Rect::default(),
        flags: 0,
    };
    if unsafe { GetMonitorInfoW(monitor, (&mut monitor_info as *mut MonitorInfo).cast()) } == 0 {
        return Err("GetMonitorInfoW failed for panel".into());
    }

    // AppBar coordinates are physical desktop coordinates. Use the monitor that
    // actually owns the panel so negative origins, portrait screens and mixed-DPI
    // topologies reserve only the intended display edge.
    let height_px = (panel_rect.bottom - panel_rect.top).max(1);
    let mut data = AppBarData {
        cb_size: size_of::<AppBarData>() as u32,
        hwnd,
        callback_message: 0,
        edge: ABE_TOP,
        rect: Rect {
            left: monitor_info.monitor.left,
            top: monitor_info.monitor.top,
            right: monitor_info.monitor.right,
            bottom: monitor_info.monitor.top + height_px,
        },
        lparam: 0,
    };

    unsafe {
        SHAppBarMessage(ABM_NEW, &mut data);
        SHAppBarMessage(ABM_QUERYPOS, &mut data);
        data.rect.bottom = data.rect.top + height_px;
        if SHAppBarMessage(ABM_SETPOS, &mut data) == 0 {
            return Err("SHAppBarMessage(ABM_SETPOS) failed".into());
        }
    }
    Ok(())
}

pub fn release(hwnd: isize) {
    if hwnd == 0 {
        return;
    }
    let mut data = AppBarData {
        cb_size: size_of::<AppBarData>() as u32,
        hwnd,
        callback_message: 0,
        edge: ABE_TOP,
        rect: Rect::default(),
        lparam: 0,
    };
    unsafe {
        SHAppBarMessage(ABM_REMOVE, &mut data);
    }
}
