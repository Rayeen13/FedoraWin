#[cfg(windows)]
pub mod appbar;
#[cfg(windows)]
pub mod apps;
#[cfg(windows)]
pub mod audio;
#[cfg(windows)]
pub mod display;
#[cfg(windows)]
pub mod frame;
#[cfg(windows)]
pub mod hotkeys;
#[cfg(windows)]
pub mod panel;
#[cfg(windows)]
pub mod thumbnails;
#[cfg(windows)]
pub mod virtual_desktop;
#[cfg(windows)]
pub mod wifi;
#[cfg(windows)]
pub mod window_events;
#[cfg(windows)]
pub mod windows_list;

#[cfg(not(windows))]
pub mod audio {
    pub fn get_master_volume() -> Result<u8, String> {
        Err("audio control is Windows-only".into())
    }
    pub fn set_master_volume(_: u8) -> Result<u8, String> {
        Err("audio control is Windows-only".into())
    }
}
#[cfg(not(windows))]
pub mod appbar {
    pub fn reserve_top(_: isize) -> Result<(), String> {
        Ok(())
    }
    pub fn release(_: isize) {}
}
#[cfg(not(windows))]
pub mod apps {
    use serde::Serialize;
    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct AppEntry {
        pub name: String,
        pub app_id: String,
        pub aliases: Vec<String>,
    }
    pub fn list() -> Result<Vec<AppEntry>, String> {
        Ok(Vec::new())
    }
    pub fn launch(_: &str) -> Result<(), String> {
        Err("application launching is Windows-only".into())
    }
}
#[cfg(not(windows))]
pub mod display {
    use serde::Serialize;
    #[derive(Clone, Copy, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct RectInfo {
        pub left: i32,
        pub top: i32,
        pub right: i32,
        pub bottom: i32,
    }
    impl RectInfo {
        pub fn width(&self) -> i32 {
            self.right - self.left
        }
        pub fn height(&self) -> i32 {
            self.bottom - self.top
        }
    }
    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct DisplayInfo {
        pub id: String,
        pub bounds: RectInfo,
        pub work_area: RectInfo,
        pub dpi_x: u32,
        pub dpi_y: u32,
        pub scale_factor: f64,
        pub primary: bool,
    }
    impl DisplayInfo {
        pub fn logical_width(&self) -> f64 {
            self.bounds.width() as f64 / self.scale_factor
        }
        pub fn logical_height(&self) -> f64 {
            self.bounds.height() as f64 / self.scale_factor
        }
        pub fn logical_to_physical(&self, value: f64) -> i32 {
            (value * self.scale_factor).round().max(1.0) as i32
        }
    }
    pub fn enumerate() -> Result<Vec<DisplayInfo>, String> {
        Ok(Vec::new())
    }
    pub fn primary() -> Result<DisplayInfo, String> {
        Err("display enumeration is Windows-only".into())
    }
}
#[cfg(not(windows))]
pub mod frame {
    use crate::shell::AppearanceState;
    use std::sync::Arc;
    pub fn apply_to_top_level_windows(_: &AppearanceState) -> Result<usize, String> {
        Ok(0)
    }
    pub fn reset_top_level_windows() {}
    pub fn start_frame_watcher<T>(_: Arc<T>)
    where
        T: Send + Sync + 'static,
    {
    }
}
#[cfg(not(windows))]
pub mod wifi {
    pub fn set_enabled(_: bool) -> Result<(), String> {
        Err("Wi-Fi control is Windows-only".into())
    }
}
#[cfg(not(windows))]
pub mod windows_list {
    use serde::Serialize;
    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct WindowEntry {
        pub handle: String,
        pub title: String,
        pub minimized: bool,
        pub desktop_id: Option<String>,
        pub on_current_workspace: bool,
    }
    pub fn list() -> Result<Vec<WindowEntry>, String> {
        Ok(Vec::new())
    }
    pub fn activate(_: &str) -> Result<(), String> {
        Err("window activation is Windows-only".into())
    }
    pub fn close(_: &str) -> Result<(), String> {
        Err("window closing is Windows-only".into())
    }
}
#[cfg(not(windows))]
pub mod thumbnails {
    use serde::Deserialize;
    #[derive(Clone, Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    pub struct ThumbnailPlacement {
        pub handle: String,
        pub left: f64,
        pub top: f64,
        pub width: f64,
        pub height: f64,
    }
    #[derive(Default)]
    pub struct ThumbnailManager;
    impl ThumbnailManager {
        pub fn sync(&self, _: isize, _: f64, _: &[ThumbnailPlacement]) -> Result<usize, String> {
            Ok(0)
        }
        pub fn clear(&self) {}
    }
}
#[cfg(not(windows))]
pub mod window_events {
    pub fn start(_: tauri::AppHandle) -> Result<(), String> {
        Ok(())
    }
}
#[cfg(not(windows))]
pub mod virtual_desktop {
    use serde::Serialize;
    #[derive(Clone, Debug, Serialize)]
    #[serde(rename_all = "camelCase")]
    pub struct WindowWorkspaceInfo {
        pub desktop_id: String,
        pub on_current_workspace: bool,
    }
    #[derive(Default)]
    pub struct WorkspaceMoveJournal;
    pub fn window_info(_: isize) -> Result<WindowWorkspaceInfo, String> {
        Err("virtual desktops are Windows-only".into())
    }
    pub fn navigate(_: i32) -> Result<(), String> {
        Err("virtual desktops are Windows-only".into())
    }
    impl WorkspaceMoveJournal {
        pub fn move_window(&self, _: &str, _: &str) -> Result<(), String> {
            Err("virtual desktops are Windows-only".into())
        }
        pub fn undo_last(&self) -> Result<Option<String>, String> {
            Ok(None)
        }
        pub fn pending_count(&self) -> usize {
            0
        }
    }
}
#[cfg(not(windows))]
pub mod panel {
    use crate::windows::display::DisplayInfo;
    pub fn start(_: tauri::AppHandle, _: DisplayInfo) -> Result<isize, String> {
        Ok(0)
    }
    pub fn relayout(_: &DisplayInfo) -> Result<(), String> {
        Ok(())
    }
    pub fn hwnd() -> Option<isize> {
        None
    }
}
