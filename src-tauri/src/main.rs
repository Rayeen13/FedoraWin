#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod shell;
mod windows;

use shell::{AppearanceState, ShellState};
use std::sync::Arc;
use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

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
    Ok(state.snapshot())
}

#[tauri::command]
fn toggle_activities(app: tauri::AppHandle) -> Result<(), String> {
    shell::toggle_activities(&app)
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
) -> tauri::Result<()> {
    let url = WebviewUrl::App(format!("index.html?view={view}").into());
    WebviewWindowBuilder::new(app, label, url)
        .title("FedoraWin")
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(visible)
        .inner_size(width, height)
        .build()?;
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

            build_window(app, "panel", "panel", logical_width, 32.0, true)?;
            build_window(app, "activities", "activities", logical_width, logical_height - 32.0, false)?;
            build_window(app, "date-menu", "date-menu", 760.0, 620.0, false)?;
            build_window(app, "quick-settings", "quick-settings", 420.0, 560.0, false)?;

            #[cfg(windows)]
            {
                if let Some(panel) = app.get_webview_window("panel") {
                    let hwnd = panel.hwnd()?;
                    windows::appbar::reserve_top(hwnd.0 as isize, 32)?;
                }
                windows::frame::start_frame_watcher(state.clone());
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("FedoraWin runtime failed");
}
