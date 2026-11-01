//! A folder's total size, read on demand because walking a tree costs far more than listing
//! the one folder on screen.
//!
//! A walk of a large folder takes seconds, so it runs on `Latest` like a folder scan: a newer
//! request supersedes a queued one, and the frame that lands the answer draws it. A walk that is
//! already running checks the same stop flag at every folder it enters, so moving the selection
//! off a big tree ends the read instead of letting it finish for nobody.

use crate::vfs as fs;
use rune::{Latest, Waker};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A finished walk, with the folder it measured so a stale answer can be dropped.
pub(super) struct Total {
    pub(super) path: PathBuf,
    pub(super) bytes: u64,
    pub(super) items: usize,
}

#[derive(Default)]
pub(super) struct Sizer {
    jobs: Latest<Total>,
    // Shared with the walk in flight: set when that walk is no longer wanted.
    stop: Option<Arc<AtomicBool>>,
}

impl Sizer {
    pub(super) fn set_waker(&mut self, waker: Waker) {
        self.jobs.set_waker(waker);
    }

    pub(super) fn request(&mut self, path: &Path) {
        // Whatever was walking measured a folder nobody is looking at now.
        self.stop_walk();
        let stop = Arc::new(AtomicBool::new(false));
        self.stop = Some(stop.clone());
        let folder = path.to_path_buf();
        self.jobs.request(folder, move |folder| {
            let (bytes, items) = folder_size(&folder, &stop);
            Total {
                path: folder,
                bytes,
                items,
            }
        });
    }

    pub(super) fn take(&mut self) -> Option<Total> {
        self.jobs.take()
    }

    pub(super) fn cancel(&mut self) {
        self.stop_walk();
        self.jobs.cancel();
    }

    pub(super) fn is_pending(&self) -> bool {
        self.jobs.is_pending()
    }

    fn stop_walk(&mut self) {
        if let Some(stop) = self.stop.take() {
            stop.store(true, Ordering::Relaxed);
        }
    }
}

/// Total bytes of the regular files under `path`, and how many items that covers. Symlinks are
/// counted as items but never followed, so a link back up the tree cannot loop. Setting `stop`
/// abandons the walk at the next folder it would enter.
pub(super) fn folder_size(path: &Path, stop: &AtomicBool) -> (u64, usize) {
    let mut bytes = 0;
    let mut items = 0;
    let mut stack: Vec<PathBuf> = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if stop.load(Ordering::Relaxed) {
            break;
        }
        let Ok(read) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in read.flatten() {
            items += 1;
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if file_type.is_dir() {
                stack.push(entry.path());
            } else if file_type.is_file() {
                bytes += entry.metadata().map(|meta| meta.len()).unwrap_or(0);
            }
        }
    }
    (bytes, items)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn walking(path: &Path) -> (u64, usize) {
        folder_size(path, &AtomicBool::new(false))
    }

    #[test]
    fn a_folder_adds_up_every_file_below_it() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "one").unwrap();
        fs::create_dir(dir.path().join("inner")).unwrap();
        fs::write(dir.path().join("inner").join("b.txt"), "four").unwrap();
        fs::create_dir(dir.path().join("inner").join("deeper")).unwrap();
        fs::write(dir.path().join("inner").join("deeper").join("c.txt"), "two").unwrap();

        let (bytes, items) = walking(dir.path());
        assert_eq!(bytes, 3 + 4 + 3, "every file's bytes are counted");
        assert_eq!(items, 5, "files and folders both count as items");
    }

    #[test]
    fn an_empty_folder_counts_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(walking(dir.path()), (0, 0));
    }

    #[test]
    fn a_walk_that_was_called_off_reads_nothing() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "one").unwrap();
        let stop = AtomicBool::new(true);
        assert_eq!(
            folder_size(dir.path(), &stop),
            (0, 0),
            "a walk that is no longer wanted does not read the tree"
        );
    }
}
