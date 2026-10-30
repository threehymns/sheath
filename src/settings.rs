//! The settings saved under the `settings` node of `config.kdl`. Keybinds live in their own
//! file, so editing one never rewrites the other. Saves change the one node they are about and
//! leave every other byte of the file alone.

use kdl::{KdlDocument, KdlNode};

/// A setting the file can name. One the file doesn't name keeps its default, and saving removes
/// its key, so resetting a setting is deleting a line rather than writing the default down.
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

/// What the browser remembers about how it was left, apart from the folders that remember their
/// own presentation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub show_hidden: bool,
    pub inspector: bool,
    /// The folder shows as a grid of tiles rather than a table of rows when no rule and no
    /// remembered state says otherwise.
    pub grid: bool,
    /// The list keeps moving after the wheel stops.
    pub scroll_inertia: bool,
    /// The list can be pulled past its first and last row, then springs back.
    pub overscroll: bool,
    /// The settings the file names; the rest are defaults a save leaves out.
    present: [bool; 5],
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            show_hidden: false,
            inspector: true,
            grid: false,
            // Both are what the platform's own scrolling does, so they start on; the panel
            // can turn either off.
            scroll_inertia: true,
            overscroll: true,
            present: [false; 5],
        }
    }
}

impl Settings {
    /// The defaults, for a store with no file.
    pub fn defaults() -> Settings {
        Settings::default()
    }

    /// The settings the `settings` node names. A key the file leaves out, or cannot be read as
    /// a boolean, keeps its default, so an edited file degrades instead of breaking the app.
    pub fn from_doc(doc: Option<&KdlDocument>) -> Settings {
        let mut settings = Settings::defaults();
        let Some(doc) = doc else {
            return settings;
        };
        for node in doc.nodes() {
            let children = node
                .children()
                .into_iter()
                .flat_map(|kids| kids.nodes())
                .filter(|_| node.name().value() == "settings");
            for child in children {
                let Some(setting) = Setting::from_name(child.name().value()) else {
                    continue;
                };
                settings.present[setting.index()] = true;
                if let Some(on) = child.get(0).and_then(|entry| entry.as_bool()) {
                    setting.set(&mut settings, on);
                }
            }
        }
        settings
    }

    /// Whether the file names this setting, so it can be reset.
    pub fn is_set(&self, setting: Setting) -> bool {
        self.present[setting.index()]
    }

    /// Whether the file names any setting, for the panel's reset affordances.
    pub fn names_anything(&self) -> bool {
        self.present.iter().any(|present| *present)
    }

    /// Writes `setting` as a child of the `settings` node, leaving the rest of the document
    /// exactly as it was: the node keeps its own whitespace and the comments above it.
    pub fn write(&self, doc: &mut KdlDocument, setting: Setting) {
        let body = format!(
            "{} {}",
            setting.name(),
            if setting.value(self) { "#true" } else { "#false" }
        );
        write_child(doc, "settings", setting.name(), &body);
    }

    /// Records the browser's current switches, naming `changed` in the file from now on.
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
    }

    /// Takes a setting back to its default, so the file no longer names it.
    pub fn reset(&mut self, setting: Setting) {
        setting.set(self, setting.default_on());
        self.present[setting.index()] = false;
    }

    /// Takes a setting's key out, so the default applies again.
    pub fn clear(doc: &mut KdlDocument, setting: Setting) {
        remove_child(doc, "settings", setting.name());
    }
}

/// The `settings` node's position, if the document has one.
fn parent_index(doc: &KdlDocument, parent: &str) -> Option<usize> {
    doc.nodes()
        .iter()
        .position(|node| node.name().value() == parent)
}

/// Writes `name value` under `parent`, replacing the node that names it and keeping that node's
/// leading whitespace and comments. A missing parent or child is created from parsed text, so it
/// picks up the indentation a hand-written file has.
pub fn write_child(doc: &mut KdlDocument, parent: &str, name: &str, body: &str) {
    let Some(index) = parent_index(doc, parent) else {
        let text = format!("\n{parent} {{\n    {body}\n}}\n");
        if let Ok(parsed) = text.parse::<KdlDocument>()
            && let Some(node) = parsed.nodes().first()
        {
            doc.nodes_mut().push(node.clone());
        }
        return;
    };
    let has_children = doc.nodes()[index].children().is_some();
    let existing = doc.nodes()[index]
        .children()
        .into_iter()
        .flat_map(|kids| kids.nodes())
        .find(|node| node.name().value() == name)
        .and_then(|node| node.format().map(|format| format.leading.clone()));
    let leading = existing.unwrap_or_else(|| "\n    ".to_owned());
    if !has_children {
        let text = format!("{leading}{parent} {{\n    {body}\n}}\n");
        if let Ok(parsed) = text.parse::<KdlDocument>()
            && let Some(node) = parsed.nodes().first()
        {
            doc.nodes_mut()[index] = node.clone();
        }
        return;
    }
    let Some(children) = doc.nodes_mut()[index].children_mut().as_mut() else {
        return;
    };
    let text = format!("{leading}{body}\n");
    let Ok(node) = text.parse::<KdlNode>() else {
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

/// Removes `name` from `parent`, leaving the parent and everything else in place.
pub fn remove_child(doc: &mut KdlDocument, parent: &str, name: &str) {
    let Some(index) = parent_index(doc, parent) else {
        return;
    };
    if let Some(children) = doc.nodes_mut()[index].children_mut().as_mut() {
        children.nodes_mut().retain(|node| node.name().value() != name);
    }
}
