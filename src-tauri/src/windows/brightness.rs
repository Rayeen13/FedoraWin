use serde::{Deserialize, Serialize};
use wmi::WMIConnection;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BrightnessStatus {
    pub supported: bool,
    pub value: Option<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct BrightnessRow {
    #[serde(rename = "InstanceName")]
    instance_name: String,
    active: bool,
    current_brightness: u8,
    level: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct BrightnessMethodRow {
    #[serde(rename = "__Path")]
    object_path: String,
    #[serde(rename = "InstanceName")]
    instance_name: String,
    active: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename = "WmiMonitorBrightnessMethods")]
struct BrightnessMethodClass;

#[derive(Debug, Serialize)]
#[serde(rename_all = "PascalCase")]
struct SetBrightnessInput {
    timeout: u32,
    brightness: u8,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct SetBrightnessOutput {
    return_value: u32,
}

fn connection() -> Result<WMIConnection, String> {
    WMIConnection::with_namespace_path("ROOT\\WMI")
        .map_err(|error| format!("brightness WMI connection failed: {error}"))
}

fn brightness_rows(connection: &WMIConnection) -> Result<Vec<BrightnessRow>, String> {
    connection
        .raw_query(
            "SELECT InstanceName, Active, CurrentBrightness, Level FROM WmiMonitorBrightness",
        )
        .map_err(|error| format!("brightness query failed: {error}"))
}

fn method_rows(connection: &WMIConnection) -> Result<Vec<BrightnessMethodRow>, String> {
    connection
        .raw_query("SELECT __Path, InstanceName, Active FROM WmiMonitorBrightnessMethods")
        .map_err(|error| format!("brightness method query failed: {error}"))
}

fn active_brightness(rows: &[BrightnessRow]) -> Option<&BrightnessRow> {
    rows.iter().find(|row| row.active).or_else(|| rows.first())
}

fn nearest_level(levels: &[u8], requested: u8) -> u8 {
    levels
        .iter()
        .copied()
        .min_by_key(|level| level.abs_diff(requested))
        .unwrap_or(requested)
}

pub fn status() -> Result<BrightnessStatus, String> {
    let connection = connection()?;
    let rows = match brightness_rows(&connection) {
        Ok(rows) => rows,
        Err(_) => {
            return Ok(BrightnessStatus {
                supported: false,
                value: None,
            })
        }
    };
    let Some(row) = active_brightness(&rows) else {
        return Ok(BrightnessStatus {
            supported: false,
            value: None,
        });
    };

    Ok(BrightnessStatus {
        supported: true,
        value: Some(row.current_brightness.min(100)),
    })
}

pub fn set(requested: u8) -> Result<u8, String> {
    let connection = connection()?;
    let brightness = brightness_rows(&connection)?;
    let row = active_brightness(&brightness).ok_or_else(|| {
        "Windows reported no brightness-controllable internal display".to_string()
    })?;
    let requested = requested.min(100);
    let target = nearest_level(&row.level, requested);

    let methods = method_rows(&connection)?;
    let method = methods
        .iter()
        .find(|method| method.instance_name == row.instance_name)
        .or_else(|| methods.iter().find(|method| method.active))
        .or_else(|| methods.first())
        .ok_or_else(|| "Windows brightness control method is unavailable".to_string())?;

    let output: SetBrightnessOutput = connection
        .exec_instance_method::<BrightnessMethodClass, _>(
            &method.object_path,
            "WmiSetBrightness",
            SetBrightnessInput {
                timeout: 0,
                brightness: target,
            },
        )
        .map_err(|error| format!("setting brightness failed: {error}"))?;

    if output.return_value != 0 {
        return Err(format!(
            "Windows brightness provider returned error {}",
            output.return_value
        ));
    }

    status()?
        .value
        .ok_or_else(|| "brightness changed but Windows did not report the new level".to_string())
}

#[cfg(test)]
mod tests {
    use super::nearest_level;

    #[test]
    fn picks_nearest_supported_brightness_level() {
        assert_eq!(nearest_level(&[0, 10, 20, 40, 60, 80, 100], 53), 60);
        assert_eq!(nearest_level(&[10, 30, 50], 42), 50);
    }

    #[test]
    fn keeps_requested_level_when_provider_has_no_step_table() {
        assert_eq!(nearest_level(&[], 67), 67);
    }
}
