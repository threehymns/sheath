//! The config folder. Each file is parsed once and edited in place, so a save changes the nodes
//! it was asked to change and nothing else: comments, blank lines, and unknown nodes survive.
//! Every write replaces the file atomically, and a file that fails to parse is never written
//! back.

use kdl::KdlDocument;
use notify::Watcher;
use rune::Waker;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::time::SystemTime;

/// The settings and rules.
pub const CONFIG: &str = "config.kdl";

/// The keyboard shortcuts, still in a file of their own.
pub const KEYBINDS: &str = "keybinds.kdl";

/// The folder the config files go in: `SHEATH_CONFIG_DIR` when set, else the platform's config
/// folder.
pub fn dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("SHEATH_CONFIG_DIR") {
        return Some(PathBuf::from(dir));
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join("Library/Application Support/Sheath"))
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join("Sheath"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .map(|dir| dir.join("sheath"))
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    {
        None
    }
}

/// The folder remembered per-folder state goes in: `SHEATH_STATE_DIR` when set, else the
/// platform's state folder. It is machine-owned, so it stays out of the config folder.
pub fn state_dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("SHEATH_STATE_DIR") {
        return Some(PathBuf::from(dir));
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join("Library/Application Support/Sheath"))
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("LOCALAPPDATA").map(|dir| PathBuf::from(dir).join("Sheath"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_STATE_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(".local/state"))
            })
            .map(|dir| dir.join("sheath"))
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    {
        None
    }
}

/// The path of `file` in the config folder, when there is one.
pub fn path(file: &str) -> Option<PathBuf> {
    dir().map(|dir| dir.join(file))
}

/// Replaces `path` in one step: the text goes to a sibling temporary file first, so a crash or a
/// full disk leaves the old file rather than half of a new one.
pub fn write_atomic(path: &Path, text: &str) -> Result<(), String> {
    let Some(dir) = path.parent() else {
        return Err(format!("{} has no folder", path.display()));
    };
    fs::create_dir_all(dir).map_err(|err| format!("Couldn’t make {}: {err}", dir.display()))?;
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "config".to_owned());
    let temp = dir.join(format!(".{name}.tmp"));
    fs::write(&temp, text).map_err(|err| format!("Couldn’t write {}: {err}", temp.display()))?;
    fs::rename(&temp, path).map_err(|err| {
        let _ = fs::remove_file(&temp);
        format!("Couldn’t replace {}: {err}", path.display())
    })
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Stamp {
    modified: Option<SystemTime>,
    len: u64,
}

fn stamp_of(path: &Path) -> Option<Stamp> {
    let meta = fs::metadata(path).ok()?;
    Some(Stamp {
        modified: meta.modified().ok(),
        len: meta.len(),
    })
}

struct Loaded {
    doc: Option<KdlDocument>,
    error: Option<String>,
    stamp: Option<Stamp>,
}

/// The config folder: one parsed document per file, and one watcher telling the app when
/// something else wrote to it.
pub struct Store {
    dir: Option<PathBuf>,
    files: BTreeMap<&'static str, Loaded>,
    changes: Option<Receiver<()>>,
    _watcher: Option<notify::RecommendedWatcher>,
    /// Why the last save could not be written, for the panels that show it.
    pub write_error: Option<String>,
}

impl Store {
    /// The user's config folder, or a store with nowhere to save when there is none.
    pub fn open() -> Store {
        match dir() {
            Some(dir) => Store::at(dir),
            None => Store::none(),
        }
    }

    /// A store with nowhere to save: the defaults apply and saving is a no-op.
    pub fn none() -> Store {
        let mut files = BTreeMap::new();
        for name in [CONFIG, KEYBINDS] {
            files.insert(
                name,
                Loaded {
                    doc: None,
                    error: None,
                    stamp: None,
                },
            );
        }
        Store {
            dir: None,
            files,
            changes: None,
            _watcher: None,
            write_error: None,
        }
    }

    /// The files in `dir`, for a config folder chosen by the caller.
    pub fn at(dir: PathBuf) -> Store {
        let mut store = Store::none();
        store.dir = Some(dir);
        for name in [CONFIG, KEYBINDS] {
            store.read(name);
        }
        store
    }

    pub fn dir(&self) -> Option<&Path> {
        self.dir.as_deref()
    }

    /// The path of a config file, when there is a folder to keep it in.
    pub fn path(&self, name: &str) -> Option<PathBuf> {
        self.dir.as_ref().map(|dir| dir.join(name))
    }

    /// The parsed document, or `None` when the file on disk could not be read as KDL.
    pub fn doc(&self, name: &str) -> Option<&KdlDocument> {
        self.files.get(name).and_then(|file| file.doc.as_ref())
    }

    /// Why a file could not be read, for the panels that show it.
    pub fn error(&self, name: &str) -> Option<&str> {
        self.files.get(name).and_then(|file| file.error.as_deref())
    }

    /// Whether a file parses, so its nodes can be edited.
    pub fn writable(&self, name: &str) -> bool {
        self.files
            .get(name)
            .is_some_and(|file| file.doc.is_some())
    }

    /// Edits the document and writes it back. The edit lands on a copy, so a failed write leaves
    /// the document the app holds exactly as it was. A file that failed to parse is never
    /// written: the caller keeps showing the error instead.
    pub fn edit(
        &mut self,
        name: &'static str,
        edit: impl FnOnce(&mut KdlDocument),
    ) -> Result<(), String> {
        let Some(path) = self.path(name) else {
            return Ok(());
        };
        let Some(file) = self.files.get(name) else {
            return Ok(());
        };
        let Some(doc) = file.doc.clone() else {
            return Err(format!(
                "{} does not parse, so nothing is written to it",
                path.display()
            ));
        };
        let mut doc = doc;
        edit(&mut doc);
        let text = doc.to_string();
        if let Err(err) = write_atomic(&path, &text) {
            self.write_error = Some(err.clone());
            return Err(err);
        }
        self.write_error = None;
        if let Some(file) = self.files.get_mut(name) {
            file.doc = Some(doc);
            file.stamp = stamp_of(&path);
            file.error = None;
        }
        Ok(())
    }

    /// Starts watching the config folder. The watcher tells the app a file changed, not what
    /// changed: the reply is a re-read, which keeps it indifferent to how an editor saves.
    pub fn watch(&mut self, wake: Waker) {
        let Some(dir) = self.dir.clone() else {
            return;
        };
        let (tx, rx) = mpsc::channel();
        let watcher = notify::recommended_watcher(
            move |event: notify::Result<notify::Event>| {
                let Ok(event) = event else {
                    return;
                };
                let ours = event.paths.iter().any(|path| {
                    path.file_name()
                        .is_some_and(|name| name == CONFIG || name == KEYBINDS)
                });
                if ours && tx.send(()).is_ok() {
                    wake();
                }
            },
        );
        let Ok(mut watcher) = watcher else {
            return;
        };
        if watcher
            .watch(&dir, notify::RecursiveMode::NonRecursive)
            .is_err()
        {
            return;
        }
        self.changes = Some(rx);
        self._watcher = Some(watcher);
    }

    /// Re-reads a file whose stamp moved. True when its contents changed.
    fn reload(&mut self, name: &'static str) -> bool {
        let Some(path) = self.path(name) else {
            return false;
        };
        let stamp = stamp_of(&path);
        if self.files.get(name).is_some_and(|file| file.stamp == stamp) {
            return false;
        }
        self.read(name);
        true
    }

    /// Reads and parses one file, keeping the old document when the new text does not parse.
    fn read(&mut self, name: &'static str) {
        let Some(path) = self.path(name) else {
            return;
        };
        let stamp = stamp_of(&path);
        let (doc, error) = match fs::read_to_string(&path) {
            Ok(text) => match text.parse::<KdlDocument>() {
                Ok(doc) => (Some(doc), None),
                Err(err) => (
                    self.files.get(name).and_then(|file| file.doc.clone()),
                    Some(format!("Couldn’t read {}: {err}", path.display())),
                ),
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => (Some(KdlDocument::new()), None),
            Err(err) => (
                self.files.get(name).and_then(|file| file.doc.clone()),
                Some(format!("Couldn’t read {}: {err}", path.display())),
            ),
        };
        self.files.insert(name, Loaded { doc, error, stamp });
        // A file that does not parse has its stamp recorded too, so every later poll does not
        // re-read it until it changes again.
    }

    /// Drains the watcher and re-reads whatever changed. True when the app should apply the new
    /// documents.
    pub fn poll(&mut self) -> bool {
        let Some(changes) = &self.changes else {
            return false;
        };
        let mut fired = false;
        loop {
            match changes.try_recv() {
                Ok(()) => fired = true,
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    self.changes = None;
                    break;
                }
            }
        }
        if !fired {
            return false;
        }
        let mut changed = false;
        for name in [CONFIG, KEYBINDS] {
            if self.reload(name) {
                changed = true;
            }
        }
        changed
    }
}
