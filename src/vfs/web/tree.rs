//! The tree the browser build's filesystem answers from.
//!
//! One flat list of items, found by path. The demo home is seeded here; a shared folder lands as
//! its listing arrives, a mirrored repository as its tree arrives, and Sheath's own KDL files are
//! put in place before the window opens.

use std::collections::BTreeSet;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

/// Where the demo's clock starts; every write moves it a second on, so a renamed row sorts as
/// the newest one.
const BASE: u64 = 1_793_000_000;

fn at(seconds: u64) -> SystemTime {
    SystemTime::UNIX_EPOCH + Duration::from_secs(seconds)
}

pub(super) struct Item {
    pub(super) path: PathBuf,
    pub(super) is_dir: bool,
    pub(super) len: u64,
    pub(super) content: Vec<u8>,
    pub(super) modified: SystemTime,
    pub(super) readonly: bool,
    /// Whether a shared file's bytes have been fetched from the browser.
    pub(super) fetched: bool,
}

pub(super) struct Tree {
    pub(super) items: Vec<Item>,
    clock: u64,
    /// Shared folders the browser has listed, so a frame never asks twice.
    pub(super) listed: BTreeSet<PathBuf>,
    /// Shared folders and files whose browser read is still in flight.
    pub(super) pending: BTreeSet<PathBuf>,
    pub(super) repo: Repo,
}

/// The repositories behind the demo's Projects folder, each fetched once when first opened.
#[derive(Default)]
pub(super) struct Repo {
    /// Folders whose tree landed, so they list and their files are fetched as they are read.
    pub(super) listed: BTreeSet<PathBuf>,
    /// Tree requests in flight, so a frame never asks twice.
    pub(super) pending: BTreeSet<PathBuf>,
    /// Files whose bytes the browser is fetching.
    pub(super) fetching: BTreeSet<PathBuf>,
}

impl Tree {
    pub(super) fn find(&self, path: &Path) -> Option<&Item> {
        self.items.iter().find(|item| item.path == path)
    }

    pub(super) fn find_mut(&mut self, path: &Path) -> Option<&mut Item> {
        self.items.iter_mut().find(|item| item.path == path)
    }

    pub(super) fn children(&self, path: &Path) -> Vec<&Item> {
        let mut children: Vec<&Item> = self
            .items
            .iter()
            .filter(|item| item.path != path && item.path.parent() == Some(path))
            .collect();
        children.sort_by(|a, b| a.path.cmp(&b.path));
        children
    }

    pub(super) fn stamp(&mut self) -> SystemTime {
        self.clock += 1;
        at(self.clock)
    }

    pub(super) fn insert(
        &mut self,
        path: PathBuf,
        is_dir: bool,
        content: Vec<u8>,
    ) -> io::Result<()> {
        if self.find(&path).is_some() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{}: already exists", path.display()),
            ));
        }
        let parent = path.parent().ok_or_else(|| super::missing(&path))?;
        if !self.find(parent).is_some_and(|item| item.is_dir) {
            return Err(super::missing(parent));
        }
        let len = content.len() as u64;
        let modified = self.stamp();
        self.items.push(Item {
            path,
            is_dir,
            len,
            content,
            modified,
            readonly: false,
            fetched: true,
        });
        Ok(())
    }

    /// Adds a file from OPFS, replacing whatever the demo seeded at that path.
    pub(super) fn put(&mut self, path: PathBuf, content: Vec<u8>) {
        let len = content.len() as u64;
        let modified = self.stamp();
        match self.find_mut(&path) {
            Some(item) => {
                item.content = content;
                item.len = len;
                item.modified = modified;
            }
            None => self.items.push(Item {
                path,
                is_dir: false,
                len,
                content,
                modified,
                readonly: false,
                fetched: true,
            }),
        }
    }

    pub(super) fn relocate(&mut self, from: &Path, to: &Path) -> io::Result<()> {
        let Some(source) = self.find(from) else {
            return Err(super::missing(from));
        };
        let source_is_dir = source.is_dir;
        // `std::fs::rename` replaces an existing file, which is how `config::write_atomic`
        // puts a temp file in place; folders still can't land on something.
        if let Some(target) = self.find(to) {
            if target.is_dir || source_is_dir {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!("{}: already exists", to.display()),
                ));
            }
            self.items.retain(|item| item.path != to);
        }
        let parent = to.parent().ok_or_else(|| super::missing(to))?.to_path_buf();
        if !self.find(&parent).is_some_and(|item| item.is_dir) {
            return Err(super::missing(&parent));
        }
        let modified = self.stamp();
        for item in &mut self.items {
            if item.path == from || item.path.starts_with(from) {
                let rest = item.path.strip_prefix(from).unwrap_or(Path::new(""));
                item.path = to.join(rest);
                item.modified = modified;
            }
        }
        Ok(())
    }

    pub(super) fn land(&mut self, dir: &Path, info: super::browser::EntryInfo) {
        let path = dir.join(&info.name);
        if self.find(&path).is_some() {
            return;
        }
        let fetched = info.dir || info.text.is_some();
        self.items.push(Item {
            path,
            is_dir: info.dir,
            len: info.size,
            content: info.text.map(String::into_bytes).unwrap_or_default(),
            modified: info
                .modified
                .map(|millis| SystemTime::UNIX_EPOCH + Duration::from_millis(millis as u64))
                .unwrap_or(SystemTime::UNIX_EPOCH),
            readonly: false,
            fetched,
        });
    }
}

pub(super) fn seed() -> Tree {
    let mut tree = Tree {
        items: Vec::new(),
        clock: BASE,
        listed: BTreeSet::new(),
        pending: BTreeSet::new(),
        repo: Repo::default(),
    };
    for (path, is_dir) in [
        ("/", true),
        ("/granted", true),
        ("/home", true),
        ("/home/you", true),
        ("/home/you/Desktop", true),
        ("/home/you/Documents", true),
        ("/home/you/Downloads", true),
        ("/home/you/Pictures", true),
        ("/home/you/Projects", true),
        ("/home/you/Projects/sheath", true),
        ("/home/you/Projects/sheath/src", true),
        ("/home/you/Projects/np", true),
        ("/home/you/.config", true),
        ("/home/you/.config/sheath", true),
        ("/home/you/.local", true),
        ("/home/you/.local/state", true),
        ("/home/you/.local/state/sheath", true),
        ("/tmp", true),
    ] {
        tree.items.push(Item {
            path: PathBuf::from(path),
            is_dir,
            len: 0,
            content: Vec::new(),
            modified: at(BASE),
            readonly: false,
            fetched: true,
        });
    }
    for (path, contents) in [
        (
            "/home/you/README.md",
            "# Sheath\n\nA file manager running in a browser tab on Rune's web target.\n\n\
             This folder lives in memory: open, rename and copy what you like, then reload to\n\
             start over. Settings are kept in the browser's origin-private filesystem.\n",
        ),
        (
            "/home/you/Documents/notes.md",
            "# Notes\n\n- The web build browses this demo tree.\n- Renames and copies stay in the tab.\n- A reload puts the demo files back.\n",
        ),
        (
            "/home/you/Documents/plan.txt",
            "1. Browse the demo folder.\n2. Switch to the grid.\n3. Read a preview.\n",
        ),
        (
            "/home/you/Downloads/release-notes.txt",
            "Sheath web preview\n\nRenders through Rune's WebGPU canvas. Settings and places\n\
             live in OPFS; folders you share with the tab can be edited.\n",
        ),
        (
            "/home/you/Pictures/README.txt",
            "Share a folder from the path bar to browse pictures here.\n",
        ),
    ] {
        let len = contents.len() as u64;
        tree.items.push(Item {
            path: PathBuf::from(path),
            is_dir: false,
            len,
            content: contents.as_bytes().to_vec(),
            modified: at(BASE),
            readonly: false,
            fetched: true,
        });
    }
    debug_assert!(
        tree.items.iter().all(|item| item.path == Path::new("/")
            || tree
                .items
                .iter()
                .any(|parent| parent.is_dir && Some(parent.path.as_path()) == item.path.parent())),
        "every seeded item has its folder"
    );
    tree
}

/// Makes sure `dir` is in the tree, creating it and its parents.
pub(super) fn ensure_dir(tree: &mut Tree, dir: &Path) {
    if tree.find(dir).is_some_and(|item| item.is_dir) {
        return;
    }
    if let Some(parent) = dir.parent() {
        ensure_dir(tree, parent);
    }
    if tree.find(dir).is_none() {
        let modified = tree.stamp();
        tree.items.push(Item {
            path: dir.to_path_buf(),
            is_dir: true,
            len: 0,
            content: Vec::new(),
            modified,
            readonly: false,
            fetched: true,
        });
    }
}
