//! The read side of the browser build's filesystem.
//!
//! Everything here answers from the tree. A shared folder or a mirrored repository that has not
//! been listed yet starts the browser request and reads empty; the next frame reads it again.

use super::grant::{grant_for, request_file as request_grant_file, request_listing};
use super::meta::{DirEntry, Metadata, ReadDir};
use super::repo::{repo_for, request_file as request_repo_file, request_repo};
use super::{missing, normalize, with};
use std::io;
use std::path::Path;

pub fn read_dir(path: &Path) -> io::Result<ReadDir> {
    let path = normalize(path);
    if let Some((id, rel)) = grant_for(&path) {
        let listed = with(|tree| tree.listed.contains(&path));
        if !listed {
            request_listing(path.clone(), id, rel);
            return Ok(ReadDir {
                items: Vec::new(),
                at: 0,
            });
        }
        let items = with(|tree| {
            tree.children(&path)
                .into_iter()
                .map(|item| DirEntry {
                    path: item.path.clone(),
                })
                .collect()
        });
        return Ok(ReadDir { items, at: 0 });
    }
    // Each mirrored Projects folder lists itself from its repository the first time it is opened.
    if let Some((dir, repo, branch, _)) = repo_for(&path)
        && !with(|tree| tree.repo.listed.contains(Path::new(dir)))
    {
        if !with(|tree| tree.repo.pending.contains(Path::new(dir))) {
            request_repo(dir, repo, branch);
        }
        return Ok(ReadDir {
            items: Vec::new(),
            at: 0,
        });
    }
    with(|tree| {
        if !tree.find(&path).is_some_and(|item| item.is_dir) {
            return Err(if tree.find(&path).is_some() {
                io::Error::new(
                    io::ErrorKind::NotADirectory,
                    format!("{}: not a folder", path.display()),
                )
            } else {
                missing(&path)
            });
        }
        let items = tree
            .children(&path)
            .into_iter()
            .map(|item| DirEntry {
                path: item.path.clone(),
            })
            .collect();
        Ok(ReadDir { items, at: 0 })
    })
}

pub fn metadata(path: &Path) -> io::Result<Metadata> {
    let path = normalize(path);
    with(|tree| {
        let item = tree.find(&path).ok_or_else(|| missing(&path))?;
        Ok(Metadata {
            is_dir: item.is_dir,
            len: item.len,
            modified: item.modified,
            readonly: item.readonly,
        })
    })
}

pub fn read(path: &Path) -> io::Result<Vec<u8>> {
    let path = normalize(path);
    if let Some((id, rel)) = grant_for(&path) {
        let state = with(|tree| {
            tree.find(&path)
                .map(|item| (item.is_dir, item.content.clone(), item.fetched))
        });
        return match state {
            Some((true, ..)) => Err(io::Error::new(
                io::ErrorKind::IsADirectory,
                format!("{}: is a folder", path.display()),
            )),
            Some((false, content, true)) => Ok(content),
            Some((false, _, false)) => {
                if !rel.is_empty() {
                    request_grant_file(path.clone(), id, rel);
                }
                Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "the browser is still reading that file",
                ))
            }
            None => Err(missing(&path)),
        };
    }
    // A mirrored repository carries a path and a size; its bytes arrive when a file is read.
    if let Some((_, repo, branch, rel)) = repo_for(&path) {
        let state = with(|tree| {
            tree.find(&path)
                .map(|item| (item.is_dir, item.content.clone(), item.fetched))
        });
        return match state {
            Some((true, ..)) => Err(io::Error::new(
                io::ErrorKind::IsADirectory,
                format!("{}: is a folder", path.display()),
            )),
            Some((false, content, true)) => Ok(content),
            Some((false, _, false)) => {
                if !rel.is_empty() {
                    request_repo_file(path.clone(), repo, branch, rel);
                }
                Err(io::Error::new(
                    io::ErrorKind::WouldBlock,
                    "GitHub is still sending that file",
                ))
            }
            None => Err(missing(&path)),
        };
    }
    with(|tree| {
        let item = tree.find(&path).ok_or_else(|| missing(&path))?;
        if item.is_dir {
            return Err(io::Error::new(
                io::ErrorKind::IsADirectory,
                format!("{}: is a folder", path.display()),
            ));
        }
        Ok(item.content.clone())
    })
}

pub fn read_to_string(path: &Path) -> io::Result<String> {
    String::from_utf8(read(path)?)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "the file is not text"))
}
