//! The demo's Projects folder, mirrored from GitHub.
//!
//! Each repository's tree is fetched the first time its folder is opened, and a file's bytes are
//! fetched when the app reads it. One attempt per page load: a tree that failed leaves the folder
//! empty rather than asking GitHub again on every frame that lists it.

use super::browser;
use super::state;
use super::tree::{Item, ensure_dir};
use super::{PROJECTS, with};
use std::path::{Path, PathBuf};

/// The mirrored folder a path is in: its root, repository and branch, and the path inside that
/// repository.
pub(super) fn repo_for(path: &Path) -> Option<(&'static str, &'static str, &'static str, String)> {
    PROJECTS.iter().find_map(|(root, repo, branch)| {
        let dir = Path::new(root);
        if path != dir && !path.starts_with(dir) {
            return None;
        }
        let rel = path
            .strip_prefix(dir)
            .unwrap_or(Path::new(""))
            .to_string_lossy()
            .into_owned();
        Some((*root, *repo, *branch, rel))
    })
}

/// Asks GitHub for one mirrored repository. The tree lands on the next frame; a failure leaves
/// the folder empty and shows in the app once.
pub(super) fn request_repo(dir: &'static str, repo: &'static str, branch: &'static str) {
    let root = PathBuf::from(dir);
    with(|tree| tree.repo.pending.insert(root.clone()));
    let listed = root.clone();
    browser::tree(repo, branch, move |result| {
        let error = match result {
            Ok(entries) => {
                with(|tree| {
                    tree.items
                        .retain(|item| item.path != root && !item.path.starts_with(&root));
                    let modified = tree.stamp();
                    tree.items.push(Item {
                        path: root.clone(),
                        is_dir: true,
                        len: 0,
                        content: Vec::new(),
                        modified,
                        readonly: false,
                        fetched: true,
                    });
                    for entry in entries {
                        let path = root.join(&entry.path);
                        if let Some(parent) = path.parent() {
                            ensure_dir(tree, parent);
                        }
                        if entry.dir {
                            ensure_dir(tree, &path);
                            continue;
                        }
                        let modified = tree.stamp();
                        tree.items.push(Item {
                            path,
                            is_dir: false,
                            len: entry.size,
                            content: Vec::new(),
                            modified,
                            readonly: false,
                            fetched: false,
                        });
                    }
                    tree.repo.listed.insert(listed.clone());
                });
                None
            }
            Err(error) => Some(error),
        };
        // One attempt per page load: a tree that failed leaves the folder empty rather than
        // asking GitHub again on every frame that lists it.
        with(|tree| {
            tree.repo.pending.remove(&listed);
            tree.repo.listed.insert(listed);
        });
        if let Some(error) = error {
            state::fail(error);
        }
        state::touched();
    });
}

/// Asks GitHub for one file's bytes, once. A failed fetch leaves the file read with no content,
/// so no frame asks for the same bytes twice.
pub(super) fn request_file(path: PathBuf, repo: &'static str, branch: &'static str, rel: String) {
    if !with(|tree| tree.repo.fetching.insert(path.clone())) {
        return;
    }
    browser::file(repo, branch, &rel, move |bytes| {
        with(|tree| {
            tree.repo.fetching.remove(&path);
            if let Some(item) = tree.find_mut(&path) {
                let bytes = bytes.unwrap_or_default();
                item.len = bytes.len() as u64;
                item.content = bytes;
                item.fetched = true;
            }
        });
        state::touched();
    });
}
