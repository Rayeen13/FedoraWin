use serde::Serialize;
use std::{
    ffi::c_void,
    mem::{size_of, zeroed},
};

const MONITORINFOF_PRIMARY: u32 = 1;
const MDT_EFFECTIVE_DPI: u32 = 0;
const DEFAULT_DPI: u32 = 96;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct Rect {
    left: i32,
    top: i32,
    right: i32,
    bottom: i32,
}

#[repr(C)]
struct MonitorInfoExW {
    cb_size: u32,
    monitor: Rect,
    work: Rect,
    flags: u32,
    device: [u16; 32],
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RectInfo {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

impl RectInfo {
    fn from_win32(rect: Rect) -> Self {
        Self {
            left: rect.left,
            top: rect.top,
            right: rect.right,
            bottom: rect.bottom,
        }
    }

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplaySignature {
    id: String,
    bounds: RectInfo,
    dpi_x: u32,
    dpi_y: u32,
    primary: bool,
}

impl From<&DisplayInfo> for DisplaySignature {
    fn from(display: &DisplayInfo) -> Self {
        Self {
            id: display.id.clone(),
            bounds: display.bounds,
            dpi_x: display.dpi_x,
            dpi_y: display.dpi_y,
            primary: display.primary,
        }
    }
}

#[link(name = "user32")]
extern "system" {
    fn EnumDisplayMonitors(
        hdc: isize,
        clip: *const Rect,
        callback: extern "system" fn(isize, isize, *mut Rect, isize) -> i32,
        data: isize,
    ) -> i32;
    fn GetMonitorInfoW(monitor: isize, info: *mut c_void) -> i32;
}

#[link(name = "Shcore")]
extern "system" {
    fn GetDpiForMonitor(monitor: isize, dpi_type: u32, dpi_x: *mut u32, dpi_y: *mut u32) -> i32;
}

extern "system" fn enum_monitor(monitor: isize, _: isize, _: *mut Rect, data: isize) -> i32 {
    let displays = unsafe { &mut *(data as *mut Vec<DisplayInfo>) };
    let mut info: MonitorInfoExW = unsafe { zeroed() };
    info.cb_size = size_of::<MonitorInfoExW>() as u32;

    if unsafe { GetMonitorInfoW(monitor, (&mut info as *mut MonitorInfoExW).cast()) } == 0 {
        return 1;
    }

    let device_len = info
        .device
        .iter()
        .position(|ch| *ch == 0)
        .unwrap_or(info.device.len());
    let id = String::from_utf16_lossy(&info.device[..device_len]);

    let mut dpi_x = DEFAULT_DPI;
    let mut dpi_y = DEFAULT_DPI;
    let dpi_result =
        unsafe { GetDpiForMonitor(monitor, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y) };
    if dpi_result != 0 || dpi_x == 0 || dpi_y == 0 {
        dpi_x = DEFAULT_DPI;
        dpi_y = DEFAULT_DPI;
    }

    displays.push(DisplayInfo {
        id,
        bounds: RectInfo::from_win32(info.monitor),
        work_area: RectInfo::from_win32(info.work),
        dpi_x,
        dpi_y,
        scale_factor: dpi_x as f64 / DEFAULT_DPI as f64,
        primary: info.flags & MONITORINFOF_PRIMARY != 0,
    });

    1
}

pub fn enumerate() -> Result<Vec<DisplayInfo>, String> {
    let mut displays: Vec<DisplayInfo> = Vec::new();
    let ok = unsafe {
        EnumDisplayMonitors(
            0,
            std::ptr::null(),
            enum_monitor,
            &mut displays as *mut Vec<DisplayInfo> as isize,
        )
    };
    if ok == 0 {
        return Err("EnumDisplayMonitors failed".into());
    }
    if displays.is_empty() {
        return Err("Windows reported no active displays".into());
    }

    displays.sort_by(|a, b| {
        b.primary
            .cmp(&a.primary)
            .then(a.bounds.top.cmp(&b.bounds.top))
            .then(a.bounds.left.cmp(&b.bounds.left))
    });
    Ok(displays)
}

pub fn primary() -> Result<DisplayInfo, String> {
    let displays = enumerate()?;
    displays
        .iter()
        .find(|display| display.primary)
        .cloned()
        .or_else(|| displays.first().cloned())
        .ok_or_else(|| "primary display is unavailable".to_string())
}

pub fn topology_signature() -> Result<Vec<DisplaySignature>, String> {
    Ok(enumerate()?.iter().map(DisplaySignature::from).collect())
}

#[cfg(test)]
mod tests {
    use super::{DisplayInfo, DisplaySignature, RectInfo};

    fn display(bounds: RectInfo, dpi: u32) -> DisplayInfo {
        DisplayInfo {
            id: r"\\.\DISPLAY_TEST".into(),
            bounds,
            work_area: bounds,
            dpi_x: dpi,
            dpi_y: dpi,
            scale_factor: dpi as f64 / 96.0,
            primary: true,
        }
    }

    #[test]
    fn keeps_negative_desktop_coordinates() {
        let bounds = RectInfo {
            left: -3440,
            top: -120,
            right: 0,
            bottom: 1320,
        };
        assert_eq!(bounds.width(), 3440);
        assert_eq!(bounds.height(), 1440);
        assert_eq!(bounds.left, -3440);
        assert_eq!(bounds.top, -120);
    }

    #[test]
    fn logical_geometry_tracks_per_monitor_dpi() {
        let monitor = display(
            RectInfo {
                left: 0,
                top: 0,
                right: 3840,
                bottom: 2160,
            },
            144,
        );
        assert_eq!(monitor.logical_width(), 2560.0);
        assert_eq!(monitor.logical_height(), 1440.0);
        assert_eq!(monitor.logical_to_physical(32.0), 48);
    }

    #[test]
    fn super_ultrawide_geometry_is_not_clamped() {
        let monitor = display(
            RectInfo {
                left: 0,
                top: 0,
                right: 5120,
                bottom: 1440,
            },
            96,
        );
        assert_eq!(monitor.logical_width(), 5120.0);
        assert_eq!(monitor.bounds.width(), 5120);
    }

    #[test]
    fn topology_signature_ignores_our_appbar_work_area() {
        let mut first = display(
            RectInfo {
                left: 0,
                top: 0,
                right: 1920,
                bottom: 1080,
            },
            96,
        );
        let mut second = first.clone();
        first.work_area.top = 32;
        second.work_area.top = 64;

        assert_eq!(
            DisplaySignature::from(&first),
            DisplaySignature::from(&second)
        );
    }
}
