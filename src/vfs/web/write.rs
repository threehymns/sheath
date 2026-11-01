//! The write side of the browser build's filesystem.
//!
//! A change lands in the tree first and goes out to the browser behind it, so a frame never waits
//! on a write. Shared folders and Sheath's own KDL files are the two places a change has to reach
//! the browser.

use super::browser;
use super::grant::{grant_for, push_to_grant};
use super::read::read;
use super::state::op_done;
use super::store;
use super::tree::Item;
use super::{missing, normalize, with};
use std::io;
use std::path::{Path, PathBuf};

pub fn write(path: impl AsRef<Path>, contents: impl AsRef<[u8]>) -> io::Result<()> {
    let path = normalize(path.as_ref());
    let contents = contents.as_ref().to_vec();
    if let Some((id, rel)) = grant_for(&path) {
        if rel.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::IsADirectory,
                format!("{}: is a folder", path.display()),
            ));
        }
        with(|tree| {
            if tree.find(&path).is_some_and(|item| item.is_dir) {
                return Err(io::Error::new(
                    io::ErrorKind::IsADirectory,
                    format!("{}: is a folder", path.display()),
                ));
            }
            if tree.find_mut(&path).is_some() {
                let modified = tree.stamp();
                let len = contents.len() as u64;
                if let Some(item) = tree.find_mut(&path) {
                    item.content = contents.clone();
                    item.len = len;
                    item.modified = modified;
                    item.fetched = true;
                }
                return Ok(());
            }
            tree.insert(path.clone(), false, contents.clone())
        })?;
        browser::write(id, &rel, &contents, op_done);
        return Ok(());
    }
    with(|tree| {
        if tree.find(&path).is_some_and(|item| item.is_dir) {
            return Err(io::Error::new(
                io::ErrorKind::IsADirectory,
                format!("{}: is a folder", path.display()),
            ));
        }
        if tree.find_mut(&path).is_some() {
            let modified = tree.stamp();
            let len = contents.len() as u64;
            if let Some(item) = tree.find_mut(&path) {
                item.content = contents;
                item.len = len;
                item.modified = modified;
            }
            return Ok(());
        }
        tree.insert(path.clone(), false, contents)
    })?;
    let saved = with(|tree| tree.find(&path).map(|item| item.content.clone()));
    if let Some(saved) = saved {
        store::persist(&path, &saved);
    }
    Ok(())
}

pub fn create_dir(path: &Path) -> io::Result<()> {
    let path = normalize(path);
    if let Some((id, rel)) = grant_for(&path) {
        with(|tree| tree.insert(path.clone(), true, Vec::new()))?;
        browser::create_dir(id, &rel, op_done);
        return Ok(());
    }
    with(|tree| tree.insert(path, true, Vec::new()))
}

pub fn create_dir_all(path: &Path) -> io::Result<()> {
    let path = normalize(path);
    with(|tree| {
        let mut built = PathBuf::from("/");
        for part in path.components() {
            let std::path::Component::Normal(part) = part else {
                continue;
            };
            built.push(part);
            match tree.find(&built) {
                Some(item) if !item.is_dir => {
                    return Err(io::Error::new(
                        io::ErrorKind::NotADirectory,
                        format!("{}: not a folder", built.display()),
                    ));
                }
                Some(_) => {}
                None => tree.insert(built.clone(), true, Vec::new())?,
            }
        }
        Ok(())
    })
}

pub fn canonicalize(path: &Path) -> io::Result<PathBuf> {
    let path = normalize(path);
    with(|tree| {
        if tree.find(&path).is_some() {
            Ok(path.clone())
        } else {
            Err(missing(&path))
        }
    })
}

pub fn rename(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<()> {
    let from = normalize(from.as_ref());
    let to = normalize(to.as_ref());
    match (grant_for(&from), grant_for(&to)) {
        (Some((id, from_rel)), Some((to_id, to_rel))) => {
            if id != to_id {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "a shared folder can only move items inside itself",
                ));
            }
            with(|tree| tree.relocate(&from, &to))?;
            browser::move_entry(id, &from_rel, &to_rel, op_done);
            Ok(())
        }
        (Some((id, from_rel)), None) => {
            let is_dir = with(|tree| {
                tree.find(&from)
                    .map(|item| item.is_dir)
                    .ok_or_else(|| missing(&from))
            })?;
            if is_dir {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "a shared folder can’t move into the demo tree; copy its files instead",
                ));
            }
            let content = read(&from)?;
            with(|tree| -> io::Result<()> {
                tree.relocate(&from, &to)?;
                if let Some(item) = tree.find_mut(&to) {
                    item.len = content.len() as u64;
                    item.content = content.clone();
                    item.fetched = true;
                }
                Ok(())
            })?;
            browser::remove_entry(id, &from_rel, false, op_done);
            Ok(())
        }
        (None, Some(_)) => {
            let ops = with(|tree| push_to_grant(tree, &from, &to));
            with(|tree| tree.relocate(&from, &to))?;
            for (id, rel, is_dir, content) in ops {
                if is_dir {
                    browser::create_dir(id, &rel, op_done);
                } else {
                    browser::write(id, &rel, &content, op_done);
                }
            }
            Ok(())
        }
        (None, None) => {
            let saved = with(|tree| -> io::Result<Option<Vec<u8>>> {
                tree.relocate(&from, &to)?;
                Ok(tree.find(&to).map(|item| item.content.clone()))
            })?;
            // `config::write_atomic` writes a temporary file and renames it into place, so the
            // saved copy is always the destination of a rename.
            if let Some(content) = saved {
                store::persist(&to, &content);
            }
            Ok(())
        }
    }
}

/// Files only: the app copies a folder by making it and copying each child here.
pub fn copy(from: impl AsRef<Path>, to: impl AsRef<Path>) -> io::Result<u64> {
    let from = normalize(from.as_ref());
    let to = normalize(to.as_ref());
    if let Some((id, from_rel)) = grant_for(&from) {
        let (len, content, fetched) = with(|tree| {
            let item = tree.find(&from).ok_or_else(|| missing(&from))?;
            if item.is_dir {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{}: is a folder", from.display()),
                ));
            }
            Ok((item.len, item.content.clone(), item.fetched))
        })?;
        match grant_for(&to) {
            Some((to_id, to_rel)) if to_id == id => {
                if fetched && !content.is_empty() {
                    browser::write(id, &to_rel, &content, op_done);
                } else {
                    browser::copy_entry(id, &from_rel, &to_rel, op_done);
                }
                with(|tree| {
                    let modified = tree.stamp();
                    tree.items.push(Item {
                        path: to,
                        is_dir: false,
                        len,
                        content,
                        modified,
                        readonly: false,
                        fetched,
                    });
                });
                Ok(len)
            }
            Some(_) => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "a shared folder can only copy items inside itself",
            )),
            None => {
                // Copying into the demo tree needs the bytes, which a listing may not have
                // fetched yet; `read` asks the browser for them.
                let content = read(&from)?;
                let len = content.len() as u64;
                with(|tree| tree.insert(to, false, content))?;
                Ok(len)
            }
        }
    } else if let Some((id, to_rel)) = grant_for(&to) {
        let content = with(|tree| {
            let item = tree.find(&from).ok_or_else(|| missing(&from))?;
            if item.is_dir {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{}: is a folder", from.display()),
                ));
            }
            Ok(item.content.clone())
        })?;
        browser::write(id, &to_rel, &content, op_done);
        let len = content.len() as u64;
        with(|tree| tree.insert(to, false, content))?;
        Ok(len)
    } else {
        with(|tree| {
            let item = tree.find(&from).ok_or_else(|| missing(&from))?;
            if item.is_dir {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("{}: is a folder", from.display()),
                ));
            }
            let contents = item.content.clone();
            let len = contents.len() as u64;
            tree.insert(to, false, contents)?;
            Ok(len)
        })
    }
}

pub fn remove_file(path: &Path) -> io::Result<()> {
    let path = normalize(path);
    if let Some((id, rel)) = grant_for(&path) {
        with(|tree| {
            let Some(item) = tree.find(&path) else {
                return Err(missing(&path));
            };
            if item.is_dir {
                return Err(io::Error::new(
                    io::ErrorKind::IsADirectory,
                    format!("{}: is a folder", path.display()),
                ));
            }
            tree.items.retain(|item| item.path != path);
            Ok(())
        })?;
        browser::remove_entry(id, &rel, false, op_done);
        return Ok(());
    }
    with(|tree| {
        let Some(item) = tree.find(&path) else {
            return Err(missing(&path));
        };
        if item.is_dir {
            return Err(io::Error::new(
                io::ErrorKind::IsADirectory,
                format!("{}: is a folder", path.display()),
            ));
        }
        tree.items.retain(|item| item.path != path);
        Ok(())
    })?;
    store::forget(&path);
    Ok(())
}

#[allow(dead_code)]
pub fn remove_dir(path: &Path) -> io::Result<()> {
    let path = normalize(path);
    if let Some((id, rel)) = grant_for(&path) {
        with(|tree| {
            let Some(item) = tree.find(&path) else {
                return Err(missing(&path));
            };
            if !item.is_dir {
                return Err(io::Error::new(
                    io::ErrorKind::NotADirectory,
                    format!("{}: not a folder", path.display()),
                ));
            }
            if !tree.children(&path).is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::DirectoryNotEmpty,
                    format!("{}: the folder is not empty", path.display()),
                ));
            }
            tree.items.retain(|item| item.path != path);
            Ok(())
        })?;
        browser::remove_entry(id, &rel, false, op_done);
        return Ok(());
    }
    with(|tree| {
        let Some(item) = tree.find(&path) else {
            return Err(missing(&path));
        };
        if !item.is_dir {
            return Err(io::Error::new(
                io::ErrorKind::NotADirectory,
                format!("{}: not a folder", path.display()),
            ));
        }
        if !tree.children(&path).is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::DirectoryNotEmpty,
                format!("{}: the folder is not empty", path.display()),
            ));
        }
        tree.items.retain(|item| item.path != path);
        Ok(())
    })
}

pub fn remove_dir_all(path: &Path) -> io::Result<()> {
    let path = normalize(path);
    if let Some((id, rel)) = grant_for(&path) {
        with(|tree| {
            if tree.find(&path).is_none() {
                return Err(missing(&path));
            }
            tree.items
                .retain(|item| item.path != path && !item.path.starts_with(&path));
            Ok(())
        })?;
        browser::remove_entry(id, &rel, true, op_done);
        return Ok(());
    }
    with(|tree| {
        if tree.find(&path).is_none() {
            return Err(missing(&path));
        }
        tree.items
            .retain(|item| item.path != path && !item.path.starts_with(&path));
        Ok(())
    })
}
