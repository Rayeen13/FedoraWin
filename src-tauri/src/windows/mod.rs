#[cfg(windows)]
pub mod appbar;
#[cfg(windows)]
pub mod apps;
#[cfg(windows)]
pub mod frame;
#[cfg(windows)]
pub mod wifi;
#[cfg(windows)]
pub mod windows_list;

#[cfg(not(windows))]
pub mod appbar {
    pub fn reserve_top(_: isize) -> Result<(), String> { Ok(()) }
    pub fn release(_: isize) {}
}
#[cfg(not(windows))]
pub mod apps {
    use serde::Serialize;
    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct AppEntry { pub name: String, pub app_id: String, pub aliases: Vec<String> }
    pub fn list() -> Result<Vec<AppEntry>, String> { Ok(Vec::new()) }
    pub fn launch(_: &str) -> Result<(), String> { Err("application launching is Windows-only".into()) }
}
#[cfg(not(windows))]
pub mod frame {
    use crate::shell::AppearanceState;
    use std::sync::Arc;
    pub fn apply_to_top_level_windows(_: &AppearanceState) -> Result<usize, String> { Ok(0) }
    pub fn reset_top_level_windows() {}
    pub fn start_frame_watcher<T>(_: Arc<T>) where T: Send + Sync + 'static {}
}
#[cfg(not(windows))]
pub mod wifi {
    pub fn set_enabled(_: bool) -> Result<(), String> { Err("Wi-Fi control is Windows-only".into()) }
}
#[cfg(not(windows))]
pub mod windows_list {
    use serde::Serialize;
    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct WindowEntry { pub handle: String, pub title: String, pub minimized: bool }
    pub fn list() -> Result<Vec<WindowEntry>, String> { Ok(Vec::new()) }
    pub fn activate(_: &str) -> Result<(), String> { Err("window activation is Windows-only".into()) }
}
