//! The open folder's watcher: something else writing to it, such as a download landing or an
//! editor saving, reloads the listing rather than leaving the rows stale until the next visit.

use notify::Watcher;
use rune::Waker;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};

/// What the watcher reports: that a folder changed, not what changed in it. The reply is a
/// re-read, which keeps it indifferent to how the change was made.
#[derive(Default)]
pub(super) struct Watch {
    /// The waker that tells the window to poll, kept so a folder opened later is watched with
    /// it too.
    wake: Option<Waker>,
    /// The folder being watched, so a change to a folder no longer on screen is not applied.
    path: Option<PathBuf>,
    changes: Option<Receiver<()>>,
    watcher: Option<notify::RecommendedWatcher>,
}

impl Watch {
    /// Starts watching `path`, and every folder `enter` moves to after this.
    pub(super) fn start(&mut self, path: &Path, wake: Waker) {
        self.wake = Some(wake);
        self.retarget(path);
    }

    /// Points the watcher at the folder now on screen. A window that never got a waker, or
    /// whose watcher could not be made, shows the folder without live updates rather than
    /// nothing at all.
    pub(super) fn retarget(&mut self, path: &Path) {
        let Some(wake) = self.wake.clone() else {
            return;
        };
        if self.changes.is_some() && self.path.as_deref() == Some(path) {
            return;
        }
        // The old watcher goes first: one left on a folder the app navigated away from would
        // reload rows that are no longer this list's.
        self.stop();
        let watched = path.to_path_buf();
        let seen = watched.clone();
        let (tx, rx) = mpsc::channel();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            let Ok(event) = event else {
                return;
            };
            // Only the folder's own entries change its listing: an event from inside a
            // subfolder moves nothing in this list.
            let ours = event
                .paths
                .iter()
                .any(|path| path.parent() == Some(seen.as_path()));
            if ours && tx.send(()).is_ok() {
                wake();
            }
        });
        let Ok(mut watcher) = watcher else {
            return;
        };
        if watcher
            .watch(&watched, notify::RecursiveMode::NonRecursive)
            .is_err()
        {
            return;
        }
        self.changes = Some(rx);
        self.watcher = Some(watcher);
        self.path = Some(watched);
    }

    /// Stops watching, for a folder the app has left.
    fn stop(&mut self) {
        self.changes = None;
        self.watcher = None;
        self.path = None;
    }

    /// Drains the watcher. True when the folder changed and the rows on screen are stale.
    pub(super) fn poll(&mut self) -> bool {
        let Some(changes) = &self.changes else {
            return false;
        };
        let mut fired = false;
        loop {
            match changes.try_recv() {
                Ok(()) => fired = true,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.stop();
                    break;
                }
            }
        }
        fired
    }
}
