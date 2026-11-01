//! The app's filesystem, behind one module.
//!
//! Native builds are `std::fs`, re-exported unchanged, so every call site reads the disk exactly
//! as before. A browser tab has no filesystem, so the web build browses an in-memory tree: the
//! demo home folder, Sheath's own KDL files (hydrated from OPFS and written back there), and the
//! folders the user shares through the File System Access API.
//!
//! `Path::exists`, `is_dir` and `is_file` go straight to the operating system and cannot be
//! intercepted, so the app asks this module for those too.

#[cfg(not(target_arch = "wasm32"))]
pub use std::fs::*;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::*;

#[cfg(target_arch = "wasm32")]
use std::io;
use std::path::Path;

/// Whether anything is at `path`.
pub fn exists(path: &Path) -> bool {
    metadata(path).is_ok()
}

/// Whether `path` is a folder.
pub fn is_dir(path: &Path) -> bool {
    metadata(path).map(|meta| meta.is_dir()).unwrap_or(false)
}

/// Whether `path` is a file.
pub fn is_file(path: &Path) -> bool {
    metadata(path).map(|meta| meta.is_file()).unwrap_or(false)
}

/// Removes a file, or a folder and everything below it. The browser build has no system Trash,
/// so trashing an item there removes it from the tree.
#[cfg(target_arch = "wasm32")]
pub fn remove_all(path: &Path) -> io::Result<()> {
    if is_dir(path) {
        remove_dir_all(path)
    } else {
        remove_file(path)
    }
}

/// Native builds have no browser storage behind them, so the web hooks do nothing.
#[cfg(not(target_arch = "wasm32"))]
mod browser_hooks {
    use rune::Waker;
    use std::path::{Path, PathBuf};

    pub fn set_waker(_wake: Waker) {}

    pub fn take_dirty() -> bool {
        false
    }

    pub fn take_granted() -> Option<PathBuf> {
        None
    }

    pub fn take_error() -> Option<String> {
        None
    }

    #[allow(dead_code)]
    pub fn pick_folder() {}

    #[allow(dead_code)]
    pub fn is_granted(_path: &Path) -> bool {
        false
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use browser_hooks::*;
