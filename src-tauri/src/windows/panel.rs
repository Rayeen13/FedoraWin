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

const DT_LEFT: u32 = 0x0000;
const DT_CENTER: u32 = 0x0001;
const DT_RIGHT: u32 = 0x0002;
const DT_VCENTER: u32 = 0x0004;
const DT_SINGLELINE: u32 = 0x0020;
const TRANSPARENT: i32 = 1;
const DEFAULT_GUI_FONT: i32 = 17;

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

fn clock_label() -> String {
    let mut time = SystemTime::default();
    unsafe { GetLocalTime(&mut time) };
    let day = match time.day_of_week {
        0 => "Sun",
        1 => "Mon",
        2 => "Tue",
        3 => "Wed",
        4 => "Thu",
        5 => "Fri",
        _ => "Sat",
    };
    format!("{day} {:02}:{:02}", time.hour, time.minute)
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
            let font = GetStockObject(DEFAULT_GUI_FONT);
            let previous = SelectObject(hdc, font);

            let width = client.right - client.left;
            draw_text(
                hdc,
                "Activities",
                Rect {
                    left: 12,
                    top: 0,
                    right: 120,
                    bottom: client.bottom,
                },
                DT_LEFT,
            );
            draw_text(
                hdc,
                &clock_label(),
                Rect {
                    left: width / 2 - 110,
                    top: 0,
                    right: width / 2 + 110,
                    bottom: client.bottom,
                },
                DT_CENTER,
            );
            draw_text(
                hdc,
                &crate::windows::power::panel_label(),
                Rect {
                    left: width - 145,
                    top: 0,
                    right: width - 12,
                    bottom: client.bottom,
                },
                DT_RIGHT,
            );

            SelectObject(hdc, previous);
            EndPaint(hwnd, &paint);
            0
        }
        WM_LBUTTONUP => {
            let mut client = Rect::default();
            GetClientRect(hwnd, &mut client);
            let width = client.right - client.left;
            let x = x_from_lparam(lparam);

            if x <= 125 {
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

pub fn relayout(display: &DisplayInfo) -> Result<(), String> {
    let Some(hwnd) = hwnd() else {
        return Ok(());
    };
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
        return Err("SetWindowPos failed for native panel".into());
    }
    crate::windows::appbar::reserve_top(hwnd)?;
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
}

#[cfg(test)]
mod tests {
    use super::x_from_lparam;

    #[test]
    fn extracts_signed_mouse_x_coordinate() {
        assert_eq!(x_from_lparam(42), 42);
        assert_eq!(x_from_lparam(0x0000_fffe), -2);
    }
}
