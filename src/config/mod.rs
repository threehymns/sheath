mod disk;

use crate::vfs as fs;
use disk::{Readback, Stamp, stamp_of};
use kdl::KdlDocument;
#[cfg(not(target_arch = "wasm32"))]
use notify::Watcher;
use rune::{Latest, Waker};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// The config folder's watcher. A browser tab has no folder to watch, so the store reads its
/// defaults once and never wakes for a change.
#[cfg(not(target_arch = "wasm32"))]
type WatchHandle = notify::RecommendedWatcher;
#[cfg(target_arch = "wasm32")]
type WatchHandle = ();
#[cfg(not(target_arch = "wasm32"))]
use std::sync::mpsc;
use std::sync::mpsc::{Receiver, TryRecvError};

pub const CONFIG: &str = "config.kdl";

pub const KEYBINDS: &str = "keybinds.kdl";

pub fn dir() -> Option<PathBuf> {
    #[cfg(target_arch = "wasm32")]
    {
        // A browser tab's config lives in OPFS, read and written through the in-memory tree.
        Some(crate::vfs::config_dir())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
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
                .or_else(|| {
                    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config"))
                })
                .map(|dir| dir.join("sheath"))
        }
        #[cfg(not(any(unix, target_os = "windows")))]
        {
            None
        }
    }
}

pub fn state_dir() -> Option<PathBuf> {
    #[cfg(target_arch = "wasm32")]
    {
        Some(crate::vfs::state_dir())
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        if let Some(dir) = std::env::var_os("SHEATH_STATE_DIR") {
            return Some(PathBuf::from(dir));
        }
        #[cfg(target_os = "macos")]
        {
            std::env::var_os("HOME")
                .map(|home| PathBuf::from(home).join("Library/Application Support/Sheath/State"))
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
                    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".local/state"))
                })
                .map(|dir| dir.join("sheath"))
        }
        #[cfg(not(any(unix, target_os = "windows")))]
        {
            None
        }
    }
}

/// Writes a sibling temporary file, then renames it, so a failed write leaves the old file whole.
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

/// A machine-owned file in the state directory: where it is, and the last problem reading or
/// writing it. Losing a state file is never fatal, so a missing one reads as `None` and the
/// caller's default stands in.
#[derive(Default)]
pub struct State {
    path: Option<PathBuf>,
    error: Option<String>,
}

impl State {
    pub fn load(name: &str) -> State {
        State {
            path: state_dir().map(|dir| dir.join(name)),
            error: None,
        }
    }

    #[cfg(test)]
    pub fn at(path: PathBuf) -> State {
        State {
            path: Some(path),
            error: None,
        }
    }

    /// The file's text. A missing file is not a problem; a read that fails for another reason is
    /// kept for the caller to report.
    pub fn read(&mut self) -> Option<String> {
        let path = self.path.as_ref()?;
        match crate::vfs::read_to_string(path) {
            Ok(text) => Some(text),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
            Err(err) => {
                self.error = Some(format!("Couldn’t read {}: {err}", path.display()));
                None
            }
        }
    }

    pub fn write(&mut self, text: &str) {
        let Some(path) = self.path.clone() else {
            return;
        };
        self.error = write_atomic(&path, text).err();
    }

    /// Records a problem the caller found in the file's text, naming the file.
    pub fn fail(&mut self, problem: &str) {
        self.error = Some(match &self.path {
            Some(path) => format!("Couldn’t read {}: {problem}", path.display()),
            None => problem.to_owned(),
        });
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }
}

#[derive(Clone, Debug)]
pub struct Problem {
    pub detail: String,
    pub report: String,
}

fn schema(name: &str) -> &'static str {
    match name {
        CONFIG => {
            "top-level nodes: `settings { show-hidden inspector grid zebra-stripes folders-first confirm-trash confirm-delete show-kind show-size show-modified scroll-inertia overscroll system-icons }`,\n\
             `folder-rule \"name\" { match path=\"~/x/**\" layout grid sort name direction descending }`,\n\
             and `entry-rule \"name\" { match path=\"~/x/y\" hidden #true }`.\n\
             A rule takes `match` and `exclude` directives plus the properties its kind allows,\n\
             and every property also takes `inherit`. README: https://raw.githubusercontent.com/threehymns/sheath/main/README.md (section: Configuration)."
        }
        KEYBINDS => {
            "one `keybinds { <command> \"<chord>\" }` node. A chord is ctrl, alt, shift or super\n\
             then a key name, and an empty chord clears the shortcut. README: https://raw.githubusercontent.com/threehymns/sheath/main/README.md (section: Configuration)."
        }
        _ => "",
    }
}

pub fn schema_problem(name: &str, path: Option<&Path>, error: &str) -> String {
    let mut report = format!("{name} has a rule it cannot read.\n\n  error: {error}\n");
    if let Some(path) = path {
        report.push_str(&format!("  file:  {}\n", path.display()));
    }
    report.push_str(&format!("\nwhat this file holds:\n  {}\n", schema(name)));
    report
}

fn syntax_problem(name: &str, path: &Path, err: &kdl::KdlError) -> Problem {
    let Some(diagnostic) = err.diagnostics.first() else {
        return Problem {
            detail: format!("{name} does not parse"),
            report: format!(
                "{name} does not parse.\n\n  file: {}\n\nwhat this file holds:\n  {}\n",
                path.display(),
                schema(name)
            ),
        };
    };
    let (line, column) = line_column(diagnostic.input.as_str(), diagnostic.span.offset());
    let message = diagnostic
        .message
        .clone()
        .or_else(|| diagnostic.label.clone())
        .unwrap_or_else(|| "the document does not parse".to_owned());
    let mut report = format!(
        "{name} does not parse.\n\n  file:  {}\n  place: line {line}, column {column}\n  error: {message}\n\n{}",
        path.display(),
        excerpt(
            diagnostic.input.as_str(),
            line,
            column,
            diagnostic.span.len()
        ),
    );
    if let Some(help) = &diagnostic.help {
        report.push_str(&format!("  help:  {help}\n"));
    }
    report.push_str(&format!("\nwhat this file holds:\n  {}\n", schema(name)));
    Problem {
        detail: format!("line {line}, column {column}: {message}"),
        report,
    }
}

fn read_problem(name: &str, path: &Path, err: &std::io::Error) -> Problem {
    Problem {
        detail: format!("Couldn’t read {name}: {err}"),
        report: format!(
            "{name} could not be read.\n\n  file:  {}\n  error: {err}\n",
            path.display()
        ),
    }
}

fn line_column(input: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    for (seen, ch) in input.chars().enumerate() {
        if seen >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

fn excerpt(input: &str, line: usize, column: usize, length: usize) -> String {
    let lines: Vec<&str> = input.lines().collect();
    let mut out = String::new();
    for at in line.saturating_sub(1).max(1)..=(line + 1).min(lines.len()) {
        let Some(text) = lines.get(at - 1) else {
            continue;
        };
        out.push_str(&format!("  {at:>4} | {text}\n"));
        if at == line {
            let indent = column.saturating_sub(1);
            let remaining = text.chars().count().saturating_sub(indent);
            let carets = length.max(1).min(remaining.max(1));
            out.push_str(&format!(
                "       | {}{}\n",
                " ".repeat(indent),
                "^".repeat(carets)
            ));
        }
    }
    out
}

struct Loaded {
    doc: Option<KdlDocument>,
    problem: Option<Problem>,
    stamp: Option<Stamp>,
}

pub struct Store {
    dir: Option<PathBuf>,
    files: BTreeMap<&'static str, Loaded>,
    changes: Option<Receiver<()>>,
    _watcher: Option<WatchHandle>,
    /// Reads run on a worker, so `poll` never touches the disk; the newest request wins.
    reads: Latest<Vec<Readback>>,
    /// Counts writes, so a read that started before one can't put the text it replaced back.
    writes: u64,
    /// The `writes` count when the pending read was asked for.
    read_writes: u64,
    pub write_error: Option<String>,
}

impl Store {
    pub fn open() -> Store {
        match dir() {
            Some(dir) => Store::at(dir),
            None => Store::none(),
        }
    }

    pub fn none() -> Store {
        let mut files = BTreeMap::new();
        for name in [CONFIG, KEYBINDS] {
            files.insert(
                name,
                Loaded {
                    doc: None,
                    problem: None,
                    stamp: None,
                },
            );
        }
        Store {
            dir: None,
            files,
            changes: None,
            _watcher: None,
            reads: Latest::default(),
            writes: 0,
            read_writes: 0,
            write_error: None,
        }
    }

    pub fn at(dir: PathBuf) -> Store {
        let mut store = Store::none();
        store.dir = Some(dir);
        for name in [CONFIG, KEYBINDS] {
            store.read(name);
        }
        store
    }

    pub fn path(&self, name: &str) -> Option<PathBuf> {
        self.dir.as_ref().map(|dir| dir.join(name))
    }

    pub fn doc(&self, name: &str) -> Option<&KdlDocument> {
        self.files.get(name).and_then(|file| file.doc.as_ref())
    }

    pub fn error(&self, name: &str) -> Option<&str> {
        self.files
            .get(name)
            .and_then(|file| file.problem.as_ref())
            .map(|problem| problem.detail.as_str())
    }

    pub fn problem(&self, name: &str) -> Option<&Problem> {
        self.files.get(name).and_then(|file| file.problem.as_ref())
    }

    pub fn writable(&self, name: &str) -> bool {
        self.files
            .get(name)
            .is_some_and(|file| file.doc.is_some() && file.problem.is_none())
    }

    pub fn edit(
        &mut self,
        name: &'static str,
        edit: impl FnOnce(&mut KdlDocument),
    ) -> Result<(), String> {
        let Some(path) = self.path(name) else {
            let error = "Couldn’t find a folder to save the config in.".to_owned();
            self.write_error = Some(error.clone());
            return Err(error);
        };
        if !self.writable(name) {
            return Err(format!(
                "{} does not parse, so nothing is written to it",
                path.display()
            ));
        }
        let Some(doc) = self.files.get(name).and_then(|file| file.doc.clone()) else {
            let error = format!(
                "{} has not been read, so nothing is written to it",
                path.display()
            );
            self.write_error = Some(error.clone());
            return Err(error);
        };
        // The edit lands on a copy, so a failed write leaves the stored document as it was.
        let mut doc = doc;
        edit(&mut doc);
        let text = doc.to_string();
        if let Err(err) = write_atomic(&path, &text) {
            self.write_error = Some(err.clone());
            return Err(err);
        }
        self.write_error = None;
        // A read that started before this write must not put the text it replaced back.
        self.writes += 1;
        if let Some(file) = self.files.get_mut(name) {
            file.doc = Some(doc);
            file.stamp = stamp_of(&path);
            file.problem = None;
        }
        Ok(())
    }

    /// Re-reads a file when it changes; the watcher reports which file changed, not what changed.
    pub fn watch(&mut self, wake: Waker) {
        #[cfg(target_arch = "wasm32")]
        let _ = wake;
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(dir) = self.dir.clone() else {
                return;
            };
            let _ = fs::create_dir_all(&dir);
            self.reads.set_waker(wake.clone());
            let (tx, rx) = mpsc::channel();
            let watcher =
                notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
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
                });
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
    }

    /// Asks a worker to read both files, so a change the watcher reports lands in a later poll.
    fn ask(&mut self) {
        let wanted: Vec<(&'static str, PathBuf)> = [CONFIG, KEYBINDS]
            .into_iter()
            .filter_map(|name| self.path(name).map(|path| (name, path)))
            .collect();
        if wanted.is_empty() {
            return;
        }
        self.read_writes = self.writes;
        self.reads.request((), move |()| disk::read_all(wanted));
    }

    /// Installs what a worker read, skipping a file that is unchanged since the last read.
    fn land(&mut self, reads: Vec<Readback>) -> bool {
        let mut changed = false;
        for back in reads {
            if self
                .files
                .get(back.name)
                .is_some_and(|file| file.stamp == back.stamp)
            {
                continue;
            }
            self.install(back);
            changed = true;
        }
        changed
    }

    /// The startup read: `Store::at` may block, since no window is waiting on it yet.
    fn read(&mut self, name: &'static str) {
        if let Some(path) = self.path(name) {
            self.install(disk::read(name, path));
        }
    }

    fn install(&mut self, back: Readback) {
        let Readback {
            name,
            path,
            stamp,
            text,
        } = back;
        let (doc, problem) = match text {
            Ok(text) => match text.parse::<KdlDocument>() {
                Ok(doc) => (Some(doc), None),
                // Keep the last document that parsed; writing it back would discard the user's
                // edits in progress.
                Err(err) => (
                    self.files.get(name).and_then(|file| file.doc.clone()),
                    Some(syntax_problem(name, &path, &err)),
                ),
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                (Some(KdlDocument::new()), None)
            }
            Err(err) => (
                self.files.get(name).and_then(|file| file.doc.clone()),
                Some(read_problem(name, &path, &err)),
            ),
        };
        self.files.insert(
            name,
            Loaded {
                doc,
                problem,
                stamp,
            },
        );
    }

    pub fn poll(&mut self) -> bool {
        let mut changed = false;
        if let Some(reads) = self.reads.take() {
            // A write since the request already put the newer text in place.
            if self.read_writes == self.writes {
                changed = self.land(reads);
            }
        }
        let mut fired = false;
        let mut lost = false;
        if let Some(changes) = &self.changes {
            loop {
                match changes.try_recv() {
                    Ok(()) => fired = true,
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        lost = true;
                        break;
                    }
                }
            }
        }
        if lost {
            self.changes = None;
        }
        if fired {
            // The read lands in a later poll, woken by its answer; this one reports no change.
            self.ask();
        }
        changed
    }
}
