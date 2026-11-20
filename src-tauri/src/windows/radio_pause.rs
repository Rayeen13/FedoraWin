use crate::windows::{bluetooth, wifi};
use serde::Serialize;
use std::sync::Mutex;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RadioReport {
    pub supported: bool,
    pub enabled: Option<bool>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RadioPauseStatus {
    pub managed: bool,
    pub paused: bool,
    pub partial: bool,
    pub wifi: RadioReport,
    pub bluetooth: RadioReport,
}

#[derive(Clone, Debug, Default)]
struct SavedRadios {
    wifi: Option<bool>,
    bluetooth: Option<bool>,
}

impl SavedRadios {
    fn is_empty(&self) -> bool {
        self.wifi.is_none() && self.bluetooth.is_none()
    }
}

#[derive(Default)]
pub struct RadioPauseManager {
    saved: Mutex<Option<SavedRadios>>,
}

fn wifi_report() -> RadioReport {
    match wifi::status() {
        Ok(status) => RadioReport {
            supported: status.supported,
            enabled: status.supported.then_some(status.enabled),
            error: None,
        },
        Err(error) => RadioReport {
            supported: false,
            enabled: None,
            error: Some(error),
        },
    }
}

fn bluetooth_report() -> RadioReport {
    match bluetooth::status() {
        Ok(status) => RadioReport {
            supported: status.supported,
            enabled: status.supported.then_some(status.enabled),
            error: None,
        },
        Err(error) => RadioReport {
            supported: false,
            enabled: None,
            error: Some(error),
        },
    }
}

fn compose(managed: bool, wifi: RadioReport, bluetooth: RadioReport) -> RadioPauseStatus {
    let reports = [&wifi, &bluetooth];
    let any_supported = reports.iter().any(|radio| radio.supported);
    let any_error = reports.iter().any(|radio| radio.error.is_some());
    let all_off = reports
        .iter()
        .filter(|radio| radio.supported)
        .all(|radio| radio.enabled == Some(false));

    RadioPauseStatus {
        managed,
        paused: managed && any_supported && all_off && !any_error,
        partial: managed && (!any_supported || !all_off || any_error),
        wifi,
        bluetooth,
    }
}

impl RadioPauseManager {
    pub fn status(&self) -> Result<RadioPauseStatus, String> {
        let managed = self
            .saved
            .lock()
            .map_err(|_| "radio pause state is unavailable".to_string())?
            .is_some();
        Ok(compose(managed, wifi_report(), bluetooth_report()))
    }

    pub fn set_paused(&self, paused: bool) -> Result<RadioPauseStatus, String> {
        let mut saved = self
            .saved
            .lock()
            .map_err(|_| "radio pause state is unavailable".to_string())?;
        let mut wifi_state = wifi_report();
        let mut bluetooth_state = bluetooth_report();

        if paused {
            if saved.is_none() {
                let before = SavedRadios {
                    wifi: wifi_state.enabled,
                    bluetooth: bluetooth_state.enabled,
                };
                if !before.is_empty() {
                    *saved = Some(before);
                }
            }

            if wifi_state.enabled == Some(true) {
                wifi_state = match wifi::set_enabled(false) {
                    Ok(value) => RadioReport {
                        supported: value.supported,
                        enabled: Some(value.enabled),
                        error: (!value.supported || value.enabled)
                            .then_some("Windows did not confirm Wi-Fi was disabled".into()),
                    },
                    Err(error) => RadioReport {
                        error: Some(error),
                        ..wifi_report()
                    },
                };
            }
            if bluetooth_state.enabled == Some(true) {
                bluetooth_state = match bluetooth::set_enabled(false) {
                    Ok(value) => RadioReport {
                        supported: value.supported,
                        enabled: Some(value.enabled),
                        error: (!value.supported || value.enabled)
                            .then_some("Windows did not confirm Bluetooth was disabled".into()),
                    },
                    Err(error) => RadioReport {
                        error: Some(error),
                        ..bluetooth_report()
                    },
                };
            }
        } else if let Some(previous) = saved.as_mut() {
            if previous.wifi == Some(true) {
                if wifi_state.enabled == Some(true) {
                    previous.wifi = None;
                } else if wifi_state.enabled == Some(false) {
                    wifi_state = match wifi::set_enabled(true) {
                        Ok(value) if value.supported && value.enabled => {
                            previous.wifi = None;
                            RadioReport {
                                supported: true,
                                enabled: Some(true),
                                error: None,
                            }
                        }
                        Ok(value) => RadioReport {
                            supported: value.supported,
                            enabled: Some(value.enabled),
                            error: Some("Windows did not confirm Wi-Fi was restored".into()),
                        },
                        Err(error) => RadioReport {
                            error: Some(error),
                            ..wifi_report()
                        },
                    };
                }
            } else {
                previous.wifi = None;
            }

            if previous.bluetooth == Some(true) {
                if bluetooth_state.enabled == Some(true) {
                    previous.bluetooth = None;
                } else if bluetooth_state.enabled == Some(false) {
                    bluetooth_state = match bluetooth::set_enabled(true) {
                        Ok(value) if value.supported && value.enabled => {
                            previous.bluetooth = None;
                            RadioReport {
                                supported: true,
                                enabled: Some(true),
                                error: None,
                            }
                        }
                        Ok(value) => RadioReport {
                            supported: value.supported,
                            enabled: Some(value.enabled),
                            error: Some("Windows did not confirm Bluetooth was restored".into()),
                        },
                        Err(error) => RadioReport {
                            error: Some(error),
                            ..bluetooth_report()
                        },
                    };
                }
            } else {
                previous.bluetooth = None;
            }

            if previous.is_empty() {
                *saved = None;
            }
        }

        Ok(compose(saved.is_some(), wifi_state, bluetooth_state))
    }
}

#[cfg(test)]
mod tests {
    use super::{compose, RadioReport, SavedRadios};

    fn report(enabled: Option<bool>) -> RadioReport {
        RadioReport {
            supported: enabled.is_some(),
            enabled,
            error: None,
        }
    }

    #[test]
    fn never_calls_unmanaged_radios_system_airplane_mode() {
        let result = compose(false, report(Some(false)), report(Some(false)));
        assert!(!result.paused);
        assert!(!result.managed);
    }

    #[test]
    fn managed_pause_needs_confirmed_supported_radios_off() {
        assert!(compose(true, report(Some(false)), report(None)).paused);
        assert!(compose(true, report(Some(true)), report(Some(false))).partial);
        assert!(!compose(true, report(None), report(None)).paused);
    }

    #[test]
    fn query_error_makes_managed_result_partial() {
        let mut failed = report(None);
        failed.error = Some("Access denied".into());
        let result = compose(true, report(Some(false)), failed);
        assert!(result.partial);
        assert!(!result.paused);
    }

    #[test]
    fn restoration_journal_keeps_initial_off_state() {
        let mut saved = SavedRadios {
            wifi: Some(true),
            bluetooth: Some(false),
        };
        assert!(!saved.is_empty());
        saved.bluetooth = None;
        assert_eq!(saved.wifi, Some(true));
    }
}
