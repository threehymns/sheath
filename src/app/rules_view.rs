//! The Rules tab's editing: turning what is on screen into a rule, deleting one, and moving one
//! in the file's order. Every change goes through the config store, so the rest of the file
//! keeps its comments and layout.

use crate::app::{App, SettingsTab};
use crate::config;
use crate::rules::{EntryRef, Kind, Rule};
use kdl::{KdlDocument, KdlNode};
use rune_kit::SortDir;
use std::path::Path;

impl App {
    /// A path with the home folder written as `~`, so a rule travels between machines.
    pub fn short_path(path: &Path) -> String {
        let text = path.to_string_lossy();
        let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) else {
            return text.into_owned();
        };
        let home = home.to_string_lossy();
        let home = home.trim_end_matches('/');
        match text.strip_prefix(home) {
            Some(rest) if rest.is_empty() || rest.starts_with('/') => format!("~{rest}"),
            _ => text.into_owned(),
        }
    }

    /// "Save this view as a rule": pins the current folder's view, sort, direction, and hidden
    /// policy in a new folder rule, appended so it applies immediately.
    pub fn save_view_rule(&mut self) {
        let pattern = App::short_path(&self.cwd);
        let name = self
            .cwd
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| "folder".to_owned());
        let view = if self.view.is_grid() { "grid" } else { "list" };
        let direction = match self.sort.1 {
            SortDir::Ascending => "ascending",
            SortDir::Descending => "descending",
        };
        let hidden = if self.show_hidden { "true" } else { "false" };
        let text = format!(
            "folder-rule \"{name}\" {{\n    match path=\"{pattern}\"\n    view {view}\n    sort {}\n    direction {direction}\n    show-hidden #{hidden}\n}}\n",
            self.sort.0,
        );
        self.append_rule(&text);
    }

    /// "Always show this entry" or "Always hide this entry", for the selected row.
    pub fn pin_entry(&mut self, hidden: bool) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let Some(entry) = self.entries.iter().find(|entry| entry.id == id) else {
            return;
        };
        let pattern = App::short_path(&entry.path);
        let name = entry.name.clone();
        let hidden = if hidden { "true" } else { "false" };
        let text = format!(
            "entry-rule \"{name}\" {{\n    match path=\"{pattern}\"\n    hidden #{hidden}\n}}\n"
        );
        self.append_rule(&text);
    }

    /// Adds a rule node at the end of the document, so it wins over every rule above it.
    fn append_rule(&mut self, text: &str) {
        let Ok(node) = text.parse::<KdlNode>() else {
            return;
        };
        let _ = self.store.edit(config::CONFIG, |doc| {
            doc.nodes_mut().push(node.clone());
        });
        self.after_rule_change();
    }

    /// Removes the rule at `index` in the tab's order.
    pub fn delete_rule(&mut self, index: usize) {
        let _ = self.store.edit(config::CONFIG, |doc| {
            if let Some(&at) = rule_positions(doc).get(index) {
                doc.nodes_mut().remove(at);
            }
        });
        self.after_rule_change();
    }

    /// Moves the rule at `from` to `to`, rewriting the order of the rule nodes only.
    pub fn move_rule(&mut self, from: usize, to: usize) {
        if from == to {
            return;
        }
        let _ = self.store.edit(config::CONFIG, |doc| {
            let positions = rule_positions(doc);
            if from >= positions.len() || to >= positions.len() {
                return;
            }
            let mut rules: Vec<KdlNode> = positions
                .iter()
                .map(|&at| doc.nodes()[at].clone())
                .collect();
            let moved = rules.remove(from);
            rules.insert(to, moved);
            for (at, node) in positions.into_iter().zip(rules) {
                doc.nodes_mut()[at] = node;
            }
        });
        self.after_rule_change();
    }

    /// Opens the picker that lists every rule by its matcher.
    pub fn open_rule_picker(&mut self) {
        self.rule_picker = Some(0);
    }

    pub fn close_rule_picker(&mut self) {
        self.rule_picker = None;
    }

    /// Moves the picker's highlight, stopping at the ends.
    pub fn rule_picker_move(&mut self, delta: isize) {
        let count = self.rules.rules.len();
        if count == 0 {
            return;
        }
        let at = self.rule_picker.unwrap_or(0) as isize;
        self.rule_picker = Some((at + delta).clamp(0, count as isize - 1) as usize);
    }

    /// Jumps from the picker to a rule's definition in the Rules tab.
    pub fn choose_rule(&mut self, index: usize) {
        self.rule_picker = None;
        if index < self.rules.rules.len() {
            self.rule_focus = Some(index);
            self.settings = Some(SettingsTab::Rules);
        }
    }

    pub fn confirm_rule_picker(&mut self) {
        if let Some(index) = self.rule_picker {
            self.choose_rule(index);
        }
    }

    /// Opens the Rules tab at the first rule matching the folder on screen.
    pub fn show_folder_rules(&mut self) {
        let count = self.entries.len();
        self.rule_focus = self.rules.first_folder(&self.cwd, count);
        self.settings = Some(SettingsTab::Rules);
    }

    /// Opens the Rules tab at the first rule matching the selected entry.
    pub fn show_entry_rules(&mut self) {
        self.rule_focus = self.selected.as_ref().and_then(|id| {
            let entry = self.entries.iter().find(|entry| &entry.id == id)?;
            let path = entry.path.to_string_lossy();
            self.rules.first_entry(&EntryRef {
                path: &path,
                name: &entry.name,
                is_dir: entry.is_dir,
            })
        });
        self.settings = Some(SettingsTab::Rules);
    }

    /// Re-reads the rules and the folder after the document changed.
    fn after_rule_change(&mut self) {
        self.reload_rules();
        self.visit = Default::default();
        self.reload();
    }
}

/// The document positions of the rule nodes, in file order.
fn rule_positions(doc: &KdlDocument) -> Vec<usize> {
    doc.nodes()
        .iter()
        .enumerate()
        .filter(|(_, node)| {
            matches!(node.name().value(), "folder-rule" | "entry-rule")
        })
        .map(|(at, _)| at)
        .collect()
}

/// Every rule the tab lists, paired with the kind the parser gave it.
pub fn rule_rows(rules: &[Rule]) -> Vec<(usize, &'static str, String)> {
    rules
        .iter()
        .enumerate()
        .map(|(index, rule)| {
            let kind = match rule.kind {
                Kind::Folder => "Folder",
                Kind::Entry => "Entry",
            };
            (index, kind, rule.summary())
        })
        .collect()
}
