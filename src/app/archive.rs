//! Archives opened as folders, the way File Explorer shows a .zip's contents.
//!
//! Only the shapes the app already carries a reader for are browsed: ZIP (and an APK, which is a
//! ZIP too) and tar, plain or gzip-compressed. A 7z or a rar has no reader in the tree, so it
//! still opens with whichever app owns it.
//!
//! Every entry is read once when the archive opens, so listing, sorting and filtering one cost
//! no more than a folder's; an entry's bytes are read out of the archive only when it is
//! previewed or opened. Folders are made from the entry names, so an archive that carries no
//! directory entries still browses as a tree.
//!
//! An archive is read-only. Its rows carry a path that runs through the archive file, which no
//! write follows, so the commands that would change something stop with a sentence instead, and
//! the archive's folders are never drop targets.

use super::entry::{is_image, preview_bytes};
use super::{App, Entry, Location, PREVIEW_LIMIT, Preview};
use crate::vfs as fs;
use rune::Image;
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// The archive shapes this app browses.
pub(super) fn is_archive(path: &Path) -> bool {
    matches!(kind(path), Some(Format::Zip | Format::Tar(_)))
}

enum Format {
    Zip,
    Tar(bool),
}

/// The archive's format, from its name. `.tar.gz` is checked before the extension, which only
/// sees the `.gz`.
fn kind(path: &Path) -> Option<Format> {
    let name = path.file_name()?.to_string_lossy().to_ascii_lowercase();
    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        return Some(Format::Tar(true));
    }
    if name.ends_with(".tar") {
        return Some(Format::Tar(false));
    }
    match path
        .extension()?
        .to_string_lossy()
        .to_ascii_lowercase()
        .as_str()
    {
        "zip" | "apk" => Some(Format::Zip),
        _ => None,
    }
}

pub(super) struct Archive {
    pub(super) file: PathBuf,
    /// The folder inside the archive on screen, empty at its root.
    pub(super) inner: String,
    items: Vec<Item>,
}

struct Item {
    // The entry's path inside the archive, with `/` separators and no `./` or `..` parts.
    inner: String,
    name: String,
    is_dir: bool,
    size: u64,
    modified: Option<SystemTime>,
    // The name the archive itself uses, which a read looks the entry up by. A synthesized
    // folder has none.
    source: String,
}

impl Archive {
    pub(super) fn open(file: PathBuf, inner: String) -> Result<Archive, String> {
        let items = read_items(&file)?;
        let inner = normalize(&inner);
        Ok(Archive { file, inner, items })
    }

    /// Reads the archive again, keeping the folder on screen when it is still there. This is what
    /// Refresh does, so an archive that changed under the window shows its new entries.
    pub(super) fn refresh(&mut self) -> Result<(), String> {
        self.items = read_items(&self.file)?;
        if !self.inner.is_empty() && !self.items.iter().any(|item| item.inner == self.inner) {
            self.inner.clear();
        }
        Ok(())
    }

    /// The folder above the one on screen, or None at the archive's root.
    pub(super) fn parent(&self) -> Option<String> {
        if self.inner.is_empty() {
            return None;
        }
        Some(match self.inner.rsplit_once('/') {
            Some((parent, _)) => parent.to_owned(),
            None => String::new(),
        })
    }

    /// The rows for the folder on screen: its immediate children, folders and files alike.
    pub(super) fn listing(&self) -> Vec<Entry> {
        let prefix = if self.inner.is_empty() {
            String::new()
        } else {
            format!("{}/", self.inner)
        };
        self.items
            .iter()
            .filter_map(|item| {
                let rest = item.inner.strip_prefix(&prefix)?;
                if rest.is_empty() || rest.contains('/') {
                    return None;
                }
                Some(Entry::archived(
                    self.file.join(&item.inner),
                    item.name.clone(),
                    item.is_dir,
                    item.size,
                    item.modified,
                ))
            })
            .collect()
    }

    /// How many items the folder holds, for the inspector's folder preview.
    pub(super) fn children(&self, inner: &str) -> usize {
        let prefix = if inner.is_empty() {
            String::new()
        } else {
            format!("{inner}/")
        };
        self.items
            .iter()
            .filter(|item| {
                item.inner
                    .strip_prefix(&prefix)
                    .is_some_and(|rest| !rest.is_empty() && !rest.contains('/'))
            })
            .count()
    }

    /// The entry's uncompressed size, or None when the name is not a file in here.
    fn size_of(&self, inner: &str) -> Option<u64> {
        self.items
            .iter()
            .find(|item| item.inner == inner && !item.is_dir)
            .map(|item| item.size)
    }

    /// Every file below `inner`, as a folder's size reads: their bytes, and how many items
    /// (folders and files) it took to hold them.
    pub(super) fn total(&self, inner: &str) -> (u64, usize) {
        let prefix = if inner.is_empty() {
            String::new()
        } else {
            format!("{inner}/")
        };
        let mut bytes = 0;
        let mut items = 0;
        for item in &self.items {
            if !item.inner.starts_with(&prefix) || item.inner == inner {
                continue;
            }
            items += 1;
            if !item.is_dir {
                bytes += item.size;
            }
        }
        (bytes, items)
    }

    /// One entry's bytes, decompressing just that entry.
    pub(super) fn read(&self, inner: &str) -> Result<Vec<u8>, String> {
        let source = self
            .items
            .iter()
            .find(|item| item.inner == inner && !item.is_dir)
            .map(|item| item.source.as_str())
            .ok_or_else(|| format!("“{inner}” is no longer in the archive."))?;
        match kind(&self.file) {
            Some(Format::Zip) => read_zip(&self.file, source),
            Some(Format::Tar(gz)) => read_tar(&self.file, gz, source),
            None => Err("That file is not an archive.".into()),
        }
    }
}

impl App {
    /// The row's path inside the open archive, or None when the row is a folder on disk.
    pub(super) fn archive_inner(&self, path: &Path) -> Option<String> {
        let archive = self.archive.as_ref()?;
        let rest = path.strip_prefix(&archive.file).ok()?;
        if rest.as_os_str().is_empty() {
            return Some(String::new());
        }
        Some(rest.to_string_lossy().replace('\\', "/"))
    }

    /// Opens a row that names an archive, or an entry inside the open one. `false` leaves the row
    /// to the app's ordinary open: a folder to navigate, a file to the desktop.
    pub(super) fn open_archived_entry(&mut self, path: &Path, is_dir: bool) -> bool {
        if let Some(inner) = self.archive_inner(path) {
            if is_dir {
                let file = self.archive.as_ref().map(|archive| archive.file.clone());
                if let Some(file) = file {
                    self.navigate_archive(file, inner);
                }
            } else if let Err(error) = self.open_archived(&inner) {
                self.error = Some(error);
            }
            return true;
        }
        if !is_dir && is_archive(path) {
            self.open_archive(path.to_path_buf());
            return true;
        }
        false
    }

    /// Shows an archive's root as a folder.
    pub(crate) fn open_archive(&mut self, file: PathBuf) {
        let location = Location::Archive {
            file,
            inner: String::new(),
        };
        if self.location() == location {
            self.reload();
            return;
        }
        self.push(location.clone());
        self.show(location, None);
    }

    /// Shows a folder inside an archive.
    pub(super) fn navigate_archive(&mut self, file: PathBuf, inner: String) {
        let location = Location::Archive { file, inner };
        if self.location() == location {
            self.reload();
            return;
        }
        self.push(location.clone());
        self.show(location, None);
    }

    /// The inspector's preview of an entry inside the open archive.
    pub(super) fn archived_preview(&mut self, inner: &str, is_dir: bool) -> Option<Preview> {
        let archive = self.archive.as_ref()?;
        if is_dir {
            return Some(Preview::Folder(archive.children(inner)));
        }
        if is_image(Path::new(inner)) {
            return Some(Preview::Image);
        }
        if archive.size_of(inner)? > PREVIEW_LIMIT {
            return Some(Preview::TooLarge);
        }
        let bytes = archive.read(inner).ok()?;
        Some(preview_bytes(&bytes))
    }

    /// Hands the inspector a picture's bytes, since it has no path the decoder could read.
    pub(super) fn archived_picture(
        &mut self,
        path: &Path,
        inner: &str,
        modified: Option<SystemTime>,
    ) -> Result<Option<Image>, String> {
        let bytes = self
            .archive
            .as_ref()
            .ok_or("No archive is open.")?
            .read(inner)?;
        Ok(self
            .previews
            .request_bytes(path, modified, Arc::from(bytes)))
    }

    /// Extracts one entry to the cache folder and hands it to the desktop, the way a file
    /// manager opens a file that lives inside an archive.
    pub(super) fn open_archived(&mut self, inner: &str) -> Result<(), String> {
        let archive = self.archive.as_ref().ok_or("No archive is open.")?;
        let file = archive.file.clone();
        let bytes = archive.read(inner)?;
        let mut target = extracted_dir(&file);
        for part in inner.split('/').filter(|part| !part.is_empty()) {
            target.push(part);
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).map_err(|err| err.to_string())?;
        }
        fs::write(&target, &bytes).map_err(|err| err.to_string())?;
        super::paths::open_external(&target)
    }

    /// A folder's size inside the archive, read from the entries already in memory.
    pub(super) fn archive_size(&mut self, inner: &str) -> Option<(u64, usize)> {
        self.archive.as_ref().map(|archive| archive.total(inner))
    }
}

/// Where an archive's entry is extracted, one folder per archive so two packages with the same
/// inner name never overwrite each other.
fn extracted_dir(file: &Path) -> PathBuf {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    file.hash(&mut hasher);
    let name = file
        .file_stem()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "archive".into());
    std::env::temp_dir()
        .join("sheath-archives")
        .join(format!("{:016x}-{name}", hasher.finish()))
}

fn read_items(file: &Path) -> Result<Vec<Item>, String> {
    match kind(file).ok_or_else(|| "that file is not an archive".to_owned())? {
        Format::Zip => zip_items(file),
        Format::Tar(gz) => tar_items(file, gz),
    }
}

fn zip_items(file: &Path) -> Result<Vec<Item>, String> {
    let handle = File::open(file).map_err(|err| err.to_string())?;
    let mut archive = zip::ZipArchive::new(handle).map_err(|err| err.to_string())?;
    let mut items = BTreeMap::new();
    for index in 0..archive.len() {
        let Ok(entry) = archive.by_index(index) else {
            continue;
        };
        let Ok(name) = entry.name() else {
            continue;
        };
        let modified = entry.last_modified().and_then(zip_time);
        add(
            &mut items,
            &name,
            &name,
            entry.is_dir(),
            entry.size(),
            modified,
        );
    }
    Ok(items.into_values().collect())
}

fn tar_items(file: &Path, gz: bool) -> Result<Vec<Item>, String> {
    let reader = open_reader(file, gz)?;
    let mut archive = tar::Archive::new(reader);
    let mut items = BTreeMap::new();
    let entries = archive.entries().map_err(|err| err.to_string())?;
    for entry in entries {
        let Ok(entry) = entry else {
            continue;
        };
        let Ok(path) = entry.path() else {
            continue;
        };
        let name = path.to_string_lossy().into_owned();
        let header = entry.header();
        let is_dir = header.entry_type().is_dir();
        let size = header.size().unwrap_or(0);
        let modified = header
            .mtime()
            .ok()
            .map(|seconds| SystemTime::UNIX_EPOCH + Duration::from_secs(seconds));
        add(&mut items, &name, &name, is_dir, size, modified);
    }
    Ok(items.into_values().collect())
}

/// Files an entry under its normalized name, adding the folders its path passes through. A
/// first write wins, so a real directory entry keeps its own timestamp over a synthesized one.
fn add(
    items: &mut BTreeMap<String, Item>,
    name: &str,
    source: &str,
    is_dir: bool,
    size: u64,
    modified: Option<SystemTime>,
) {
    let inner = normalize(name);
    if inner.is_empty() {
        return;
    }
    let mut parts: Vec<&str> = inner.split('/').collect();
    let leaf = parts.pop().unwrap_or_default().to_owned();
    let mut path = String::new();
    for part in parts {
        if !path.is_empty() {
            path.push('/');
        }
        path.push_str(part);
        items.entry(path.clone()).or_insert_with(|| Item {
            inner: path.clone(),
            name: part.to_owned(),
            is_dir: true,
            size: 0,
            modified,
            source: String::new(),
        });
    }
    items.entry(inner.clone()).or_insert(Item {
        inner,
        name: leaf,
        is_dir,
        size: if is_dir { 0 } else { size },
        modified,
        source: if is_dir {
            String::new()
        } else {
            source.to_owned()
        },
    });
}

/// An archive's own path, with `/` separators and no `.` or `..` parts. An absolute path loses
/// its root, so an entry can never name a place outside the archive.
fn normalize(name: &str) -> String {
    let name = name.replace('\\', "/");
    let mut parts = Vec::new();
    for part in name.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

fn open_reader(file: &Path, gz: bool) -> Result<Box<dyn Read>, String> {
    let handle = File::open(file).map_err(|err| err.to_string())?;
    if gz {
        Ok(Box::new(flate2::read::GzDecoder::new(handle)))
    } else {
        Ok(Box::new(handle))
    }
}

fn read_zip(file: &Path, name: &str) -> Result<Vec<u8>, String> {
    let handle = File::open(file).map_err(|err| err.to_string())?;
    let mut archive = zip::ZipArchive::new(handle).map_err(|err| err.to_string())?;
    let mut entry = archive.by_name(name).map_err(|err| err.to_string())?;
    let mut bytes = Vec::with_capacity(entry.size() as usize);
    entry
        .read_to_end(&mut bytes)
        .map_err(|err| err.to_string())?;
    Ok(bytes)
}

fn read_tar(file: &Path, gz: bool, name: &str) -> Result<Vec<u8>, String> {
    let reader = open_reader(file, gz)?;
    let mut archive = tar::Archive::new(reader);
    let entries = archive.entries().map_err(|err| err.to_string())?;
    for entry in entries {
        let Ok(mut entry) = entry else {
            continue;
        };
        let Ok(path) = entry.path() else {
            continue;
        };
        if normalize(&path.to_string_lossy()) != name {
            continue;
        }
        let mut bytes = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut bytes)
            .map_err(|err| err.to_string())?;
        return Ok(bytes);
    }
    Err(format!("“{name}” is no longer in the archive."))
}

/// A ZIP's DOS timestamp, read as UTC. The format carries no zone, so this is as close as the
/// entry gets to an instant.
fn zip_time(time: zip::DateTime) -> Option<SystemTime> {
    let date = chrono::NaiveDate::from_ymd_opt(
        time.year() as i32,
        time.month() as u32,
        time.day() as u32,
    )?;
    let clock = chrono::NaiveTime::from_hms_opt(
        time.hour() as u32,
        time.minute() as u32,
        time.second() as u32,
    )?;
    Some(
        chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
            chrono::NaiveDateTime::new(date, clock),
            chrono::Utc,
        )
        .into(),
    )
}
