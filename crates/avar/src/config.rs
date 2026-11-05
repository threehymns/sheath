//! The two files Avar keeps: the keybind overrides and the settings. There is no settings system
//! behind them — a document is read once, one child is rewritten per change, and the rest of the
//! file survives.
use std::fs;
use std::path::{Path, PathBuf};

use kdl::{KdlDocument, KdlNode};

/// The file the editor writes, under the platform's config folder for this app.
pub const KEYBINDS: &str = "keybinds.kdl";
/// The file the settings window writes, beside it.
pub const SETTINGS: &str = "settings.kdl";
/// How a tab gives way to the next one: `cut`, `fade`, `slide-horizontal` or
/// `slide-vertical`.
pub const TAB_TRANSITION: &str = "tab-transition";
/// The spring that tab transition runs on, by its Rune motion token name.
pub const TAB_TIMING: &str = "tab-transition-timing";

/// What the settings file can name. A setting the file does not name keeps its default, and
/// saving the default takes the line out instead of writing it down.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Setting {
    /// The tabs as a bar across the top of the window, instead of the rail down its side.
    HorizontalTabs,
    /// A header above each pane, naming its folder and offering to split, zoom and close it.
    PaneHeaders,
    /// A slim bar above the panes, with the tab's name.
    TopBar,
}

impl Setting {
    pub const ALL: [Setting; 3] = [
        Setting::HorizontalTabs,
        Setting::PaneHeaders,
        Setting::TopBar,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Setting::HorizontalTabs => "horizontal-tabs",
            Setting::PaneHeaders => "pane-headers",
            Setting::TopBar => "top-bar",
        }
    }

    pub fn id(self) -> &'static str {
        self.name()
    }

    pub fn default_on(self) -> bool {
        match self {
            Setting::HorizontalTabs => false,
            Setting::PaneHeaders => false,
            Setting::TopBar => false,
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

    /// A choice the file names, such as `tab-transition "fade"`, or `None` when it leaves the
    /// setting at its default.
    pub fn choice(&self, name: &str) -> Option<String> {
        choice_in(self.doc()?, name)
    }

    /// Writes one choice, taking the line out when it is back at its default.
    pub fn set_choice(&mut self, name: &str, value: &str, default: &str) {
        self.edit(|doc| {
            if value == default {
                remove_setting(doc, name);
            } else {
                write_setting(doc, name, value);
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

/// The value of one string setting in a parsed document.
fn choice_in(doc: &KdlDocument, name: &str) -> Option<String> {
    doc.nodes()
        .iter()
        .find(|node| node.name().value() == "settings")?
        .children()?
        .nodes()
        .iter()
        .find(|node| node.name().value() == name)?
        .get(0)?
        .as_string()
        .map(str::to_owned)
}

/// Rewrites one child of the `settings` block, leaving the rest of the document as it was.
fn write_setting(doc: &mut KdlDocument, name: &str, value: &str) {
    let body = format!("{name} {value:?}");
    let Some(index) = doc
        .nodes()
        .iter()
        .position(|node| node.name().value() == "settings")
    else {
        let text = format!("\nsettings {{\n    {body}\n}}\n");
        if let Ok(parsed) = text.parse::<KdlDocument>()
            && let Some(node) = parsed.nodes().first()
        {
            doc.nodes_mut().push(node.clone());
        }
        return;
    };
    if doc.nodes()[index].children().is_none() {
        let text = format!("settings {{\n    {body}\n}}\n");
        if let Ok(parsed) = text.parse::<KdlDocument>() {
            doc.nodes_mut()[index] = parsed.nodes()[0].clone();
        }
        return;
    }
    let Ok(node) = body.parse::<KdlNode>() else {
        return;
    };
    let Some(children) = doc.nodes_mut()[index].children_mut().as_mut() else {
        return;
    };
    if let Some(slot) = children
        .nodes_mut()
        .iter_mut()
        .find(|child| child.name().value() == name)
    {
        *slot = node;
    } else {
        children.nodes_mut().push(node);
    }
}

fn remove_setting(doc: &mut KdlDocument, name: &str) {
    let Some(settings) = doc
        .nodes_mut()
        .iter_mut()
        .find(|node| node.name().value() == "settings")
    else {
        return;
    };
    if let Some(children) = settings.children_mut().as_mut() {
        children
            .nodes_mut()
            .retain(|child| child.name().value() != name);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_choice_writes_reads_and_removes_beside_the_switches() {
        let mut doc: KdlDocument = "settings {\n    // keep me\n    pane-headers #true\n}\n"
            .parse()
            .unwrap();
        write_setting(&mut doc, TAB_TRANSITION, "fade");
        write_setting(&mut doc, TAB_TIMING, "spring");
        assert_eq!(choice_in(&doc, TAB_TRANSITION).as_deref(), Some("fade"));
        assert_eq!(choice_in(&doc, TAB_TIMING).as_deref(), Some("spring"));
        assert_eq!(choice_in(&doc, "pane-headers"), None);

        remove_setting(&mut doc, TAB_TRANSITION);
        assert_eq!(choice_in(&doc, TAB_TRANSITION), None);
        assert_eq!(choice_in(&doc, TAB_TIMING).as_deref(), Some("spring"));
        assert!(doc.to_string().contains("pane-headers #true"));
    }

    #[test]
    fn a_choice_makes_the_settings_block_when_the_file_has_none() {
        let mut doc: KdlDocument = "keybinds {\n    new-tab \"ctrl+t\"\n}\n".parse().unwrap();
        write_setting(&mut doc, TAB_TRANSITION, "slide-vertical");
        assert_eq!(
            choice_in(&doc, TAB_TRANSITION).as_deref(),
            Some("slide-vertical")
        );
        assert!(doc.to_string().contains("keybinds"));
    }
}
