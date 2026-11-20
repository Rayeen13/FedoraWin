use parking_lot::RwLock;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::Manager;

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppearanceState {
    pub theme: ThemeMode,
    pub accent: Accent,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Accent {
    Blue,
    Teal,
    Green,
    Yellow,
    Orange,
    Red,
    Pink,
    Purple,
    Slate,
}

impl AppearanceState {
    pub fn parse(theme: &str, accent: &str) -> Result<Self, String> {
        let theme = match theme {
            "system" => ThemeMode::System,
            "light" => ThemeMode::Light,
            "dark" => ThemeMode::Dark,
            _ => return Err(format!("unsupported theme: {theme}")),
        };
        let accent = match accent {
            "blue" => Accent::Blue,
            "teal" => Accent::Teal,
            "green" => Accent::Green,
            "yellow" => Accent::Yellow,
            "orange" => Accent::Orange,
            "red" => Accent::Red,
            "pink" => Accent::Pink,
            "purple" => Accent::Purple,
            "slate" => Accent::Slate,
            _ => return Err(format!("unsupported accent: {accent}")),
        };
        Ok(Self { theme, accent })
    }
}

impl Default for AppearanceState {
    fn default() -> Self {
        Self {
            theme: ThemeMode::Dark,
            accent: Accent::Blue,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellSnapshot {
    pub appearance: AppearanceState,
    pub activities_open: bool,
}

pub struct ShellState {
    appearance: RwLock<AppearanceState>,
    activities_open: AtomicBool,
}

impl Default for ShellState {
    fn default() -> Self {
        Self {
            appearance: RwLock::new(AppearanceState::default()),
            activities_open: AtomicBool::new(false),
        }
    }
}

impl ShellState {
    pub fn snapshot(&self) -> ShellSnapshot {
        ShellSnapshot {
            appearance: self.appearance.read().clone(),
            activities_open: self.activities_open.load(Ordering::SeqCst),
        }
    }

    pub fn set_appearance(&self, appearance: AppearanceState) -> Result<(), String> {
        *self.appearance.write() = appearance;
        Ok(())
    }

    fn set_activities_open(&self, open: bool) {
        self.activities_open.store(open, Ordering::SeqCst);
    }
}

fn hide_popovers(app: &tauri::AppHandle) {
    for label in ["date-menu", "quick-settings"] {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.close();
        }
    }
}

pub fn hide_activities(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<std::sync::Arc<ShellState>>();
    if let Some(window) = app.get_webview_window("activities") {
        app.state::<crate::windows::thumbnails::ThumbnailManager>()
            .clear();
        window.close().map_err(|e| e.to_string())?;
    }
    state.set_activities_open(false);
    Ok(())
}

pub fn toggle_surface(app: &tauri::AppHandle, label: &str) -> Result<(), String> {
    if !matches!(label, "date-menu" | "quick-settings") {
        return Err("unsupported shell surface".into());
    }

    if let Some(window) = app.get_webview_window(label) {
        if window.is_visible().map_err(|error| error.to_string())? {
            window.close().map_err(|error| error.to_string())?;
            return Ok(());
        }
    }

    hide_activities(app)?;
    for other in ["date-menu", "quick-settings"] {
        if other != label {
            if let Some(window) = app.get_webview_window(other) {
                let _ = window.close();
            }
        }
    }

    let window = crate::ensure_shell_surface(app, label, label, true, None)?;
    if let Err(error) = window.set_focus() {
        let _ = window.close();
        return Err(error.to_string());
    }
    Ok(())
}

pub fn toggle_activities(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<std::sync::Arc<ShellState>>();
    let window = crate::ensure_activities_window(app)?;
    let currently_visible = window.is_visible().map_err(|e| e.to_string())?;

    if currently_visible {
        app.state::<crate::windows::thumbnails::ThumbnailManager>()
            .clear();
        window.close().map_err(|e| e.to_string())?;
        state.set_activities_open(false);
        return Ok(());
    }

    hide_popovers(app);
    window.show().map_err(|e| e.to_string())?;
    if let Err(error) = window.set_focus() {
        let _ = window.close();
        state.set_activities_open(false);
        return Err(error.to_string());
    }

    state.set_activities_open(true);
    Ok(())
}
