use crate::app::Entry;
use rune_kit::SortDir;
use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

use super::entry::compare_time;

pub(super) struct Scan {
    pub(super) path: PathBuf,
    pub(super) entries: Vec<Entry>,
    pub(super) error: Option<String>,
    pub(super) keep: Option<String>,
    pub(super) sort: (String, SortDir),
    pub(super) folders_first: bool,
}

pub(super) fn sort_rows(
    entries: &mut [Entry],
    column: &str,
    direction: SortDir,
    folders_first: bool,
) {
    entries.sort_by(|a, b| {
        let folders = if folders_first {
            b.is_dir.cmp(&a.is_dir)
        } else {
            Ordering::Equal
        };
        folders.then_with(|| {
            let order = match column {
                "kind" => a
                    .kind()
                    .cmp(b.kind())
                    .then_with(|| rune_kit::natural_cmp(&a.name, &b.name)),
                "size" => a.size.cmp(&b.size),
                "modified" => compare_time(a.modified, b.modified),
                _ => rune_kit::natural_cmp(&a.name, &b.name),
            };
            direction.apply(order)
        })
    });
}

// A folder bigger than this is read on the worker pool, not in the frame that opens it.
pub(crate) const SYNC_SCAN_LIMIT: usize = 2_000;

// How many matches a search below the folder collects before it stops walking.
pub(super) const SEARCH_LIMIT: usize = 5_000;

pub(super) struct Found {
    pub(super) entries: Vec<Entry>,
    pub(super) truncated: bool,
}

/// Every entry named like `query`, from `root` and the folders below it. Hidden names stay out of
/// the walk unless asked for, so a search does not crawl through a dot directory's contents.
pub(super) fn search_below(root: &Path, query: &str, show_hidden: bool, limit: usize) -> Found {
    let mut entries = Vec::new();
    let mut truncated = false;
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(read) = fs::read_dir(&dir) else {
            continue;
        };
        for item in read.flatten() {
            let Some(mut entry) = Entry::read(&item) else {
                continue;
            };
            if !show_hidden && entry.hidden() {
                continue;
            }
            if entry.is_dir {
                stack.push(entry.path.clone());
            }
            if !entry.name.to_lowercase().contains(query) {
                continue;
            }
            entry.relative = entry
                .path
                .strip_prefix(root)
                .ok()
                .and_then(|rest| rest.parent())
                .filter(|parent| !parent.as_os_str().is_empty())
                .map(|parent| parent.to_string_lossy().into_owned());
            // A name is only unique within its own folder, so a match from below carries the
            // folder it was found in. Without it two `notes.txt` rows would share one id, and
            // every action that resolves a row by id would take the first of them. Slashes
            // become colons, which keeps the id clear of the `/`-separated key paths Rune uses.
            if let Some(relative) = entry.relative.clone() {
                let folder = relative.replace(['/', '\\'], ":");
                entry.id = format!("f:{folder}:{}", entry.name);
                entry.name_key = format!("name-{}", entry.id);
            }
            entries.push(entry);
            if entries.len() >= limit {
                truncated = true;
                break;
            }
        }
        if truncated {
            break;
        }
    }
    // The folder's own matches lead, then each folder below in path order.
    entries.sort_by(|a, b| {
        a.relative
            .as_deref()
            .unwrap_or("")
            .cmp(b.relative.as_deref().unwrap_or(""))
            .then_with(|| rune_kit::natural_cmp(&a.name, &b.name))
    });
    Found { entries, truncated }
}

impl Scan {
    pub(super) fn read(
        path: PathBuf,
        keep: Option<String>,
        sort: (String, SortDir),
        folders_first: bool,
    ) -> Scan {
        let mut entries = Vec::new();
        let mut error = None;
        match fs::read_dir(&path) {
            Ok(read) => {
                for item in read.flatten() {
                    if let Some(entry) = Entry::read(&item) {
                        entries.push(entry);
                    }
                }
            }
            Err(err) => {
                error = Some(format!("Couldn’t open {}: {err}", path.display()));
            }
        }
        sort_rows(&mut entries, &sort.0, sort.1, folders_first);
        Scan {
            path,
            entries,
            error,
            keep,
            sort,
            folders_first,
        }
    }

    pub(super) fn try_read(
        path: PathBuf,
        keep: Option<String>,
        sort: (String, SortDir),
        limit: usize,
        folders_first: bool,
    ) -> Option<Scan> {
        let read = fs::read_dir(&path).ok()?;
        let mut items = Vec::with_capacity(limit + 1);
        for item in read {
            let Ok(item) = item else {
                continue;
            };
            items.push(item);
            if items.len() > limit {
                return None;
            }
        }
        let mut entries = Vec::with_capacity(items.len());
        for item in &items {
            if let Some(entry) = Entry::read(item) {
                entries.push(entry);
            }
        }
        sort_rows(&mut entries, &sort.0, sort.1, folders_first);
        Some(Scan {
            path,
            entries,
            error: None,
            keep,
            sort,
            folders_first,
        })
    }
}
