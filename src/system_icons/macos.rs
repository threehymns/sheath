//! The workspace's icon for a folder and for each file type.
//!
//! macOS has no icon theme: `NSWorkspace` returns the picture the Finder shows, from the
//! file's type and any custom icon its path carries. That image comes back as TIFF, which the
//! app's existing image decoders shrink to the size the rows and the inspector draw. A type
//! the workspace declines keeps the built-in glyph.

use crate::app::Entry;
use image::ImageFormat;
use objc2_app_kit::NSWorkspace;
use objc2_foundation::NSString;
use rune::{Image, ImageCache};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

/// The size an icon is shrunk to: the inspector's 52 points at a little over one device pixel
/// each, which the list and the grid scale down from.
const ICON_PIXELS: u32 = 64;

#[derive(Default)]
pub struct SystemIcons {
    cache: Option<Arc<ImageCache>>,
    images: RefCell<HashMap<String, Option<Image>>>,
}

impl SystemIcons {
    pub fn attach(&mut self, cache: Arc<ImageCache>) {
        self.cache = Some(cache);
    }

    /// The workspace's picture for a row, if it has one for that folder or type.
    pub fn image(&self, entry: &Entry) -> Option<Image> {
        let cache = self.cache.as_ref()?;
        let key = key(entry);
        if let Some(kept) = self.images.borrow().get(&key) {
            return kept.clone();
        }
        // Folders share one generic picture; a custom icon is per path, which the
        // extension-keyed cache cannot hold for every folder.
        let path: &Path = if entry.is_dir {
            Path::new("/")
        } else {
            entry.path.as_path()
        };
        let image = draw(cache, path);
        self.images.borrow_mut().insert(key, image.clone());
        image
    }
}

/// The cache key: the folder, or one picture per extension, since the icon follows the type.
fn key(entry: &Entry) -> String {
    if entry.is_dir {
        return "folder".to_owned();
    }
    let extension = entry
        .path
        .extension()
        .map(|ext| ext.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    format!("file:{extension}")
}

fn draw(cache: &ImageCache, path: &Path) -> Option<Image> {
    let path = NSString::from_str(&path.to_string_lossy());
    let workspace = NSWorkspace::sharedWorkspace();
    let icon = workspace.iconForFile(&path);
    let tiff = icon.TIFFRepresentation()?;
    let decoded = image::load_from_memory_with_format(&tiff.to_vec(), ImageFormat::Tiff).ok()?;
    let decoded = decoded.thumbnail(ICON_PIXELS, ICON_PIXELS).into_rgba8();
    let (width, height) = decoded.dimensions();
    Some(cache.rgba(width, height, decoded.into_raw()))
}
