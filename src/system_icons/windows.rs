//! The shell's registered icon for a folder and for each file type.
//!
//! Windows has no icon theme: the icon belongs to the file type, registered by the app that
//! handles it. `SHGetFileInfoW` returns that icon as an `HICON` without touching the file,
//! which is turned into RGBA through a 32-bit top-down DIB. A type the shell declines keeps
//! the built-in glyph.

use crate::app::Entry;
use rune::{Image, ImageCache};
use std::cell::RefCell;
use std::collections::HashMap;
use std::ffi::c_void;
use std::mem::{size_of, zeroed};
use std::sync::Arc;
use windows_sys::Win32::Graphics::Gdi::{
    BI_RGB, BITMAP, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, DIB_RGB_COLORS, DeleteDC,
    DeleteObject, GetDIBits, GetObjectW, HBITMAP,
};
use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL};
use windows_sys::Win32::UI::Shell::{
    SHFILEINFOW, SHGFI_ICON, SHGFI_LARGEICON, SHGFI_USEFILEATTRIBUTES, SHGetFileInfoW,
};
use windows_sys::Win32::UI::WindowsAndMessaging::{DestroyIcon, GetIconInfo, HICON, ICONINFO};

#[derive(Default)]
pub struct SystemIcons {
    cache: Option<Arc<ImageCache>>,
    images: RefCell<HashMap<String, Option<Image>>>,
}

impl SystemIcons {
    pub fn attach(&mut self, cache: Arc<ImageCache>) {
        self.cache = Some(cache);
    }

    /// The shell's picture for a row, if it has one for that folder or type.
    pub fn image(&self, entry: &Entry) -> Option<Image> {
        let cache = self.cache.as_ref()?;
        let key = key(entry);
        if let Some(kept) = self.images.borrow().get(&key) {
            return kept.clone();
        }
        let image = draw(cache, &key);
        self.images.borrow_mut().insert(key, image.clone());
        image
    }
}

/// The cache key, which is also what the shell is asked about: the folder itself, or one
/// dummy name per extension, since the icon belongs to the type rather than the file.
fn key(entry: &Entry) -> String {
    if entry.is_dir {
        return "d:".to_owned();
    }
    let extension = entry
        .path
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    format!("f:{extension}")
}

fn draw(cache: &ImageCache, key: &str) -> Option<Image> {
    let (kind, extension) = key.split_once(':')?;
    let folder = kind == "d";
    let name = match (folder, extension.is_empty()) {
        (true, _) => "folder".to_owned(),
        (false, true) => "file".to_owned(),
        (false, false) => format!("file.{extension}"),
    };
    let (width, height, pixels) = shell_rgba(&name, folder)?;
    Some(cache.rgba(width, height, pixels))
}

fn shell_rgba(name: &str, folder: bool) -> Option<(u32, u32, Vec<u8>)> {
    let wide: Vec<u16> = name.encode_utf16().chain(std::iter::once(0)).collect();
    let attributes = if folder {
        FILE_ATTRIBUTE_DIRECTORY
    } else {
        FILE_ATTRIBUTE_NORMAL
    };
    let flags = SHGFI_ICON | SHGFI_LARGEICON | SHGFI_USEFILEATTRIBUTES;
    let mut info: SHFILEINFOW = unsafe { zeroed() };
    let found = unsafe {
        SHGetFileInfoW(
            wide.as_ptr(),
            attributes,
            &mut info,
            size_of::<SHFILEINFOW>() as u32,
            flags,
        )
    };
    if found == 0 || info.hIcon.is_null() {
        return None;
    }
    let icon = info.hIcon;
    let pixels = icon_rgba(icon);
    unsafe {
        DestroyIcon(icon);
    }
    pixels
}

fn icon_rgba(icon: HICON) -> Option<(u32, u32, Vec<u8>)> {
    let mut info: ICONINFO = unsafe { zeroed() };
    if unsafe { GetIconInfo(icon, &mut info) } == 0 {
        return None;
    }
    let color = info.hbmColor;
    let mask = info.hbmMask;
    let pixels = if color.is_null() {
        None
    } else {
        bitmap_rgba(color)
    };
    unsafe {
        if !color.is_null() {
            DeleteObject(color);
        }
        if !mask.is_null() {
            DeleteObject(mask);
        }
    }
    pixels
}

fn bitmap_rgba(bitmap: HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
    let mut size: BITMAP = unsafe { zeroed() };
    let read = unsafe {
        GetObjectW(
            bitmap,
            size_of::<BITMAP>() as i32,
            &mut size as *mut _ as *mut c_void,
        )
    };
    if read == 0 {
        return None;
    }
    let width = u32::try_from(size.bmWidth).ok()?;
    let height = u32::try_from(size.bmHeight).ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    let dc = unsafe { CreateCompatibleDC(std::ptr::null_mut()) };
    if dc.is_null() {
        return None;
    }
    let mut info: BITMAPINFO = unsafe { zeroed() };
    info.bmiHeader.biSize = size_of::<BITMAPINFOHEADER>() as u32;
    info.bmiHeader.biWidth = width as i32;
    // A negative height asks for the rows top-down, matching Rune's RGBA layout.
    info.bmiHeader.biHeight = -(height as i32);
    info.bmiHeader.biPlanes = 1;
    info.bmiHeader.biBitCount = 32;
    info.bmiHeader.biCompression = BI_RGB;
    let mut pixels = vec![0u8; width as usize * height as usize * 4];
    let copied = unsafe {
        GetDIBits(
            dc,
            bitmap,
            0,
            height,
            pixels.as_mut_ptr().cast(),
            &mut info,
            DIB_RGB_COLORS,
        )
    };
    unsafe {
        DeleteDC(dc);
    }
    if copied == 0 {
        return None;
    }
    // A DIB is blue, green, red, alpha; Rune draws red, green, blue, alpha.
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    // A mask-only icon carries no alpha; its pixels are opaque where it painted them.
    if pixels.chunks_exact(4).all(|pixel| pixel[3] == 0) {
        for pixel in pixels.chunks_exact_mut(4) {
            pixel[3] = 255;
        }
    }
    Some((width, height, pixels))
}
