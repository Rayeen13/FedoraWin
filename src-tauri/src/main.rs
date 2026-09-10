#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod shell;
mod windows;

use shell::{AppearanceState, ShellState};
use std::sync::Arc;
use tauri::{LogicalPosition, Manager, WebviewUrl, WebviewWindowBuilder};

const PANEL_HEIGHT: f64 = 32.0;

#[tauri::command]
fn get_shell_state(state: tauri::State<'_, Arc<ShellState>>) -> shell::ShellSnapshot {
    state.snapshot()
}

#[tauri::command]
fn set_appearance(
    state: tauri::State<'_, Arc<ShellState>>,
    theme: String,
    accent: String,
) -> Result<shell::ShellSnapshot, String> {
    state.set_appearance(AppearanceState::parse(&theme, &accent)?)?;
    let snapshot = state.snapshot();
    windows::frame::apply_to_top_level_windows(&snapshot.appearance).map_err(|e| e.to_string())?;
    Ok(snapshot)
}

#[tauri::command]
fn toggle_activities(app: tauri::AppHandle) -> Result<(), String> {
    shell::toggle_activities(&app)
}

#[tauri::command]
fn toggle_surface(app: tauri::AppHandle, label: String) -> Result<(), String> {
    if !matches!(label.as_str(), "date-menu" | "quick-settings") {
        return Err("unsupported shell surface".into());
    }
    let window = app
        .get_webview_window(&label)
        .ok_or_else(|| format!("{label} window is unavailable"))?;
    let visible = window.is_visible().map_err(|e| e.to_string())?;
    if visible {
        window.hide().map_err(|e| e.to_string())?;
    } else {
        for other in ["date-menu", "quick-settings"] {
            if other != label {
                if let Some(w) = app.get_webview_window(other) {
                    let _ = w.hide();
                }
            }
        }
        window.show().map_err(|e| e.to_string())?;
        window.set_focus().map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn list_apps() -> Result<Vec<windows::apps::AppEntry>, String> {
    windows::apps::list()
}

#[tauri::command]
fn launch_app(app_id: String) -> Result<(), String> {
    windows::apps::launch(&app_id)
}

#[tauri::command]
fn list_windows() -> Result<Vec<windows::windows_list::WindowEntry>, String> {
    windows::windows_list::list()
}

#[tauri::command]
fn activate_window(handle: String) -> Result<(), String> {
    windows::windows_list::activate(&handle)
}

#[tauri::command]
fn set_wifi_enabled(enabled: bool) -> Result<(), String> {
    windows::wifi::set_enabled(enabled).map_err(|e| e.to_string())
}

#[tauri::command]
fn refresh_window_frames(state: tauri::State<'_, Arc<ShellState>>) -> Result<usize, String> {
    windows::frame::apply_to_top_level_windows(&state.snapshot().appearance)
        .map_err(|e| e.to_string())
}

fn build_window(
    app: &tauri::App,
    label: &str,
    view: &str,
    width: f64,
    height: f64,
    visible: bool,
    position: LogicalPosition<f64>,
) -> tauri::Result<()> {
    let url = WebviewUrl::App(format!("index.html?view={view}").into());
    let window = WebviewWindowBuilder::new(app, label, url)
        .title("FedoraWin")
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(visible)
        .inner_size(width, height)
        .build()?;
    window.set_position(position)?;
    Ok(())
}

fn main() {
    let state = Arc::new(ShellState::default());

    tauri::Builder::default()
        .manage(state.clone())
        .invoke_handler(tauri::generate_handler![
            get_shell_state,
            set_appearance,
            toggle_activities,
            toggle_surface,
            list_apps,
            launch_app,
            list_windows,
            activate_window,
            set_wifi_enabled,
            refresh_window_frames
        ])
        .setup(move |app| {
            let monitor = app
                .primary_monitor()?
                .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "primary monitor unavailable"))?;
            let size = monitor.size();
            let scale = monitor.scale_factor();
            let logical_width = size.width as f64 / scale;
            let logical_height = size.height as f64 / scale;

            build_window(app, "panel", "panel", logical_width, PANEL_HEIGHT, true, LogicalPosition::new(0.0, 0.0))?;
            build_window(app, "activities", "activities", logical_width, logical_height - PANEL_HEIGHT, false, LogicalPosition::new(0.0, PANEL_HEIGHT))?;
            build_window(app, "date-menu", "date-menu", 760.0, 540.0, false, LogicalPosition::new(((logical_width - 760.0) / 2.0).max(0.0), PANEL_HEIGHT))?;
            build_window(app, "quick-settings", "quick-settings", 408.0, 510.0, false, LogicalPosition::new((logical_width - 416.0).max(0.0), PANEL_HEIGHT))?;

            #[cfg(windows)]
            {
                if let Some(panel) = app.get_webview_window("panel") {
                    let hwnd = panel.hwnd()?;
                    windows::appbar::reserve_top(hwnd.0 as isize)?;
                }
                windows::frame::start_frame_watcher(state.clone());
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("FedoraWin runtime failed");
}
