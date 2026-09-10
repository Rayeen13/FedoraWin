#[cfg(windows)]
pub mod appbar;
#[cfg(windows)]
pub mod frame;
#[cfg(windows)]
pub mod wifi;

#[cfg(not(windows))]
pub mod appbar {
    pub fn reserve_top(_: isize, _: i32) -> Result<(), String> { Ok(()) }
}
#[cfg(not(windows))]
pub mod frame {
    use crate::shell::AppearanceState;
    use std::sync::Arc;
    pub fn apply_to_top_level_windows(_: &AppearanceState) -> Result<usize, String> { Ok(0) }
    pub fn start_frame_watcher<T>(_: Arc<T>) where T: Send + Sync + 'static {}
}
#[cfg(not(windows))]
pub mod wifi {
    pub fn set_enabled(_: bool) -> Result<(), String> { Err("Wi-Fi control is Windows-only".into()) }
}
