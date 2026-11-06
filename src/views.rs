//! What each folder was left showing. Machine-owned state in the state directory, capped at
//! `LIMIT` folders and ignored when it cannot be read.

use crate::app::Layout;
use crate::config;
use crate::rules::{Resolved, SortKey};
use kdl::{KdlDocument, KdlEntry, KdlNode};
use rune_kit::SortDir;
use std::path::{Path, PathBuf};

pub const FILE: &str = "views.kdl";

const LIMIT: usize = 200;

#[derive(Default)]
pub struct Views {
    state: config::State,
    entries: Vec<(PathBuf, Resolved)>,
}

impl Views {
    pub fn load() -> Views {
        Views::open(config::State::load(FILE))
    }

    #[cfg(test)]
    pub fn at(path: PathBuf) -> Views {
        Views::open(config::State::at(path))
    }

    fn open(state: config::State) -> Views {
        let mut views = Views {
            state,
            ..Views::default()
        };
        let Some(text) = views.state.read() else {
            return views;
        };
        match parse(&text) {
            Ok(entries) => views.entries = entries,
            Err(err) => views
                .state
                .fail(&format!("{err}. Folders open with their defaults.")),
        }
        views
    }

    pub fn error(&self) -> Option<&str> {
        self.state.error()
    }

    pub fn get(&self, path: &Path) -> Resolved {
        self.entries
            .iter()
            .find(|(remembered, _)| remembered == path)
            .map(|(_, state)| *state)
            .unwrap_or_default()
    }

    /// Follows a folder that moved, so the next visit still opens the way it was left.
    pub fn move_path(&mut self, old: &Path, new: &Path) {
        let mut moved = false;
        for (path, _) in &mut self.entries {
            let Ok(rest) = path.strip_prefix(old).map(Path::to_path_buf) else {
                continue;
            };
            *path = new.join(rest);
            moved = true;
        }
        if moved {
            self.save();
        }
    }

    pub fn set(&mut self, path: &Path, state: Resolved) {
        self.entries.retain(|(remembered, _)| remembered != path);
        self.entries.push((path.to_path_buf(), state));
        if self.entries.len() > LIMIT {
            self.entries.remove(0);
        }
        self.save();
    }

    fn save(&mut self) {
        self.state.write(&render(&self.entries));
    }
}

fn render(entries: &[(PathBuf, Resolved)]) -> String {
    let mut doc = KdlDocument::new();
    for (path, state) in entries {
        let mut node = KdlNode::new("folder");
        node.push(KdlEntry::new(path.to_string_lossy().into_owned()));
        let mut children = KdlDocument::new();
        if let Some(layout) = state.layout {
            let mut child = KdlNode::new("layout");
            child.push(KdlEntry::new(layout.as_str()));
            children.nodes_mut().push(child);
        }
        if let Some(sort) = state.sort {
            let mut child = KdlNode::new("sort");
            child.push(KdlEntry::new(sort.as_str()));
            children.nodes_mut().push(child);
        }
        if let Some(direction) = state.direction {
            let mut child = KdlNode::new("direction");
            child.push(KdlEntry::new(match direction {
                SortDir::Ascending => "ascending",
                SortDir::Descending => "descending",
            }));
            children.nodes_mut().push(child);
        }
        if let Some(show_hidden) = state.show_hidden {
            let mut child = KdlNode::new("show-hidden");
            child.push(KdlEntry::new(show_hidden));
            children.nodes_mut().push(child);
        }
        node.set_children(children);
        doc.nodes_mut().push(node);
    }
    doc.autoformat();
    format!("// What each folder was left showing. Sheath owns this file.\n{doc}")
}

fn parse(text: &str) -> Result<Vec<(PathBuf, Resolved)>, String> {
    let doc: KdlDocument = text.parse().map_err(|err: kdl::KdlError| err.to_string())?;
    let mut entries = Vec::new();
    for node in doc.nodes() {
        if node.name().value() != "folder" {
            continue;
        }
        let Some(path) = node.get(0).and_then(|entry| entry.as_string()) else {
            continue;
        };
        let mut state = Resolved::default();
        for child in node.children().into_iter().flat_map(|kids| kids.nodes()) {
            let value = child.get(0).and_then(|entry| entry.as_string());
            match (child.name().value(), value) {
                ("layout", Some(value)) => state.layout = Layout::from_str(value).or(state.layout),
                ("sort", Some(value)) => state.sort = SortKey::from_column(value).or(state.sort),
                ("direction", Some("ascending")) => state.direction = Some(SortDir::Ascending),
                ("direction", Some("descending")) => state.direction = Some(SortDir::Descending),
                ("show-hidden", _) => {
                    state.show_hidden = child.get(0).and_then(|entry| entry.as_bool())
                }
                _ => {}
            }
        }
        entries.push((PathBuf::from(path), state));
    }
    Ok(entries)
}
