use crate::layout::PANEL_HEIGHT;
use crate::windows::display::DisplayInfo;
use std::mem::{size_of, zeroed};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::{mpsc, OnceLock};
use std::thread;
use std::time::Duration;

const WM_DESTROY: u32 = 0x0002;
const WM_PAINT: u32 = 0x000F;
const WM_CLOSE: u32 = 0x0010;
const WM_TIMER: u32 = 0x0113;
const WM_LBUTTONUP: u32 = 0x0202;

const WS_POPUP: u32 = 0x8000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_EX_TOPMOST: u32 = 0x0000_0008;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const WS_EX_NOACTIVATE: u32 = 0x0800_0000;

const SW_SHOW: i32 = 5;
const SWP_NOACTIVATE: u32 = 0x0010;
const SWP_SHOWWINDOW: u32 = 0x0040;
const HWND_TOPMOST: isize = -1;

const DT_CENTER: u32 = 0x0001;
const DT_RIGHT: u32 = 0x0002;
const DT_VCENTER: u32 = 0x0004;
const DT_SINGLELINE: u32 = 0x0020;
const TRANSPARENT: i32 = 1;
const DEFAULT_GUI_FONT: i32 = 17;
const NULL_PEN: i32 = 8;
const FONT_WEIGHT_NORMAL: i32 = 400;
const FONT_WEIGHT_SEMIBOLD: i32 = 600;
const DEFAULT_CHARSET: u32 = 1;
const CLEARTYPE_QUALITY: u32 = 5;

const TIMER_CLOCK: usize = 1;

static PANEL_HWND: AtomicIsize = AtomicIsize::new(0);
static ACTION_SENDER: OnceLock<mpsc::Sender<PanelAction>> = OnceLock::new();

#[derive(Clone, Copy)]
enum PanelAction {
    Activities,
    DateMenu,
    QuickSettings,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Point {
    x: i32,
    y: i32,
}

#[repr(C)]
#[derive(Clone, Copy)]
struct Msg {
    hwnd: isize,
    message: u32,
    wparam: usize,
    lparam: isize,
    time: u32,
    point: Point,
    private: u32,
}

#[repr(C)]
struct PaintStruct {
    hdc: isize,
    erase: i32,
    paint: Rect,
    restore: i32,
    inc_update: i32,
    reserved: [u8; 32],
}

#[repr(C)]
struct WndClassExW {
    size: u32,
    style: u32,
    wnd_proc: Option<unsafe extern "system" fn(isize, u32, usize, isize) -> isize>,
    class_extra: i32,
    window_extra: i32,
    instance: isize,
    icon: isize,
    cursor: isize,
    background: isize,
    menu_name: *const u16,
    class_name: *const u16,
    icon_small: isize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct SystemTime {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    milliseconds: u16,
}

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn colorref(red: u8, green: u8, blue: u8) -> u32 {
    red as u32 | ((green as u32) << 8) | ((blue as u32) << 16)
}

fn x_from_lparam(lparam: isize) -> i32 {
    (lparam as u32 & 0xffff) as i16 as i32
}

fn send_action(action: PanelAction) {
    if let Some(sender) = ACTION_SENDER.get() {
        let _ = sender.send(action);
    }
}

fn scale_to_panel(value: i32, panel_height: i32) -> i32 {
    ((value as f64) * (panel_height.max(1) as f64 / PANEL_HEIGHT)).round() as i32
}

fn clock_label() -> String {
    let mut time = SystemTime::default();
    unsafe { GetLocalTime(&mut time) };
    let month = match time.month {
        1 => "Jan",
        2 => "Feb",
        3 => "Mar",
        4 => "Apr",
        5 => "May",
        6 => "Jun",
        7 => "Jul",
        8 => "Aug",
        9 => "Sep",
        10 => "Oct",
        11 => "Nov",
        _ => "Dec",
    };
    format!("{month} {}  {:02}:{:02}", time.day, time.hour, time.minute)
}

unsafe fn create_font(face: &str, logical_height: i32, panel_height: i32, weight: i32) -> isize {
    let face = wide(face);
    CreateFontW(
        -scale_to_panel(logical_height, panel_height).max(1),
        0,
        0,
        0,
        weight,
        0,
        0,
        0,
        DEFAULT_CHARSET,
        0,
        0,
        CLEARTYPE_QUALITY,
        0,
        face.as_ptr(),
    )
}

unsafe fn draw_workspace_indicator(hdc: isize, panel_height: i32) {
    let white = CreateSolidBrush(colorref(245, 245, 247));
    let muted = CreateSolidBrush(colorref(142, 142, 148));
    let null_pen = GetStockObject(NULL_PEN);
    let previous_pen = SelectObject(hdc, null_pen);
    let previous_brush = SelectObject(hdc, white);

    let pill_left = scale_to_panel(12, panel_height);
    let pill_top = scale_to_panel(12, panel_height);
    let pill_right = scale_to_panel(30, panel_height);
    let pill_bottom = scale_to_panel(20, panel_height);
    let radius = scale_to_panel(8, panel_height);
    RoundRect(
        hdc,
        pill_left,
        pill_top,
        pill_right,
        pill_bottom,
        radius,
        radius,
    );

    SelectObject(hdc, muted);
    let dot_left = scale_to_panel(36, panel_height);
    let dot_top = scale_to_panel(14, panel_height);
    let dot_size = scale_to_panel(5, panel_height);
    Ellipse(
        hdc,
        dot_left,
        dot_top,
        dot_left + dot_size,
        dot_top + dot_size,
    );

    SelectObject(hdc, previous_brush);
    SelectObject(hdc, previous_pen);
    DeleteObject(white);
    DeleteObject(muted);
}

unsafe fn draw_text(hdc: isize, text: &str, mut rect: Rect, format: u32) {
    let text = wide(text);
    DrawTextW(
        hdc,
        text.as_ptr(),
        -1,
        &mut rect,
        format | DT_VCENTER | DT_SINGLELINE,
    );
}

unsafe extern "system" fn wnd_proc(
    hwnd: isize,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    match message {
        WM_PAINT => {
            let mut paint: PaintStruct = zeroed();
            let hdc = BeginPaint(hwnd, &mut paint);
            let mut client = Rect::default();
            GetClientRect(hwnd, &mut client);

            let background = CreateSolidBrush(colorref(29, 29, 32));
            FillRect(hdc, &client, background);
            DeleteObject(background);

            SetBkMode(hdc, TRANSPARENT);
            SetTextColor(hdc, colorref(255, 255, 255));
            let text_font = create_font(
                "Segoe UI Variable Text",
                13,
                client.bottom,
                FONT_WEIGHT_SEMIBOLD,
            );
            let text_font = if text_font != 0 {
                text_font
            } else {
                GetStockObject(DEFAULT_GUI_FONT)
            };
            let previous = SelectObject(hdc, text_font);

            let width = client.right - client.left;
            draw_workspace_indicator(hdc, client.bottom);
            draw_text(
                hdc,
                &clock_label(),
                Rect {
                    left: width / 2 - scale_to_panel(120, client.bottom),
                    top: 0,
                    right: width / 2 + scale_to_panel(120, client.bottom),
                    bottom: client.bottom,
                },
                DT_CENTER,
            );

            let icon_font =
                create_font("Segoe Fluent Icons", 14, client.bottom, FONT_WEIGHT_NORMAL);
            let mut status_icons = String::from("\u{E701}  \u{E767}");
            let battery = crate::windows::power::panel_label();
            if !battery.is_empty() {
                status_icons.push_str("  ");
                status_icons.push_str(&battery);
            }
            if icon_font != 0 {
                SelectObject(hdc, icon_font);
            }
            draw_text(
                hdc,
                &status_icons,
                Rect {
                    left: width - scale_to_panel(150, client.bottom),
                    top: 0,
                    right: width - scale_to_panel(12, client.bottom),
                    bottom: client.bottom,
                },
                DT_RIGHT,
            );

            SelectObject(hdc, previous);
            if icon_font != 0 {
                DeleteObject(icon_font);
            }
            if text_font != GetStockObject(DEFAULT_GUI_FONT) {
                DeleteObject(text_font);
            }
            EndPaint(hwnd, &paint);
            0
        }
        WM_LBUTTONUP => {
            let mut client = Rect::default();
            GetClientRect(hwnd, &mut client);
            let width = client.right - client.left;
            let x = x_from_lparam(lparam);

            if x <= scale_to_panel(58, client.bottom) {
                send_action(PanelAction::Activities);
            } else if x >= width / 2 - 125 && x <= width / 2 + 125 {
                send_action(PanelAction::DateMenu);
            } else if x >= width - 165 {
                send_action(PanelAction::QuickSettings);
            }
            0
        }
        WM_TIMER if wparam == TIMER_CLOCK => {
            InvalidateRect(hwnd, std::ptr::null(), 0);
            0
        }
        WM_CLOSE => {
            DestroyWindow(hwnd);
            0
        }
        WM_DESTROY => {
            KillTimer(hwnd, TIMER_CLOCK);
            crate::windows::appbar::release(hwnd);
            PANEL_HWND.store(0, Ordering::SeqCst);
            PostQuitMessage(0);
            0
        }
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
}

fn panel_bounds(display: &DisplayInfo) -> (i32, i32, i32, i32) {
    (
        display.bounds.left,
        display.bounds.top,
        display.bounds.width().max(1),
        display.logical_to_physical(PANEL_HEIGHT),
    )
}

pub fn hwnd() -> Option<isize> {
    let hwnd = PANEL_HWND.load(Ordering::SeqCst);
    (hwnd != 0).then_some(hwnd)
}

fn restore_previous_layout(hwnd: isize, previous: Rect) -> Result<(), String> {
    let width = (previous.right - previous.left).max(1);
    let height = (previous.bottom - previous.top).max(1);
    let position_restored = unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            previous.left,
            previous.top,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        ) != 0
    };
    let appbar_restored = crate::windows::appbar::reserve_top(hwnd);

    match (position_restored, appbar_restored) {
        (true, Ok(())) => Ok(()),
        (false, Ok(())) => Err(
            "failed to restore native panel geometry after relayout failure; AppBar was re-reserved"
                .into(),
        ),
        (true, Err(error)) => Err(format!(
            "native panel geometry was restored but AppBar recovery failed: {error}"
        )),
        (false, Err(error)) => Err(format!(
            "native panel geometry and AppBar recovery both failed: {error}"
        )),
    }
}

pub fn relayout(display: &DisplayInfo) -> Result<(), String> {
    let Some(hwnd) = hwnd() else {
        return Ok(());
    };

    let mut previous = Rect::default();
    if unsafe { GetWindowRect(hwnd, &mut previous) } == 0 {
        return Err("GetWindowRect failed before native panel relayout".into());
    }

    let (x, y, width, height) = panel_bounds(display);
    crate::windows::appbar::release(hwnd);
    let ok = unsafe {
        SetWindowPos(
            hwnd,
            HWND_TOPMOST,
            x,
            y,
            width,
            height,
            SWP_NOACTIVATE | SWP_SHOWWINDOW,
        )
    };
    if ok == 0 {
        return match restore_previous_layout(hwnd, previous) {
            Ok(()) => Err(
                "SetWindowPos failed for native panel; previous layout was restored".into(),
            ),
            Err(rollback) => Err(format!(
                "SetWindowPos failed for native panel; rollback failed: {rollback}"
            )),
        };
    }

    if let Err(error) = crate::windows::appbar::reserve_top(hwnd) {
        return match restore_previous_layout(hwnd, previous) {
            Ok(()) => Err(format!(
                "native panel AppBar reservation failed; previous layout was restored: {error}"
            )),
            Err(rollback) => Err(format!(
                "native panel AppBar reservation failed: {error}; rollback failed: {rollback}"
            )),
        };
    }

    unsafe {
        InvalidateRect(hwnd, std::ptr::null(), 0);
    }
    Ok(())
}

pub fn start(app: tauri::AppHandle, display: DisplayInfo) -> Result<isize, String> {
    let (action_tx, action_rx) = mpsc::channel();
    ACTION_SENDER
        .set(action_tx)
        .map_err(|_| "native panel action dispatcher is already running".to_string())?;

    let action_app = app.clone();
    thread::Builder::new()
        .name("fedorawin-panel-actions".into())
        .spawn(move || {
            while let Ok(action) = action_rx.recv() {
                match action {
                    PanelAction::Activities => {
                        let _ = crate::shell::toggle_activities(&action_app);
                    }
                    PanelAction::DateMenu => {
                        let _ = crate::shell::toggle_surface(&action_app, "date-menu");
                    }
                    PanelAction::QuickSettings => {
                        let _ = crate::shell::toggle_surface(&action_app, "quick-settings");
                    }
                }
            }
        })
        .map_err(|error| format!("failed to start native panel action thread: {error}"))?;

    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("fedorawin-native-panel".into())
        .spawn(move || unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let class_name = wide("FedoraWin.NativePanel");
            let title = wide("FedoraWin — panel");
            let class = WndClassExW {
                size: size_of::<WndClassExW>() as u32,
                style: 0,
                wnd_proc: Some(wnd_proc),
                class_extra: 0,
                window_extra: 0,
                instance,
                icon: 0,
                cursor: 0,
                background: 0,
                menu_name: std::ptr::null(),
                class_name: class_name.as_ptr(),
                icon_small: 0,
            };
            RegisterClassExW(&class);

            let (x, y, width, height) = panel_bounds(&display);
            let hwnd = CreateWindowExW(
                WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                class_name.as_ptr(),
                title.as_ptr(),
                WS_POPUP | WS_VISIBLE,
                x,
                y,
                width,
                height,
                0,
                0,
                instance,
                std::ptr::null_mut(),
            );
            if hwnd == 0 {
                let _ = ready_tx.send(Err("CreateWindowExW failed for native panel".into()));
                return;
            }

            PANEL_HWND.store(hwnd, Ordering::SeqCst);
            SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            ShowWindow(hwnd, SW_SHOW);
            UpdateWindow(hwnd);
            SetTimer(hwnd, TIMER_CLOCK, 30_000, None);

            if let Err(error) = crate::windows::appbar::reserve_top(hwnd) {
                DestroyWindow(hwnd);
                let _ = ready_tx.send(Err(error));
                return;
            }

            let _ = ready_tx.send(Ok(hwnd));

            let mut message: Msg = zeroed();
            while GetMessageW(&mut message, 0, 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        })
        .map_err(|error| format!("failed to start native panel thread: {error}"))?;

    ready_rx
        .recv_timeout(Duration::from_secs(3))
        .map_err(|_| "native panel did not initialize".to_string())?
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(module_name: *const u16) -> isize;
}

#[link(name = "user32")]
extern "system" {
    fn RegisterClassExW(class: *const WndClassExW) -> u16;
    fn CreateWindowExW(
        ex_style: u32,
        class_name: *const u16,
        window_name: *const u16,
        style: u32,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        parent: isize,
        menu: isize,
        instance: isize,
        param: *mut std::ffi::c_void,
    ) -> isize;
    fn DefWindowProcW(hwnd: isize, message: u32, wparam: usize, lparam: isize) -> isize;
    fn DestroyWindow(hwnd: isize) -> i32;
    fn ShowWindow(hwnd: isize, command: i32) -> i32;
    fn UpdateWindow(hwnd: isize) -> i32;
    fn SetWindowPos(
        hwnd: isize,
        insert_after: isize,
        x: i32,
        y: i32,
        width: i32,
        height: i32,
        flags: u32,
    ) -> i32;
    fn GetWindowRect(hwnd: isize, rect: *mut Rect) -> i32;
    fn GetClientRect(hwnd: isize, rect: *mut Rect) -> i32;
    fn GetMessageW(message: *mut Msg, hwnd: isize, min: u32, max: u32) -> i32;
    fn TranslateMessage(message: *const Msg) -> i32;
    fn DispatchMessageW(message: *const Msg) -> isize;
    fn PostQuitMessage(exit_code: i32);
    fn BeginPaint(hwnd: isize, paint: *mut PaintStruct) -> isize;
    fn EndPaint(hwnd: isize, paint: *const PaintStruct) -> i32;
    fn InvalidateRect(hwnd: isize, rect: *const Rect, erase: i32) -> i32;
    fn FillRect(hdc: isize, rect: *const Rect, brush: isize) -> i32;
    fn DrawTextW(hdc: isize, text: *const u16, count: i32, rect: *mut Rect, format: u32) -> i32;
    fn SetBkMode(hdc: isize, mode: i32) -> i32;
    fn SetTextColor(hdc: isize, color: u32) -> u32;
    fn SetTimer(
        hwnd: isize,
        id: usize,
        interval: u32,
        callback: Option<unsafe extern "system" fn(isize, u32, usize, u32)>,
    ) -> usize;
    fn KillTimer(hwnd: isize, id: usize) -> i32;
    fn GetLocalTime(time: *mut SystemTime);
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateSolidBrush(color: u32) -> isize;
    fn DeleteObject(object: isize) -> i32;
    fn GetStockObject(object: i32) -> isize;
    fn SelectObject(hdc: isize, object: isize) -> isize;
    fn RoundRect(
        hdc: isize,
        left: i32,
        top: i32,
        right: i32,
        bottom: i32,
        ellipse_width: i32,
        ellipse_height: i32,
    ) -> i32;
    fn Ellipse(hdc: isize, left: i32, top: i32, right: i32, bottom: i32) -> i32;
    fn CreateFontW(
        height: i32,
        width: i32,
        escapement: i32,
        orientation: i32,
        weight: i32,
        italic: u32,
        underline: u32,
        strike_out: u32,
        char_set: u32,
        out_precision: u32,
        clip_precision: u32,
        quality: u32,
        pitch_and_family: u32,
        face_name: *const u16,
    ) -> isize;
}

#[cfg(test)]
mod tests {
    use super::{scale_to_panel, x_from_lparam};

    #[test]
    fn extracts_signed_mouse_x_coordinate() {
        assert_eq!(x_from_lparam(42), 42);
        assert_eq!(x_from_lparam(0x0000_fffe), -2);
    }

    #[test]
    fn logical_panel_geometry_scales_with_dpi_height() {
        assert_eq!(scale_to_panel(12, 32), 12);
        assert_eq!(scale_to_panel(12, 48), 18);
    }
}
