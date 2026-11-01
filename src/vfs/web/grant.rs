//! Folders the user shares with the tab.
//!
//! A shared folder is mounted under `/granted` and browsed through the File System Access API:
//! its listing and its files arrive from the browser, and a write goes back out to it. A request
//! that has not landed only marks the path pending, so a frame never blocks on it.

use super::browser::{self, Listing, Picked};
use super::state;
pub(super) use super::state::grant_for;
use super::tree::Tree;
use super::{GRANT_ROOT, with};
use std::path::{Path, PathBuf};

/// Whether `path` is inside a folder the user shared with this tab.
pub fn is_granted(path: &Path) -> bool {
    state::grant_for(path).is_some()
}

fn sanitized(name: &str) -> String {
    name.replace(['/', '\\'], "-")
}

pub(super) fn mount(picked: Picked) -> PathBuf {
    let base = Path::new(GRANT_ROOT).join(sanitized(&picked.name));
    let mut root = base.clone();
    let mut n = 2;
    while with(|tree| tree.find(&root).is_some()) {
        root = base.with_file_name(format!(
            "{} {n}",
            base.file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| "Folder".to_owned())
        ));
        n += 1;
    }
    with(|tree| {
        let _ = tree.insert(root.clone(), true, Vec::new());
    });
    state::add_grant(root.clone(), picked.id);
    root
}

/// The browser writes a demo folder moving into a shared one needs: every row below `from`,
/// in path order, with the shared path it lands at.
pub(super) fn push_to_grant(
    tree: &Tree,
    from: &Path,
    to: &Path,
) -> Vec<(u32, String, bool, Vec<u8>)> {
    let mut rows: Vec<(PathBuf, bool, Vec<u8>)> = tree
        .items
        .iter()
        .filter(|item| item.path == from || item.path.starts_with(from))
        .map(|item| (item.path.clone(), item.is_dir, item.content.clone()))
        .collect();
    rows.sort_by_key(|(path, ..)| path.components().count());
    rows.into_iter()
        .filter_map(|(path, is_dir, content)| {
            let rest = path.strip_prefix(from).unwrap_or(Path::new(""));
            let target = to.join(rest);
            state::grant_for(&target).map(|(id, rel)| (id, rel, is_dir, content))
        })
        .collect()
}

pub(super) fn request_listing(path: PathBuf, id: u32, rel: String) {
    if !with(|tree| tree.pending.insert(path.clone())) {
        return;
    }
    browser::list(id, &rel, move |listing| {
        let error = match listing {
            Listing::Entries(entries) => {
                with(|tree| {
                    for entry in entries {
                        tree.land(&path, entry);
                    }
                });
                None
            }
            Listing::Failed(error) => Some(error),
        };
        with(|tree| {
            tree.pending.remove(&path);
            tree.listed.insert(path);
        });
        if let Some(error) = error {
            state::fail(error);
        }
        state::touched();
    });
}

pub(super) fn request_file(path: PathBuf, id: u32, rel: String) {
    if !with(|tree| tree.pending.insert(path.clone())) {
        return;
    }
    browser::read(id, &rel, move |bytes| {
        with(|tree| {
            tree.pending.remove(&path);
            if let Some(item) = tree.find_mut(&path)
                && let Some(bytes) = bytes
            {
                item.len = bytes.len() as u64;
                item.content = bytes;
                item.fetched = true;
            }
        });
        state::touched();
    });
}
