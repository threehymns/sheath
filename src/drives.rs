//! External drives, and the sidebar's "Drives" section.
//!
//! A mounted drive opens like any other place. Its row carries an unmount button on the left.
//! A drive that is plugged in but not mounted is listed too: pressing it mounts it, then opens
//! its folder. The platform modules do the probing and the mounting; this module owns the list
//! the sidebar reads and the rules the buttons follow.
//!
//! The web build has no drives: a browser tab cannot mount one.

#[cfg(all(unix, not(target_os = "macos")))]
mod linux;
#[cfg(all(unix, not(target_os = "macos")))]
use linux as platform;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
use macos as platform;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
use windows as platform;
#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
use web as platform;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// How often a running window looks for a drive that was plugged in or pulled out.
const REFRESH_EVERY: Duration = Duration::from_secs(5);

/// One external drive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Drive {
    /// Stable across a remount where the platform has a filesystem UUID for it.
    pub id: String,
    pub name: String,
    /// What the platform's mount and unmount commands take.
    pub device: String,
    /// Where it is mounted, or `None` while it is not.
    pub mountpoint: Option<PathBuf>,
}

impl Drive {
    pub fn is_mounted(&self) -> bool {
        self.mountpoint.is_some()
    }

    pub fn path(&self) -> Option<&Path> {
        self.mountpoint.as_deref()
    }
}

/// The sidebar's drives.
///
/// A window starts with [`Drives::load`], which probes the machine once. `poll` refreshes it
/// every few seconds so a drive plugged in later appears without a restart.
#[derive(Default)]
pub struct Drives {
    drives: Vec<Drive>,
    checked: Option<Instant>,
    // A blank app (tests, the settings preview) has no machine to probe until it loads.
    active: bool,
    #[cfg(test)]
    fixed: bool,
}

impl Drives {
    pub fn load() -> Drives {
        let mut drives = Drives {
            active: true,
            ..Drives::default()
        };
        drives.refresh();
        drives
    }

    /// A list that never probes or runs a command; the sidebar tests set this.
    #[cfg(test)]
    pub fn fixed(drives: Vec<Drive>) -> Drives {
        Drives {
            drives,
            checked: Some(Instant::now()),
            active: true,
            fixed: true,
        }
    }

    pub fn drives(&self) -> &[Drive] {
        &self.drives
    }

    pub fn is_empty(&self) -> bool {
        self.drives.is_empty()
    }

    pub fn find(&self, id: &str) -> Option<&Drive> {
        self.drives.iter().find(|drive| drive.id == id)
    }

    /// Looks for drives again when the interval has passed. Says whether the sidebar's list
    /// changed, so a frame that found the same drives can skip rebuilding the view.
    pub fn poll(&mut self) -> bool {
        #[cfg(test)]
        if self.fixed {
            return false;
        }
        if !self.active {
            return false;
        }
        if self
            .checked
            .is_some_and(|checked| checked.elapsed() < REFRESH_EVERY)
        {
            return false;
        }
        self.refresh()
    }

    fn refresh(&mut self) -> bool {
        self.checked = Some(Instant::now());
        let Ok(drives) = platform::probe() else {
            // A probe that cannot run (no `lsblk`, no `diskutil`) is not worth a toast on
            // every try; the list keeps what it last knew.
            return false;
        };
        if drives == self.drives {
            return false;
        }
        self.drives = drives;
        true
    }

    /// Mounts a drive if it is not mounted, then answers with the folder to open.
    pub fn mount(&mut self, id: &str) -> Result<PathBuf, String> {
        let Some(drive) = self.find(id).cloned() else {
            return Err("That drive is no longer plugged in.".to_owned());
        };
        if let Some(path) = drive.mountpoint.clone() {
            return Ok(path);
        }
        #[cfg(test)]
        if self.fixed {
            let path = std::env::temp_dir().join(format!("sheath-test-{id}"));
            if let Some(drive) = self.drives.iter_mut().find(|drive| drive.id == id) {
                drive.mountpoint = Some(path.clone());
            }
            return Ok(path);
        }
        let mounted = platform::mount(&drive)?;
        self.refresh();
        self.find(id)
            .and_then(|drive| drive.mountpoint.clone())
            .or(mounted)
            .ok_or_else(|| format!("Mounted “{}” but could not find its folder.", drive.name))
    }

    /// Unmounts a drive, answering with where it was mounted so the app can leave that folder.
    pub fn unmount(&mut self, id: &str) -> Result<Option<PathBuf>, String> {
        let Some(drive) = self.find(id).cloned() else {
            return Err("That drive is no longer plugged in.".to_owned());
        };
        let Some(path) = drive.mountpoint.clone() else {
            return Err(format!("“{}” is not mounted.", drive.name));
        };
        #[cfg(test)]
        if self.fixed {
            if let Some(drive) = self.drives.iter_mut().find(|drive| drive.id == id) {
                drive.mountpoint = None;
            }
            return Ok(Some(path));
        }
        platform::unmount(&drive)?;
        self.refresh();
        Ok(Some(path))
    }
}
