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

pub fn panel_label() -> String {
    match status() {
        Ok(power) => match power.battery_percent {
            Some(percent) => format!("●  ●  {percent}%"),
            None if power.ac_online => "●  ●  AC".into(),
            None => "●  ●  ●".into(),
        },
        Err(_) => "●  ●  ●".into(),
    }
}

#[link(name = "kernel32")]
extern "system" {
    fn GetSystemPowerStatus(status: *mut SystemPowerStatus) -> i32;
}

#[cfg(test)]
mod tests {
    use super::{PowerStatus, BATTERY_FLAG_CHARGING, BATTERY_FLAG_NO_BATTERY};

    #[test]
    fn battery_flags_match_win32_values() {
        assert_eq!(BATTERY_FLAG_CHARGING, 0x08);
        assert_eq!(BATTERY_FLAG_NO_BATTERY, 0x80);
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
