use crate::windows::display::DisplayInfo;

pub const PANEL_HEIGHT: f64 = 32.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SurfaceGeometry {
    pub width: f64,
    pub height: f64,
    pub x: i32,
    pub y: i32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ShellLayout {
    pub panel: SurfaceGeometry,
    pub activities: SurfaceGeometry,
    pub date_menu: SurfaceGeometry,
    pub quick_settings: SurfaceGeometry,
}

fn fit_surface(preferred: f64, available: f64, margin: f64) -> f64 {
    preferred.min((available - margin * 2.0).max(1.0))
}

pub fn for_display(display: &DisplayInfo) -> ShellLayout {
    let logical_width = display.logical_width();
    let logical_height = display.logical_height();
    let shell_height = (logical_height - PANEL_HEIGHT).max(1.0);
    let panel_height_px = display.logical_to_physical(PANEL_HEIGHT);
    let shell_top = display.bounds.top + panel_height_px;

    let date_width = fit_surface(760.0, logical_width, 12.0);
    let date_height = fit_surface(540.0, shell_height, 12.0);
    let date_width_px = display.logical_to_physical(date_width);
    let date_x = display.bounds.left + ((display.bounds.width() - date_width_px) / 2).max(0);

    let quick_width = fit_surface(408.0, logical_width, 8.0);
    let quick_height = fit_surface(510.0, shell_height, 8.0);
    let quick_width_px = display.logical_to_physical(quick_width);
    let quick_margin_px = display.logical_to_physical(8.0);
    let quick_x =
        display.bounds.left + (display.bounds.width() - quick_width_px - quick_margin_px).max(0);

    ShellLayout {
        panel: SurfaceGeometry {
            width: logical_width,
            height: PANEL_HEIGHT,
            x: display.bounds.left,
            y: display.bounds.top,
        },
        activities: SurfaceGeometry {
            width: logical_width,
            height: shell_height,
            x: display.bounds.left,
            y: shell_top,
        },
        date_menu: SurfaceGeometry {
            width: date_width,
            height: date_height,
            x: date_x,
            y: shell_top,
        },
        quick_settings: SurfaceGeometry {
            width: quick_width,
            height: quick_height,
            x: quick_x,
            y: shell_top,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::{for_display, PANEL_HEIGHT};
    use crate::windows::display::{DisplayInfo, RectInfo};

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
    fn super_ultrawide_keeps_native_width() {
        let layout = for_display(&display(
            RectInfo {
                left: 0,
                top: 0,
                right: 5120,
                bottom: 1440,
            },
            96,
        ));

        assert_eq!(layout.panel.width, 5120.0);
        assert_eq!(layout.panel.height, PANEL_HEIGHT);
        assert_eq!(layout.quick_settings.x, 4704);
    }

    #[test]
    fn portrait_display_clamps_large_popovers() {
        let layout = for_display(&display(
            RectInfo {
                left: 0,
                top: 0,
                right: 1080,
                bottom: 1920,
            },
            144,
        ));

        assert_eq!(layout.panel.width, 720.0);
        assert_eq!(layout.date_menu.width, 696.0);
        assert!(layout.date_menu.height <= layout.activities.height);
    }

    #[test]
    fn negative_monitor_origin_is_preserved() {
        let layout = for_display(&display(
            RectInfo {
                left: -3440,
                top: -120,
                right: 0,
                bottom: 1320,
            },
            96,
        ));

        assert_eq!(layout.panel.x, -3440);
        assert_eq!(layout.panel.y, -120);
        assert_eq!(layout.activities.x, -3440);
        assert_eq!(layout.activities.y, -88);
    }

    #[test]
    fn mixed_dpi_uses_physical_offset_for_panel_height() {
        let layout = for_display(&display(
            RectInfo {
                left: 3840,
                top: 0,
                right: 7680,
                bottom: 2160,
            },
            144,
        ));

        assert_eq!(layout.panel.width, 2560.0);
        assert_eq!(layout.activities.y, 48);
        assert_eq!(layout.activities.x, 3840);
    }
}
