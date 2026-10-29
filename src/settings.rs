//! The settings the app saves between launches, in their own `config.kdl`. Keybinds are not
//! here: they live in `keybinds.kdl`, so editing one never rewrites the other.

use std::fs;
use std::path::PathBuf;

use kdl::{KdlDocument, KdlEntry, KdlNode};

use crate::config;

/// The file name under the config folder. Only these settings live in it.
const FILE: &str = "config.kdl";

/// What the browser remembers about how it was left.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub show_hidden: bool,
    pub inspector: bool,
    /// The list keeps moving after the wheel stops.
    pub scroll_inertia: bool,
    /// The list can be pulled past its first and last row, then springs back.
    pub overscroll: bool,
    path: Option<PathBuf>,
    /// Why the file couldn't be read or written, for the settings panel.
    pub error: Option<String>,
}

impl Default for Settings {
    fn default() -> Settings {
        Settings {
            show_hidden: false,
            inspector: true,
            // Both are what the platform's own scrolling does, so they start on; the panel
            // can turn either off.
            scroll_inertia: true,
            overscroll: true,
            path: None,
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
                Ok(view) => {
                    settings.show_hidden = view.0;
                    settings.inspector = view.1;
                    settings.scroll_inertia = view.2;
                    settings.overscroll = view.3;
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

    /// Records the browser's current state and saves it. A failure keeps the value for this
    /// session and reports why the next launch won't have it.
    pub fn remember(
        &mut self,
        show_hidden: bool,
        inspector: bool,
        scroll_inertia: bool,
        overscroll: bool,
    ) {
        self.show_hidden = show_hidden;
        self.inspector = inspector;
        self.scroll_inertia = scroll_inertia;
        self.overscroll = overscroll;
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

/// The written form: a `settings` node of `name value` pairs.
fn render(settings: &Settings) -> String {
    let mut doc = KdlDocument::new();
    let mut node = KdlNode::new("settings");
    let mut children = KdlDocument::new();
    for (name, on) in [
        ("show-hidden", settings.show_hidden),
        ("inspector", settings.inspector),
        ("scroll-inertia", settings.scroll_inertia),
        ("overscroll", settings.overscroll),
    ] {
        let mut child = KdlNode::new(name);
        child.push(KdlEntry::new(on));
        children.nodes_mut().push(child);
    }
    node.set_children(children);
    doc.nodes_mut().push(node);
    // The file is meant to be read and edited by hand, so indent it.
    doc.autoformat();
    format!("// Sheath settings. Keybinds live in keybinds.kdl, not here.\n{doc}")
}

/// The settings the file names, in the order the file writes them; anything missing or
/// unreadable keeps its default, so an edited file degrades instead of breaking the app.
fn parse(text: &str) -> Result<(bool, bool, bool, bool), String> {
    let doc: KdlDocument = text.parse().map_err(|err: kdl::KdlError| err.to_string())?;
    let defaults = Settings::defaults();
    let (mut show_hidden, mut inspector) = (defaults.show_hidden, defaults.inspector);
    let (mut scroll_inertia, mut overscroll) = (defaults.scroll_inertia, defaults.overscroll);
    for node in doc.nodes() {
        for child in node.children().into_iter().flat_map(|kids| kids.nodes()) {
            let Some(on) = child.get(0).and_then(|value| value.as_bool()) else {
                continue;
            };
            match child.name().value() {
                "show-hidden" => show_hidden = on,
                "inspector" => inspector = on,
                "scroll-inertia" => scroll_inertia = on,
                "overscroll" => overscroll = on,
                _ => {}
            }
        }
    }
    Ok((show_hidden, inspector, scroll_inertia, overscroll))
}
