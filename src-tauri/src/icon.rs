use std::collections::BTreeMap;

use crate::config::{ItemKind, LaunchItem};

pub fn for_items(items: &[LaunchItem]) -> BTreeMap<String, String> {
    items
        .iter()
        .filter_map(|item| {
            data_url(&item.target, &item.kind).map(|url| (item.id.clone(), url))
        })
        .collect()
}

fn data_url(target: &str, kind: &ItemKind) -> Option<String> {
    #[cfg(windows)]
    {
        win::data_url(target, kind)
    }
    #[cfg(not(windows))]
    {
        let _ = (target, kind);
        None
    }
}

#[cfg(windows)]
mod win {
    use super::*;
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    const SHGFI_ICON: u32 = 0x0000_0100;
    const SHGFI_LARGEICON: u32 = 0x0000_0000;
    const SHGFI_USEFILEATTRIBUTES: u32 = 0x0000_0010;
    const FILE_ATTRIBUTE_NORMAL: u32 = 0x0000_0080;
    const FILE_ATTRIBUTE_DIRECTORY: u32 = 0x0000_0010;
    const DIB_RGB_COLORS: u32 = 0;
    const BI_RGB: u32 = 0;

    #[allow(dead_code)]
    #[repr(C)]
    struct ShFileInfoW {
        h_icon: isize,
        i_icon: i32,
        dw_attributes: u32,
        sz_display_name: [u16; 260],
        sz_type_name: [u16; 80],
    }

    #[allow(dead_code)]
    #[repr(C)]
    struct IconInfo {
        f_icon: i32,
        x_hotspot: u32,
        y_hotspot: u32,
        hbm_mask: isize,
        hbm_color: isize,
    }

    #[allow(dead_code)]
    #[repr(C)]
    struct Bitmap {
        bm_type: i32,
        bm_width: i32,
        bm_height: i32,
        bm_width_bytes: i32,
        bm_planes: u16,
        bm_bits_pixel: u16,
        bm_bits: *mut u8,
    }

    #[repr(C)]
    struct BitmapInfoHeader {
        bi_size: u32,
        bi_width: i32,
        bi_height: i32,
        bi_planes: u16,
        bi_bit_count: u16,
        bi_compression: u32,
        bi_size_image: u32,
        bi_x_pels_per_meter: i32,
        bi_y_pels_per_meter: i32,
        bi_clr_used: u32,
        bi_clr_important: u32,
    }

    #[repr(C)]
    struct BitmapInfo {
        header: BitmapInfoHeader,
        bmi_colors: [u32; 1],
    }

    #[link(name = "shell32")]
    extern "system" {
        fn SHGetFileInfoW(
            psz_path: *const u16,
            dw_file_attributes: u32,
            psfi: *mut ShFileInfoW,
            cb_file_info: u32,
            u_flags: u32,
        ) -> usize;
    }

    #[link(name = "user32")]
    extern "system" {
        fn GetIconInfo(hicon: isize, info: *mut IconInfo) -> i32;
        fn DestroyIcon(hicon: isize) -> i32;
        fn GetDC(hwnd: isize) -> isize;
        fn ReleaseDC(hwnd: isize, hdc: isize) -> i32;
    }

    #[link(name = "gdi32")]
    extern "system" {
        fn GetObjectW(handle: isize, size: i32, out: *mut core::ffi::c_void) -> i32;
        fn GetDIBits(
            hdc: isize,
            hbm: isize,
            start: u32,
            lines: u32,
            bits: *mut u8,
            info: *mut BitmapInfo,
            usage: u32,
        ) -> i32;
        fn DeleteObject(handle: isize) -> i32;
    }

    pub fn data_url(target: &str, kind: &ItemKind) -> Option<String> {
        let bmp = extract(target, kind)?;
        Some(format!("data:image/bmp;base64,{}", b64(&bmp)))
    }

    fn extract(target: &str, kind: &ItemKind) -> Option<Vec<u8>> {
        unsafe { extract_icon(target, kind) }
    }

    unsafe fn extract_icon(target: &str, kind: &ItemKind) -> Option<Vec<u8>> {
        let (path, attrs, flags) = match kind {
            ItemKind::Url => (
                wide("webpage.url"),
                FILE_ATTRIBUTE_NORMAL,
                SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES,
            ),
            ItemKind::Folder => {
                if std::path::Path::new(target).is_dir() {
                    (wide(target), 0, SHGFI_ICON | SHGFI_LARGEICON)
                } else {
                    (
                        wide(target),
                        FILE_ATTRIBUTE_DIRECTORY,
                        SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES,
                    )
                }
            }
            ItemKind::App | ItemKind::File => {
                if std::path::Path::new(target).exists() {
                    (wide(target), 0, SHGFI_ICON | SHGFI_LARGEICON)
                } else {
                    (
                        wide(target),
                        FILE_ATTRIBUTE_NORMAL,
                        SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES,
                    )
                }
            }
        };

        let mut info = std::mem::zeroed::<ShFileInfoW>();
        let ok = SHGetFileInfoW(
            path.as_ptr(),
            attrs,
            &mut info,
            std::mem::size_of::<ShFileInfoW>() as u32,
            flags,
        );
        if ok == 0 || info.h_icon == 0 {
            return None;
        }
        let png = hicon_to_bmp(info.h_icon);
        DestroyIcon(info.h_icon);
        png
    }

    unsafe fn hicon_to_bmp(hicon: isize) -> Option<Vec<u8>> {
        let mut icon = std::mem::zeroed::<IconInfo>();
        if GetIconInfo(hicon, &mut icon) == 0 {
            return None;
        }
        let color = if icon.hbm_color != 0 {
            icon.hbm_color
        } else {
            icon.hbm_mask
        };
        let result = bitmap_to_bmp(color);
        if icon.hbm_color != 0 {
            DeleteObject(icon.hbm_color);
        }
        if icon.hbm_mask != 0 {
            DeleteObject(icon.hbm_mask);
        }
        result
    }

    unsafe fn bitmap_to_bmp(hbm: isize) -> Option<Vec<u8>> {
        if hbm == 0 {
            return None;
        }
        let mut bitmap = std::mem::zeroed::<Bitmap>();
        if GetObjectW(
            hbm,
            std::mem::size_of::<Bitmap>() as i32,
            &mut bitmap as *mut _ as *mut core::ffi::c_void,
        ) == 0
        {
            return None;
        }
        let width = bitmap.bm_width;
        let height = bitmap.bm_height.abs();
        if width <= 0 || height <= 0 || width > 256 || height > 256 {
            return None;
        }
        let mut bmi = BitmapInfo {
            header: BitmapInfoHeader {
                bi_size: std::mem::size_of::<BitmapInfoHeader>() as u32,
                bi_width: width,
                bi_height: -height,
                bi_planes: 1,
                bi_bit_count: 32,
                bi_compression: BI_RGB,
                bi_size_image: 0,
                bi_x_pels_per_meter: 0,
                bi_y_pels_per_meter: 0,
                bi_clr_used: 0,
                bi_clr_important: 0,
            },
            bmi_colors: [0],
        };
        let mut pixels = vec![0u8; (width * height * 4) as usize];
        let hdc = GetDC(0);
        if hdc == 0 {
            return None;
        }
        let copied = GetDIBits(
            hdc,
            hbm,
            0,
            height as u32,
            pixels.as_mut_ptr(),
            &mut bmi,
            DIB_RGB_COLORS,
        );
        if copied == 0 {
            bmi.header.bi_height = height;
            let copied = GetDIBits(
                hdc,
                hbm,
                0,
                height as u32,
                pixels.as_mut_ptr(),
                &mut bmi,
                DIB_RGB_COLORS,
            );
            ReleaseDC(0, hdc);
            if copied == 0 {
                return None;
            }
            return Some(encode_bmp_bottom_up(width, height, &pixels));
        }
        ReleaseDC(0, hdc);
        Some(encode_bmp(width, height, &pixels))
    }

    fn encode_bmp(width: i32, height: i32, bgra: &[u8]) -> Vec<u8> {
        let row = (width as usize) * 4;
        let pixel_bytes = row * height as usize;
        let file_size = 14 + 40 + pixel_bytes;
        let mut out = Vec::with_capacity(file_size);
        out.extend_from_slice(b"BM");
        out.extend_from_slice(&(file_size as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&54u32.to_le_bytes());
        out.extend_from_slice(&40u32.to_le_bytes());
        out.extend_from_slice(&width.to_le_bytes());
        out.extend_from_slice(&height.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
        out.extend_from_slice(&[0u8; 16]);
        for y in (0..height as usize).rev() {
            let start = y * row;
            out.extend_from_slice(&bgra[start..start + row]);
        }
        out
    }

    fn encode_bmp_bottom_up(width: i32, height: i32, bgra: &[u8]) -> Vec<u8> {
        let row = (width as usize) * 4;
        let pixel_bytes = row * height as usize;
        let file_size = 14 + 40 + pixel_bytes;
        let mut out = Vec::with_capacity(file_size);
        out.extend_from_slice(b"BM");
        out.extend_from_slice(&(file_size as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&54u32.to_le_bytes());
        out.extend_from_slice(&40u32.to_le_bytes());
        out.extend_from_slice(&width.to_le_bytes());
        out.extend_from_slice(&height.to_le_bytes());
        out.extend_from_slice(&1u16.to_le_bytes());
        out.extend_from_slice(&32u16.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&(pixel_bytes as u32).to_le_bytes());
        out.extend_from_slice(&[0u8; 16]);
        out.extend_from_slice(&bgra[..pixel_bytes]);
        out
    }

    fn wide(value: impl AsRef<OsStr>) -> Vec<u16> {
        value
            .as_ref()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect()
    }

    fn b64(data: &[u8]) -> String {
        const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
        for chunk in data.chunks(3) {
            let a = chunk[0] as u32;
            let b = chunk.get(1).copied().unwrap_or(0) as u32;
            let c = chunk.get(2).copied().unwrap_or(0) as u32;
            let n = (a << 16) | (b << 8) | c;
            out.push(T[((n >> 18) & 63) as usize] as char);
            out.push(T[((n >> 12) & 63) as usize] as char);
            if chunk.len() > 1 {
                out.push(T[((n >> 6) & 63) as usize] as char);
            } else {
                out.push('=');
            }
            if chunk.len() > 2 {
                out.push(T[(n & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
        out
    }
}

#[cfg(all(test, windows))]
mod tests {
    use super::*;
    use crate::config::{ItemKind, LaunchItem};

    #[test]
    fn extracts_notepad_icon() {
        let item = LaunchItem {
            id: "n".into(),
            name: "Notepad".into(),
            target: r"C:\Windows\System32\notepad.exe".into(),
            args: String::new(),
            kind: ItemKind::App,
            enabled: true,
            delay_ms: 0,
        };
        let icons = for_items(&[item]);
        let url = icons.get("n").expect("notepad icon");
        assert!(url.starts_with("data:image/bmp;base64,"), "{url}");
        assert!(url.len() > 200, "{}", url.len());
    }
}
