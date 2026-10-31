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

/// A file the app could not read: one line for the panels, and a report that has to stand on its
/// own away from the app, because Copy hands it to an editor or an assistant.
#[derive(Clone, Debug)]
pub struct Problem {
    /// One line, for the settings panels and the toast's body.
    pub detail: String,
    /// The file, the place, the line itself, and what the file is meant to hold.
    pub report: String,
}

/// What `name` is meant to hold, so a report read out of context still says what was being
/// parsed. The README has the long version.
fn schema(name: &str) -> &'static str {
    match name {
        CONFIG => {
            "top-level nodes: `settings { show-hidden inspector grid scroll-inertia overscroll }`,\n\
             `folder-rule \"name\" { match path=\"~/x/**\" layout grid sort name direction descending }`,\n\
             and `entry-rule \"name\" { match path=\"~/x/y\" hidden #true }`.\n\
             A rule takes `match` and `exclude` directives plus the properties its kind allows,\n\
             and every property also takes `inherit`. See README > Configuration."
        }
        KEYBINDS => {
            "one `keybinds { <command> \"<chord>\" }` node. A chord is ctrl, alt, shift or super\n\
             then a key name, and an empty chord clears the shortcut. See README > Configuration."
        }
        _ => "",
    }
}

/// A report for a rule the schema refuses. The parser names the rule's problem in a sentence but
/// has no line to point at, so the report says what the file holds instead.
pub fn schema_problem(name: &str, path: Option<&Path>, error: &str) -> String {
    let mut report = format!("{name} has a rule it cannot read.\n\n  error: {error}\n");
    if let Some(path) = path {
        report.push_str(&format!("  file:  {}\n", path.display()));
    }
    report.push_str(&format!("\nwhat this file holds:\n  {}\n", schema(name)));
    report
}

/// The whole report for a file that does not parse: the place, the line the parser stopped on,
/// and the shape the file was being read against.
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

/// A file that exists but could not be read at all: no place to point at.
fn read_problem(name: &str, path: &Path, err: &std::io::Error) -> Problem {
    Problem {
        detail: format!("Couldn’t read {name}: {err}"),
        report: format!(
            "{name} could not be read.\n\n  file:  {}\n  error: {err}\n",
            path.display()
        ),
    }
}

/// Where a character offset falls, one-based, as an editor counts lines and columns.
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

/// The line the parser stopped on between its neighbours, with a caret under the place it named.
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
    problem: Option<Problem>,
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

    /// The path of a config file, when there is a folder to keep it in.
    pub fn path(&self, name: &str) -> Option<PathBuf> {
        self.dir.as_ref().map(|dir| dir.join(name))
    }

    /// The parsed document, or `None` when the file on disk could not be read as KDL.
    pub fn doc(&self, name: &str) -> Option<&KdlDocument> {
        self.files.get(name).and_then(|file| file.doc.as_ref())
    }

    /// Why a file could not be read, in one line, for the panels that show it.
    pub fn error(&self, name: &str) -> Option<&str> {
        self.files
            .get(name)
            .and_then(|file| file.problem.as_ref())
            .map(|problem| problem.detail.as_str())
    }

    /// The whole problem with a file, for the toast's Copy button.
    pub fn problem(&self, name: &str) -> Option<&Problem> {
        self.files.get(name).and_then(|file| file.problem.as_ref())
    }

    /// Whether a file parses, so its nodes can be edited.
    pub fn writable(&self, name: &str) -> bool {
        self.files
            .get(name)
            .is_some_and(|file| file.doc.is_some() && file.problem.is_none())
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
            // A store with no folder cannot write anything. Reporting the save instead would
            // leave the panels saying a rule landed while nothing was written.
            let error = "Couldn’t find a folder to save the config in.".to_owned();
            self.write_error = Some(error.clone());
            return Err(error);
        };
        // The document kept here is the last one that parsed, not what is on disk now. Writing
        // it back would throw away whatever the user is in the middle of typing.
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
            file.problem = None;
        }
        Ok(())
    }

    /// Starts watching the config folder. The watcher tells the app a file changed, not what
    /// changed: the reply is a re-read, which keeps it indifferent to how an editor saves.
    pub fn watch(&mut self, wake: Waker) {
        let Some(dir) = self.dir.clone() else {
            return;
        };
        // A first run has no config folder yet, and there is nothing to watch until it exists.
        let _ = fs::create_dir_all(&dir);
        let (tx, rx) = mpsc::channel();
        let watcher = notify::recommended_watcher(move |event: notify::Result<notify::Event>| {
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
        let (doc, problem) = match fs::read_to_string(&path) {
            Ok(text) => match text.parse::<KdlDocument>() {
                Ok(doc) => (Some(doc), None),
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
