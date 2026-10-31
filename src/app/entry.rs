use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

const PREVIEW_LIMIT: u64 = 24 * 1024;

pub struct Entry {
    pub id: String,
    // The row's name element key, built once so a scrolling frame need not format it.
    pub name_key: String,
    pub name: String,
    pub path: PathBuf,
    // Read once, so sorting and the Kind column need no per-frame work.
    kind: String,
    pub is_dir: bool,
    pub is_symlink: bool,
    // The folder this row was found in, below the one on screen, when a search reached it.
    pub relative: Option<String>,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub created: Option<SystemTime>,
    pub readonly: bool,
    pub permissions: String,
}

impl Entry {
    pub(super) fn read(dir_entry: &fs::DirEntry) -> Option<Entry> {
        let path = dir_entry.path();
        let name = dir_entry.file_name().to_string_lossy().into_owned();
        // `DirEntry::metadata` reports the link itself, so a symlink's size and kind come from
        // its target.
        let link_meta = dir_entry.metadata().ok()?;
        let is_symlink = link_meta.file_type().is_symlink();
        let target_meta = if is_symlink {
            fs::metadata(&path).ok()
        } else {
            None
        };
        let meta = target_meta.as_ref().unwrap_or(&link_meta);
        let is_dir = meta.is_dir();
        // A colon keeps the id clear of the `/`-separated key paths Rune uses.
        let id = format!("f:{name}");
        let kind = crate::format::kind(&name, is_dir, is_symlink);
        Some(Entry {
            name_key: format!("name-{id}"),
            id,
            name,
            path,
            kind,
            is_dir,
            is_symlink,
            relative: None,
            size: if is_dir { 0 } else { meta.len() },
            modified: meta.modified().or_else(|_| link_meta.modified()).ok(),
            created: meta.created().or_else(|_| link_meta.created()).ok(),
            readonly: meta.permissions().readonly(),
            permissions: crate::format::permissions(meta),
        })
    }

    pub fn hidden(&self) -> bool {
        self.name.starts_with('.')
    }

    pub fn kind(&self) -> &str {
        &self.kind
    }
}

pub enum Preview {
    Text(String),
    Image,
    Folder(usize),
    TooLarge,
    Binary,
}

fn is_image(path: &Path) -> bool {
    let extension = path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    matches!(
        extension.as_str(),
        "png"
            | "jpg"
            | "jpeg"
            | "gif"
            | "webp"
            | "bmp"
            | "tif"
            | "tiff"
            | "svg"
            | "svgz"
            | "avif"
            | "heic"
            | "heif"
            | "ico"
    )
}

pub(super) fn compare_time(a: Option<SystemTime>, b: Option<SystemTime>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

pub(super) fn read_preview(path: &Path, is_dir: bool) -> Option<Preview> {
    if is_dir {
        return fs::read_dir(path)
            .ok()
            .map(|read| Preview::Folder(read.count()));
    }
    // A picture is drawn, not read: its bytes never pass through the text preview.
    if is_image(path) {
        return Some(Preview::Image);
    }
    let meta = fs::metadata(path).ok()?;
    if meta.len() > PREVIEW_LIMIT {
        return Some(Preview::TooLarge);
    }
    let bytes = fs::read(path).ok()?;
    if bytes.contains(&0) {
        return Some(Preview::Binary);
    }
    match String::from_utf8(bytes) {
        Ok(text) => Some(Preview::Text(text)),
        Err(_) => Some(Preview::Binary),
    }
}
