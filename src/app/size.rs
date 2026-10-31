//! A folder's total size, read on demand because walking a tree costs far more than listing the
//! one folder on screen.

use std::fs;
use std::path::{Path, PathBuf};

/// Total bytes of the regular files under `path`, and how many items that covers. Symlinks are
/// counted as items but never followed, so a link back up the tree cannot loop.
pub(super) fn folder_size(path: &Path) -> (u64, usize) {
    let mut bytes = 0;
    let mut items = 0;
    let mut stack: Vec<PathBuf> = vec![path.to_path_buf()];
    while let Some(dir) = stack.pop() {
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

    #[test]
    fn a_folder_adds_up_every_file_below_it() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), "one").unwrap();
        fs::create_dir(dir.path().join("inner")).unwrap();
        fs::write(dir.path().join("inner").join("b.txt"), "four").unwrap();
        fs::create_dir(dir.path().join("inner").join("deeper")).unwrap();
        fs::write(dir.path().join("inner").join("deeper").join("c.txt"), "two").unwrap();

        let (bytes, items) = folder_size(dir.path());
        assert_eq!(bytes, 3 + 4 + 3, "every file's bytes are counted");
        assert_eq!(items, 5, "files and folders both count as items");
    }

    #[test]
    fn an_empty_folder_counts_nothing() {
        let dir = tempfile::tempdir().unwrap();
        assert_eq!(folder_size(dir.path()), (0, 0));
    }
}
