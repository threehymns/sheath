//! Where the window is looking: a folder on disk, or a folder inside an archive.
//!
//! The list, the history and the path bar all speak this one type. An archive is a file, so
//! opening its row switches the list from the folder around it to the archive's own entries, and
//! leaving it switches back. Keeping both cases here is what lets Back, Forward, Up and a crumb
//! cross that boundary without knowing which side they are on.

use super::App;
use super::scan::{Scan, sort_rows};
use rune_kit::SortDir;
use std::path::{Component, PathBuf};

/// Where the window is looking.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Location {
    Folder(PathBuf),
    // Only the desktop build opens archives, so the web build never builds this variant.
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    Archive {
        file: PathBuf,
        inner: String,
    },
}

impl Location {
    /// The path a person reads, with the archive file standing in for a folder.
    pub fn label(&self) -> String {
        match self {
            Location::Folder(path) => path.display().to_string(),
            Location::Archive { file, inner } if inner.is_empty() => file.display().to_string(),
            Location::Archive { file, inner } => format!("{}/{inner}", file.display()),
        }
    }
}

impl App {
    /// Whether the list shows an archive's entries. Both builds compile this; only the desktop
    /// build can open one.
    pub fn in_archive(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.archive.is_some()
        }
        #[cfg(target_arch = "wasm32")]
        {
            false
        }
    }

    /// Where the window is looking, for the path bar's accessibility label.
    pub fn location_label(&self) -> String {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(archive) = &self.archive {
            return Location::Archive {
                file: archive.file.clone(),
                inner: archive.inner.clone(),
            }
            .label();
        }
        self.cwd.display().to_string()
    }

    /// The path bar's crumbs: the folder's parts, then the archive and its folders when one is
    /// open. Each crumb carries where pressing it goes.
    pub fn crumbs(&self) -> Vec<(String, Location)> {
        let mut out = Vec::new();
        let mut acc = PathBuf::new();
        for part in self.cwd.components() {
            acc.push(part.as_os_str());
            let name = match part {
                Component::RootDir => std::path::MAIN_SEPARATOR.to_string(),
                other => other.as_os_str().to_string_lossy().into_owned(),
            };
            out.push((name, Location::Folder(acc.clone())));
        }
        if out.is_empty() {
            out.push((
                self.cwd.display().to_string(),
                Location::Folder(self.cwd.clone()),
            ));
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(archive) = &self.archive {
            let name = archive
                .file
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| archive.file.display().to_string());
            out.push((
                name,
                Location::Archive {
                    file: archive.file.clone(),
                    inner: String::new(),
                },
            ));
            let mut inner = String::new();
            for part in archive.inner.split('/').filter(|part| !part.is_empty()) {
                if !inner.is_empty() {
                    inner.push('/');
                }
                inner.push_str(part);
                out.push((
                    part.to_owned(),
                    Location::Archive {
                        file: archive.file.clone(),
                        inner: inner.clone(),
                    },
                ));
            }
        }
        out
    }

    /// Opens a crumb: a folder, or a folder inside the archive that is open.
    pub fn go_location(&mut self, location: Location) {
        match location {
            Location::Folder(path) => self.navigate(path),
            #[cfg(not(target_arch = "wasm32"))]
            Location::Archive { file, inner } => self.navigate_archive(file, inner),
            #[cfg(target_arch = "wasm32")]
            Location::Archive { .. } => {}
        }
    }

    /// The location the window is showing now.
    pub(super) fn location(&self) -> Location {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(archive) = &self.archive {
            return Location::Archive {
                file: archive.file.clone(),
                inner: archive.inner.clone(),
            };
        }
        Location::Folder(self.cwd.clone())
    }

    /// Shows a location and lists it. `select` names a row chosen once the read lands, which is
    /// how Up leaves the archive's own row chosen in the folder around it.
    pub(super) fn show(&mut self, location: Location, select: Option<String>) {
        #[cfg(not(target_arch = "wasm32"))]
        let archive = match &location {
            Location::Folder(_) => None,
            Location::Archive { file, inner } => {
                match super::archive::Archive::open(file.clone(), inner.clone()) {
                    Ok(archive) => Some(archive),
                    Err(error) => {
                        self.error = Some(format!("Couldn’t open “{}”: {error}", file.display()));
                        return;
                    }
                }
            }
        };
        self.stop_scrolling();
        self.menu = None;
        self.empty_menu = None;
        self.header_menu = None;
        self.place_menu = None;
        self.marquee = None;
        self.file_drag = None;
        self.clear_typeahead();
        self.search.clear();
        self.clear_selection();
        self.entries.clear();
        self.visible.clear();
        self.unhidden = 0;
        self.name_at.clear();
        self.pending_select = None;
        self.parked = None;
        self.search_truncated = false;
        self.layout_cut = true;
        match &location {
            Location::Folder(path) => {
                self.watch.retarget(path);
                self.save_session(|session| session.remember(path));
                self.cwd = path.clone();
            }
            Location::Archive { file, .. } => {
                let folder = file
                    .parent()
                    .map(std::path::Path::to_path_buf)
                    .unwrap_or_else(super::paths::root_dir);
                self.watch.retarget(&folder);
                self.save_session(|session| session.remember(&folder));
                self.cwd = folder;
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.archive = archive;
        }
        self.visit = Default::default();
        self.pending_select = select.map(|name| (vec![format!("f:{name}")], false));
        self.apply_rules();
        self.reload();
    }

    /// Lists the open archive's folder on this thread: its entries were read when the archive
    /// opened, so picking the children out of memory is the whole read.
    #[cfg(not(target_arch = "wasm32"))]
    pub(super) fn read_archive(
        &mut self,
        keep: Option<String>,
        sort: (String, SortDir),
        folders_first: bool,
    ) -> Option<Scan> {
        let path = self.cwd.clone();
        let archive = self.archive.as_mut()?;
        let error = archive.refresh().err();
        let file = archive.file.display().to_string();
        let mut entries = archive.listing();
        sort_rows(&mut entries, &sort.0, sort.1, folders_first);
        Some(Scan {
            path,
            entries,
            error: error.map(|error| format!("Couldn’t re-read “{file}”: {error}")),
            archived: true,
            keep,
            sort,
            folders_first,
        })
    }
}
