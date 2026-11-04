use parking_lot::Mutex;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

const DWM_TNP_RECTDESTINATION: u32 = 0x0000_0001;
const DWM_TNP_VISIBLE: u32 = 0x0000_0008;
const DWM_TNP_SOURCECLIENTAREAONLY: u32 = 0x0000_0010;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct Size {
    cx: i32,
    cy: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct ThumbnailProperties {
    flags: u32,
    destination: Rect,
    source: Rect,
    opacity: u8,
    visible: i32,
    source_client_area_only: i32,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThumbnailPlacement {
    pub handle: String,
    pub left: f64,
    pub top: f64,
    pub width: f64,
    pub height: f64,
}

// DWM thumbnail handles are only valid for the HWND's original owning
// process lifetime. Windows may recycle the same numeric HWND for another app.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct SourceIdentity {
    pid: u32,
    created: u64,
}

#[derive(Clone, Copy)]
struct ThumbnailRegistration {
    handle: isize,
    source: SourceIdentity,
}

#[derive(Default)]
pub struct ThumbnailManager {
    thumbnails: Mutex<HashMap<isize, ThumbnailRegistration>>,
}

#[link(name = "user32")]
extern "system" {
    fn GetWindowThreadProcessId(hwnd: isize, pid: *mut u32) -> u32;
}

fn source_identity(hwnd: isize) -> Option<SourceIdentity> {
    // This guard deliberately allows cloaked windows on another workspace but
    // refuses FedoraWin windows, Explorer shell surfaces and owned tool windows.
    if !super::windows_list::is_workspace_safe_window(hwnd) {
        return None;
    }
    let mut pid = 0u32;
    if unsafe { GetWindowThreadProcessId(hwnd, &mut pid) } == 0 || pid == 0 {
        return None;
    }
    let created = super::frame_recovery::process_creation_time(pid)?;
    let mut current_pid = 0u32;
    if !super::windows_list::is_workspace_safe_window(hwnd)
        || unsafe { GetWindowThreadProcessId(hwnd, &mut current_pid) } == 0
        || current_pid != pid
        || super::frame_recovery::process_creation_time(pid) != Some(created)
    {
        return None;
    }
    Some(SourceIdentity { pid, created })
}

fn registration_matches(
    registered: SourceIdentity,
    current: Option<SourceIdentity>,
) -> bool {
    current == Some(registered)
}

#[link(name = "dwmapi")]
extern "system" {
    fn DwmRegisterThumbnail(destination: isize, source: isize, thumbnail: *mut isize) -> i32;
    fn DwmUnregisterThumbnail(thumbnail: isize) -> i32;
    fn DwmUpdateThumbnailProperties(
        thumbnail: isize,
        properties: *const ThumbnailProperties,
    ) -> i32;
    fn DwmQueryThumbnailSourceSize(thumbnail: isize, size: *mut Size) -> i32;
}

fn fit_rect(placement: &ThumbnailPlacement, scale: f64, source: Size) -> Rect {
    let left = (placement.left * scale).round() as i32;
    let top = (placement.top * scale).round() as i32;
    let max_width = (placement.width * scale).round().max(1.0) as i32;
    let max_height = (placement.height * scale).round().max(1.0) as i32;

    if source.cx <= 0 || source.cy <= 0 {
        return Rect {
            left,
            top,
            right: left + max_width,
            bottom: top + max_height,
        };
    }

    let factor = (max_width as f64 / source.cx as f64).min(max_height as f64 / source.cy as f64);
    let width = (source.cx as f64 * factor).round().max(1.0) as i32;
    let height = (source.cy as f64 * factor).round().max(1.0) as i32;
    let x = left + (max_width - width) / 2;
    let y = top + (max_height - height) / 2;

    Rect {
        left: x,
        top: y,
        right: x + width,
        bottom: y + height,
    }
}

impl ThumbnailManager {
    pub fn sync(
        &self,
        destination: isize,
        scale: f64,
        placements: &[ThumbnailPlacement],
    ) -> Result<usize, String> {
        let mut thumbnails = self.thumbnails.lock();
        let requested: HashSet<isize> = placements
            .iter()
            .filter_map(|placement| placement.handle.parse::<isize>().ok())
            .collect();

        thumbnails.retain(|source, registration| {
            if requested.contains(source)
                && registration_matches(registration.source, source_identity(*source))
            {
                true
            } else {
                unsafe {
                    DwmUnregisterThumbnail(registration.handle);
                }
                false
            }
        });

        let mut visible = 0usize;
        for placement in placements {
            let source = placement
                .handle
                .parse::<isize>()
                .map_err(|_| "invalid window handle in thumbnail placement")?;
            let Some(identity) = source_identity(source) else {
                continue;
            };
            // A recycled HWND must not inherit a stale DWM registration.
            // Recheck here as well: the lifetime may change after retain().
            if thumbnails
                .get(&source)
                .is_some_and(|entry| !registration_matches(entry.source, Some(identity)))
            {
                if let Some(previous) = thumbnails.remove(&source) {
                    unsafe {
                        DwmUnregisterThumbnail(previous.handle);
                    }
                }
            }

            let thumbnail = if let Some(existing) = thumbnails.get(&source) {
                existing.handle
            } else {
                let mut thumbnail = 0isize;
                let result = unsafe { DwmRegisterThumbnail(destination, source, &mut thumbnail) };
                if result != 0 || thumbnail == 0 {
                    continue;
                }
                if !registration_matches(identity, source_identity(source)) {
                    unsafe {
                        DwmUnregisterThumbnail(thumbnail);
                    }
                    continue;
                }
                thumbnails.insert(
                    source,
                    ThumbnailRegistration {
                        handle: thumbnail,
                        source: identity,
                    },
                );
                thumbnail
            };
            // Do not update a registration if its HWND now names a different
            // process lifetime or is no longer an eligible app window.
            if !registration_matches(identity, source_identity(source)) {
                if let Some(previous) = thumbnails.remove(&source) {
                    unsafe {
                        DwmUnregisterThumbnail(previous.handle);
                    }
                }
                continue;
            }

            let mut source_size = Size::default();
            let _ = unsafe { DwmQueryThumbnailSourceSize(thumbnail, &mut source_size) };
            let properties = ThumbnailProperties {
                flags: DWM_TNP_RECTDESTINATION | DWM_TNP_VISIBLE | DWM_TNP_SOURCECLIENTAREAONLY,
                destination: fit_rect(placement, scale, source_size),
                source: Rect::default(),
                opacity: 255,
                visible: 1,
                // GNOME owns the overview frame; DWM supplies the live app content.
                source_client_area_only: 1,
            };

            let updated = unsafe { DwmUpdateThumbnailProperties(thumbnail, &properties) } == 0;
            if !registration_matches(identity, source_identity(source)) {
                if let Some(previous) = thumbnails.remove(&source) {
                    unsafe {
                        DwmUnregisterThumbnail(previous.handle);
                    }
                }
                continue;
            }
            if updated {
                visible += 1;
            }
        }

        Ok(visible)
    }

    pub fn clear(&self) {
        let mut thumbnails = self.thumbnails.lock();
        for (_, registration) in thumbnails.drain() {
            unsafe {
                DwmUnregisterThumbnail(registration.handle);
            }
        }
    }
}

impl Drop for ThumbnailManager {
    fn drop(&mut self) {
        self.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::{fit_rect, registration_matches, Size, SourceIdentity, ThumbnailPlacement};

    #[test]
    fn thumbnail_fit_preserves_source_aspect_ratio() {
        let placement = ThumbnailPlacement {
            handle: "1".into(),
            left: 10.0,
            top: 20.0,
            width: 400.0,
            height: 300.0,
        };
        let rect = fit_rect(&placement, 1.0, Size { cx: 1600, cy: 900 });
        assert_eq!(rect.right - rect.left, 400);
        assert_eq!(rect.bottom - rect.top, 225);
        assert_eq!(rect.top, 57);
    }

    #[test]
    fn thumbnail_fit_tracks_webview_scale_factor() {
        let placement = ThumbnailPlacement {
            handle: "1".into(),
            left: 20.0,
            top: 30.0,
            width: 300.0,
            height: 200.0,
        };
        let rect = fit_rect(&placement, 1.5, Size { cx: 1200, cy: 800 });
        assert_eq!(rect.left, 30);
        assert_eq!(rect.top, 45);
        assert_eq!(rect.right - rect.left, 450);
        assert_eq!(rect.bottom - rect.top, 300);
    }

    #[test]
    fn thumbnail_registration_rejects_recycled_or_unverifiable_source() {
        let original = SourceIdentity {
            pid: 41,
            created: 100,
        };
        assert!(registration_matches(original, Some(original)));
        assert!(!registration_matches(
            original,
            Some(SourceIdentity {
                pid: 42,
                created: 100,
            })
        ));
        assert!(!registration_matches(
            original,
            Some(SourceIdentity {
                pid: 41,
                created: 101,
            })
        ));
        assert!(!registration_matches(original, None));
    }
}
