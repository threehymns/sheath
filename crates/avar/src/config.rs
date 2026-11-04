//! The one file Avar keeps: the keybind overrides. There is no settings system behind it — the
//! document is read once, one child is rewritten per change, and the rest of the file survives.
use std::fs;
use std::path::{Path, PathBuf};

use kdl::KdlDocument;

/// The file the editor writes, under the platform's config folder for this app.
pub const KEYBINDS: &str = "keybinds.kdl";

pub struct Store {
    path: PathBuf,
    /// The file as last read or written. `None` until something edits it, so an app that never
    /// records a shortcut never creates the file.
    doc: Option<KdlDocument>,
    error: Option<String>,
}

impl Store {
    pub fn open() -> Store {
        let path = dirs::config_dir()
            .map(|dir| dir.join("avar").join(KEYBINDS))
            .unwrap_or_else(|| PathBuf::from(KEYBINDS));
        let mut store = Store {
            path,
            doc: None,
            error: None,
        };
        store.read();
        store
    }

    fn read(&mut self) {
        match fs::read_to_string(&self.path) {
            Ok(text) => match text.parse::<KdlDocument>() {
                Ok(doc) => self.doc = Some(doc),
                Err(error) => self.error = Some(error.to_string()),
            },
            // No file yet is the normal first run, not an error to show.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => self.error = Some(error.to_string()),
        }
    }

    /// The document, or `None` when the file does not exist yet.
    pub fn doc(&self) -> Option<&KdlDocument> {
        self.doc.as_ref()
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    /// Changes the document and writes it back through a temporary file, keeping the last
    /// failure for the editor's footer.
    pub fn edit(&mut self, change: impl FnOnce(&mut KdlDocument)) {
        let mut doc = self.doc.take().unwrap_or_default();
        change(&mut doc);
        self.doc = Some(doc);
        match self.write() {
            Ok(()) => self.error = None,
            Err(error) => self.error = Some(error),
        }
    }

    fn write(&self) -> Result<(), String> {
        if let Some(dir) = self.path.parent()
            && !dir.as_os_str().is_empty()
        {
            fs::create_dir_all(dir).map_err(|error| error.to_string())?;
        }
        let text = self
            .doc
            .as_ref()
            .map(ToString::to_string)
            .unwrap_or_default();
        let temporary = self.path.with_extension("kdl.tmp");
        fs::write(&temporary, text).map_err(|error| error.to_string())?;
        fs::rename(&temporary, &self.path).map_err(|error| error.to_string())
    }
}
