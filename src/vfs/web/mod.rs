//! The browser build's filesystem.
//!
//! A tab has no files, so this tree starts with a demo home folder. Sheath's own KDL files are
//! hydrated from OPFS before the window opens, and every write to them goes back to OPFS, so
//! settings and remembered folders survive a reload. Folders the user shares through the File
//! System Access API are mounted under `/granted` and browsed read-only.
//!
//! This module is the interface: the `std::fs`-shaped calls the app makes, and the browser hooks
//! the frame drains. The implementation is split by job — the tree itself, the browser mailbox,
//! shared folders, mirrored repositories, OPFS, and the read and write sides of the facade.

mod browser;
mod grant;
mod meta;
mod read;
mod repo;
mod state;
mod store;
mod tree;
mod write;

pub use grant::is_granted;
pub use meta::{DirEntry, Metadata};
pub use read::{metadata, read, read_dir, read_to_string};
pub use state::{
    Decoded, pick_folder, request_preview, request_thumbnail, set_waker, take_dirty, take_error,
    take_granted, take_preview, take_thumbnail,
};
pub use store::hydrate;
pub use write::{
    canonicalize, copy, create_dir, create_dir_all, remove_dir_all, remove_file, rename, write,
};

use std::cell::RefCell;
use std::io;
use std::path::{Component, Path, PathBuf};
use tree::Tree;

/// The demo's home folder, and where the window opens.
const HOME: &str = "/home/you";
/// The demo's Projects folder mirrors real repositories, each fetched the first time its folder
/// is opened rather than seeded with a stand-in tree: the demo path, the repository it shows, and
/// the branch to list (each repository's default, since not every one is `main`).
const PROJECTS: [(&str, &str, &str); 2] = [
    ("/home/you/Projects/sheath", "threehymns/sheath", "main"),
    ("/home/you/Projects/np", "threehymns/np", "master"),
];
const CONFIG_DIR: &str = "/home/you/.config/sheath";
const STATE_DIR: &str = "/home/you/.local/state/sheath";
/// Where a folder shared with the tab is mounted.
const GRANT_ROOT: &str = "/granted";

pub fn home() -> PathBuf {
    PathBuf::from(HOME)
}

pub fn config_dir() -> PathBuf {
    PathBuf::from(CONFIG_DIR)
}

pub fn state_dir() -> PathBuf {
    PathBuf::from(STATE_DIR)
}

thread_local! {
    static TREE: RefCell<Tree> = RefCell::new(tree::seed());
}

fn with<R>(f: impl FnOnce(&mut Tree) -> R) -> R {
    TREE.with(|tree| f(&mut tree.borrow_mut()))
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::from("/");
    for component in path.components() {
        match component {
            Component::Normal(part) => out.push(part),
            Component::ParentDir => {
                out.pop();
            }
            Component::RootDir | Component::CurDir | Component::Prefix(_) => {}
        }
    }
    out
}

fn missing(path: &Path) -> io::Error {
    io::Error::new(
        io::ErrorKind::NotFound,
        format!("{}: no such file or folder", path.display()),
    )
}
