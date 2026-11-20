#[cfg(not(target_arch = "wasm32"))]
use notify::Watcher;
use rune::Waker;
use std::path::Path;
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

#[derive(Default)]
pub(super) struct Watch {
    #[cfg(not(target_arch = "wasm32"))]
    wake: Option<Waker>,
    // The folder whose changes reach this app, once a worker has its watcher open.
    #[cfg(not(target_arch = "wasm32"))]
    path: Option<PathBuf>,
    // The folder the app is on, whose watcher a worker is still opening.
    #[cfg(not(target_arch = "wasm32"))]
    want: Option<PathBuf>,
    // A folder the platform would not watch, so the next frame does not ask again.
    #[cfg(not(target_arch = "wasm32"))]
    refused: Option<PathBuf>,
    #[cfg(not(target_arch = "wasm32"))]
    changes: Option<Receiver<()>>,
    // The worker's answer once its watcher is open: the folder it watches, and its changes.
    #[cfg(not(target_arch = "wasm32"))]
    opened: Option<Receiver<(PathBuf, Receiver<()>)>>,
    // Dropping this tells the worker to drop its watcher and stop.
    #[cfg(not(target_arch = "wasm32"))]
    stopped: Option<Sender<()>>,
}

#[cfg(not(target_arch = "wasm32"))]
impl Watch {
    pub(super) fn start(&mut self, path: &Path, wake: Waker) {
        self.wake = Some(wake);
        self.retarget(path);
    }

    /// Points the watch at `path`.
    ///
    /// Opening a folder on a drive that has spun down, or over a network, takes seconds, and the
    /// frame that switches folders must not wait for it. So a worker opens the watcher and the
    /// first frame that sees it lands it, exactly like a folder read that lands in `poll`.
    pub(super) fn retarget(&mut self, path: &Path) {
        let Some(wake) = self.wake.clone() else {
            return;
        };
        if self.want.as_deref() == Some(path)
            || (self.changes.is_some() && self.path.as_deref() == Some(path))
            || self.refused.as_deref() == Some(path)
        {
            return;
        }
        self.stop();
        self.want = Some(path.to_path_buf());
        let watched = path.to_path_buf();
        let seen = watched.clone();
        let (opened_tx, opened) = mpsc::channel();
        let (stopped_tx, stopped) = mpsc::channel();
        let spawned = std::thread::Builder::new()
            .name("sheath-watch".to_owned())
            .spawn(move || {
                let (changes_tx, changes) = mpsc::channel();
                let built =
                    notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
                        let Ok(event) = event else {
                            return;
                        };
                        if changed(&event, &seen) && changes_tx.send(()).is_ok() {
                            wake();
                        }
                    });
                let Ok(mut watcher) = built else {
                    return;
                };
                if watcher
                    .watch(&watched, notify::RecursiveMode::NonRecursive)
                    .is_err()
                {
                    return;
                }
                // Hand the changes over, then hold the watcher in this thread until the window
                // asks it to stop: the platform types are not all safe to move between threads.
                // A folder the platform declines leaves the sender to drop, which is how the
                // frame hears that this one cannot be watched.
                if opened_tx.send((watched, changes)).is_err() {
                    return;
                }
                let _ = stopped.recv();
            });
        if spawned.is_err() {
            self.want = None;
            self.refused = Some(path.to_path_buf());
            return;
        }
        self.opened = Some(opened);
        self.stopped = Some(stopped_tx);
    }

    fn stop(&mut self) {
        self.changes = None;
        self.path = None;
        self.want = None;
        self.opened = None;
        self.stopped = None;
    }

    /// Lands the watcher a worker opened, or gives up on a folder the platform will not watch.
    /// A watcher for a folder the window has already left is dropped instead.
    fn install(&mut self) {
        let Some(opened) = self.opened.take() else {
            return;
        };
        match opened.try_recv() {
            Ok((path, changes)) => {
                if self.want.as_deref() != Some(path.as_path()) {
                    return;
                }
                self.path = Some(path);
                self.changes = Some(changes);
            }
            Err(TryRecvError::Empty) => self.opened = Some(opened),
            Err(TryRecvError::Disconnected) => {
                self.refused = self.want.take();
                self.stopped = None;
            }
        }
    }

    /// How many watchers this app holds open. Retargeting must keep it at one, whatever the
    /// folder it visits.
    #[cfg(test)]
    pub(super) fn watchers(&self) -> usize {
        usize::from(self.changes.is_some())
    }

    /// Waits for the worker to open the watcher, for the tests that count the app's handles.
    #[cfg(test)]
    pub(super) fn settle(&mut self) {
        while self.opened.is_some() {
            let _ = self.poll();
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    pub(super) fn poll(&mut self) -> bool {
        self.install();
        let mut fired = false;
        let mut gone = false;
        if let Some(changes) = &self.changes {
            loop {
                match changes.try_recv() {
                    Ok(()) => fired = true,
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        gone = true;
                        break;
                    }
                }
            }
        }
        if gone {
            self.stop();
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

    #[cfg(test)]
    pub(super) fn settle(&mut self) {}

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
