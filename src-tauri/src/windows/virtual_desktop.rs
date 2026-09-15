use parking_lot::Mutex;
use serde::Serialize;
use std::ffi::c_void;
use std::mem::size_of;
use std::ptr;

const CLSCTX_INPROC_SERVER: u32 = 0x1;
const COINIT_MULTITHREADED: u32 = 0x0;
const RPC_E_CHANGED_MODE: i32 = 0x80010106u32 as i32;

const INPUT_KEYBOARD: u32 = 1;
const KEYEVENTF_KEYUP: u32 = 0x0002;
const VK_CONTROL: u16 = 0x11;
const VK_LEFT: u16 = 0x25;
const VK_RIGHT: u16 = 0x27;
const VK_LWIN: u16 = 0x5B;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

impl Guid {
    fn parse(value: &str) -> Result<Self, String> {
        let cleaned = value.trim().trim_matches(|ch| ch == '{' || ch == '}');
        let parts: Vec<&str> = cleaned.split('-').collect();
        if parts.len() != 5 || parts[3].len() != 4 || parts[4].len() != 12 {
            return Err("invalid virtual desktop id".into());
        }

        let data1 = u32::from_str_radix(parts[0], 16).map_err(|_| "invalid virtual desktop id")?;
        let data2 = u16::from_str_radix(parts[1], 16).map_err(|_| "invalid virtual desktop id")?;
        let data3 = u16::from_str_radix(parts[2], 16).map_err(|_| "invalid virtual desktop id")?;
        let mut data4 = [0u8; 8];
        data4[0] =
            u8::from_str_radix(&parts[3][0..2], 16).map_err(|_| "invalid virtual desktop id")?;
        data4[1] =
            u8::from_str_radix(&parts[3][2..4], 16).map_err(|_| "invalid virtual desktop id")?;
        for index in 0..6 {
            let start = index * 2;
            data4[index + 2] = u8::from_str_radix(&parts[4][start..start + 2], 16)
                .map_err(|_| "invalid virtual desktop id")?;
        }

        Ok(Self {
            data1,
            data2,
            data3,
            data4,
        })
    }

    fn as_string(&self) -> String {
        format!(
            "{:08x}-{:04x}-{:04x}-{:02x}{:02x}-{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
            self.data1,
            self.data2,
            self.data3,
            self.data4[0],
            self.data4[1],
            self.data4[2],
            self.data4[3],
            self.data4[4],
            self.data4[5],
            self.data4[6],
            self.data4[7]
        )
    }
}

const CLSID_VIRTUAL_DESKTOP_MANAGER: Guid = Guid {
    data1: 0xaa509086,
    data2: 0x5ca9,
    data3: 0x4c25,
    data4: [0x8f, 0x95, 0x58, 0x9d, 0x3c, 0x07, 0xb4, 0x8a],
};

const IID_VIRTUAL_DESKTOP_MANAGER: Guid = Guid {
    data1: 0xa5cd92ff,
    data2: 0x29be,
    data3: 0x454c,
    data4: [0x8d, 0x04, 0xd8, 0x28, 0x79, 0xfb, 0x3f, 0x1b],
};

#[repr(C)]
struct VirtualDesktopManagerVTable {
    query_interface:
        unsafe extern "system" fn(*mut c_void, *const Guid, *mut *mut c_void) -> i32,
    add_ref: unsafe extern "system" fn(*mut c_void) -> u32,
    release: unsafe extern "system" fn(*mut c_void) -> u32,
    is_window_on_current_virtual_desktop:
        unsafe extern "system" fn(*mut c_void, isize, *mut i32) -> i32,
    get_window_desktop_id: unsafe extern "system" fn(*mut c_void, isize, *mut Guid) -> i32,
    move_window_to_desktop: unsafe extern "system" fn(*mut c_void, isize, *const Guid) -> i32,
}

#[repr(C)]
struct VirtualDesktopManagerObject {
    vtable: *const VirtualDesktopManagerVTable,
}

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

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowWorkspaceInfo {
    pub desktop_id: String,
    pub on_current_workspace: bool,
}

struct VirtualDesktopManager {
    object: *mut VirtualDesktopManagerObject,
    uninitialize_com: bool,
}

impl VirtualDesktopManager {
    fn new() -> Result<Self, String> {
        let initialized = unsafe { CoInitializeEx(ptr::null_mut(), COINIT_MULTITHREADED) };
        let uninitialize_com = initialized >= 0;
        if initialized < 0 && initialized != RPC_E_CHANGED_MODE {
            return Err(format!(
                "CoInitializeEx failed: 0x{:08x}",
                initialized as u32
            ));
        }

        let mut object: *mut c_void = ptr::null_mut();
        let result = unsafe {
            CoCreateInstance(
                &CLSID_VIRTUAL_DESKTOP_MANAGER,
                ptr::null_mut(),
                CLSCTX_INPROC_SERVER,
                &IID_VIRTUAL_DESKTOP_MANAGER,
                &mut object,
            )
        };
        if result < 0 || object.is_null() {
            if uninitialize_com {
                unsafe { CoUninitialize() };
            }
            return Err(format!(
                "VirtualDesktopManager is unavailable: 0x{:08x}",
                result as u32
            ));
        }

        Ok(Self {
            object: object as *mut VirtualDesktopManagerObject,
            uninitialize_com,
        })
    }

    fn vtable(&self) -> &VirtualDesktopManagerVTable {
        unsafe { &*(*self.object).vtable }
    }

    fn window_info(&self, hwnd: isize) -> Result<WindowWorkspaceInfo, String> {
        let mut id = Guid::default();
        let desktop_result =
            unsafe { (self.vtable().get_window_desktop_id)(self.object.cast(), hwnd, &mut id) };
        if desktop_result < 0 {
            return Err(format!(
                "GetWindowDesktopId failed: 0x{:08x}",
                desktop_result as u32
            ));
        }

        let mut current = 0i32;
        let current_result = unsafe {
            (self.vtable().is_window_on_current_virtual_desktop)(
                self.object.cast(),
                hwnd,
                &mut current,
            )
        };
        if current_result < 0 {
            return Err(format!(
                "IsWindowOnCurrentVirtualDesktop failed: 0x{:08x}",
                current_result as u32
            ));
        }

        Ok(WindowWorkspaceInfo {
            desktop_id: id.as_string(),
            on_current_workspace: current != 0,
        })
    }

    fn move_window(&self, hwnd: isize, desktop_id: &str) -> Result<(), String> {
        let id = Guid::parse(desktop_id)?;
        let result =
            unsafe { (self.vtable().move_window_to_desktop)(self.object.cast(), hwnd, &id) };
        if result < 0 {
            return Err(format!(
                "MoveWindowToDesktop failed: 0x{:08x}",
                result as u32
            ));
        }
        Ok(())
    }
}

impl Drop for VirtualDesktopManager {
    fn drop(&mut self) {
        if !self.object.is_null() {
            unsafe {
                (self.vtable().release)(self.object.cast());
            }
        }
        if self.uninitialize_com {
            unsafe { CoUninitialize() };
        }
    }
}

pub fn window_info(hwnd: isize) -> Result<WindowWorkspaceInfo, String> {
    VirtualDesktopManager::new()?.window_info(hwnd)
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

/// Switches the Windows virtual desktop only after an explicit user action.
/// This does not change Explorer configuration or write persistent shell state.
pub fn navigate(direction: i32) -> Result<(), String> {
    let arrow = match direction.signum() {
        -1 => VK_LEFT,
        1 => VK_RIGHT,
        _ => return Err("workspace direction must be -1 or 1".into()),
    };

    let inputs = [
        keyboard_input(VK_LWIN, false),
        keyboard_input(VK_CONTROL, false),
        keyboard_input(arrow, false),
        keyboard_input(arrow, true),
        keyboard_input(VK_CONTROL, true),
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
        return Err("Windows did not accept the workspace navigation input".into());
    }
    Ok(())
}

pub struct WorkspaceMoveJournal {
    original_desktops: Mutex<Vec<(String, String)>>,
}

impl Default for WorkspaceMoveJournal {
    fn default() -> Self {
        Self {
            original_desktops: Mutex::new(Vec::new()),
        }
    }
}

impl WorkspaceMoveJournal {
    pub fn move_window(&self, handle: &str, target_desktop_id: &str) -> Result<(), String> {
        let hwnd = handle
            .parse::<isize>()
            .map_err(|_| "invalid window handle")?;
        let manager = VirtualDesktopManager::new()?;
        let origin = manager.window_info(hwnd)?.desktop_id;
        if origin.eq_ignore_ascii_case(target_desktop_id) {
            return Ok(());
        }

        {
            let mut journal = self.original_desktops.lock();
            if !journal
                .iter()
                .any(|(saved_handle, _)| saved_handle == handle)
            {
                journal.push((handle.to_string(), origin));
            }
        }

        manager.move_window(hwnd, target_desktop_id)
    }

    pub fn undo_last(&self) -> Result<Option<String>, String> {
        let entry = self.original_desktops.lock().pop();
        let Some((handle, desktop_id)) = entry else {
            return Ok(None);
        };

        let hwnd = handle
            .parse::<isize>()
            .map_err(|_| "invalid window handle")?;
        VirtualDesktopManager::new()?.move_window(hwnd, &desktop_id)?;
        Ok(Some(handle))
    }

    pub fn pending_count(&self) -> usize {
        self.original_desktops.lock().len()
    }
}

#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(reserved: *mut c_void, apartment: u32) -> i32;
    fn CoUninitialize();
    fn CoCreateInstance(
        class_id: *const Guid,
        outer: *mut c_void,
        context: u32,
        interface_id: *const Guid,
        object: *mut *mut c_void,
    ) -> i32;
}

#[link(name = "user32")]
extern "system" {
    fn SendInput(count: u32, inputs: *const Input, size: i32) -> u32;
}

#[cfg(test)]
mod tests {
    use super::Guid;

    #[test]
    fn virtual_desktop_guid_round_trips() {
        let value = "01234567-89ab-cdef-0123-456789abcdef";
        let guid = Guid::parse(value).unwrap();
        assert_eq!(guid.as_string(), value);
    }

    #[test]
    fn malformed_desktop_guid_is_rejected() {
        assert!(Guid::parse("not-a-guid").is_err());
    }
}
