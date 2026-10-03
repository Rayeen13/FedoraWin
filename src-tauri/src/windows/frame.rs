use crate::shell::{AppearanceState, ThemeMode};
use crate::windows::frame_recovery::{self, Snapshot};
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::mem::size_of;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

const GWL_STYLE: i32 = -16;
const GWL_EXSTYLE: i32 = -20;
const WS_CAPTION: isize = 0x00C0_0000;
const WS_EX_TOOLWINDOW: isize = 0x00000080;
const WS_EX_NOACTIVATE: isize = 0x08000000;
const WS_EX_LAYERED: isize = 0x00080000;
const WS_EX_NOREDIRECTIONBITMAP: isize = 0x00200000;
const GW_OWNER: u32 = 4;
const DWMWA_CLOAKED: u32 = 14;
const DWMWA_USE_IMMERSIVE_DARK_MODE: u32 = 20;
const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
const DWMWA_BORDER_COLOR: u32 = 34;
const DWMWA_CAPTION_COLOR: u32 = 35;
const DWMWA_TEXT_COLOR: u32 = 36;
const DWMWCP_DONOTROUND: i32 = 1;
const DWMWCP_ROUND: i32 = 2;
const DWMWA_COLOR_NONE: u32 = 0xFFFF_FFFE;

#[link(name = "user32")]
extern "system" {
    fn EnumWindows(callback: unsafe extern "system" fn(isize, isize) -> i32, lparam: isize) -> i32;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn IsWindow(hwnd: isize) -> i32;
    fn IsIconic(hwnd: isize) -> i32;
    fn GetWindow(hwnd: isize, command: u32) -> isize;
    fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
    fn GetClassNameW(hwnd: isize, class_name: *mut u16, capacity: i32) -> i32;
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
    created: u64,
    dark: Option<i32>,
    corner: Option<i32>,
    border: Option<u32>,
    caption: Option<u32>,
    text: Option<u32>,
    // Track only attributes FedoraWin actually changed successfully.
    changed_dark: bool,
    changed_corner: bool,
    changed_border: bool,
    changed_caption: bool,
    changed_text: bool,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
struct WindowIdentity {
    hwnd: isize,
    pid: u32,
    created: u64,
}

static FRAME_WATCHER_ENABLED: AtomicBool = AtomicBool::new(false);
static ORIGINAL_FRAMES: OnceLock<Mutex<HashMap<isize, OriginalFrame>>> = OnceLock::new();
static FRAME_EXCLUSIONS: OnceLock<Mutex<HashSet<WindowIdentity>>> = OnceLock::new();

fn originals() -> &'static Mutex<HashMap<isize, OriginalFrame>> {
    ORIGINAL_FRAMES.get_or_init(|| Mutex::new(HashMap::new()))
}

fn exclusions() -> &'static Mutex<HashSet<WindowIdentity>> {
    FRAME_EXCLUSIONS.get_or_init(|| Mutex::new(HashSet::new()))
}

fn window_identity(hwnd: isize) -> Option<WindowIdentity> {
    if hwnd == 0 || unsafe { IsWindow(hwnd) } == 0 {
        return None;
    }
    let pid = unsafe { window_pid(hwnd) };
    if pid == 0 {
        return None;
    }
    let created = frame_recovery::process_creation_time(pid)?;
    Some(WindowIdentity { hwnd, pid, created })
}

fn identity_is_excluded(identity: WindowIdentity) -> bool {
    exclusions()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .contains(&identity)
}

pub fn is_window_excluded(handle: &str) -> Result<bool, String> {
    let hwnd = handle
        .parse::<isize>()
        .map_err(|_| "invalid window handle".to_string())?;
    let identity =
        window_identity(hwnd).ok_or_else(|| "window is no longer available".to_string())?;
    Ok(identity_is_excluded(identity))
}

pub fn exclude_window(handle: &str) -> Result<bool, String> {
    let hwnd = handle
        .parse::<isize>()
        .map_err(|_| "invalid window handle".to_string())?;
    let identity =
        window_identity(hwnd).ok_or_else(|| "window is no longer available".to_string())?;

    let inserted = exclusions()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(identity);

    let mut journal = originals()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    if let Some(original) = journal.get(&hwnd).copied() {
        if original.pid == identity.pid && original.created == identity.created {
            journal.remove(&hwnd);
            unsafe { restore_frame(hwnd, original) };
            persist_originals(&journal)?;
        }
    }
    Ok(inserted)
}

pub fn include_window(handle: &str) -> Result<bool, String> {
    let hwnd = handle
        .parse::<isize>()
        .map_err(|_| "invalid window handle".to_string())?;
    let identity =
        window_identity(hwnd).ok_or_else(|| "window is no longer available".to_string())?;
    Ok(exclusions()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .remove(&identity))
}

fn persist_originals(journal: &HashMap<isize, OriginalFrame>) -> Result<(), String> {
    let frames: Vec<Snapshot> = journal
        .iter()
        .map(|(&hwnd, original)| Snapshot {
            hwnd,
            pid: original.pid,
            created: original.created,
            dark: original.dark,
            corner: original.corner,
            border: original.border,
            caption: original.caption,
            text: original.text,
        })
        .collect();
    frame_recovery::persist(&frames)
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

unsafe fn snapshot_frame(hwnd: isize, pid: u32, created: u64) -> OriginalFrame {
    OriginalFrame {
        pid,
        created,
        dark: read_attr(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE),
        corner: read_attr(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE),
        border: read_attr(hwnd, DWMWA_BORDER_COLOR),
        caption: read_attr(hwnd, DWMWA_CAPTION_COLOR),
        text: read_attr(hwnd, DWMWA_TEXT_COLOR),
        changed_dark: false,
        changed_corner: false,
        changed_border: false,
        changed_caption: false,
        changed_text: false,
    }
}

unsafe fn restore_colors(hwnd: isize, original: &mut OriginalFrame) {
    if original.changed_dark {
        if let Some(value) = original.dark {
            set_attr(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &value);
        }
        original.changed_dark = false;
    }
    if original.changed_caption {
        if let Some(value) = original.caption {
            set_attr(hwnd, DWMWA_CAPTION_COLOR, &value);
        }
        original.changed_caption = false;
    }
    if original.changed_text {
        if let Some(value) = original.text {
            set_attr(hwnd, DWMWA_TEXT_COLOR, &value);
        }
        original.changed_text = false;
    }
}

unsafe fn restore_frame(hwnd: isize, mut original: OriginalFrame) {
    if IsWindow(hwnd) == 0
        || window_pid(hwnd) != original.pid
        || frame_recovery::process_creation_time(original.pid) != Some(original.created)
    {
        return;
    }
    restore_colors(hwnd, &mut original);
    if original.changed_corner {
        if let Some(value) = original.corner {
            // An application can switch to a self-drawn frame mid-session.
            // Respect a new DO_NOT_ROUND request instead of undoing its opt-out.
            if read_attr::<i32>(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE) == Some(DWMWCP_ROUND) {
                set_attr(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &value);
            }
        }
    }
    if original.changed_border {
        if let Some(value) = original.border {
            // The target app may have changed its own border during our session.
            // Restore only when the current value is still FedoraWin's sentinel.
            if read_attr::<u32>(hwnd, DWMWA_BORDER_COLOR) == Some(DWMWA_COLOR_NONE) {
                set_attr(hwnd, DWMWA_BORDER_COLOR, &value);
            }
        }
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
        // System means Windows remains authoritative for light/dark and caption colors.
        ThemeMode::System => FramePalette {
            dark: None,
            caption: None,
            text: None,
        },
    }
}

unsafe fn set_attr<T>(hwnd: isize, attribute: u32, value: &T) -> bool {
    DwmSetWindowAttribute(
        hwnd,
        attribute,
        value as *const T as *const c_void,
        size_of::<T>() as u32,
    ) == 0
}

// A Win32 WS_CAPTION bit is not evidence that the application actually
// paints standard non-client chrome: Chromium and other frameworks may keep
// this bit while rendering their own draggable region and controls. These
// classes are a conservative veto, not an exhaustive "custom titlebar" test.
//
// Keep the family label alongside the prefix so the future opt-in frame engine
// can explain why a window was rejected instead of exposing only a boolean.
const SELF_DRAWN_CLASS_PREFIXES: [(&str, &str); 11] = [
    ("chrome_widgetwin", "Chromium custom chrome"),
    ("mozillawindowclass", "Firefox custom chrome"),
    ("gdk", "GDK/GTK custom chrome"),
    ("gtk", "GTK custom chrome"),
    ("qt", "Qt custom chrome"),
    ("sdl", "SDL custom chrome"),
    ("glfw", "GLFW custom chrome"),
    (
        "cascadia_hosting_window_class",
        "Windows Terminal custom chrome",
    ),
    ("winuidesktopwin32windowclass", "WinUI custom chrome"),
    ("applicationframewindow", "Windows application frame"),
    ("windows.ui.core.corewindow", "Windows CoreWindow"),
];

fn self_drawn_chrome_family(class_name: &str) -> Option<&'static str> {
    let name = class_name.to_ascii_lowercase();
    SELF_DRAWN_CLASS_PREFIXES
        .iter()
        .find_map(|(prefix, family)| name.starts_with(prefix).then_some(*family))
}

#[cfg(test)]
fn has_known_self_drawn_chrome(class_name: &str) -> bool {
    self_drawn_chrome_family(class_name).is_some()
}

unsafe fn owns_standard_caption(hwnd: isize) -> bool {
    let mut class_name = [0u16; 256];
    let length = GetClassNameW(hwnd, class_name.as_mut_ptr(), class_name.len() as i32);
    if length <= 0 {
        return false; // Unknown window class: fail closed.
    }
    let name = String::from_utf16_lossy(&class_name[..length as usize]);
    if self_drawn_chrome_family(&name).is_some() {
        return false;
    }
    // Respect explicit app DWM opt-out, even if WS_CAPTION remains set.
    if read_attr::<i32>(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE) == Some(DWMWCP_DONOTROUND) {
        return false;
    }
    true
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
    if exstyle & (WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE | WS_EX_LAYERED | WS_EX_NOREDIRECTIONBITMAP)
        != 0
    {
        return false;
    }
    // Leave custom/borderless titlebars and their hit testing to the owning application.
    if GetWindowLongPtrW(hwnd, GWL_STYLE) & WS_CAPTION != WS_CAPTION {
        return false;
    }
    if !owns_standard_caption(hwnd) {
        return false;
    }
    let pid = window_pid(hwnd);
    if pid == 0 || pid == std::process::id() {
        return false;
    }
    let Some(identity) = window_identity(hwnd) else {
        return false;
    };
    if identity_is_excluded(identity) {
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

unsafe extern "system" fn apply_callback(hwnd: isize, lparam: isize) -> i32 {
    let ctx = unsafe { &mut *(lparam as *mut EnumContext) };
    unsafe {
        let is_eligible = eligible(hwnd);
        let pid = window_pid(hwnd);
        let mut journal = originals()
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        // Recheck under the journal lock: a reset must close this gate even mid-scan.
        if !FRAME_WATCHER_ENABLED.load(Ordering::SeqCst) {
            return 0;
        }
        if !is_eligible {
            if let Some(original) = journal.remove(&hwnd) {
                restore_frame(hwnd, original);
                let _ = persist_originals(&journal);
            }
            return 1;
        }

        let Some(created) = frame_recovery::process_creation_time(pid) else {
            return 1;
        };
        if journal
            .get(&hwnd)
            .is_none_or(|original| original.pid != pid || original.created != created)
        {
            journal.insert(hwnd, snapshot_frame(hwnd, pid, created));
            // Persist original DWM values before applying any changes to a foreign HWND.
            if persist_originals(&journal).is_err() {
                journal.remove(&hwnd);
                return 1;
            }
        }
        // Never replace Win32 caption buttons/hit-testing or fake a titlebar.
        // Only successfully written DWM attributes enter the in-memory recovery flags.
        let original = journal.get_mut(&hwnd).expect("frame journal entry");
        if let Some(dark) = ctx.palette.dark {
            if original.dark.is_some() {
                original.changed_dark |= set_attr(hwnd, DWMWA_USE_IMMERSIVE_DARK_MODE, &dark);
            }
        } else {
            restore_colors(hwnd, original);
        }
        if original.corner.is_some() {
            original.changed_corner |=
                set_attr(hwnd, DWMWA_WINDOW_CORNER_PREFERENCE, &DWMWCP_ROUND);
        }
        if let Some(caption) = ctx.palette.caption {
            if original.caption.is_some() {
                original.changed_caption |= set_attr(hwnd, DWMWA_CAPTION_COLOR, &caption);
            }
        }
        if let Some(text) = ctx.palette.text {
            if original.text.is_some() {
                original.changed_text |= set_attr(hwnd, DWMWA_TEXT_COLOR, &text);
            }
        }
        if original.border.is_some() {
            original.changed_border |= set_attr(hwnd, DWMWA_BORDER_COLOR, &DWMWA_COLOR_NONE);
        }
        ctx.count += 1;
    }
    1
}

pub fn apply_to_top_level_windows(appearance: &AppearanceState) -> Result<usize, String> {
    if !FRAME_WATCHER_ENABLED.load(Ordering::SeqCst) {
        return Err("frame styling has been shut down".into());
    }
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
    FRAME_WATCHER_ENABLED.store(false, Ordering::SeqCst);
    exclusions()
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .clear();
    let mut journal = originals()
        .lock()
        .unwrap_or_else(|error| error.into_inner());
    for (hwnd, original) in journal.drain() {
        unsafe { restore_frame(hwnd, original) };
    }
    frame_recovery::remove_journal();
}

pub fn start_frame_watcher(state: Arc<crate::shell::ShellState>) -> Result<(), String> {
    frame_recovery::start_guardian()?;
    FRAME_WATCHER_ENABLED.store(true, Ordering::SeqCst);
    thread::spawn(move || loop {
        if !FRAME_WATCHER_ENABLED.load(Ordering::SeqCst) {
            break;
        }
        let appearance = state.snapshot().appearance;
        let _ = apply_to_top_level_windows(&appearance);
        thread::sleep(Duration::from_millis(750));
    });
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        exclusions, has_known_self_drawn_chrome, identity_is_excluded, palette,
        self_drawn_chrome_family, WindowIdentity,
    };
    use crate::shell::{AppearanceState, ThemeMode};

    fn appearance(theme: ThemeMode) -> AppearanceState {
        AppearanceState {
            theme,
            ..Default::default()
        }
    }

    #[test]
    fn known_self_drawn_window_classes_are_not_restyled() {
        for (class, expected_family) in [
            ("Chrome_WidgetWin_1", "Chromium custom chrome"),
            ("MozillaWindowClass", "Firefox custom chrome"),
            ("gdkWin32Window", "GDK/GTK custom chrome"),
            ("GtkWindow", "GTK custom chrome"),
            ("Qt661QWindowIcon", "Qt custom chrome"),
            ("SDL_app", "SDL custom chrome"),
            ("GLFW30", "GLFW custom chrome"),
            (
                "CASCADIA_HOSTING_WINDOW_CLASS",
                "Windows Terminal custom chrome",
            ),
            ("WinUIDesktopWin32WindowClass", "WinUI custom chrome"),
            ("ApplicationFrameWindow", "Windows application frame"),
            ("Windows.UI.Core.CoreWindow", "Windows CoreWindow"),
        ] {
            assert!(has_known_self_drawn_chrome(class), "{class}");
            assert_eq!(
                self_drawn_chrome_family(class),
                Some(expected_family),
                "{class}"
            );
        }
        for class in ["WindowsForms10.Window.8.app.0.1234", "#32770", "Notepad"] {
            assert!(!has_known_self_drawn_chrome(class), "{class}");
            assert_eq!(self_drawn_chrome_family(class), None, "{class}");
        }
    }

    #[test]
    fn per_window_exclusion_is_bound_to_process_lifetime() {
        let first = WindowIdentity {
            hwnd: 42,
            pid: 100,
            created: 1000,
        };
        let recycled = WindowIdentity {
            hwnd: 42,
            pid: 200,
            created: 2000,
        };
        {
            let mut entries = exclusions()
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            entries.clear();
            entries.insert(first);
        }
        assert!(identity_is_excluded(first));
        assert!(!identity_is_excluded(recycled));
        exclusions()
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .clear();
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
