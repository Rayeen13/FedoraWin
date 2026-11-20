use std::mem::zeroed;
use std::sync::{mpsc, OnceLock};
use std::thread;
use std::time::{Duration, Instant};
use tauri::Emitter;

const EVENT_SYSTEM_FOREGROUND: u32 = 0x0003;
const EVENT_SYSTEM_MINIMIZESTART: u32 = 0x0016;
const EVENT_SYSTEM_MINIMIZEEND: u32 = 0x0017;
const EVENT_SYSTEM_DESKTOPSWITCH: u32 = 0x0020;
const EVENT_OBJECT_CREATE: u32 = 0x8000;
const EVENT_OBJECT_DESTROY: u32 = 0x8001;
const EVENT_OBJECT_SHOW: u32 = 0x8002;
const EVENT_OBJECT_HIDE: u32 = 0x8003;
const EVENT_OBJECT_NAMECHANGE: u32 = 0x800C;
const EVENT_OBJECT_CLOAKED: u32 = 0x8017;
const EVENT_OBJECT_UNCLOAKED: u32 = 0x8018;
const OBJID_WINDOW: i32 = 0;
const WINEVENT_OUTOFCONTEXT: u32 = 0x0000;
const WINEVENT_SKIPOWNPROCESS: u32 = 0x0002;
const REFRESH_INTERVAL: Duration = Duration::from_millis(85);

// A capacity-one queue collapses bursts of cross-process Windows notifications
// rather than allocating one queued message per event.
static EVENT_SENDER: OnceLock<mpsc::SyncSender<()>> = OnceLock::new();

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

fn should_refresh(event: u32, hwnd: isize, object_id: i32) -> bool {
    match event {
        // The system may supply no HWND for a virtual desktop transition.
        EVENT_SYSTEM_DESKTOPSWITCH => true,
        EVENT_SYSTEM_FOREGROUND | EVENT_SYSTEM_MINIMIZESTART | EVENT_SYSTEM_MINIMIZEEND => {
            hwnd != 0
        }
        EVENT_OBJECT_CREATE
        | EVENT_OBJECT_DESTROY
        | EVENT_OBJECT_SHOW
        | EVENT_OBJECT_HIDE
        | EVENT_OBJECT_NAMECHANGE
        | EVENT_OBJECT_CLOAKED
        | EVENT_OBJECT_UNCLOAKED => hwnd != 0 && object_id == OBJID_WINDOW,
        _ => false,
    }
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
    if !should_refresh(event, hwnd, object_id) {
        return;
    }
    if let Some(sender) = EVENT_SENDER.get() {
        // Full means a refresh is already pending; never block a WinEvent hook.
        let _ = sender.try_send(());
    }
}

fn coalesce_burst(receiver: &mpsc::Receiver<()>) {
    // Fixed deadline, not a sliding debounce: continuous event streams must
    // still refresh the Activities overview instead of postponing forever.
    let deadline = Instant::now() + REFRESH_INTERVAL;
    while !deadline.saturating_duration_since(Instant::now()).is_zero() {
        if receiver
            .recv_timeout(deadline.saturating_duration_since(Instant::now()))
            .is_err()
        {
            break;
        }
    }
}

pub fn start(app: tauri::AppHandle) -> Result<(), String> {
    if EVENT_SENDER.get().is_some() {
        return Err("window event watcher is already running".into());
    }

    // Report hook registration failure rather than silently leaving Activities
    // with a stale window/workspace overview.
    let (ready_tx, ready_rx) = mpsc::sync_channel(1);
    thread::Builder::new()
        .name("fedorawin-winevent-hook".into())
        .spawn(move || unsafe {
            let flags = WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS;
            let object_hook = SetWinEventHook(
                EVENT_OBJECT_CREATE,
                EVENT_OBJECT_UNCLOAKED,
                0,
                window_event_callback,
                0,
                0,
                flags,
            );
            let system_hook = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_DESKTOPSWITCH,
                0,
                window_event_callback,
                0,
                0,
                flags,
            );

            if object_hook == 0 || system_hook == 0 {
                if object_hook != 0 {
                    UnhookWinEvent(object_hook);
                }
                if system_hook != 0 {
                    UnhookWinEvent(system_hook);
                }
                let _ = ready_tx.send(false);
                return;
            }
            let _ = ready_tx.send(true);

            let mut message: Msg = zeroed();
            while GetMessageW(&mut message, 0, 0, 0) > 0 {}

            UnhookWinEvent(object_hook);
            UnhookWinEvent(system_hook);
        })
        .map_err(|error| format!("failed to start WinEvent hook thread: {error}"))?;

    match ready_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(true) => {}
        Ok(false) => return Err("Windows refused to register required WinEvent hooks".into()),
        Err(_) => return Err("WinEvent hook registration did not complete".into()),
    }

    let (tx, rx) = mpsc::sync_channel(1);
    EVENT_SENDER
        .set(tx)
        .map_err(|_| "window event watcher is already running".to_string())?;
    thread::Builder::new()
        .name("fedorawin-window-events".into())
        .spawn(move || {
            while rx.recv().is_ok() {
                coalesce_burst(&rx);
                let _ = app.emit("fedorawin://windows-changed", ());
            }
        })
        .map_err(|error| format!("failed to start window event dispatcher: {error}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        should_refresh, EVENT_OBJECT_CLOAKED, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE,
        EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_SHOW, EVENT_OBJECT_UNCLOAKED,
        EVENT_SYSTEM_DESKTOPSWITCH, EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND,
    };
    use std::sync::mpsc;

    #[test]
    fn workspace_switch_and_window_lifecycle_trigger_refresh() {
        assert!(should_refresh(EVENT_SYSTEM_DESKTOPSWITCH, 0, -4));
        assert!(should_refresh(EVENT_SYSTEM_FOREGROUND, 123, 0));
        assert!(should_refresh(EVENT_SYSTEM_MINIMIZEEND, 123, 0));
        for event in [
            EVENT_OBJECT_DESTROY,
            EVENT_OBJECT_SHOW,
            EVENT_OBJECT_HIDE,
            EVENT_OBJECT_NAMECHANGE,
            EVENT_OBJECT_CLOAKED,
            EVENT_OBJECT_UNCLOAKED,
        ] {
            assert!(should_refresh(event, 123, 0));
            assert!(!should_refresh(event, 123, -4));
            assert!(!should_refresh(event, 0, 0));
        }
        assert!(!should_refresh(EVENT_SYSTEM_FOREGROUND, 0, 0));
        assert!(!should_refresh(0x8010, 123, 0));
    }

    #[test]
    fn window_event_queue_drops_duplicate_pending_notifications() {
        let (tx, rx) = mpsc::sync_channel(1);
        assert!(tx.try_send(()).is_ok());
        assert!(matches!(tx.try_send(()), Err(mpsc::TrySendError::Full(()))));
        assert!(rx.try_recv().is_ok());
        assert!(tx.try_send(()).is_ok());
    }
}
