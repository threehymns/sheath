//! What each folder was left showing. Machine-owned state, kept in the state directory rather
//! than the config folder, capped so it cannot grow without bound, and ignored when it cannot be
//! read: a folder always opens.

use crate::app::Layout;
use crate::config;
use crate::rules::{Resolved, SortKey};
use kdl::{KdlDocument, KdlEntry, KdlNode};
use rune_kit::SortDir;
use std::path::{Path, PathBuf};

/// The file under the state directory.
const FILE: &str = "views.kdl";

/// How many folders are remembered. Past this, the oldest visit is forgotten.
const LIMIT: usize = 200;

/// The folders remembered, in the order they were first seen.
pub struct Views {
    path: Option<PathBuf>,
    entries: Vec<(PathBuf, Resolved)>,
    /// Why the file could not be read or written. Damaged state is reported, never fatal.
    pub error: Option<String>,
}

impl Default for Views {
    fn default() -> Self {
        Views {
            path: None,
            entries: Vec::new(),
            error: None,
        }
    }
}

impl Views {
    /// The state saved for this user, or nothing when there is none to read.
    pub fn load() -> Views {
        let Some(dir) = config::state_dir() else {
            return Views::default();
        };
        Views::at(dir.join(FILE))
    }

    /// The state in `path`, for a state folder chosen by the caller.
    pub fn at(path: PathBuf) -> Views {
        let mut views = Views {
            path: Some(path.clone()),
            ..Views::default()
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return views;
        };
        match parse(&text) {
            Ok(entries) => views.entries = entries,
            Err(err) => {
                views.error = Some(format!(
                    "Couldn’t read {}: {err}. Folders open with their defaults.",
                    path.display()
                ));
            }
        }
        views
    }

    /// The presentation this folder was left in.
    pub fn get(&self, path: &Path) -> Resolved {
        self.entries
            .iter()
            .find(|(remembered, _)| remembered == path)
            .map(|(_, state)| *state)
            .unwrap_or_default()
    }

    /// Records what a folder was left showing and writes the file.
    pub fn set(&mut self, path: &Path, state: Resolved) {
        if let Some(entry) = self
            .entries
            .iter_mut()
            .find(|(remembered, _)| remembered == path)
        {
            entry.1 = state;
        } else {
            self.entries.push((path.to_path_buf(), state));
            if self.entries.len() > LIMIT {
                self.entries.remove(0);
            }
        }
        self.save();
    }

    fn save(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        match config::write_atomic(&path, &render(&self.entries)) {
            Ok(()) => self.error = None,
            Err(err) => self.error = Some(err),
        }
    }
}

/// The file: a `folder` node per remembered path, holding only what was not at its default.
fn render(entries: &[(PathBuf, Resolved)]) -> String {
    let mut doc = KdlDocument::new();
    for (path, state) in entries {
        let mut node = KdlNode::new("folder");
        node.push(KdlEntry::new(path.to_string_lossy().into_owned()));
        let mut children = KdlDocument::new();
        if let Some(layout) = state.layout {
            let mut child = KdlNode::new("layout");
            child.push(KdlEntry::new(if layout.is_grid() {
                "grid"
            } else {
                "list"
            }));
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
                ("layout", Some("grid")) => state.layout = Some(Layout::Grid),
                ("layout", Some("list")) => state.layout = Some(Layout::List),
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
