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
        Self { theme: ThemeMode::Dark, accent: Accent::Blue }
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

    fn flip_activities(&self) -> bool {
        let previous = self.activities_open.fetch_xor(true, Ordering::SeqCst);
        !previous
    }
}

pub fn toggle_activities(app: &tauri::AppHandle) -> Result<(), String> {
    let state = app.state::<std::sync::Arc<ShellState>>();
    let open = state.flip_activities();
    let window = app
        .get_webview_window("activities")
        .ok_or_else(|| "activities window is unavailable".to_string())?;

    if open {
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    } else {
        window.hide().map_err(|e| e.to_string())?;
    }
    Ok(())
}
