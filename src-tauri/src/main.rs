#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod layout;
mod performance;
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
fn get_memory_snapshot() -> Result<performance::MemorySnapshot, String> {
    performance::snapshot()
}
#[tauri::command]
fn toggle_surface(app: tauri::AppHandle, label: String) -> Result<(), String> {
    shell::toggle_surface(&app, &label)
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
fn close_window(handle: String) -> Result<(), String> {
    windows::windows_list::close(&handle)
}
#[tauri::command]
fn move_window_to_workspace(
    state: tauri::State<'_, windows::virtual_desktop::WorkspaceMoveJournal>,
    handle: String,
    desktop_id: String,
) -> Result<(), String> {
    state.move_window(&handle, &desktop_id)
}
#[tauri::command]
fn undo_workspace_move(
    state: tauri::State<'_, windows::virtual_desktop::WorkspaceMoveJournal>,
) -> Result<Option<String>, String> {
    state.undo_last()
}
#[tauri::command]
fn workspace_move_status(
    state: tauri::State<'_, windows::virtual_desktop::WorkspaceMoveJournal>,
) -> usize {
    state.pending_count()
}
#[tauri::command]
fn navigate_workspace(app: tauri::AppHandle, direction: i32) -> Result<(), String> {
    shell::hide_activities(&app)?;
    windows::virtual_desktop::navigate(direction)
}
#[tauri::command]
fn sync_window_thumbnails(
    app: tauri::AppHandle,
    state: tauri::State<'_, windows::thumbnails::ThumbnailManager>,
    items: Vec<windows::thumbnails::ThumbnailPlacement>,
) -> Result<usize, String> {
    #[cfg(windows)]
    {
        let activities = app
            .get_webview_window("activities")
            .ok_or_else(|| "activities window is unavailable".to_string())?;
        let hwnd = activities.hwnd().map_err(|e| e.to_string())?;
        let scale = activities.scale_factor().map_err(|e| e.to_string())?;
        return state.sync(hwnd.0 as isize, scale, &items);
    }
    #[cfg(not(windows))]
    {
        let _ = (app, state, items);
        Ok(0)
    }
}
#[tauri::command]
fn clear_window_thumbnails(
    state: tauri::State<'_, windows::thumbnails::ThumbnailManager>,
) -> Result<(), String> {
    state.clear();
    Ok(())
}
#[tauri::command]
fn get_master_volume() -> Result<u8, String> {
    windows::audio::get_master_volume()
}
#[tauri::command]
fn set_master_volume(value: u8) -> Result<u8, String> {
    windows::audio::set_master_volume(value)
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
#[tauri::command]
fn mark_capture_ready(app: tauri::AppHandle, label: String) -> Result<(), String> {
    if !matches!(
        label.as_str(),
        "activities" | "date-menu" | "quick-settings"
    ) {
        return Err("unsupported capture surface".into());
    }
    let window = app
        .get_webview_window(&label)
        .ok_or_else(|| format!("{label} window is unavailable"))?;
    window
        .set_title(&format!("FedoraWin — {label} — ready"))
        .map_err(|e| e.to_string())
}

fn build_window(
    app: &tauri::AppHandle,
    label: &str,
    view: &str,
    geometry: layout::SurfaceGeometry,
    visible: bool,
    capture_mode: Option<&str>,
) -> Result<tauri::WebviewWindow, String> {
    let url = match capture_mode {
        Some(mode) => format!("index.html?view={view}&capture={mode}"),
        None => format!("index.html?view={view}"),
    };
    let window = WebviewWindowBuilder::new(app, label, WebviewUrl::App(url.into()))
        .title(format!("FedoraWin — {label}"))
        .decorations(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .visible(visible)
        .inner_size(geometry.width, geometry.height)
        .build()
        .map_err(|e| e.to_string())?;
    window
        .set_position(PhysicalPosition::new(geometry.x, geometry.y))
        .map_err(|e| e.to_string())?;
    Ok(window)
}
fn apply_surface_geometry(
    window: &tauri::WebviewWindow,
    geometry: layout::SurfaceGeometry,
) -> Result<(), String> {
    window
        .set_size(LogicalSize::new(geometry.width, geometry.height))
        .map_err(|e| e.to_string())?;
    window
        .set_position(PhysicalPosition::new(geometry.x, geometry.y))
        .map_err(|e| e.to_string())
}
fn surface_geometry(label: &str) -> Result<layout::SurfaceGeometry, String> {
    let display = windows::display::primary()?;
    let l = layout::for_display(&display);
    match label {
        "activities" => Ok(l.activities),
        "date-menu" => Ok(l.date_menu),
        "quick-settings" => Ok(l.quick_settings),
        _ => Err(format!("unsupported shell surface: {label}")),
    }
}
fn ensure_shell_surface(
    app: &tauri::AppHandle,
    label: &str,
    view: &str,
    visible: bool,
    capture_mode: Option<&str>,
) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(label) {
        apply_surface_geometry(&window, surface_geometry(label)?)?;
        if visible {
            window.show().map_err(|e| e.to_string())?;
        }
        return Ok(window);
    }
    build_window(
        app,
        label,
        view,
        surface_geometry(label)?,
        visible,
        capture_mode,
    )
}
pub(crate) fn ensure_activities_window(
    app: &tauri::AppHandle,
) -> Result<tauri::WebviewWindow, String> {
    ensure_shell_surface(app, "activities", "activities", false, None)
}
fn relayout_shell_surfaces(app: &tauri::AppHandle) -> Result<(), String> {
    let display = windows::display::primary()?;
    let l = layout::for_display(&display);
    #[cfg(windows)]
    windows::panel::relayout(&display)?;
    for (label, g) in [
        ("activities", l.activities),
        ("date-menu", l.date_menu),
        ("quick-settings", l.quick_settings),
    ] {
        if let Some(w) = app.get_webview_window(label) {
            apply_surface_geometry(&w, g)?;
        }
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
                Ok(s) => s,
                Err(_) => continue,
            };
            if last.as_ref() == Some(&next) {
                continue;
            }
            thread::sleep(Duration::from_millis(180));
            let _ = relayout_shell_surfaces(&app);
            last = windows::display::topology_signature().ok().or(Some(next));
        }
    });
}

fn main() {
    let state = Arc::new(ShellState::default());
    let capture_view = std::env::var("FEDORAWIN_CAPTURE_VIEW").ok();
    let capture_mode = std::env::var("FEDORAWIN_CAPTURE_MODE").ok();
    if let Ok(theme) = std::env::var("FEDORAWIN_CAPTURE_THEME") {
        if let Ok(a) = AppearanceState::parse(&theme, "blue") {
            let _ = state.set_appearance(a);
        }
    }
    let app = tauri::Builder::default()
        .manage(state.clone())
        .manage(windows::thumbnails::ThumbnailManager::default())
        .manage(windows::virtual_desktop::WorkspaceMoveJournal::default())
        .invoke_handler(tauri::generate_handler![
            get_shell_state,
            set_appearance,
            toggle_activities,
            toggle_surface,
            get_memory_snapshot,
            list_apps,
            list_displays,
            launch_app,
            list_windows,
            activate_window,
            close_window,
            move_window_to_workspace,
            undo_workspace_move,
            workspace_move_status,
            navigate_workspace,
            sync_window_thumbnails,
            clear_window_thumbnails,
            get_master_volume,
            set_master_volume,
            set_wifi_enabled,
            refresh_window_frames,
            mark_capture_ready
        ])
        .setup(move |app| {
            let display = windows::display::primary().map_err(std::io::Error::other)?;
            let l = layout::for_display(&display);
            let capture = capture_mode.as_deref();
            let av = capture_view.as_deref() == Some("activities");
            let dv = capture_view.as_deref() == Some("date-menu");
            let qv = capture_view.as_deref() == Some("quick-settings");
            #[cfg(windows)]
            windows::panel::start(app.handle().clone(), display.clone())
                .map_err(std::io::Error::other)?;
            #[cfg(not(windows))]
            build_window(app.handle(), "panel", "panel", l.panel, true, capture)
                .map_err(std::io::Error::other)?;
            if av {
                build_window(
                    app.handle(),
                    "activities",
                    "activities",
                    l.activities,
                    true,
                    capture,
                )
                .map_err(std::io::Error::other)?;
            }
            if dv {
                build_window(
                    app.handle(),
                    "date-menu",
                    "date-menu",
                    l.date_menu,
                    true,
                    capture,
                )
                .map_err(std::io::Error::other)?;
            }
            if qv {
                build_window(
                    app.handle(),
                    "quick-settings",
                    "quick-settings",
                    l.quick_settings,
                    true,
                    capture,
                )
                .map_err(std::io::Error::other)?;
            }
            #[cfg(windows)]
            {
                windows::frame::start_frame_watcher(state.clone());
                let _ = windows::hotkeys::start_activities_hotkey(app.handle().clone());
                let _ = windows::window_events::start(app.handle().clone());
                start_display_topology_watcher(app.handle().clone());
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("FedoraWin runtime failed");
    app.run(|_, event| {
        if matches!(event, tauri::RunEvent::Exit) {
            #[cfg(windows)]
            windows::frame::reset_top_level_windows();
        }
    });
}
