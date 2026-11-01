//! The `std::fs` shapes the app reads a listing through: metadata, file types and directory
//! entries, answered from the tree.

use std::io;
use std::path::PathBuf;
use std::time::SystemTime;

pub struct Metadata {
    pub(super) is_dir: bool,
    pub(super) len: u64,
    pub(super) modified: SystemTime,
    pub(super) readonly: bool,
}

impl Metadata {
    pub fn is_dir(&self) -> bool {
        self.is_dir
    }

    pub fn is_file(&self) -> bool {
        !self.is_dir
    }

    pub fn len(&self) -> u64 {
        self.len
    }

    pub fn modified(&self) -> io::Result<SystemTime> {
        Ok(self.modified)
    }

    pub fn created(&self) -> io::Result<SystemTime> {
        Ok(self.modified)
    }

    pub fn permissions(&self) -> Permissions {
        Permissions {
            readonly: self.readonly,
        }
    }

    pub fn file_type(&self) -> FileType {
        FileType {
            is_dir: self.is_dir,
        }
    }
}

pub struct Permissions {
    readonly: bool,
}

impl Permissions {
    pub fn readonly(&self) -> bool {
        self.readonly
    }
}

pub struct FileType {
    is_dir: bool,
}

impl FileType {
    pub fn is_dir(&self) -> bool {
        self.is_dir
    }

    pub fn is_file(&self) -> bool {
        !self.is_dir
    }

    /// The demo tree and shared folders hold no links.
    pub fn is_symlink(&self) -> bool {
        false
    }
}

pub struct DirEntry {
    pub(super) path: PathBuf,
}

impl DirEntry {
    pub fn path(&self) -> PathBuf {
        self.path.clone()
    }

    pub fn file_name(&self) -> std::ffi::OsString {
        self.path
            .file_name()
            .map(ToOwned::to_owned)
            .unwrap_or_default()
    }

    pub fn metadata(&self) -> io::Result<Metadata> {
        super::read::metadata(&self.path)
    }

    pub fn file_type(&self) -> io::Result<FileType> {
        Ok(self.metadata()?.file_type())
    }
}

pub struct ReadDir {
    pub(super) items: Vec<DirEntry>,
    pub(super) at: usize,
}

impl Iterator for ReadDir {
    type Item = io::Result<DirEntry>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.items.get(self.at)?;
        self.at += 1;
        Some(Ok(DirEntry {
            path: item.path.clone(),
        }))
    }
}
