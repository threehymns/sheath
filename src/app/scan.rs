use crate::app::Entry;
use rune_kit::SortDir;
use std::fs;
use std::path::PathBuf;

use super::entry::compare_time;

pub(super) struct Scan {
    pub(super) path: PathBuf,
    pub(super) entries: Vec<Entry>,
    pub(super) error: Option<String>,
    pub(super) keep: Option<String>,
    pub(super) sort: (String, SortDir),
}

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

// A folder bigger than this is read on the worker pool, not in the frame that opens it.
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
