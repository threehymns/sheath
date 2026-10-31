use notify::Watcher;
use rune::Waker;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};

#[derive(Default)]
pub(super) struct Watch {
    wake: Option<Waker>,
    path: Option<PathBuf>,
    changes: Option<Receiver<()>>,
    watcher: Option<notify::RecommendedWatcher>,
}

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

    fn stop(&mut self) {
        self.changes = None;
        self.watcher = None;
        self.path = None;
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
