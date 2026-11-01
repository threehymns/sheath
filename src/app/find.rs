//! Recursive search below the folder on screen.
//!
//! The desktop build gives the walk and the ranking to fff, which indexes the folder once per
//! read and scores every match. A browser tab has no filesystem to index, so the web demo keeps
//! the plain walk over its in-memory tree.

use std::path::Path;

use super::scan::Found;

#[cfg(not(target_arch = "wasm32"))]
use super::Entry;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

#[cfg(not(target_arch = "wasm32"))]
use fff_search::{
    FilePicker, FilePickerOptions, FileSearchConfig, FuzzySearchOptions, MixedItemRef,
    PaginationArgs, QueryParser,
};

/// The search below the folder on screen, and the index fff keeps for it.
#[derive(Default)]
pub(super) struct Finder {
    #[cfg(not(target_arch = "wasm32"))]
    index: Option<Index>,
}

impl Finder {
    /// Forgets the index, so the next search walks the folder again.
    pub(super) fn invalidate(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.index = None;
        }
    }

    /// Every entry under `root` that matches `query`, best matches first.
    pub(super) fn find(
        &mut self,
        root: &Path,
        query: &str,
        show_hidden: bool,
        limit: usize,
    ) -> Found {
        #[cfg(target_arch = "wasm32")]
        {
            super::scan::search_below(root, query, show_hidden, limit)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            // fff decides what it indexes; hidden and ignored matches are handled with the rows.
            let _ = show_hidden;
            self.native(root, query, limit)
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct Index {
    root: PathBuf,
    picker: FilePicker,
}

#[cfg(not(target_arch = "wasm32"))]
impl Finder {
    fn native(&mut self, root: &Path, query: &str, limit: usize) -> Found {
        if self.index.as_ref().is_none_or(|index| index.root != root) {
            self.index = Index::read(root);
        }
        let Some(index) = &self.index else {
            // fff could not index the folder, so nothing below it can be searched.
            return Found {
                entries: Vec::new(),
                truncated: false,
            };
        };
        index.search(query, limit)
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl Index {
    fn read(root: &Path) -> Option<Index> {
        let mut picker = FilePicker::new(FilePickerOptions {
            base_path: root.to_string_lossy().into_owned(),
            // The app has its own watcher, and a reload builds this index again.
            watch: false,
            // A file manager searches wherever the user is, home folder and root included.
            enable_home_dir_scanning: true,
            enable_fs_root_scanning: true,
            ..Default::default()
        })
        .ok()?;
        picker.collect_files().ok()?;
        Some(Index {
            root: root.to_path_buf(),
            picker,
        })
    }

    fn search(&self, query: &str, limit: usize) -> Found {
        let parsed = QueryParser::new(FileSearchConfig).parse(query);
        let found = self.picker.fuzzy_search_mixed(
            &parsed,
            None,
            FuzzySearchOptions {
                max_threads: 0,
                pagination: PaginationArgs { offset: 0, limit },
                ..Default::default()
            },
        );
        let truncated = found.total_matched > found.items.len();
        let mut entries = Vec::with_capacity(found.items.len());
        for item in found.items {
            let relative = match item {
                MixedItemRef::File(file) => file.relative_path(&self.picker),
                MixedItemRef::Dir(dir) => dir.relative_path(&self.picker),
            };
            // A folder comes with the separator that marks a search for folders only.
            let relative = relative.trim_end_matches(['/', '\\']).to_owned();
            let folder = Path::new(&relative)
                .parent()
                .filter(|parent| !parent.as_os_str().is_empty())
                .map(|parent| parent.to_string_lossy().into_owned());
            if let Some(entry) = Entry::at(self.root.join(&relative), folder) {
                entries.push(entry);
            }
        }
        Found { entries, truncated }
    }
}
