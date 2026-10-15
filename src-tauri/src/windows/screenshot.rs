use std::mem::size_of;
use std::thread;
use std::time::Duration;

const INPUT_KEYBOARD: u32 = 1;
const KEYEVENTF_KEYUP: u32 = 0x0002;
const VK_SHIFT: u16 = 0x10;
const VK_S: u16 = 0x53;
const VK_LWIN: u16 = 0x5b;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct KeyboardInput {
    virtual_key: u16,
    scan_code: u16,
    flags: u32,
    time: u32,
    extra_info: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct MouseInput {
    dx: i32,
    dy: i32,
    mouse_data: u32,
    flags: u32,
    time: u32,
    extra_info: usize,
}

#[repr(C)]
union InputPayload {
    mouse: MouseInput,
    keyboard: KeyboardInput,
}

#[repr(C)]
struct Input {
    kind: u32,
    payload: InputPayload,
}

fn keyboard_input(virtual_key: u16, key_up: bool) -> Input {
    Input {
        kind: INPUT_KEYBOARD,
        payload: InputPayload {
            keyboard: KeyboardInput {
                virtual_key,
                flags: if key_up { KEYEVENTF_KEYUP } else { 0 },
                ..KeyboardInput::default()
            },
        },
    }
}

/// Opens Windows' native screenshot selection overlay after FedoraWin dismisses
/// its Quick Settings surface. This is equivalent to the documented Win+Shift+S
/// user shortcut and does not replace or patch Snipping Tool.
pub fn open_overlay() -> Result<(), String> {
    thread::sleep(Duration::from_millis(90));
    let inputs = [
        keyboard_input(VK_LWIN, false),
        keyboard_input(VK_SHIFT, false),
        keyboard_input(VK_S, false),
        keyboard_input(VK_S, true),
        keyboard_input(VK_SHIFT, true),
        keyboard_input(VK_LWIN, true),
    ];
    let sent = unsafe {
        SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            size_of::<Input>() as i32,
        )
    };
    if sent != inputs.len() as u32 {
        return Err("Windows did not accept the screenshot shortcut input".into());
    }
    Ok(())
}

#[link(name = "user32")]
extern "system" {
    fn SendInput(count: u32, inputs: *const Input, size: i32) -> u32;
}

#[cfg(test)]
mod tests {
    use super::{keyboard_input, KEYEVENTF_KEYUP, VK_S};

    #[test]
    fn screenshot_key_release_sets_keyup_flag() {
        let press = keyboard_input(VK_S, false);
        let release = keyboard_input(VK_S, true);
        let press_keyboard = unsafe { press.payload.keyboard };
        let release_keyboard = unsafe { release.payload.keyboard };
        assert_eq!(press_keyboard.flags, 0);
        assert_eq!(release_keyboard.flags, KEYEVENTF_KEYUP);
    }
}
