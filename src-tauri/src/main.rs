#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod layout;
mod shell;
mod windows;

use shell::{AppearanceState, ShellState};
use std::sync::Arc;
#[cfg(windows)]
use std::{thread, time::Duration};
use tauri::{LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

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
        shell::hide_activities(&app)?;
        for other in ["date-menu", "quick-settings"] {
            if other != label {
                if let Some(w) = app.get_webview_window(other) {
                    let _ = w.hide();
                }
            }
        }
        window.show().map_err(|e| e.to_string())?;
        if let Err(error) = window.set_focus() {
            let _ = window.hide();
            return Err(error.to_string());
        }
    }
    Ok(())
}

#[tauri::command]
fn list_apps() -> Result<Vec<windows::apps::AppEntry>, String> {
    windows::apps::list()
}

#[tauri::command]
fn list_displays() -> Result<Vec<windows::display::DisplayInfo>, String> {
    windows::display::enumerate()
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
    geometry: layout::SurfaceGeometry,
    visible: bool,
) -> tauri::Result<()> {
    let url = WebviewUrl::App(format!("index.html?view={view}").into());
    let window = WebviewWindowBuilder::new(app, label, url)
        .title("FedoraWin")
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(visible)
        .inner_size(geometry.width, geometry.height)
        .build()?;
    window.set_position(PhysicalPosition::new(geometry.x, geometry.y))?;
    Ok(())
}

fn apply_surface_geometry(
    window: &tauri::WebviewWindow,
    geometry: layout::SurfaceGeometry,
) -> Result<(), String> {
    window
        .set_size(LogicalSize::new(geometry.width, geometry.height))
        .map_err(|error| error.to_string())?;
    window
        .set_position(PhysicalPosition::new(geometry.x, geometry.y))
        .map_err(|error| error.to_string())
}

fn relayout_shell_surfaces(app: &tauri::AppHandle) -> Result<(), String> {
    let display = windows::display::primary()?;
    let shell_layout = layout::for_display(&display);

    #[cfg(windows)]
    let panel_hwnd = app
        .get_webview_window("panel")
        .and_then(|panel| panel.hwnd().ok())
        .map(|hwnd| hwnd.0 as isize);

    #[cfg(windows)]
    if let Some(hwnd) = panel_hwnd {
        windows::appbar::release(hwnd);
    }

    for (label, geometry) in [
        ("panel", shell_layout.panel),
        ("activities", shell_layout.activities),
        ("date-menu", shell_layout.date_menu),
        ("quick-settings", shell_layout.quick_settings),
    ] {
        let window = app
            .get_webview_window(label)
            .ok_or_else(|| format!("{label} window is unavailable"))?;
        apply_surface_geometry(&window, geometry)?;
    }

    #[cfg(windows)]
    if let Some(hwnd) = panel_hwnd {
        windows::appbar::reserve_top(hwnd)?;
    }

    Ok(())
}

#[cfg(windows)]
fn start_display_topology_watcher(app: tauri::AppHandle) {
    thread::spawn(move || {
        let mut last = windows::display::topology_signature().ok();

        loop {
            thread::sleep(Duration::from_millis(900));

            let next = match windows::display::topology_signature() {
                Ok(signature) => signature,
                Err(_) => continue,
            };

            if last.as_ref() == Some(&next) {
                continue;
            }

            // Give Windows a short settle window so a dock/undock or orientation
            // change can publish its complete monitor topology before we reflow.
            thread::sleep(Duration::from_millis(180));
            let _ = relayout_shell_surfaces(&app);
            last = windows::display::topology_signature().ok().or(Some(next));
        }
    });
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
            list_displays,
            launch_app,
            list_windows,
            activate_window,
            set_wifi_enabled,
            refresh_window_frames
        ])
        .setup(move |app| {
            let display = windows::display::primary().map_err(std::io::Error::other)?;
            let shell_layout = layout::for_display(&display);

            build_window(app, "panel", "panel", shell_layout.panel, true)?;
            build_window(
                app,
                "activities",
                "activities",
                shell_layout.activities,
                false,
            )?;
            build_window(app, "date-menu", "date-menu", shell_layout.date_menu, false)?;
            build_window(
                app,
                "quick-settings",
                "quick-settings",
                shell_layout.quick_settings,
                false,
            )?;

            #[cfg(windows)]
            {
                if let Some(panel) = app.get_webview_window("panel") {
                    let hwnd = panel.hwnd()?;
                    windows::appbar::reserve_top(hwnd.0 as isize)?;
                }
                windows::frame::start_frame_watcher(state.clone());
                let _ = windows::hotkeys::start_activities_hotkey(app.handle().clone());
                start_display_topology_watcher(app.handle().clone());
            }

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("FedoraWin runtime failed");
}
