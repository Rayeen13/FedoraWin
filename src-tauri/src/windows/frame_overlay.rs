use crate::shell::{ShellState, ThemeMode};
use std::collections::{HashMap, HashSet};
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::thread;
 
const GWL_EXSTYLE: i32 = -20;
const GWLP_USERDATA: i32 = -21;
const GW_OWNER: u32 = 4;

const WS_POPUP: u32 = 0x8000_0000;
const WS_VISIBLE: u32 = 0x1000_0000;
const WS_EX_TOOLWINDOW: u32 = 0x0000_0080;
const WS_EX_NOACTIVATE: u32 = 0x0800_0000;

const SWP_NOSIZE: u32 = 0x0001;
const SWP_NOMOVE: u32 = 0x0002;
const SWP_NOZORDER: u32 = 0x0004;
const SWP_NOACTIVATE: u32 = 0x0010;
const SWP_SHOWWINDOW: u32 = 0x0040;

const WM_DESTROY: u32 = 0x0002;
const WM_PAINT: u32 = 0x000F;
const WM_LBUTTONUP: u32 = 0x0202;
const WM_ERASEBKGND: u32 = 0x0014;
const WM_SYSCOMMAND: u32 = 0x0112;

const SC_MINIMIZE: usize = 0xF020;
const SC_MAXIMIZE: usize = 0xF030;
const SC_CLOSE: usize = 0xF060;
const SC_RESTORE: usize = 0xF120;

const PM_NOREMOVE: u32 = 0x0000;
const WM_APP_FRAME_SYNC: u32 = 0x804F;
const WM_QUIT: u32 = 0x0012;
const WM_TIMER: u32 = 0x0113;
const TIMER_RECONCILE: usize = 1;
const PS_SOLID: i32 = 0;
const TRANSPARENT: i32 = 1;

const DWMWA_CAPTION_BUTTON_BOUNDS: u32 = 5;
const DWMWA_EXTENDED_FRAME_BOUNDS: u32 = 9;
const DWMWA_CLOAKED: u32 = 14;

static THREAD_ID: AtomicU32 = AtomicU32::new(0);
static DARK: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

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

fn wide(value: &str) -> Vec<u16> {
    value.encode_utf16().chain(std::iter::once(0)).collect()
}

fn colorref(red: u8, green: u8, blue: u8) -> u32 {
    red as u32 | ((green as u32) << 8) | ((blue as u32) << 16)
}

fn x_from_lparam(lparam: isize) -> i32 {
    (lparam as u32 & 0xffff) as i16 as i32
}

unsafe fn target_for_overlay(hwnd: isize) -> isize {
    GetWindowLongPtrW(hwnd, GWLP_USERDATA)
}

unsafe fn draw_line(hdc: isize, x1: i32, y1: i32, x2: i32, y2: i32, color: u32) {
    let pen = CreatePen(PS_SOLID, 1, color);
    let previous = SelectObject(hdc, pen);
    MoveToEx(hdc, x1, y1, std::ptr::null_mut());
    LineTo(hdc, x2, y2);
    SelectObject(hdc, previous);
    DeleteObject(pen);
}

unsafe fn paint_controls(hwnd: isize) {
    let mut paint: PaintStruct = zeroed();
    let hdc = BeginPaint(hwnd, &mut paint);
    let mut client = Rect::default();
    GetClientRect(hwnd, &mut client);

    let dark = DARK.load(Ordering::Relaxed);
    let background = if dark {
        colorref(46, 46, 50)
    } else {
        colorref(255, 255, 255)
    };
    let foreground = if dark {
        colorref(255, 255, 255)
    } else {
        colorref(32, 32, 34)
    };
    let button = if dark {
        colorref(66, 66, 71)
    } else {
        colorref(235, 235, 238)
    };

    let background_brush = CreateSolidBrush(background);
    FillRect(hdc, &client, background_brush);
    DeleteObject(background_brush);
    SetBkMode(hdc, TRANSPARENT);

    let width = (client.right - client.left).max(3);
    let height = (client.bottom - client.top).max(1);
    let segment = (width / 3).max(1);
    let diameter = (height - 10).clamp(18, 26);
    let radius = diameter / 2;
    let center_y = height / 2;

    let brush = CreateSolidBrush(button);
    let previous_brush = SelectObject(hdc, brush);
    for index in 0..3 {
        let center_x = segment * index + segment / 2;
        Ellipse(
            hdc,
            center_x - radius,
            center_y - radius,
            center_x + radius,
            center_y + radius,
        );
    }
    SelectObject(hdc, previous_brush);
    DeleteObject(brush);

    // Adwaita-style simple glyphs inside circular controls.
    let first_x = segment / 2;
    draw_line(
        hdc,
        first_x - 4,
        center_y + 2,
        first_x + 4,
        center_y + 2,
        foreground,
    );

    let second_x = segment + segment / 2;
    draw_line(hdc, second_x - 4, center_y - 4, second_x + 4, center_y - 4, foreground);
    draw_line(hdc, second_x + 4, center_y - 4, second_x + 4, center_y + 4, foreground);
    draw_line(hdc, second_x + 4, center_y + 4, second_x - 4, center_y + 4, foreground);
    draw_line(hdc, second_x - 4, center_y + 4, second_x - 4, center_y - 4, foreground);

    let third_x = segment * 2 + segment / 2;
    draw_line(
        hdc,
        third_x - 4,
        center_y - 4,
        third_x + 4,
        center_y + 4,
        foreground,
    );
    draw_line(
        hdc,
        third_x + 4,
        center_y - 4,
        third_x - 4,
        center_y + 4,
        foreground,
    );

    EndPaint(hwnd, &paint);
}

unsafe extern "system" fn overlay_wnd_proc(
    hwnd: isize,
    message: u32,
    wparam: usize,
    lparam: isize,
) -> isize {
    match message {
        WM_PAINT => {
            paint_controls(hwnd);
            0
        }
        WM_ERASEBKGND => 1,
        WM_LBUTTONUP => {
            let target = target_for_overlay(hwnd);
            if target == 0 || IsWindow(target) == 0 {
                return 0;
            }
            let mut client = Rect::default();
            GetClientRect(hwnd, &mut client);
            let width = (client.right - client.left).max(3);
            let segment = (width / 3).max(1);
            let x = x_from_lparam(lparam).clamp(0, width - 1);
            let command = match (x / segment).min(2) {
                0 => SC_MINIMIZE,
                1 => {
                    if IsZoomed(target) != 0 {
                        SC_RESTORE
                    } else {
                        SC_MAXIMIZE
                    }
                }
                _ => SC_CLOSE,
            };
            SendMessageW(target, WM_SYSCOMMAND, command, 0);
            0
        }
        WM_DESTROY => 0,
        _ => DefWindowProcW(hwnd, message, wparam, lparam),
    }
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
    if exstyle & WS_EX_TOOLWINDOW as isize != 0 || exstyle & WS_EX_NOACTIVATE as isize != 0 {
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

    let mut buttons = Rect::default();
    DwmGetWindowAttribute(
        hwnd,
        DWMWA_CAPTION_BUTTON_BOUNDS,
        &mut buttons as *mut Rect as *mut c_void,
        size_of::<Rect>() as u32,
    ) == 0
        && buttons.right > buttons.left
        && buttons.bottom > buttons.top
}

extern "system" fn enum_callback(hwnd: isize, lparam: isize) -> i32 {
    let targets = unsafe { &mut *(lparam as *mut Vec<isize>) };
    unsafe {
        if eligible(hwnd) {
            targets.push(hwnd);
        }
    }
    1
}

unsafe fn overlay_rect(target: isize) -> Option<Rect> {
    let mut frame = Rect::default();
    if DwmGetWindowAttribute(
        target,
        DWMWA_EXTENDED_FRAME_BOUNDS,
        &mut frame as *mut Rect as *mut c_void,
        size_of::<Rect>() as u32,
    ) != 0
    {
        return None;
    }

    let mut buttons = Rect::default();
    if DwmGetWindowAttribute(
        target,
        DWMWA_CAPTION_BUTTON_BOUNDS,
        &mut buttons as *mut Rect as *mut c_void,
        size_of::<Rect>() as u32,
    ) != 0
    {
        return None;
    }

    let width = buttons.right - buttons.left;
    let height = buttons.bottom - buttons.top;
    if width <= 0 || height <= 0 {
        return None;
    }

    Some(Rect {
        left: frame.left + buttons.left,
        top: frame.top + buttons.top,
        right: frame.left + buttons.right,
        bottom: frame.top + buttons.bottom,
    })
}

unsafe fn create_overlay(class_name: *const u16, target: isize) -> Option<isize> {
    let rect = overlay_rect(target)?;
    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
        class_name,
        wide("FedoraWin Adwaita Controls").as_ptr(),
        WS_POPUP | WS_VISIBLE,
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
        target,
        0,
        GetModuleHandleW(std::ptr::null()),
        std::ptr::null_mut(),
    );
    if hwnd == 0 {
        return None;
    }
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, target);
    SetWindowPos(
        hwnd,
        0,
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
        SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW,
    );
    Some(hwnd)
}

unsafe fn sync_overlay(overlay: isize, target: isize) -> bool {
    let Some(rect) = overlay_rect(target) else {
        return false;
    };
    SetWindowPos(
        overlay,
        0,
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
        SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW,
    ) != 0
}

unsafe fn reconcile(
    overlays: &mut HashMap<isize, isize>,
    class_name: *const u16,
    state: &ShellState,
) {
    DARK.store(
        !matches!(state.snapshot().appearance.theme, ThemeMode::Light),
        Ordering::Relaxed,
    );

    let mut targets = Vec::new();
    EnumWindows(
        enum_callback,
        &mut targets as *mut Vec<isize> as isize,
    );
    let active: HashSet<isize> = targets.iter().copied().collect();

    overlays.retain(|target, overlay| {
        if !active.contains(target)
            || IsWindow(*target) == 0
            || !sync_overlay(*overlay, *target)
        {
            DestroyWindow(*overlay);
            false
        } else {
            InvalidateRect(*overlay, std::ptr::null(), 0);
            true
        }
    });

    for target in targets {
        overlays
            .entry(target)
            .or_insert_with(|| create_overlay(class_name, target).unwrap_or(0));
    }
    overlays.retain(|_, overlay| *overlay != 0);
}

pub fn notify() {
    let thread_id = THREAD_ID.load(Ordering::SeqCst);
    if thread_id != 0 {
        unsafe {
            PostThreadMessageW(thread_id, WM_APP_FRAME_SYNC, 0, 0);
        }
    }
}

pub fn start(state: Arc<ShellState>) -> Result<(), String> {
    thread::Builder::new()
        .name("fedorawin-adwaita-frame-overlay".into())
        .spawn(move || unsafe {
            let instance = GetModuleHandleW(std::ptr::null());
            let class_name = wide("FedoraWin.AdwaitaCaptionButtons");
            let class = WndClassExW {
                size: size_of::<WndClassExW>() as u32,
                style: 0,
                wnd_proc: Some(overlay_wnd_proc),
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

            // Force this thread's message queue into existence before publishing
            // the thread id used by WinEvent callbacks.
            let mut message: Msg = zeroed();
            PeekMessageW(&mut message, 0, 0, 0, PM_NOREMOVE);
            THREAD_ID.store(GetCurrentThreadId(), Ordering::SeqCst);

            let mut overlays: HashMap<isize, isize> = HashMap::new();
            reconcile(&mut overlays, class_name.as_ptr(), &state);
            SetTimer(0, TIMER_RECONCILE, 2_000, None);

            loop {
                let result = GetMessageW(&mut message, 0, 0, 0);
                if result <= 0 || message.message == WM_QUIT {
                    break;
                }

                if message.hwnd == 0
                    && (message.message == WM_APP_FRAME_SYNC
                        || (message.message == WM_TIMER
                            && message.wparam == TIMER_RECONCILE))
                {
                    // Coalesce event bursts: one reconciliation is enough to move,
                    // create, destroy and repaint all caption overlays.
                    while PeekMessageW(&mut message, 0, WM_APP_FRAME_SYNC, WM_APP_FRAME_SYNC, 0x0001)
                        != 0
                    {}
                    reconcile(&mut overlays, class_name.as_ptr(), &state);
                    continue;
                }

                TranslateMessage(&message);
                DispatchMessageW(&message);
            }

            KillTimer(0, TIMER_RECONCILE);
            for (_, overlay) in overlays.drain() {
                DestroyWindow(overlay);
            }
            THREAD_ID.store(0, Ordering::SeqCst);
        })
        .map_err(|error| format!("failed to start Adwaita frame overlay: {error}"))?;
    Ok(())
}

pub fn stop() {
    let thread_id = THREAD_ID.load(Ordering::SeqCst);
    if thread_id != 0 {
        unsafe {
            PostThreadMessageW(thread_id, WM_QUIT, 0, 0);
        }
    }
}

#[link(name = "kernel32")]
extern "system" {
    fn GetModuleHandleW(module_name: *const u16) -> isize;
    fn GetCurrentThreadId() -> u32;
}

#[link(name = "dwmapi")]
extern "system" {
    fn DwmGetWindowAttribute(hwnd: isize, attribute: u32, value: *mut c_void, size: u32) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn EnumWindows(callback: extern "system" fn(isize, isize) -> i32, lparam: isize) -> i32;
    fn IsWindow(hwnd: isize) -> i32;
    fn IsWindowVisible(hwnd: isize) -> i32;
    fn IsIconic(hwnd: isize) -> i32;
    fn IsZoomed(hwnd: isize) -> i32;
    fn GetWindow(hwnd: isize, command: u32) -> isize;
    fn GetWindowLongPtrW(hwnd: isize, index: i32) -> isize;
    fn SetWindowLongPtrW(hwnd: isize, index: i32, value: isize) -> isize;
    fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
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
        param: *mut c_void,
    ) -> isize;
    fn DestroyWindow(hwnd: isize) -> i32;
    fn DefWindowProcW(hwnd: isize, message: u32, wparam: usize, lparam: isize) -> isize;
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
    fn SendMessageW(hwnd: isize, message: u32, wparam: usize, lparam: isize) -> isize;
    fn BeginPaint(hwnd: isize, paint: *mut PaintStruct) -> isize;
    fn EndPaint(hwnd: isize, paint: *const PaintStruct) -> i32;
    fn FillRect(hdc: isize, rect: *const Rect, brush: isize) -> i32;
    fn InvalidateRect(hwnd: isize, rect: *const Rect, erase: i32) -> i32;
    fn PeekMessageW(message: *mut Msg, hwnd: isize, min: u32, max: u32, remove: u32) -> i32;
    fn GetMessageW(message: *mut Msg, hwnd: isize, min: u32, max: u32) -> i32;
    fn PostThreadMessageW(thread_id: u32, message: u32, wparam: usize, lparam: isize) -> i32;
    fn SetTimer(
        hwnd: isize,
        id: usize,
        interval: u32,
        callback: Option<unsafe extern "system" fn(isize, u32, usize, u32)>,
    ) -> usize;
    fn KillTimer(hwnd: isize, id: usize) -> i32;
    fn TranslateMessage(message: *const Msg) -> i32;
    fn DispatchMessageW(message: *const Msg) -> isize;
    fn SetBkMode(hdc: isize, mode: i32) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateSolidBrush(color: u32) -> isize;
    fn CreatePen(style: i32, width: i32, color: u32) -> isize;
    fn DeleteObject(object: isize) -> i32;
    fn SelectObject(hdc: isize, object: isize) -> isize;
    fn MoveToEx(hdc: isize, x: i32, y: i32, previous: *mut Point) -> i32;
    fn LineTo(hdc: isize, x: i32, y: i32) -> i32;
    fn Ellipse(hdc: isize, left: i32, top: i32, right: i32, bottom: i32) -> i32;
}

#[cfg(test)]
mod tests {
    use super::x_from_lparam;

    #[test]
    fn extracts_signed_caption_click_coordinate() {
        assert_eq!(x_from_lparam(24), 24);
        assert_eq!(x_from_lparam(0x0000_fffc), -4);
    }
}
