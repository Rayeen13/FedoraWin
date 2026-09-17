use crate::{layout, windows};
use serde::Serialize;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc,
};
use std::{thread, time::Duration};
use tauri::{Emitter, LogicalSize, Manager, PhysicalPosition, WebviewUrl, WebviewWindowBuilder};

const OSD_LABEL: &str = "osd";
const OSD_EVENT: &str = "fedorawin://osd";
const OSD_TIMEOUT_MS: u64 = 1600;

#[derive(Clone, Default)]
pub struct OsdState {
    revision: Arc<AtomicU64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OsdPayload {
    kind: String,
    value: Option<u8>,
    detail: Option<String>,
}

impl OsdPayload {
    fn new(kind: &str, value: Option<u8>, detail: Option<&str>) -> Result<Self, String> {
        if !matches!(kind, "volume" | "brightness" | "power" | "media") {
            return Err(format!("unsupported OSD kind: {kind}"));
        }
        Ok(Self {
            kind: kind.to_string(),
            value: value.map(|value| value.min(100)),
            detail: detail.map(str::to_string),
        })
    }

    fn initial_url(&self) -> String {
        let value = self.value.map(|value| value.to_string()).unwrap_or_default();
        let detail = self.detail.as_deref().unwrap_or_default();
        format!(
            "osd.html?kind={}&value={value}&detail={detail}",
            self.kind
        )
    }
}

fn position_window(window: &tauri::WebviewWindow) -> Result<(), String> {
    let display = windows::display::primary()?;
    let geometry = layout::for_display(&display).osd;
    window
        .set_size(LogicalSize::new(geometry.width, geometry.height))
        .map_err(|error| error.to_string())?;
    window
        .set_position(PhysicalPosition::new(geometry.x, geometry.y))
        .map_err(|error| error.to_string())
}

fn ensure_window(
    app: &tauri::AppHandle,
    payload: &OsdPayload,
) -> Result<tauri::WebviewWindow, String> {
    if let Some(window) = app.get_webview_window(OSD_LABEL) {
        position_window(&window)?;
        window
            .emit(OSD_EVENT, payload.clone())
            .map_err(|error| error.to_string())?;
        window.show().map_err(|error| error.to_string())?;
        return Ok(window);
    }

    let display = windows::display::primary()?;
    let geometry = layout::for_display(&display).osd;
    let window = WebviewWindowBuilder::new(
        app,
        OSD_LABEL,
        WebviewUrl::App(payload.initial_url().into()),
    )
    .title("FedoraWin — control OSD")
    .decorations(false)
    .resizable(false)
    .skip_taskbar(true)
    .always_on_top(true)
    .transparent(true)
    .visible(true)
    .inner_size(geometry.width, geometry.height)
    .build()
    .map_err(|error| error.to_string())?;
    window
        .set_position(PhysicalPosition::new(geometry.x, geometry.y))
        .map_err(|error| error.to_string())?;
    window
        .set_ignore_cursor_events(true)
        .map_err(|error| error.to_string())?;
    Ok(window)
}

pub fn show(
    app: &tauri::AppHandle,
    state: &OsdState,
    kind: &str,
    value: Option<u8>,
    detail: Option<&str>,
) -> Result<(), String> {
    let payload = OsdPayload::new(kind, value, detail)?;
    let revision = state.revision.fetch_add(1, Ordering::SeqCst) + 1;
    let _ = ensure_window(app, &payload)?;

    let app = app.clone();
    let live_revision = state.revision.clone();
    thread::Builder::new()
        .name("fedorawin-osd-timeout".into())
        .spawn(move || {
            thread::sleep(Duration::from_millis(OSD_TIMEOUT_MS));
            if live_revision.load(Ordering::SeqCst) != revision {
                return;
            }
            if let Some(window) = app.get_webview_window(OSD_LABEL) {
                let _ = window.close();
            }
        })
        .map_err(|error| format!("failed to start OSD timeout: {error}"))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::OsdPayload;

    #[test]
    fn rejects_unknown_osd_kinds() {
        assert!(OsdPayload::new("unknown", None, None).is_err());
    }

    #[test]
    fn clamps_osd_levels() {
        let payload = OsdPayload::new("volume", Some(180), None).unwrap();
        assert_eq!(payload.value, Some(100));
    }

    #[test]
    fn initial_url_keeps_controlled_values_compact() {
        let payload = OsdPayload::new("power", None, Some("balanced")).unwrap();
        assert_eq!(
            payload.initial_url(),
            "osd.html?kind=power&value=&detail=balanced"
        );
    }
}
