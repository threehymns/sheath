//! The two files Avar keeps: the keybind overrides and the settings. There is no settings system
//! behind them — a document is read once, one child is rewritten per change, and the rest of the
//! file survives.
use std::fs;
use std::path::{Path, PathBuf};

use kdl::KdlDocument;

/// The file the editor writes, under the platform's config folder for this app.
pub const KEYBINDS: &str = "keybinds.kdl";
/// The file the settings window writes, beside it.
pub const SETTINGS: &str = "settings.kdl";

/// What the settings file can name. A setting the file does not name keeps its default, and
/// saving the default takes the line out instead of writing it down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Setting {
    /// The tabs as a bar across the top of the window, instead of the rail down its side.
    HorizontalTabs,
}

impl Setting {
    pub const ALL: [Setting; 1] = [Setting::HorizontalTabs];

    pub fn name(self) -> &'static str {
        match self {
            Setting::HorizontalTabs => "horizontal-tabs",
        }
    }

    pub fn id(self) -> &'static str {
        self.name()
    }

    pub fn default_on(self) -> bool {
        match self {
            Setting::HorizontalTabs => false,
        }
    }
}

impl actions::Setting for Setting {
    const ALL: &'static [Self] = &Setting::ALL;

    fn from_name(name: &str) -> Option<Setting> {
        Setting::ALL
            .into_iter()
            .find(|setting| setting.name() == name)
    }

    fn name(self) -> &'static str {
        Setting::name(self)
    }

    fn id(self) -> &'static str {
        Setting::id(self)
    }
}

pub struct Store {
    path: PathBuf,
    /// The file as last read or written. `None` until something edits it, so an app that never
    /// records a shortcut never creates the file.
    doc: Option<KdlDocument>,
    error: Option<String>,
}

impl Store {
    /// Opens one file of the app's config folder, which reads as absent the first time.
    pub fn open(file: &str) -> Store {
        let path = dirs::config_dir()
            .map(|dir| dir.join("avar").join(file))
            .unwrap_or_else(|| PathBuf::from(file));
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

    /// A setting the file names, or `None` for one it leaves at its default.
    pub fn setting(&self, setting: Setting) -> Option<bool> {
        actions_kdl::settings::read::<Setting>(self.doc())
            .into_iter()
            .find(|(saved, _)| *saved == setting)
            .map(|(_, on)| on)
    }

    /// Writes one setting, taking the line out when it is back at its default.
    pub fn set_setting(&mut self, setting: Setting, on: bool) {
        self.edit(|doc| {
            if on == setting.default_on() {
                actions_kdl::settings::remove(doc, setting);
            } else {
                actions_kdl::settings::write(doc, setting, on);
            }
        });
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
