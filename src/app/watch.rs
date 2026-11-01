#[cfg(not(target_arch = "wasm32"))]
use notify::Watcher;
use rune::Waker;
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::{self, Receiver, TryRecvError};

#[derive(Default)]
pub(super) struct Watch {
    #[cfg(not(target_arch = "wasm32"))]
    wake: Option<Waker>,
    #[cfg(not(target_arch = "wasm32"))]
    path: Option<PathBuf>,
    #[cfg(not(target_arch = "wasm32"))]
    changes: Option<Receiver<()>>,
    #[cfg(not(target_arch = "wasm32"))]
    watcher: Option<notify::RecommendedWatcher>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Watch {
    pub(super) fn start(&mut self, path: &Path, wake: Waker) {
        self.wake = Some(wake);
        self.retarget(path);
    }

    pub(super) fn retarget(&mut self, path: &Path) {
        let Some(wake) = self.wake.clone() else {
            return;
        };
        if self.changes.is_some() && self.path.as_deref() == Some(path) {
            return;
        }
        self.stop();
        let watched = path.to_path_buf();
        let seen = watched.clone();
        let (tx, rx) = mpsc::channel();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
            let Ok(event) = event else {
                return;
            };
            if changed(&event, &seen) && tx.send(()).is_ok() {
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

    fn stop(&mut self) {
        self.changes = None;
        self.watcher = None;
        self.path = None;
    }

    /// How many watchers this app holds open. Retargeting must keep it at one, whatever the
    /// folder it visits.
    #[cfg(test)]
    pub(super) fn watchers(&self) -> usize {
        usize::from(self.watcher.is_some())
    }

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

/// A browser tab has no folder to watch; its tree only changes through this app, which reloads
/// the listing itself after every write.
#[cfg(target_arch = "wasm32")]
impl Watch {
    pub(super) fn start(&mut self, _path: &Path, _wake: Waker) {}

    pub(super) fn retarget(&mut self, _path: &Path) {}

    #[cfg(test)]
    pub(super) fn watchers(&self) -> usize {
        0
    }

    pub(super) fn poll(&mut self) -> bool {
        false
    }
}

/// Whether an event means the folder's listing or one of its files changed. Opens, reads and
/// closes are access events: listing the folder and previewing a picture both open files, so
/// treating those as changes would reload the folder once per preview, in a loop.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn changed(event: &notify::Event, dir: &Path) -> bool {
    !matches!(event.kind, notify::EventKind::Access(_))
        && event.paths.iter().any(|path| path.parent() == Some(dir))
}
