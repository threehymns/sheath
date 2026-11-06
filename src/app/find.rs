//! Recursive search below the folder on screen.
//!
//! The desktop build gives the walk and the ranking to fff, which indexes the folder once and
//! scores every match. That index means walking the whole tree, so it is built on `Latest` like a
//! folder scan: the keystroke that asks for the first search returns at once, the walk runs on a
//! worker, and the frame that lands the index searches again. The query itself is cheap against a
//! ready index, so it stays in the frame. A browser tab has no filesystem to index, so the web
//! demo keeps the plain walk over its in-memory tree.

use std::path::{Path, PathBuf};

use super::scan::Found;

use rune::Waker;

#[cfg(not(target_arch = "wasm32"))]
use rune::Latest;

#[cfg(not(target_arch = "wasm32"))]
use super::Entry;

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
    // The build in flight. `Latest` keeps the newest request only, so a walk nobody is waiting
    // for any more is dropped rather than run.
    #[cfg(not(target_arch = "wasm32"))]
    jobs: Latest<Index>,
    // The folder the build in flight covers, so typing again in the same folder does not ask for
    // the same walk twice.
    #[cfg(not(target_arch = "wasm32"))]
    building: Option<PathBuf>,
}

impl Finder {
    pub(super) fn set_waker(&mut self, waker: Waker) {
        #[cfg(not(target_arch = "wasm32"))]
        self.jobs.set_waker(waker);
        #[cfg(target_arch = "wasm32")]
        let _ = waker;
    }

    /// Forgets the index, so the next search walks the folder again.
    pub(super) fn invalidate(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.index = None;
            self.building = None;
            self.jobs.cancel();
        }
    }

    /// Whether the folder is still being walked for the search.
    pub(super) fn is_building(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.building.is_some() && self.jobs.is_pending()
        }
        #[cfg(target_arch = "wasm32")]
        {
            false
        }
    }

    /// The folder an index that just finished covers, so the caller searches again over it.
    pub(super) fn land(&mut self) -> Option<PathBuf> {
        #[cfg(not(target_arch = "wasm32"))]
        {
            let index = self.jobs.take()?;
            self.building = None;
            let root = index.root.clone();
            self.index = Some(index);
            Some(root)
        }
        #[cfg(target_arch = "wasm32")]
        {
            None
        }
    }

    /// Every entry under `root` that matches `query`, best matches first. The first search in a
    /// folder comes back empty and starts the walk instead; the caller searches again when
    /// [`land`](Finder::land) says the index is ready.
    pub(super) fn find(&mut self, root: &Path, query: &str, limit: usize) -> Found {
        #[cfg(target_arch = "wasm32")]
        {
            // The walk sees hidden names too; `apply_filter` decides whether a row is shown, so
            // a search and a plain folder listing hide the same rows.
            super::scan::search_below(root, query, limit)
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.index.as_ref().is_none_or(|index| index.root != root) {
                self.index = None;
                self.build(root);
            }
            self.index
                .as_ref()
                .map_or_else(Found::default, |index| index.search(query, limit))
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn build(&mut self, root: &Path) {
        if self.building.as_deref() == Some(root) {
            return;
        }
        let folder = root.to_path_buf();
        self.building = Some(folder.clone());
        self.jobs.request(folder, Index::read);
    }
}

#[cfg(not(target_arch = "wasm32"))]
struct Index {
    root: PathBuf,
    // None when fff could not open the folder: nothing below it can be searched.
    picker: Option<FilePicker>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Index {
    /// Walks `root` and keeps the index. This is the expensive half of a search, which is why it
    /// runs on the worker and not in the frame that typed the query.
    fn read(root: PathBuf) -> Index {
        let picker = FilePicker::new(FilePickerOptions {
            base_path: root.to_string_lossy().into_owned(),
            // The app has its own watcher, and a reload builds this index again.
            watch: false,
            // A file manager searches wherever the user is, home folder and root included.
            enable_home_dir_scanning: true,
            enable_fs_root_scanning: true,
            ..Default::default()
        })
        .ok()
        .and_then(|mut picker| picker.collect_files().ok().map(|_| picker));
        Index { root, picker }
    }

    fn search(&self, query: &str, limit: usize) -> Found {
        let Some(picker) = &self.picker else {
            return Found::default();
        };
        let parsed = QueryParser::new(FileSearchConfig).parse(query);
        let found = picker.fuzzy_search_mixed(
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
                MixedItemRef::File(file) => file.relative_path(picker),
                MixedItemRef::Dir(dir) => dir.relative_path(picker),
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
