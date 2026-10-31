use kdl::{KdlDocument, KdlNode};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
// A setting the file does not name keeps its default; saving removes the key instead of writing
// the default down.
pub enum Setting {
    ShowHidden,
    Inspector,
    GridLayout,
    ScrollInertia,
    Overscroll,
}

impl Setting {
    pub const ALL: [Setting; 5] = [
        Setting::ShowHidden,
        Setting::Inspector,
        Setting::GridLayout,
        Setting::ScrollInertia,
        Setting::Overscroll,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Setting::ShowHidden => "show-hidden",
            Setting::Inspector => "inspector",
            Setting::GridLayout => "grid",
            Setting::ScrollInertia => "scroll-inertia",
            Setting::Overscroll => "overscroll",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Setting::ShowHidden => "hidden",
            Setting::Inspector => "inspector",
            Setting::GridLayout => "grid",
            Setting::ScrollInertia => "inertia",
            Setting::Overscroll => "overscroll",
        }
    }

    pub fn default_on(self) -> bool {
        match self {
            Setting::ShowHidden | Setting::GridLayout => false,
            Setting::Inspector | Setting::ScrollInertia | Setting::Overscroll => true,
        }
    }

    fn index(self) -> usize {
        match self {
            Setting::ShowHidden => 0,
            Setting::Inspector => 1,
            Setting::GridLayout => 2,
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
            Setting::GridLayout => settings.grid,
            Setting::ScrollInertia => settings.scroll_inertia,
            Setting::Overscroll => settings.overscroll,
        }
    }

    fn set(self, settings: &mut Settings, on: bool) {
        match self {
            Setting::ShowHidden => settings.show_hidden = on,
            Setting::Inspector => settings.inspector = on,
            Setting::GridLayout => settings.grid = on,
            Setting::ScrollInertia => settings.scroll_inertia = on,
            Setting::Overscroll => settings.overscroll = on,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Settings {
    pub show_hidden: bool,
    pub inspector: bool,
    pub grid: bool,
    pub scroll_inertia: bool,
    pub overscroll: bool,
    present: [bool; 5],
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            show_hidden: false,
            inspector: true,
            grid: false,
            scroll_inertia: true,
            overscroll: true,
            present: [false; 5],
        }
    }
}

impl Settings {
    pub fn defaults() -> Settings {
        Settings::default()
    }

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

    pub fn is_set(&self, setting: Setting) -> bool {
        self.present[setting.index()]
    }

    pub fn write(&self, doc: &mut KdlDocument, setting: Setting) {
        let body = format!(
            "{} {}",
            setting.name(),
            if setting.value(self) {
                "#true"
            } else {
                "#false"
            }
        );
        write_child(doc, "settings", setting.name(), &body);
    }

    pub fn remember(&mut self, changed: Setting, on: bool) {
        changed.set(self, on);
        self.present[changed.index()] = true;
    }

    pub fn reset(&mut self, setting: Setting) {
        setting.set(self, setting.default_on());
        self.present[setting.index()] = false;
    }

    pub fn clear(doc: &mut KdlDocument, setting: Setting) {
        remove_child(doc, "settings", setting.name());
    }
}

fn parent_index(doc: &KdlDocument, parent: &str) -> Option<usize> {
    doc.nodes()
        .iter()
        .position(|node| node.name().value() == parent)
}

// Rewrites the one child and keeps the parent's indentation, so the file's comments and layout
// survive a save.
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

pub fn remove_child(doc: &mut KdlDocument, parent: &str, name: &str) {
    let Some(index) = parent_index(doc, parent) else {
        return;
    };
    if let Some(children) = doc.nodes_mut()[index].children_mut().as_mut() {
        children
            .nodes_mut()
            .retain(|node| node.name().value() != name);
    }
}
