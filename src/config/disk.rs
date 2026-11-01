//! The config folder's disk access, kept apart from the store so `Store::poll` never reads a file.
//! A change the watcher reports is read and parsed on a worker thread, and only the finished
//! [`Readback`] reaches the store, in a later `poll`.

use crate::vfs as fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) struct Stamp {
    pub modified: Option<SystemTime>,
    pub len: u64,
}

pub(super) fn stamp_of(path: &Path) -> Option<Stamp> {
    let meta = fs::metadata(path).ok()?;
    Some(Stamp {
        modified: meta.modified().ok(),
        len: meta.len(),
    })
}

/// One file as a worker found it: the stamp it had, and its text or why it couldn't be read.
pub(super) struct Readback {
    pub name: &'static str,
    pub path: PathBuf,
    pub stamp: Option<Stamp>,
    pub text: std::io::Result<String>,
}

pub(super) fn read(name: &'static str, path: PathBuf) -> Readback {
    // The stamp comes first, so a write that lands mid-read is caught by the next read rather
    // than being recorded under an older stamp.
    let stamp = stamp_of(&path);
    let text = fs::read_to_string(&path);
    Readback {
        name,
        path,
        stamp,
        text,
    }
}

pub(super) fn read_all(wanted: Vec<(&'static str, PathBuf)>) -> Vec<Readback> {
    wanted
        .into_iter()
        .map(|(name, path)| read(name, path))
        .collect()
}
