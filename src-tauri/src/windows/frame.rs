use crate::shell::{AppearanceState, ThemeMode};
use std::ffi::c_void;
use std::collections::HashMap;
use std::mem::size_of;
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

const GWL_STYLE: i32 = -16;
const GWL_EXSTYLE: i32 = -20;
const WS_CAPTION: isize = 0x00C0_0000;
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
    fn IsWindow(hwnd: isize) -> i32;
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
    dark: Option<i32>,
    caption: Option<u32>,
    text: Option<u32>,
}

// FedoraWin may only restore state it actually observed and changed. Save
// process identity alongside HWND to avoid restoring a recycled handle.
#[derive(Clone, Copy)]
struct OriginalFrame {
    pid: u32,
    dark: Option<i32>,
    corner: Option<i32>,
    border: Option<u32>,
    caption: Option<u32>,
    text: Option<u32>,
}

static ORIGINAL_FRAMES: OnceLock<Mutex<HashMap<isize, OriginalFrame>>> = OnceLock::new();

fn originals() -> &'static Mutex<HashMap<isize, OriginalFrame>> {
    ORIGINAL_FRAMES.get_or_init(|| Mutex::new(HashMap::new()))
}

unsafe fn read_attr<T: Copy + Default>(hwnd: isize, attribute: u32) -> Option<T> {
    let mut value = T::default();
    (DwmGetWindowAttribute(
        hwnd,
        attribute,
        (&mut value as *mut T).cast(),
        size_of::<T>() as u32,
    ) == 0)
        .then_some(value)
}

unsafe fn snapshot_frame(hwnd: isize, pid: u32) -> OriginalFrame {
    OriginalFrame {
        pid,
        dark: read_attr(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE),
        corner: read_attr(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE),
        border: read_attr(hwnd, DWMWA_BORDER_COLOR),
        caption: read_attr(hwnd, DWMWA_CAPTION_COLOR),
        text: read_attr(hwnd, DWMWA_TEXT_COLOR),
    }
}

unsafe fn restore_frame(hwnd: isize, original: OriginalFrame) {
    if IsWindow(hwnd) == 0 || window_pid(hwnd) != original.pid {
        return;
    }
    if let Some(value) = original.dark {
        set_attr(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &value);
    }
    if let Some(value) = original.corner {
        set_attr(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &value);
    }
    if let Some(value) = original.border {
        set_attr(hwnd, DWMWA_BORDER_COLOR, &value);
    }
    if let Some(value) = original.caption {
        set_attr(hwnd, DWMWA_CAPTION_COLOR, &value);
    }
    if let Some(value) = original.text {
        set_attr(hwnd, DWMWA_TEXT_COLOR, &value);
    }
}

unsafe fn window_pid(hwnd: isize) -> u32 {
    let mut pid = 0u32;
    GetWindowThreadProcessId(hwnd, &mut pid);
    pid
}

fn colorref(r: u8, g: u8, b: u8) -> u32 {
    (r as u32) | ((g as u32) << 8) | ((b as u32) << 16)
}

fn palette(appearance: &AppearanceState) -> FramePalette {
    match appearance.theme {
        ThemeMode::Dark => FramePalette {
            dark: Some(1),
            caption: Some(colorref(46, 46, 50)),
            text: Some(colorref(255, 255, 255)),
        },
        ThemeMode::Light => FramePalette {
            dark: Some(0),
            caption: Some(colorref(255, 255, 255)),
            text: Some(colorref(32, 32, 34)),
        },
        // System means Windows remains authoritative for light/dark. FedoraWin still
        // applies the GNOME-like corner/border treatment, but must not silently turn
        // every real application frame dark when the OS is using its light theme.
        ThemeMode::System => FramePalette {
            dark: None,
            caption: None,
            text: None,
        },
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

unsafe fn set_optional_color(hwnd: isize, attribute: u32, value: Option<u32>) {
    let value = value.unwrap_or(0xFFFF_FFFFu32);
    set_attr(hwnd, attribute, &value);
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
    // Leave Electron, UWP and other self-drawn/borderless titlebars alone.
    // Third-party processes must keep ownership of custom frame hit testing.
    if GetWindowLongPtrW(hwnd, GWL_STYLE) & WS_CAPTION != WS_CAPTION {
        return false;
    }
    let pid = window_pid(hwnd);
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
            let pid = window_pid(hwnd);
            let mut journal = originals().lock().unwrap_or_else(|error| error.into_inner());
            if journal.get(&hwnd).is_none_or(|original| original.pid != pid) {
                journal.insert(hwnd, snapshot_frame(hwnd, pid));
            }
            let original = journal[&hwnd];
            // Never mutate an attribute we cannot read back for restoration.
            if let (Some(dark), Some(_)) = (ctx.palette.dark, original.dark) {
                set_attr(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark);
            }
            if original.corner.is_some() {
                set_attr(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &DWMWCP_ROUND);
            }
            if original.caption.is_some() && ctx.palette.caption.is_some() {
                set_optional_color(hwnd, DWMWA_CAPTION_COLOR, ctx.palette.caption);
            }
            if original.text.is_some() && ctx.palette.text.is_some() {
                set_optional_color(hwnd, DWMWA_TEXT_COLOR, ctx.palette.text);
            }
            if original.border.is_some() {
                set_attr(hwnd, DWMWA_BORDER_COLOR, &DWMWA_COLOR_NONE);
            }
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
    let mut journal = originals().lock().unwrap_or_else(|error| error.into_inner());
    for (hwnd, original) in journal.drain() {
        unsafe { restore_frame(hwnd, original) };
    }
}

pub fn start_frame_watcher(state: Arc<crate::shell::ShellState>) {
    thread::spawn(move || loop {
        let appearance = state.snapshot().appearance;
        let _ = apply_to_top_level_windows(&appearance);
        thread::sleep(Duration::from_millis(750));
    });
}

#[cfg(test)]
mod tests {
    use super::palette;
    use crate::shell::{AppearanceState, ThemeMode};

    fn appearance(theme: ThemeMode) -> AppearanceState {
        AppearanceState {
            theme,
            ..Default::default()
        }
    }

    #[test]
    fn system_theme_does_not_force_non_client_colors() {
        let palette = palette(&appearance(ThemeMode::System));
        assert_eq!(palette.dark, None);
        assert_eq!(palette.caption, None);
        assert_eq!(palette.text, None);
    }

    #[test]
    fn explicit_themes_still_drive_native_frames() {
        let dark = palette(&appearance(ThemeMode::Dark));
        let light = palette(&appearance(ThemeMode::Light));
        assert_eq!(dark.dark, Some(1));
        assert_eq!(light.dark, Some(0));
        assert!(dark.caption.is_some());
        assert!(light.caption.is_some());
    }
}
