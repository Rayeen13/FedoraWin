use std::mem::zeroed;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const ACTIVITIES_HOTKEY_ID: i32 = 0x4657;
const MOD_ALT: u32 = 0x0001;
const MOD_NOREPEAT: u32 = 0x4000;
const VK_F1: u32 = 0x70;
const WM_HOTKEY: u32 = 0x0312;

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
    fn RegisterHotKey(hwnd: isize, id: i32, modifiers: u32, virtual_key: u32) -> i32;
    fn UnregisterHotKey(hwnd: isize, id: i32) -> i32;
    fn GetMessageW(message: *mut Msg, hwnd: isize, min: u32, max: u32) -> i32;
}

fn is_activities_hotkey(message: &Msg) -> bool {
    message.message == WM_HOTKEY && message.wparam == ACTIVITIES_HOTKEY_ID as usize
}

pub fn start_activities_hotkey(app: tauri::AppHandle) -> Result<(), String> {
    let (status_tx, status_rx) = mpsc::sync_channel(1);

    thread::Builder::new()
        .name("fedorawin-hotkeys".into())
        .spawn(move || {
            let registered = unsafe {
                RegisterHotKey(
                    0,
                    ACTIVITIES_HOTKEY_ID,
                    MOD_ALT | MOD_NOREPEAT,
                    VK_F1,
                )
            } != 0;

            let _ = status_tx.send(registered);
            if !registered {
                return;
            }

            let mut message: Msg = unsafe { zeroed() };
            loop {
                let result = unsafe { GetMessageW(&mut message, 0, 0, 0) };
                if result <= 0 {
                    break;
                }

                if is_activities_hotkey(&message) {
                    let _ = crate::shell::toggle_activities(&app);
                }
            }

            unsafe {
                UnregisterHotKey(0, ACTIVITIES_HOTKEY_ID);
            }
        })
        .map_err(|error| format!("failed to start hotkey thread: {error}"))?;

    match status_rx.recv_timeout(Duration::from_secs(2)) {
        Ok(true) => Ok(()),
        Ok(false) => Err("Alt+F1 is already reserved by another application".into()),
        Err(_) => Err("hotkey thread did not initialize".into()),
    }
}

#[cfg(test)]
mod tests {
    use super::{is_activities_hotkey, Msg, Point, ACTIVITIES_HOTKEY_ID, WM_HOTKEY};

    #[test]
    fn matches_only_our_activities_hotkey() {
        let message = Msg {
            hwnd: 0,
            message: WM_HOTKEY,
            wparam: ACTIVITIES_HOTKEY_ID as usize,
            lparam: 0,
            time: 0,
            point: Point { x: 0, y: 0 },
            private: 0,
        };
        assert!(is_activities_hotkey(&message));

        let other = Msg {
            wparam: 123,
            ..message
        };
        assert!(!is_activities_hotkey(&other));
    }
}
