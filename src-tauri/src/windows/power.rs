use serde::Serialize;

const BATTERY_FLAG_CHARGING: u8 = 0x08;
const BATTERY_FLAG_NO_BATTERY: u8 = 0x80;
const UNKNOWN: u8 = 0xff;

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

#[cfg(test)]
mod tests {
    use super::{battery_glyph, PowerStatus, BATTERY_FLAG_CHARGING, BATTERY_FLAG_NO_BATTERY};

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
}
