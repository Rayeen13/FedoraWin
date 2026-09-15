use std::mem::zeroed;
use std::sync::{mpsc, OnceLock};
use std::thread;
use std::time::Duration;
use tauri::Emitter;

const EVENT_SYSTEM_MINIMIZESTART: u32 = 0x0016;
const EVENT_SYSTEM_MINIMIZEEND: u32 = 0x0017;
const EVENT_OBJECT_CREATE: u32 = 0x8000;
const EVENT_OBJECT_NAMECHANGE: u32 = 0x800C;
const OBJID_WINDOW: i32 = 0;
const WINEVENT_OUTOFCONTEXT: u32 = 0x0000;
const WINEVENT_SKIPOWNPROCESS: u32 = 0x0002;

static EVENT_SENDER: OnceLock<mpsc::Sender<()>> = OnceLock::new();

#[repr(C)]
#[derive(Clone, Copy)]
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

#[link(name = "user32")]
extern "system" {
    fn SetWinEventHook(
        event_min: u32,
        event_max: u32,
        module: isize,
        callback: extern "system" fn(isize, u32, isize, i32, i32, u32, u32),
        process_id: u32,
        thread_id: u32,
        flags: u32,
    ) -> isize;
    fn UnhookWinEvent(hook: isize) -> i32;
    fn GetMessageW(message: *mut Msg, hwnd: isize, min: u32, max: u32) -> i32;
}

extern "system" fn window_event_callback(
    _: isize,
    event: u32,
    hwnd: isize,
    object_id: i32,
    _: i32,
    _: u32,
    _: u32,
) {
    if hwnd == 0 {
        return;
    }
    if event >= EVENT_OBJECT_CREATE && object_id != OBJID_WINDOW {
        return;
    }
    if let Some(sender) = EVENT_SENDER.get() {
        let _ = sender.send(());
    }
    crate::windows::frame_overlay::notify();
}

pub fn start(app: tauri::AppHandle) -> Result<(), String> {
    let (tx, rx) = mpsc::channel();
    EVENT_SENDER
        .set(tx)
        .map_err(|_| "window event watcher is already running".to_string())?;

    thread::Builder::new()
        .name("fedorawin-window-events".into())
        .spawn(move || {
            while rx.recv().is_ok() {
                // Coalesce title/show/hide/minimize bursts into one shell refresh.
                while rx.recv_timeout(Duration::from_millis(85)).is_ok() {}
                let _ = app.emit("fedorawin://windows-changed", ());
            }
        })
        .map_err(|error| format!("failed to start window event dispatcher: {error}"))?;

    thread::Builder::new()
        .name("fedorawin-winevent-hook".into())
        .spawn(move || unsafe {
            let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
            let object_hook = SetWinEventHook(
                EVENT_OBJECT_CREATE,
                EVENT_OBJECT_NAMECHANGE,
                0,
                window_event_callback,
                0,
                0,
                flags,
            );
            let minimize_hook = SetWinEventHook(
                EVENT_SYSTEM_MINIMIZESTART,
                EVENT_SYSTEM_MINIMIZEEND,
                0,
                window_event_callback,
                0,
                0,
                flags,
            );

            if object_hook == 0 || minimize_hook == 0 {
                if object_hook != 0 {
                    UnhookWinEvent(object_hook);
                }
                if minimize_hook != 0 {
                    UnhookWinEvent(minimize_hook);
                }
                return;
            }

            let mut message: Msg = zeroed();
            while GetMessageW(&mut message, 0, 0, 0) > 0 {}

            UnhookWinEvent(object_hook);
            UnhookWinEvent(minimize_hook);
        })
        .map_err(|error| format!("failed to start WinEvent hook thread: {error}"))?;

    Ok(())
}
