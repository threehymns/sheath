//! External drives, and the sidebar's "Drives" section.
//!
//! A mounted drive opens like any other place. Its row carries an unmount button on the left.
//! A drive that is plugged in but not mounted is listed too: pressing it mounts it, then opens
//! its folder. The platform modules do the probing and the mounting; this module owns the list
//! the sidebar reads and the rules the buttons follow.
//!
//! Both probing and mounting run on worker threads: `diskutil` can take a while on a sleeping
//! drive, and a macOS refresh makes several calls, so the window must keep painting its spinner
//! meanwhile.
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

/// A list of drives being read on a worker.
struct Probe {
    result: Receiver<Result<Vec<Drive>, String>>,
}

/// The sidebar's drives.
///
/// A window starts with [`Drives::load`], which probes the machine once. `poll` refreshes it
/// every few seconds so a drive plugged in later appears without a restart.
#[derive(Default)]
pub struct Drives {
    drives: Vec<Drive>,
    /// When the last probe finished, so the next one waits out [`REFRESH_EVERY`].
    checked: Option<Instant>,
    // A blank app (tests, the settings preview) has no machine to probe until it loads.
    active: bool,
    // Mount and unmount commands in flight, by drive id. One at a time per drive.
    jobs: HashMap<String, Job>,
    // At most one probe runs at a time, and none runs while a command is in flight.
    probe: Option<Probe>,
    wake: Option<Waker>,
    #[cfg(test)]
    fixed: bool,
    #[cfg(test)]
    busy: HashMap<String, DriveAction>,
}

impl Drives {
    pub fn load() -> Drives {
        // The first frame starts the probe, once `setup` has attached its waker. A browser tab
        // has no drives, so it never probes and never starts a worker.
        #[cfg(target_arch = "wasm32")]
        {
            Drives::default()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            Drives {
                active: true,
                ..Drives::default()
            }
        }
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

    /// The window's waker, so a probe or command that finishes wakes the frame that lands it.
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

    /// Lands a finished probe or command, starts the next probe when one is due, and says
    /// whether the sidebar's list changed.
    pub fn poll(&mut self) -> DrivesPoll {
        #[cfg(test)]
        if self.fixed {
            return DrivesPoll::default();
        }
        if !self.active {
            return DrivesPoll::default();
        }
        let mut poll = DrivesPoll::default();

        if let Some(probe) = self.probe.as_ref() {
            match probe.result.try_recv() {
                Ok(result) => {
                    self.probe = None;
                    self.checked = Some(Instant::now());
                    if let Ok(found) = result
                        && found != self.drives
                    {
                        self.drives = found;
                        poll.changed = true;
                    }
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => {
                    self.probe = None;
                    self.checked = Some(Instant::now());
                }
            }
        }

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
        for (id, job, answer) in landed {
            // The row's spinner goes whether the command worked or not, so the next frame
            // paints the result either way.
            poll.changed = true;
            self.checked = Some(Instant::now());
            let result = answer.unwrap_or_else(|| {
                Err(format!(
                    "Could not {} “{}”: the command stopped unexpectedly.",
                    job.action.command(),
                    job.name
                ))
            });
            let (path, error) = match (job.action, result) {
                (DriveAction::Mount, Ok(path)) => {
                    // `udisksctl` names the folder it mounted; macOS asks `diskutil info`
                    // in the same worker. A platform that only says "mounted" leaves the
                    // next probe to find it.
                    let path = self
                        .find(&id)
                        .and_then(|drive| drive.mountpoint.clone())
                        .or(path);
                    match path {
                        Some(path) => {
                            if let Some(drive) = self.drives.iter_mut().find(|drive| drive.id == id)
                            {
                                drive.mountpoint = Some(path.clone());
                            }
                            (Some(path), None)
                        }
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
                (DriveAction::Unmount, Ok(_)) => {
                    if let Some(drive) = self.drives.iter_mut().find(|drive| drive.id == id) {
                        drive.mountpoint = None;
                    }
                    (job.mountpoint, None)
                }
                (DriveAction::Unmount, Err(error)) => (None, Some(error)),
            };
            poll.finished.push(Finished {
                action: job.action,
                path,
                error,
            });
        }

        let due = self
            .checked
            .is_none_or(|checked| checked.elapsed() >= REFRESH_EVERY);
        if due && self.probe.is_none() && self.jobs.is_empty() {
            self.start_probe();
        }
        poll
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
        self.start_with(id, drive, action, move |worker| match action {
            DriveAction::Mount => platform::mount(&worker),
            DriveAction::Unmount => platform::unmount(&worker).map(|()| None),
        })
    }

    /// The worker plumbing, with the platform call passed in so a test can hold a real thread
    /// open and then release it.
    fn start_with<F>(
        &mut self,
        id: &str,
        drive: Drive,
        action: DriveAction,
        work: F,
    ) -> Result<(), String>
    where
        F: FnOnce(Drive) -> Result<Option<PathBuf>, String> + Send + 'static,
    {
        // A probe that started before this command could land after it and put the old state
        // back, so the command's own result is the newest word on this drive.
        self.probe = None;
        let (tx, rx) = mpsc::channel();
        let wake = self.wake.clone();
        let name = drive.name.clone();
        let mountpoint = drive.mountpoint.clone();
        let spawned = std::thread::Builder::new()
            .name("sheath-drive".to_owned())
            .spawn(move || {
                let answer = work(drive);
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
                mountpoint,
                result: rx,
            },
        );
        Ok(())
    }

    /// Reads the machine's drives on a worker. The frame that asks for it never waits.
    fn start_probe(&mut self) {
        if self.probe.is_some() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        let wake = self.wake.clone();
        let spawned = std::thread::Builder::new()
            .name("sheath-drive-probe".to_owned())
            .spawn(move || {
                let answer = platform::probe();
                let _ = tx.send(answer);
                if let Some(wake) = wake {
                    wake();
                }
            });
        if spawned.is_ok() {
            self.probe = Some(Probe { result: rx });
        } else {
            // Do not retry a machine that cannot start a thread on every frame.
            self.checked = Some(Instant::now());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_mount_worker_round_trip_clears_the_busy_row() {
        let path = PathBuf::from("/tmp/sheath-worker-stick");
        let drive = Drive {
            id: "volume:stick".to_owned(),
            name: "Stick".to_owned(),
            device: "/dev/sdb1".to_owned(),
            mountpoint: None,
        };
        let mut drives = Drives {
            drives: vec![drive.clone()],
            checked: Some(Instant::now()),
            active: true,
            ..Drives::default()
        };

        let (release_tx, release_rx) = mpsc::channel();
        let (started_tx, started_rx) = mpsc::channel();
        let worker_path = path.clone();
        drives
            .start_with("volume:stick", drive, DriveAction::Mount, move |_| {
                started_tx.send(()).expect("the test is still waiting");
                release_rx.recv().expect("the test releases the worker");
                Ok(Some(worker_path))
            })
            .expect("the worker starts");

        assert_eq!(
            drives.busy("volume:stick"),
            Some(DriveAction::Mount),
            "the row is busy while the worker is held"
        );
        started_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("the worker reaches its gate");
        release_tx.send(()).expect("the worker waits for this");

        let polled = (0..400)
            .find_map(|_| {
                let polled = drives.poll();
                if polled.finished.is_empty() {
                    std::thread::sleep(Duration::from_millis(5));
                    None
                } else {
                    Some(polled)
                }
            })
            .expect("the worker's answer lands in a poll");

        assert!(
            drives.busy("volume:stick").is_none(),
            "landing the answer clears the busy row"
        );
        assert_eq!(
            drives.find("volume:stick").and_then(Drive::path),
            Some(path.as_path()),
            "the mounted folder is in the list"
        );
        assert_eq!(polled.finished.len(), 1);
        assert_eq!(polled.finished[0].action, DriveAction::Mount);
        assert_eq!(polled.finished[0].path.as_deref(), Some(path.as_path()));
        assert!(polled.finished[0].error.is_none());
    }
}
