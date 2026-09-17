use serde::Serialize;

const BATTERY_FLAG_CHARGING: u8 = 0x08;
const BATTERY_FLAG_NO_BATTERY: u8 = 0x80;
const UNKNOWN: u8 = 0xff;
const ERROR_SUCCESS: u32 = 0;

#[repr(C)]
#[allow(non_snake_case)]
struct SystemPowerStatus {
    ACLineStatus: u8,
    BatteryFlag: u8,
    BatteryLifePercent: u8,
    SystemStatusFlag: u8,
    BatteryLifeTime: u32,
    BatteryFullLifeTime: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Guid {
    data1: u32,
    data2: u16,
    data3: u16,
    data4: [u8; 8],
}

impl Guid {
    const ZERO: Self = Self {
        data1: 0,
        data2: 0,
        data3: 0,
        data4: [0; 8],
    };
}

const GUID_POWER_MODE_BEST_EFFICIENCY: Guid = Guid {
    data1: 0x961c_c777,
    data2: 0x2547,
    data3: 0x4f9d,
    data4: [0x81, 0x74, 0x7d, 0x86, 0x18, 0x1b, 0x8a, 0x7a],
};
const GUID_POWER_MODE_NONE: Guid = Guid::ZERO;
const GUID_POWER_MODE_BEST_PERFORMANCE: Guid = Guid {
    data1: 0xded5_74b5,
    data2: 0x45a0,
    data3: 0x4f42,
    data4: [0x87, 0x37, 0x46, 0x34, 0x5c, 0x09, 0xc2, 0x38],
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PowerMode {
    BestEfficiency,
    Balanced,
    BestPerformance,
}

impl PowerMode {
    pub fn parse(value: &str) -> Result<Self, String> {
        match value.trim().to_ascii_lowercase().as_str() {
            "bestefficiency" | "best-efficiency" | "efficiency" => Ok(Self::BestEfficiency),
            "balanced" => Ok(Self::Balanced),
            "bestperformance" | "best-performance" | "performance" => Ok(Self::BestPerformance),
            _ => Err(format!("unsupported power mode: {value}")),
        }
    }

    fn guid(self) -> Guid {
        match self {
            Self::BestEfficiency => GUID_POWER_MODE_BEST_EFFICIENCY,
            Self::Balanced => GUID_POWER_MODE_NONE,
            Self::BestPerformance => GUID_POWER_MODE_BEST_PERFORMANCE,
        }
    }

    fn from_guid(guid: Guid) -> Result<Self, String> {
        match guid {
            GUID_POWER_MODE_BEST_EFFICIENCY => Ok(Self::BestEfficiency),
            GUID_POWER_MODE_NONE => Ok(Self::Balanced),
            GUID_POWER_MODE_BEST_PERFORMANCE => Ok(Self::BestPerformance),
            _ => Err("Windows returned an unsupported user-configured power mode".into()),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerStatus {
    pub battery_present: bool,
    pub battery_percent: Option<u8>,
    pub charging: bool,
    pub ac_online: bool,
}

pub fn status() -> Result<PowerStatus, String> {
    let mut raw = SystemPowerStatus {
        ACLineStatus: UNKNOWN,
        BatteryFlag: UNKNOWN,
        BatteryLifePercent: UNKNOWN,
        SystemStatusFlag: 0,
        BatteryLifeTime: u32::MAX,
        BatteryFullLifeTime: u32::MAX,
    };
    let ok = unsafe { GetSystemPowerStatus(&mut raw) };
    if ok == 0 {
        return Err("GetSystemPowerStatus failed".into());
    }

    let battery_present = raw.BatteryFlag != UNKNOWN
        && raw.BatteryFlag & BATTERY_FLAG_NO_BATTERY == 0
        && raw.BatteryLifePercent != UNKNOWN;
    let battery_percent = battery_present.then(|| raw.BatteryLifePercent.min(100));

    Ok(PowerStatus {
        battery_present,
        battery_percent,
        charging: battery_present && raw.BatteryFlag & BATTERY_FLAG_CHARGING != 0,
        ac_online: raw.ACLineStatus == 1,
    })
}

pub fn configured_mode() -> Result<PowerMode, String> {
    let ac_online = status()?.ac_online;
    let mut guid = Guid::ZERO;
    let code = unsafe {
        if ac_online {
            PowerGetUserConfiguredACPowerMode(&mut guid)
        } else {
            PowerGetUserConfiguredDCPowerMode(&mut guid)
        }
    };
    if code != ERROR_SUCCESS {
        return Err(format!(
            "PowerGetUserConfigured{}PowerMode failed with Win32 error {code}",
            if ac_online { "AC" } else { "DC" }
        ));
    }
    PowerMode::from_guid(guid)
}

pub fn set_configured_mode(mode: PowerMode) -> Result<PowerMode, String> {
    let ac_online = status()?.ac_online;
    let guid = mode.guid();
    let code = unsafe {
        if ac_online {
            PowerSetUserConfiguredACPowerMode(&guid)
        } else {
            PowerSetUserConfiguredDCPowerMode(&guid)
        }
    };
    if code != ERROR_SUCCESS {
        return Err(format!(
            "PowerSetUserConfigured{}PowerMode failed with Win32 error {code}",
            if ac_online { "AC" } else { "DC" }
        ));
    }
    configured_mode()
}

fn battery_glyph(percent: u8, charging: bool) -> char {
    let bucket = ((u16::from(percent.min(100)) + 5) / 10).min(10) as u32;
    let codepoint = if charging {
        if bucket >= 9 {
            0xe83e
        } else {
            0xe85a + bucket
        }
    } else if bucket >= 10 {
        0xe83f
    } else {
        0xe850 + bucket
    };
    char::from_u32(codepoint).unwrap_or('\u{e996}')
}

pub fn panel_label() -> String {
    status()
        .ok()
        .and_then(|power| {
            power
                .battery_percent
                .map(|percent| battery_glyph(percent, power.charging).to_string())
        })
        .unwrap_or_default()
}

#[link(name = "kernel32")]
extern "system" {
    fn GetSystemPowerStatus(status: *mut SystemPowerStatus) -> i32;
}

#[link(name = "PowrProf")]
extern "system" {
    fn PowerGetUserConfiguredACPowerMode(power_mode_guid: *mut Guid) -> u32;
    fn PowerGetUserConfiguredDCPowerMode(power_mode_guid: *mut Guid) -> u32;
    fn PowerSetUserConfiguredACPowerMode(power_mode_guid: *const Guid) -> u32;
    fn PowerSetUserConfiguredDCPowerMode(power_mode_guid: *const Guid) -> u32;
}

#[cfg(test)]
mod tests {
    use super::{
        battery_glyph, Guid, PowerMode, PowerStatus, BATTERY_FLAG_CHARGING,
        BATTERY_FLAG_NO_BATTERY, GUID_POWER_MODE_BEST_EFFICIENCY,
        GUID_POWER_MODE_BEST_PERFORMANCE, GUID_POWER_MODE_NONE,
    };

    #[test]
    fn battery_flags_match_win32_values() {
        assert_eq!(BATTERY_FLAG_CHARGING, 0x08);
        assert_eq!(BATTERY_FLAG_NO_BATTERY, 0x80);
    }

    #[test]
    fn battery_glyph_tracks_charge_level_and_charging_state() {
        assert_eq!(battery_glyph(0, false), '\u{e850}');
        assert_eq!(battery_glyph(100, false), '\u{e83f}');
        assert_eq!(battery_glyph(100, true), '\u{e83e}');
    }

    #[test]
    fn serialized_shape_is_stable() {
        let value = serde_json::to_value(PowerStatus {
            battery_present: true,
            battery_percent: Some(73),
            charging: true,
            ac_online: true,
        })
        .expect("power status serializes");
        assert_eq!(value["batteryPresent"], true);
        assert_eq!(value["batteryPercent"], 73);
        assert_eq!(value["charging"], true);
        assert_eq!(value["acOnline"], true);
    }

    #[test]
    fn power_modes_match_windows_11_guids() {
        assert_eq!(GUID_POWER_MODE_NONE, Guid::ZERO);
        assert_eq!(
            PowerMode::BestEfficiency.guid(),
            GUID_POWER_MODE_BEST_EFFICIENCY
        );
        assert_eq!(
            PowerMode::BestPerformance.guid(),
            GUID_POWER_MODE_BEST_PERFORMANCE
        );
    }

    #[test]
    fn power_mode_parser_accepts_ui_values() {
        assert_eq!(PowerMode::parse("bestEfficiency").unwrap(), PowerMode::BestEfficiency);
        assert_eq!(PowerMode::parse("balanced").unwrap(), PowerMode::Balanced);
        assert_eq!(
            PowerMode::parse("bestPerformance").unwrap(),
            PowerMode::BestPerformance
        );
        assert!(PowerMode::parse("turbo").is_err());
    }
}
