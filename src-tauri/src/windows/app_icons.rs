//! Read the actual icon of an AppsFolder entry, including packaged UWP apps.
//! Shell parsing and image extraction stay off the WebView/UI thread.
use base64::Engine as _;
use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::ptr::{null, null_mut};
use std::sync::{Mutex, OnceLock};

const SHGFI_ICON: u32 = 0x0000_0100;
const SHGFI_PIDL: u32 = 0x0000_0008;
const DIB_RGB_COLORS: u32 = 0;
const BI_RGB: u32 = 0;
const DI_NORMAL: u32 = 0x0003;
const COINIT_APARTMENTTHREADED: u32 = 0x2;
const ICON_SIZE: i32 = 64;
const MAX_CACHE_ENTRIES: usize = 256;

#[repr(C)]
struct ShellFileInfo {
    icon: isize,
    icon_index: i32,
    attributes: u32,
    display_name: [u16; 260],
    type_name: [u16; 80],
}

#[repr(C)]
struct BitmapInfoHeader {
    size: u32,
    width: i32,
    height: i32,
    planes: u16,
    bit_count: u16,
    compression: u32,
    image_size: u32,
    x_pels_per_meter: i32,
    y_pels_per_meter: i32,
    colors_used: u32,
    colors_important: u32,
}

#[repr(C)]
struct BitmapInfo {
    header: BitmapInfoHeader,
    color: [u32; 3],
}

#[link(name = "ole32")]
extern "system" {
    fn CoInitializeEx(reserved: *const c_void, model: u32) -> i32;
    fn CoUninitialize();
    fn CoTaskMemFree(memory: *mut c_void);
}

#[link(name = "shell32")]
extern "system" {
    fn SHParseDisplayName(
        name: *const u16,
        bind_ctx: *const c_void,
        pidl: *mut *mut c_void,
        attributes: u32,
        result_attributes: *mut u32,
    ) -> i32;
    fn SHGetFileInfoW(
        name_or_pidl: *const u16,
        attributes: u32,
        output: *mut ShellFileInfo,
        output_size: u32,
        flags: u32,
    ) -> usize;
}

#[link(name = "user32")]
extern "system" {
    fn DestroyIcon(icon: isize) -> i32;
    fn DrawIconEx(
        hdc: isize,
        x: i32,
        y: i32,
        icon: isize,
        width: i32,
        height: i32,
        step: u32,
        brush: isize,
        flags: u32,
    ) -> i32;
}

#[link(name = "gdi32")]
extern "system" {
    fn CreateCompatibleDC(hdc: isize) -> isize;
    fn DeleteDC(hdc: isize) -> i32;
    fn CreateDIBSection(
        hdc: isize,
        info: *const BitmapInfo,
        usage: u32,
        bits: *mut *mut c_void,
        section: isize,
        offset: u32,
    ) -> isize;
    fn SelectObject(hdc: isize, object: isize) -> isize;
    fn DeleteObject(object: isize) -> i32;
}

fn cache() -> &'static Mutex<HashMap<String, Option<String>>> {
    static ICONS: OnceLock<Mutex<HashMap<String, Option<String>>>> = OnceLock::new();
    ICONS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn valid_app_id(app_id: &str) -> bool {
    !app_id.is_empty()
        && app_id.len() <= 320
        && !app_id
            .chars()
            .any(|value| matches!(value, '\0' | '\r' | '\n'))
}

pub fn icon_data_uri(app_id: &str) -> Option<String> {
    if !valid_app_id(app_id) {
        return None;
    }
    if let Some(value) = cache().lock().ok()?.get(app_id).cloned() {
        return value;
    }
    let icon = unsafe { extract(app_id) };
    if let Ok(mut icons) = cache().lock() {
        if icons.len() >= MAX_CACHE_ENTRIES {
            icons.clear();
        }
        icons.insert(app_id.to_owned(), icon.clone());
    }
    icon
}

unsafe fn extract(app_id: &str) -> Option<String> {
    let initialized = CoInitializeEx(null(), COINIT_APARTMENTTHREADED) >= 0;
    // RPC_E_CHANGED_MODE means COM was already initialized in another mode.
    // Keep the caller's COM apartment, and only uninitialize our own.
    let result = extract_in_apartment(app_id);
    if initialized {
        CoUninitialize();
    }
    result
}

unsafe fn extract_in_apartment(app_id: &str) -> Option<String> {
    let path: Vec<u16> = format!("shell:AppsFolder\\{app_id}")
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let mut pidl = null_mut();
    if SHParseDisplayName(path.as_ptr(), null(), &mut pidl, 0, null_mut()) < 0 || pidl.is_null() {
        return None;
    }
    let mut info: ShellFileInfo = zeroed();
    let found = SHGetFileInfoW(
        pidl as *const u16,
        0,
        &mut info,
        size_of::<ShellFileInfo>() as u32,
        SHGFI_ICON | SHGFI_PIDL,
    );
    CoTaskMemFree(pidl);
    if found == 0 || info.icon == 0 {
        return None;
    }
    let png = draw_icon(info.icon);
    DestroyIcon(info.icon);
    png.map(|bytes| {
        format!(
            "data:image/png;base64,{}",
            base64::engine::general_purpose::STANDARD.encode(bytes)
        )
    })
}

unsafe fn draw_icon(icon: isize) -> Option<Vec<u8>> {
    let dc = CreateCompatibleDC(0);
    if dc == 0 {
        return None;
    }
    let bitmap_info = BitmapInfo {
        header: BitmapInfoHeader {
            size: size_of::<BitmapInfoHeader>() as u32,
            width: ICON_SIZE,
            height: -ICON_SIZE, // top-down BGRA
            planes: 1,
            bit_count: 32,
            compression: BI_RGB,
            image_size: 0,
            x_pels_per_meter: 0,
            y_pels_per_meter: 0,
            colors_used: 0,
            colors_important: 0,
        },
        color: [0; 3],
    };
    let mut pixels = null_mut();
    let bitmap = CreateDIBSection(dc, &bitmap_info, DIB_RGB_COLORS, &mut pixels, 0, 0);
    if bitmap == 0 || pixels.is_null() {
        DeleteDC(dc);
        return None;
    }
    let original = SelectObject(dc, bitmap);
    let ok = DrawIconEx(dc, 0, 0, icon, ICON_SIZE, ICON_SIZE, 0, 0, DI_NORMAL) != 0;
    let data = if ok {
        let length = (ICON_SIZE * ICON_SIZE * 4) as usize;
        let bytes = std::slice::from_raw_parts(pixels as *const u8, length);
        let mut rgba = Vec::with_capacity(length);
        for pixel in bytes.chunks_exact(4) {
            rgba.extend_from_slice(&[pixel[2], pixel[1], pixel[0], pixel[3]]);
        }
        if rgba.chunks_exact(4).all(|pixel| pixel[3] == 0) {
            None
        } else {
            let mut output = Vec::new();
            let result = (|| {
                let mut encoder =
                    png::Encoder::new(&mut output, ICON_SIZE as u32, ICON_SIZE as u32);
                encoder.set_color(png::ColorType::Rgba);
                encoder.set_depth(png::BitDepth::Eight);
                let mut writer = encoder.write_header().ok()?;
                writer.write_image_data(&rgba).ok()?;
                writer.finish().ok()?;
                Some(())
            })();
            result.map(|()| output)
        }
    } else {
        None
    };
    SelectObject(dc, original);
    DeleteObject(bitmap);
    DeleteDC(dc);
    data
}

#[cfg(test)]
mod tests {
    use super::{valid_app_id, ICON_SIZE, MAX_CACHE_ENTRIES};
    #[test]
    fn validates_shell_id_not_path_injection() {
        assert!(valid_app_id("Microsoft.WindowsTerminal_8wekyb3d8bbwe!App"));
        assert!(!valid_app_id("app\0id"));
        assert!(!valid_app_id(""));
        assert!(!valid_app_id(&"a".repeat(321)));
    }
    #[test]
    fn bounds_icon_memory() {
        assert_eq!(ICON_SIZE, 64);
        assert_eq!(MAX_CACHE_ENTRIES, 256);
    }
}
