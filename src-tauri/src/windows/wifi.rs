use serde::Serialize;
use std::ffi::c_void;
use std::ptr::{null, null_mut};

const WLAN_CLIENT_VERSION_LONGHORN: u32 = 2;
const WLAN_INTF_OPCODE_RADIO_STATE: u32 = 4;
const WLAN_INTF_OPCODE_CURRENT_CONNECTION: u32 = 7;
const WLAN_INTERFACE_STATE_CONNECTED: u32 = 1;
const DOT11_RADIO_STATE_ON: u32 = 1;
const DOT11_RADIO_STATE_OFF: u32 = 2;
const ERROR_SUCCESS: u32 = 0;
const WLAN_MAX_PHY_INDEX: usize = 64;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WifiStatus {
    pub supported: bool,
    pub enabled: bool,
    pub connected: bool,
    pub network_name: Option<String>,
    pub interface_name: Option<String>,
}

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
#[derive(Clone, Copy, Default)]
struct WlanPhyRadioState {
    phy_index: u32,
    software_state: u32,
    hardware_state: u32,
}

#[repr(C)]
struct WlanRadioState {
    count: u32,
    states: [WlanPhyRadioState; WLAN_MAX_PHY_INDEX],
}

#[repr(C)]
struct Dot11Ssid {
    length: u32,
    bytes: [u8; 32],
}

#[repr(C)]
struct WlanAssociationAttributes {
    ssid: Dot11Ssid,
    bss_type: u32,
    bssid: [u8; 6],
    phy_type: u32,
    phy_index: u32,
    signal_quality: u32,
    rx_rate: u32,
    tx_rate: u32,
}

#[repr(C)]
struct WlanConnectionAttributesPrefix {
    interface_state: u32,
    connection_mode: u32,
    profile_name: [u16; 256],
    association: WlanAssociationAttributes,
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
    fn WlanQueryInterface(
        handle: isize,
        guid: *const Guid,
        opcode: u32,
        reserved: *const c_void,
        data_size: *mut u32,
        data: *mut *mut c_void,
        opcode_value_type: *mut u32,
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

impl Client {
    fn open() -> Result<Self, String> {
        let mut negotiated = 0u32;
        let mut handle = 0isize;
        let result = unsafe {
            WlanOpenHandle(
                WLAN_CLIENT_VERSION_LONGHORN,
                null(),
                &mut negotiated,
                &mut handle,
            )
        };
        if result != ERROR_SUCCESS {
            return Err(format!("WlanOpenHandle failed with {result}"));
        }
        Ok(Self(handle))
    }
}

impl Drop for Client {
    fn drop(&mut self) {
        unsafe {
            WlanCloseHandle(self.0, null());
        }
    }
}

struct WlanMemory(*mut c_void);

impl WlanMemory {
    fn new(pointer: *mut c_void) -> Self {
        Self(pointer)
    }
}

impl Drop for WlanMemory {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe { WlanFreeMemory(self.0) };
        }
    }
}

fn interface_name(interface: &WlanInterfaceInfo) -> Option<String> {
    let len = interface
        .description
        .iter()
        .position(|value| *value == 0)
        .unwrap_or(interface.description.len());
    let value = String::from_utf16_lossy(&interface.description[..len])
        .trim()
        .to_string();
    (!value.is_empty()).then_some(value)
}

fn ssid_name(connection: &WlanConnectionAttributesPrefix) -> Option<String> {
    let len = usize::try_from(connection.association.ssid.length)
        .unwrap_or(0)
        .min(connection.association.ssid.bytes.len());
    if len == 0 {
        return None;
    }
    let value = String::from_utf8_lossy(&connection.association.ssid.bytes[..len])
        .trim_matches(char::from(0))
        .trim()
        .to_string();
    (!value.is_empty()).then_some(value)
}

unsafe fn query_radio_state(client: &Client, guid: &Guid) -> Result<bool, String> {
    let mut size = 0u32;
    let mut data: *mut c_void = null_mut();
    let result = WlanQueryInterface(
        client.0,
        guid,
        WLAN_INTF_OPCODE_RADIO_STATE,
        null(),
        &mut size,
        &mut data,
        null_mut(),
    );
    if result != ERROR_SUCCESS || data.is_null() {
        return Err(format!(
            "WlanQueryInterface radio state failed with {result}"
        ));
    }
    let _memory = WlanMemory::new(data);
    let state = &*(data as *const WlanRadioState);
    let count = usize::try_from(state.count)
        .unwrap_or(0)
        .min(WLAN_MAX_PHY_INDEX);
    Ok(state.states[..count].iter().any(|phy| {
        phy.software_state == DOT11_RADIO_STATE_ON && phy.hardware_state == DOT11_RADIO_STATE_ON
    }))
}

unsafe fn query_network_name(client: &Client, guid: &Guid) -> Option<String> {
    let mut size = 0u32;
    let mut data: *mut c_void = null_mut();
    let result = WlanQueryInterface(
        client.0,
        guid,
        WLAN_INTF_OPCODE_CURRENT_CONNECTION,
        null(),
        &mut size,
        &mut data,
        null_mut(),
    );
    if result != ERROR_SUCCESS || data.is_null() {
        return None;
    }
    let _memory = WlanMemory::new(data);
    ssid_name(&*(data as *const WlanConnectionAttributesPrefix))
}

pub fn status() -> Result<WifiStatus, String> {
    unsafe {
        let client = Client::open()?;
        let mut list_ptr: *mut WlanInterfaceInfoList = null_mut();
        let result = WlanEnumInterfaces(client.0, null(), &mut list_ptr);
        if result != ERROR_SUCCESS || list_ptr.is_null() {
            return Err(format!("WlanEnumInterfaces failed with {result}"));
        }
        let _list_memory = WlanMemory::new(list_ptr.cast());

        let count = (*list_ptr).count as usize;
        if count == 0 {
            return Ok(WifiStatus {
                supported: false,
                enabled: false,
                connected: false,
                network_name: None,
                interface_name: None,
            });
        }

        let first = &(*list_ptr).first as *const WlanInterfaceInfo;
        let interfaces = std::slice::from_raw_parts(first, count);
        let mut enabled = false;
        let mut radio_observed = false;
        let mut connected = false;
        let mut network_name = None;
        let mut selected_interface = None;

        for interface in interfaces {
            if selected_interface.is_none() {
                selected_interface = interface_name(interface);
            }
            if let Ok(interface_enabled) = query_radio_state(&client, &interface.interface_guid) {
                radio_observed = true;
                enabled |= interface_enabled;
            }
            if interface.state == WLAN_INTERFACE_STATE_CONNECTED {
                connected = true;
                selected_interface = interface_name(interface).or(selected_interface);
                network_name = query_network_name(&client, &interface.interface_guid);
            }
        }

        if !radio_observed {
            return Err("Windows did not report a Wi-Fi radio state".into());
        }

        Ok(WifiStatus {
            supported: true,
            enabled,
            connected,
            network_name,
            interface_name: selected_interface,
        })
    }
}

pub fn set_enabled(enabled: bool) -> Result<WifiStatus, String> {
    unsafe {
        let client = Client::open()?;
        let mut list_ptr: *mut WlanInterfaceInfoList = null_mut();
        let result = WlanEnumInterfaces(client.0, null(), &mut list_ptr);
        if result != ERROR_SUCCESS || list_ptr.is_null() {
            return Err(format!("WlanEnumInterfaces failed with {result}"));
        }
        let _list_memory = WlanMemory::new(list_ptr.cast());

        let count = (*list_ptr).count as usize;
        let first = &(*list_ptr).first as *const WlanInterfaceInfo;
        let interfaces = std::slice::from_raw_parts(first, count);
        let mut changed = 0usize;

        for interface in interfaces {
            let state = WlanPhyRadioState {
                phy_index: u32::MAX,
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

        if changed == 0 {
            return Err("no Wi-Fi interface accepted the radio-state change".into());
        }
    }

    status()
}

#[cfg(test)]
mod tests {
    use super::{ssid_name, Dot11Ssid, WlanAssociationAttributes, WlanConnectionAttributesPrefix};

    fn connection(ssid: &[u8]) -> WlanConnectionAttributesPrefix {
        let mut bytes = [0u8; 32];
        let len = ssid.len().min(bytes.len());
        bytes[..len].copy_from_slice(&ssid[..len]);
        WlanConnectionAttributesPrefix {
            interface_state: 1,
            connection_mode: 0,
            profile_name: [0; 256],
            association: WlanAssociationAttributes {
                ssid: Dot11Ssid {
                    length: len as u32,
                    bytes,
                },
                bss_type: 1,
                bssid: [0; 6],
                phy_type: 0,
                phy_index: 0,
                signal_quality: 80,
                rx_rate: 0,
                tx_rate: 0,
            },
        }
    }

    #[test]
    fn decodes_connected_ssid() {
        assert_eq!(
            ssid_name(&connection(b"FedoraNet")),
            Some("FedoraNet".into())
        );
    }

    #[test]
    fn empty_ssid_is_not_displayed() {
        assert_eq!(ssid_name(&connection(b"")), None);
    }
}
