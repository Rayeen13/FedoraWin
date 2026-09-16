use std::thread;

use windows::Win32::{
    Media::Audio::{
        eConsole, eRender, Endpoints::IAudioEndpointVolume, IMMDeviceEnumerator, MMDeviceEnumerator,
    },
    System::Com::{
        CoCreateInstance, CoInitializeEx, CoUninitialize, CLSCTX_ALL, COINIT_MULTITHREADED,
    },
};

struct ComApartment;

impl Drop for ComApartment {
    fn drop(&mut self) {
        unsafe { CoUninitialize() };
    }
}

fn with_default_endpoint<T>(
    operation: impl FnOnce(&IAudioEndpointVolume) -> windows::core::Result<T>,
) -> Result<T, String> {
    unsafe {
        CoInitializeEx(None, COINIT_MULTITHREADED)
            .ok()
            .map_err(|error| format!("Core Audio COM initialization failed: {error}"))?;
        let _apartment = ComApartment;
        let enumerator: IMMDeviceEnumerator =
            CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)
                .map_err(|error| format!("audio endpoint enumeration failed: {error}"))?;
        let device = enumerator
            .GetDefaultAudioEndpoint(eRender, eConsole)
            .map_err(|error| format!("default audio endpoint unavailable: {error}"))?;
        let endpoint: IAudioEndpointVolume = device
            .Activate(CLSCTX_ALL, None)
            .map_err(|error| format!("audio endpoint volume activation failed: {error}"))?;
        operation(&endpoint).map_err(|error| format!("audio endpoint operation failed: {error}"))
    }
}

fn on_audio_thread<T: Send + 'static>(
    operation: impl FnOnce() -> Result<T, String> + Send + 'static,
) -> Result<T, String> {
    thread::spawn(operation)
        .join()
        .map_err(|_| "Core Audio worker thread panicked".to_string())?
}

fn scalar_to_percent(level: f32) -> u8 {
    (level.clamp(0.0, 1.0) * 100.0).round() as u8
}

pub fn get_master_volume() -> Result<u8, String> {
    on_audio_thread(|| {
        with_default_endpoint(|endpoint| unsafe { endpoint.GetMasterVolumeLevelScalar() })
            .map(scalar_to_percent)
    })
}

pub fn set_master_volume(value: u8) -> Result<u8, String> {
    if value > 100 {
        return Err("volume must be between 0 and 100".into());
    }
    on_audio_thread(move || {
        with_default_endpoint(|endpoint| unsafe {
            endpoint.SetMasterVolumeLevelScalar(f32::from(value) / 100.0, std::ptr::null())?;
            endpoint.GetMasterVolumeLevelScalar()
        })
        .map(scalar_to_percent)
    })
}

#[cfg(test)]
mod tests {
    use super::scalar_to_percent;

    #[test]
    fn scalar_volume_is_clamped_and_rounded() {
        assert_eq!(scalar_to_percent(-0.1), 0);
        assert_eq!(scalar_to_percent(0.504), 50);
        assert_eq!(scalar_to_percent(0.506), 51);
        assert_eq!(scalar_to_percent(1.2), 100);
    }
}
