use std::ffi::c_void;
use std::mem::size_of;

const ABM_NEW: u32 = 0x00000000;
const ABM_REMOVE: u32 = 0x00000001;
const ABM_QUERYPOS: u32 = 0x00000002;
const ABM_SETPOS: u32 = 0x00000003;
const ABE_TOP: u32 = 1;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
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
}

pub fn reserve_top(hwnd: isize) -> Result<(), String> {
    if hwnd == 0 {
        return Err("invalid panel HWND".into());
    }

    let mut rect = Rect::default();
    let ok = unsafe { GetWindowRect(hwnd, &mut rect) };
    if ok == 0 {
        return Err("GetWindowRect failed for panel".into());
    }

    // GetWindowRect is in physical pixels, so reserving the panel's real HWND height
    // remains correct at 125/150/200% display scaling.
    let height_px = (rect.bottom - rect.top).max(1);
    let mut data = AppBarData {
        cb_size: size_of::<AppBarData>() as u32,
        hwnd,
        callback_message: 0,
        edge: ABE_TOP,
        rect: Rect { left: rect.left, top: rect.top, right: rect.right, bottom: rect.top + height_px },
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
    if hwnd == 0 { return; }
    let mut data = AppBarData {
        cb_size: size_of::<AppBarData>() as u32,
        hwnd,
        callback_message: 0,
        edge: ABE_TOP,
        rect: Rect::default(),
        lparam: 0,
    };
    unsafe { SHAppBarMessage(ABM_REMOVE, &mut data); }
}

#[allow(dead_code)]
fn _keep_c_void(_: *const c_void) {}
