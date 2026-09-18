use serde::Serialize;
use std::{thread, time::Duration};
use windows::Devices::Radios::{Radio, RadioAccessStatus, RadioKind, RadioState};

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BluetoothStatus {
    pub supported: bool,
    pub enabled: bool,
    pub name: Option<String>,
    pub state: String,
}

fn windows_error(context: &str, error: impl std::fmt::Display) -> String {
    format!("{context}: {error}")
}

fn radio_state_name(state: RadioState) -> &'static str {
    if state == RadioState::On {
        "on"
    } else if state == RadioState::Off {
        "off"
    } else if state == RadioState::Disabled {
        "disabled"
    } else {
        "unknown"
    }
}

fn bluetooth_radio() -> Result<Option<Radio>, String> {
    windows::core::init_mta().map_err(|error| windows_error("WinRT initialization failed", error))?;
    let radios = Radio::GetRadiosAsync()
        .map_err(|error| windows_error("Bluetooth radio enumeration failed", error))?
        .join()
        .map_err(|error| windows_error("Bluetooth radio enumeration failed", error))?;

    let size = radios
        .Size()
        .map_err(|error| windows_error("Bluetooth radio collection failed", error))?;
    for index in 0..size {
        let radio = radios
            .GetAt(index)
            .map_err(|error| windows_error("Bluetooth radio lookup failed", error))?;
        let kind = radio
            .Kind()
            .map_err(|error| windows_error("Bluetooth radio kind failed", error))?;
        if kind == RadioKind::Bluetooth {
            return Ok(Some(radio));
        }
    }
    Ok(None)
}

fn from_radio(radio: &Radio) -> Result<BluetoothStatus, String> {
    let state = radio
        .State()
        .map_err(|error| windows_error("Bluetooth state read failed", error))?;
    let name = radio
        .Name()
        .ok()
        .map(|value| value.to_string())
        .filter(|value| !value.trim().is_empty());

    Ok(BluetoothStatus {
        supported: true,
        enabled: state == RadioState::On,
        name,
        state: radio_state_name(state).to_string(),
    })
}

pub fn status() -> Result<BluetoothStatus, String> {
    let Some(radio) = bluetooth_radio()? else {
        return Ok(BluetoothStatus {
            supported: false,
            enabled: false,
            name: None,
            state: "unavailable".into(),
        });
    };
    from_radio(&radio)
}

pub fn set_enabled(enabled: bool) -> Result<BluetoothStatus, String> {
    let radio = bluetooth_radio()?
        .ok_or_else(|| "Windows reported no Bluetooth radio".to_string())?;

    let access = Radio::RequestAccessAsync()
        .map_err(|error| windows_error("Bluetooth access request failed", error))?
        .join()
        .map_err(|error| windows_error("Bluetooth access request failed", error))?;
    if access != RadioAccessStatus::Allowed {
        return Err(format!("Windows denied Bluetooth radio control: {access:?}"));
    }

    let target = if enabled {
        RadioState::On
    } else {
        RadioState::Off
    };
    let result = radio
        .SetStateAsync(target)
        .map_err(|error| windows_error("Bluetooth state change failed", error))?
        .join()
        .map_err(|error| windows_error("Bluetooth state change failed", error))?;
    if result != RadioAccessStatus::Allowed {
        return Err(format!("Windows rejected Bluetooth state change: {result:?}"));
    }

    for _ in 0..8 {
        let current = radio
            .State()
            .map_err(|error| windows_error("Bluetooth state confirmation failed", error))?;
        if current == target {
            return from_radio(&radio);
        }
        thread::sleep(Duration::from_millis(40));
    }

    from_radio(&radio)
}

#[cfg(test)]
mod tests {
    use super::radio_state_name;
    use windows::Devices::Radios::RadioState;

    #[test]
    fn maps_bluetooth_radio_states() {
        assert_eq!(radio_state_name(RadioState::On), "on");
        assert_eq!(radio_state_name(RadioState::Off), "off");
        assert_eq!(radio_state_name(RadioState::Disabled), "disabled");
        assert_eq!(radio_state_name(RadioState::Unknown), "unknown");
    }
}
