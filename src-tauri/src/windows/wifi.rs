use std::ffi::c_void;
use std::ptr::{null, null_mut};

const WLAN_CLIENT_VERSION_LONGHORN: u32 = 2;
const WLAN_INTF_OPCODE_RADIO_STATE: u32 = 4;
const DOT11_RADIO_STATE_ON: u32 = 1;
const DOT11_RADIO_STATE_OFF: u32 = 2;
const ERROR_SUCCESS: u32 = 0;

#[repr(C)]
#[derive(Clone, Copy)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

#[repr(C)]
struct WlanInterfaceInfo {
    interface_guid: Guid,
    description: [u16; 256],
    state: u32,
}

#[repr(C)]
struct WlanInterfaceInfoList {
    count: u32,
    index: u32,
    first: WlanInterfaceInfo,
}

#[repr(C)]
struct WlanPhyRadioState {
    phy_index: u32,
    software_state: u32,
    hardware_state: u32,
}

#[link(name = "wlanapi")]
extern "system" {
    fn WlanOpenHandle(
        client_version: u32,
        reserved: *const c_void,
        negotiated_version: *mut u32,
        handle: *mut isize,
    ) -> u32;
    fn WlanEnumInterfaces(
        handle: isize,
        reserved: *const c_void,
        list: *mut *mut WlanInterfaceInfoList,
    ) -> u32;
    fn WlanSetInterface(
        handle: isize,
        guid: *const Guid,
        opcode: u32,
        data_size: u32,
        data: *const c_void,
        reserved: *const c_void,
    ) -> u32;
    fn WlanFreeMemory(memory: *mut c_void);
    fn WlanCloseHandle(handle: isize, reserved: *const c_void) -> u32;
}

struct Client(isize);
impl Drop for Client {
    fn drop(&mut self) {
        unsafe {
            WlanCloseHandle(self.0, null());
        }
    }
}

pub fn set_enabled(enabled: bool) -> Result<(), String> {
    unsafe {
        let mut negotiated = 0u32;
        let mut handle = 0isize;
        let result = WlanOpenHandle(
            WLAN_CLIENT_VERSION_LONGHORN,
            null(),
            &mut negotiated,
            &mut handle,
        );
        if result != ERROR_SUCCESS {
            return Err(format!("WlanOpenHandle failed with {result}"));
        }
        let client = Client(handle);
        let mut list_ptr: *mut WlanInterfaceInfoList = null_mut();
        let result = WlanEnumInterfaces(client.0, null(), &mut list_ptr);
        if result != ERROR_SUCCESS || list_ptr.is_null() {
            return Err(format!("WlanEnumInterfaces failed with {result}"));
        }

        let count = (*list_ptr).count as usize;
        let first = &(*list_ptr).first as *const WlanInterfaceInfo;
        let interfaces = std::slice::from_raw_parts(first, count);
        let mut changed = 0usize;

        for interface in interfaces {
            // 0xFFFF_FFFF asks the WLAN service to apply the software state across all PHYs.
            let state = WlanPhyRadioState {
                phy_index: 0xFFFF_FFFF,
                software_state: if enabled {
                    DOT11_RADIO_STATE_ON
                } else {
                    DOT11_RADIO_STATE_OFF
                },
                hardware_state: DOT11_RADIO_STATE_ON,
            };
            let result = WlanSetInterface(
                client.0,
                &interface.interface_guid,
                WLAN_INTF_OPCODE_RADIO_STATE,
                std::mem::size_of::<WlanPhyRadioState>() as u32,
                &state as *const WlanPhyRadioState as *const c_void,
                null(),
            );
            if result == ERROR_SUCCESS {
                changed += 1;
            }
        }
        WlanFreeMemory(list_ptr as *mut c_void);
        if changed == 0 {
            return Err("no Wi-Fi interface accepted the radio-state change".into());
        }
        Ok(())
    }
}
