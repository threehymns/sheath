//! External drives, and the sidebar's "Drives" section.
//!
//! A mounted drive opens like any other place. Its row carries an unmount button on the left.
//! A drive that is plugged in but not mounted is listed too: pressing it mounts it, then opens
//! its folder. The platform modules do the probing and the mounting; this module owns the list
//! the sidebar reads and the rules the buttons follow.
//!
//! Mounting and unmounting run on a worker thread: `udisksctl` and `diskutil` can take a while
//! on a sleeping drive, and the window must keep painting its spinner meanwhile.
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

use rune::Waker;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::{Duration, Instant};

/// How often a running window looks for a drive that was plugged in or pulled out.
const REFRESH_EVERY: Duration = Duration::from_secs(5);

/// The work a drive row can start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DriveAction {
    Mount,
    Unmount,
}

impl DriveAction {
    /// The word the sidebar shows while the worker runs.
    pub fn verb(self) -> &'static str {
        match self {
            DriveAction::Mount => "Mounting",
            DriveAction::Unmount => "Unmounting",
        }
    }

    fn command(self) -> &'static str {
        match self {
            DriveAction::Mount => "mount",
            DriveAction::Unmount => "unmount",
        }
    }
}

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

/// A finished mount or unmount, ready for the app to act on.
pub struct Finished {
    pub action: DriveAction,
    /// The folder to open after mounting, or the folder to leave after unmounting.
    pub path: Option<PathBuf>,
    /// Why the command failed, when it did.
    pub error: Option<String>,
}

/// What a frame's drive poll found.
#[derive(Default)]
pub struct DrivesPoll {
    /// Whether the sidebar's list or one of its rows changed.
    pub changed: bool,
    pub finished: Vec<Finished>,
}

struct Job {
    action: DriveAction,
    name: String,
    /// The folder the drive was mounted at when an unmount started.
    mountpoint: Option<PathBuf>,
    result: Receiver<Result<Option<PathBuf>, String>>,
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
    // Mount and unmount commands in flight, by drive id. One at a time per drive.
    jobs: HashMap<String, Job>,
    wake: Option<Waker>,
    #[cfg(test)]
    fixed: bool,
    #[cfg(test)]
    busy: HashMap<String, DriveAction>,
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
            ..Drives::default()
        }
    }

    /// Marks a drive as busy without starting a worker, for the sidebar's busy-state tests.
    #[cfg(test)]
    pub fn fixed_busy(mut self, id: &str, action: DriveAction) -> Drives {
        self.busy.insert(id.to_owned(), action);
        self
    }

    /// The window's waker, so a command that finishes wakes the frame that clears the busy row.
    pub fn set_waker(&mut self, wake: Waker) {
        self.wake = Some(wake);
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

    /// The work in flight for a drive, so its row can show a spinner instead of its action.
    pub fn busy(&self, id: &str) -> Option<DriveAction> {
        #[cfg(test)]
        if let Some(action) = self.busy.get(id) {
            return Some(*action);
        }
        self.jobs.get(id).map(|job| job.action)
    }

    /// Looks for drives again when the interval has passed, and lands every finished command.
    /// Says whether the sidebar's list changed, so a frame that found the same drives can skip
    /// rebuilding the view.
    pub fn poll(&mut self) -> DrivesPoll {
        #[cfg(test)]
        if self.fixed {
            return DrivesPoll::default();
        }
        if !self.active {
            return DrivesPoll::default();
        }
        let mut poll = DrivesPoll::default();
        let mut landed = Vec::new();
        if !self.jobs.is_empty() {
            let ids: Vec<String> = self.jobs.keys().cloned().collect();
            for id in ids {
                let Some(job) = self.jobs.get(&id) else {
                    continue;
                };
                let answer = match job.result.try_recv() {
                    Ok(answer) => Some(answer),
                    Err(TryRecvError::Empty) => continue,
                    Err(TryRecvError::Disconnected) => None,
                };
                let job = self.jobs.remove(&id).expect("the job was just found");
                landed.push((id, job, answer));
            }
        }
        let due = self
            .checked
            .is_none_or(|checked| checked.elapsed() >= REFRESH_EVERY);
        if !landed.is_empty() || due {
            poll.changed = self.refresh();
        }
        for (id, job, answer) in landed {
            // The row's spinner goes whether the command worked or not, so the next frame
            // paints the result either way.
            poll.changed = true;
            let result = answer.unwrap_or_else(|| {
                Err(format!(
                    "Could not {} “{}”: the command stopped unexpectedly.",
                    job.action.command(),
                    job.name
                ))
            });
            let (path, error) = match (job.action, result) {
                (DriveAction::Mount, Ok(path)) => {
                    // `udisksctl` names the folder it mounted; a platform that only says
                    // "mounted" leaves the refreshed list to find it.
                    let path = self
                        .find(&id)
                        .and_then(|drive| drive.mountpoint.clone())
                        .or(path);
                    match path {
                        Some(path) => (Some(path), None),
                        None => (
                            None,
                            Some(format!(
                                "Mounted “{}” but could not find its folder.",
                                job.name
                            )),
                        ),
                    }
                }
                (DriveAction::Mount, Err(error)) => (None, Some(error)),
                // The command succeeded; leave the folder the drive was mounted at.
                (DriveAction::Unmount, Ok(_)) => (job.mountpoint, None),
                (DriveAction::Unmount, Err(error)) => (None, Some(error)),
            };
            poll.finished.push(Finished {
                action: job.action,
                path,
                error,
            });
        }
        poll
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

    /// Mounts a drive if it is not mounted. A mounted drive answers with its folder at once;
    /// an unmounted one starts a worker and answers `None` while the row shows its spinner.
    pub fn mount(&mut self, id: &str) -> Result<Option<PathBuf>, String> {
        let Some(drive) = self.find(id).cloned() else {
            return Err("That drive is no longer plugged in.".to_owned());
        };
        if self.busy(id).is_some() {
            return Ok(None);
        }
        if let Some(path) = drive.mountpoint.clone() {
            return Ok(Some(path));
        }
        #[cfg(test)]
        if self.fixed {
            let path = std::env::temp_dir().join(format!("sheath-test-{id}"));
            if let Some(drive) = self.drives.iter_mut().find(|drive| drive.id == id) {
                drive.mountpoint = Some(path.clone());
            }
            return Ok(Some(path));
        }
        self.start(id, drive, DriveAction::Mount)?;
        Ok(None)
    }

    /// Unmounts a drive, answering with where it was mounted so the app can leave that folder.
    /// The answer is `None` while the worker runs and the row shows its spinner.
    pub fn unmount(&mut self, id: &str) -> Result<Option<PathBuf>, String> {
        let Some(drive) = self.find(id).cloned() else {
            return Err("That drive is no longer plugged in.".to_owned());
        };
        if self.busy(id).is_some() {
            return Ok(None);
        }
        if drive.mountpoint.is_none() {
            return Err(format!("“{}” is not mounted.", drive.name));
        }
        #[cfg(test)]
        if self.fixed {
            let path = drive
                .mountpoint
                .clone()
                .expect("the drive was checked as mounted");
            if let Some(drive) = self.drives.iter_mut().find(|drive| drive.id == id) {
                drive.mountpoint = None;
            }
            return Ok(Some(path));
        }
        self.start(id, drive, DriveAction::Unmount)?;
        Ok(None)
    }

    /// Runs the platform command off the UI thread. The answer lands in `poll`, which is where
    /// the list and the app's navigation change.
    fn start(&mut self, id: &str, drive: Drive, action: DriveAction) -> Result<(), String> {
        let (tx, rx) = mpsc::channel();
        let wake = self.wake.clone();
        let worker = drive.clone();
        let name = drive.name.clone();
        let spawned = std::thread::Builder::new()
            .name("sheath-drive".to_owned())
            .spawn(move || {
                let answer = match action {
                    DriveAction::Mount => platform::mount(&worker),
                    DriveAction::Unmount => platform::unmount(&worker).map(|()| None),
                };
                let _ = tx.send(answer);
                if let Some(wake) = wake {
                    wake();
                }
            });
        if let Err(error) = spawned {
            return Err(format!(
                "Could not {} “{}”: {error}",
                action.command(),
                name
            ));
        }
        self.jobs.insert(
            id.to_owned(),
            Job {
                action,
                name,
                mountpoint: drive.mountpoint,
                result: rx,
            },
        );
        Ok(())
    }
}
