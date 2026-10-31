//! The folder read: the rows of a folder and what went wrong opening it. A folder small
//! enough is read in the frame that opens it; a bigger one on Rune's worker pool, where the
//! frame that asked for it never waits on it.

use crate::app::Entry;
use rune_kit::SortDir;
use std::fs;
use std::path::PathBuf;

use super::entry::compare_time;

/// One folder read, done off the main thread: the rows of a folder and what went wrong
/// opening it. A folder with tens of thousands of entries takes long enough to read that
/// doing it in the frame that opens it would stall the window.
///
/// `Latest` runs it on Rune's worker pool and drops the answer of a read a later one
/// superseded, so navigating away mid-read never lands the folder left behind.
pub(super) struct Scan {
    pub(super) path: PathBuf,
    pub(super) entries: Vec<Entry>,
    pub(super) error: Option<String>,
    /// The row the read was asked to keep selected, if the folder still has it.
    pub(super) keep: Option<String>,
    /// The sort the read applied, so a landing sorts again only when the list's sort moved
    /// while the read ran.
    pub(super) sort: (String, SortDir),
}

/// Orders a folder's rows: folders first, then the list's sort column and direction. The read
/// runs it, so the frame that opens a folder only filters the rows.
pub(super) fn sort_rows(entries: &mut [Entry], column: &str, direction: SortDir) {
    entries.sort_by(|a, b| {
        b.is_dir.cmp(&a.is_dir).then_with(|| {
            let order = match column {
                "size" => a.size.cmp(&b.size),
                "modified" => compare_time(a.modified, b.modified),
                _ => rune_kit::natural_cmp(&a.name, &b.name),
            };
            direction.apply(order)
        })
    });
}

/// Folders up to this many entries are read in the handler that opens them, so the frame
/// after a switch already draws the new rows. At about 8 µs an entry for the read and the
/// sort, that keeps the work inside a 60 Hz frame; a bigger folder is read on the worker,
/// where the frame that opens it would otherwise have waited longer than it has.
pub(crate) const SYNC_SCAN_LIMIT: usize = 2_000;

impl Scan {
    pub(super) fn read(path: PathBuf, keep: Option<String>, sort: (String, SortDir)) -> Scan {
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
        sort_rows(&mut entries, &sort.0, sort.1);
        Scan {
            path,
            entries,
            error,
            keep,
            sort,
        }
    }

    /// Reads the folder here, unless it holds more than `limit` entries: `None` means it is
    /// big enough that the read belongs on the worker. Only the directory is listed before
    /// the limit is checked, so a huge folder costs a few hundred entries' names and no
    /// metadata before it goes off the main thread. A folder that cannot be opened goes to
    /// the worker too, for the message a failed read gives.
    pub(super) fn try_read(
        path: PathBuf,
        keep: Option<String>,
        sort: (String, SortDir),
        limit: usize,
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
        sort_rows(&mut entries, &sort.0, sort.1);
        Some(Scan {
            path,
            entries,
            error: None,
            keep,
            sort,
        })
    }
}
