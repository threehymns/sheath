//! The settings the app saves between launches, in their own `config.kdl`. Keybinds are not
//! here: they live in `keybinds.kdl`, so editing one never rewrites the other.

use std::fs;
use std::path::PathBuf;

use kdl::{KdlDocument, KdlEntry, KdlNode};

use crate::config;

/// The file name under the config folder. Only these settings live in it.
const FILE: &str = "config.kdl";

/// A setting the file can name. One the file doesn't name keeps its default, and saving leaves
/// it out, so resetting a setting is removing its key rather than writing the default down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Setting {
    ShowHidden,
    Inspector,
    GridView,
    ScrollInertia,
    Overscroll,
}

impl Setting {
    pub const ALL: [Setting; 5] = [
        Setting::ShowHidden,
        Setting::Inspector,
        Setting::GridView,
        Setting::ScrollInertia,
        Setting::Overscroll,
    ];

    /// The key the file uses, and the stable name the element keys use.
    pub fn name(self) -> &'static str {
        match self {
            Setting::ShowHidden => "show-hidden",
            Setting::Inspector => "inspector",
            Setting::GridView => "grid",
            Setting::ScrollInertia => "scroll-inertia",
            Setting::Overscroll => "overscroll",
        }
    }

    /// The short name a setting's controls are keyed by.
    pub fn id(self) -> &'static str {
        match self {
            Setting::ShowHidden => "hidden",
            Setting::Inspector => "inspector",
            Setting::GridView => "grid",
            Setting::ScrollInertia => "inertia",
            Setting::Overscroll => "overscroll",
        }
    }

    /// What the app does when the file doesn't name it.
    pub fn default_on(self) -> bool {
        match self {
            Setting::ShowHidden | Setting::GridView => false,
            Setting::Inspector | Setting::ScrollInertia | Setting::Overscroll => true,
        }
    }

    fn index(self) -> usize {
        match self {
            Setting::ShowHidden => 0,
            Setting::Inspector => 1,
            Setting::GridView => 2,
            Setting::ScrollInertia => 3,
            Setting::Overscroll => 4,
        }
    }

    fn from_name(name: &str) -> Option<Setting> {
        Setting::ALL
            .into_iter()
            .find(|setting| setting.name() == name)
    }

    fn value(self, settings: &Settings) -> bool {
        match self {
            Setting::ShowHidden => settings.show_hidden,
            Setting::Inspector => settings.inspector,
            Setting::GridView => settings.grid,
            Setting::ScrollInertia => settings.scroll_inertia,
            Setting::Overscroll => settings.overscroll,
        }
    }

    fn set(self, settings: &mut Settings, on: bool) {
        match self {
            Setting::ShowHidden => settings.show_hidden = on,
            Setting::Inspector => settings.inspector = on,
            Setting::GridView => settings.grid = on,
            Setting::ScrollInertia => settings.scroll_inertia = on,
            Setting::Overscroll => settings.overscroll = on,
        }
    }
}

/// What the browser remembers about how it was left.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub show_hidden: bool,
    pub inspector: bool,
    /// The folder shows as a grid of tiles rather than a table of rows.
    pub grid: bool,
    /// The list keeps moving after the wheel stops.
    pub scroll_inertia: bool,
    /// The list can be pulled past its first and last row, then springs back.
    pub overscroll: bool,
    path: Option<PathBuf>,
    /// The settings the file names; the rest are defaults a save leaves out.
    present: [bool; 5],
    /// Why the file couldn't be read or written, for the settings panel.
    pub error: Option<String>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            show_hidden: false,
            inspector: true,
            grid: false,
            // Both are what the platform's own scrolling does, so they start on; the panel
            // can turn either off.
            scroll_inertia: true,
            overscroll: true,
            path: None,
            present: [false; 5],
            error: None,
        }
    }
}

impl Settings {
    /// The defaults, with nowhere to save.
    pub fn defaults() -> Settings {
        Settings::default()
    }

    /// The settings saved for this user, or the defaults when there's no readable file. A file
    /// with a problem leaves the defaults in place and reports it through [`Settings::error`].
    pub fn load() -> Settings {
        match config::path(FILE) {
            Some(path) => Settings::at(path),
            None => {
                let mut settings = Settings::defaults();
                settings.error = Some("Couldn’t find a folder to save settings in.".into());
                settings
            }
        }
    }

    /// The settings in `path`, for a config folder chosen by the caller.
    pub fn at(path: PathBuf) -> Settings {
        let mut settings = Settings {
            path: Some(path.clone()),
            ..Settings::defaults()
        };
        match fs::read_to_string(&path) {
            Ok(text) => match parse(&text) {
                Ok(mut parsed) => {
                    parsed.path = Some(path);
                    settings = parsed;
                }
                Err(err) => {
                    settings.error = Some(format!(
                        "Couldn’t read {}: {err}. Using the defaults.",
                        path.display()
                    ));
                }
            },
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => {
                settings.error = Some(format!(
                    "Couldn’t read {}: {err}. Using the defaults.",
                    path.display()
                ));
            }
        }
        settings
    }

    /// The file settings are saved in, when there is one.
    pub fn file() -> Option<PathBuf> {
        config::path(FILE)
    }

    /// Whether the file names this setting, so it can be reset.
    pub fn is_set(&self, setting: Setting) -> bool {
        self.present[setting.index()]
    }

    /// Records the browser's current state and saves it. `changed` is what the user just
    /// changed: only settings the file names are written, and a changed one is named from now
    /// on. A failure keeps the value for this session and reports why the next launch won't
    /// have it.
    pub fn remember(
        &mut self,
        changed: Setting,
        show_hidden: bool,
        inspector: bool,
        grid: bool,
        scroll_inertia: bool,
        overscroll: bool,
    ) {
        self.show_hidden = show_hidden;
        self.inspector = inspector;
        self.grid = grid;
        self.scroll_inertia = scroll_inertia;
        self.overscroll = overscroll;
        self.present[changed.index()] = true;
        self.save();
    }

    /// Takes a setting back to its default and removes its key, so the file only names what
    /// the user chose.
    pub fn reset(&mut self, setting: Setting) {
        setting.set(self, setting.default_on());
        self.present[setting.index()] = false;
        self.save();
    }

    fn save(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        if let Some(dir) = path.parent()
            && let Err(err) = fs::create_dir_all(dir)
        {
            self.error = Some(format!("Couldn’t make {}: {err}", dir.display()));
            return;
        }
        match fs::write(&path, render(self)) {
            Ok(()) => self.error = None,
            Err(err) => self.error = Some(format!("Couldn’t save {}: {err}", path.display())),
        }
    }
}

/// The written form: a `settings` node of `name value` pairs, for the settings the file names.
fn render(settings: &Settings) -> String {
    let mut doc = KdlDocument::new();
    let mut node = KdlNode::new("settings");
    let mut children = KdlDocument::new();
    for setting in Setting::ALL {
        if !settings.is_set(setting) {
            continue;
        }
        let mut child = KdlNode::new(setting.name());
        child.push(KdlEntry::new(setting.value(settings)));
        children.nodes_mut().push(child);
    }
    node.set_children(children);
    doc.nodes_mut().push(node);
    // The file is meant to be read and edited by hand, so indent it.
    doc.autoformat();
    format!("// Sheath settings. Keybinds live in keybinds.kdl, not here.\n{doc}")
}

/// The settings the file names; anything missing or unreadable keeps its default, so an
/// edited file degrades instead of breaking the app. A named setting is remembered even when
/// its value can't be read, so resetting it still takes the key out.
fn parse(text: &str) -> Result<Settings, String> {
    let doc: KdlDocument = text.parse().map_err(|err: kdl::KdlError| err.to_string())?;
    let mut settings = Settings::defaults();
    for node in doc.nodes() {
        for child in node.children().into_iter().flat_map(|kids| kids.nodes()) {
            let Some(setting) = Setting::from_name(child.name().value()) else {
                continue;
            };
            settings.present[setting.index()] = true;
            if let Some(on) = child.get(0).and_then(|value| value.as_bool()) {
                setting.set(&mut settings, on);
            }
        }
    }
    Ok(settings)
}
