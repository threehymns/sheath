use crate::app::{App, SettingsTab};
use crate::config;
use crate::rules::{EntryRef, Kind, SortKey};
use kdl::KdlNode;
use rune_kit::SortDir;
use std::path::Path;

use super::{rule_insertion, rule_key, rule_positions};

impl App {
    pub fn short_path(path: &Path) -> String {
        let text = path.to_string_lossy();
        let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))
        else {
            return text.into_owned();
        };
        let home = home.to_string_lossy();
        let home = home.trim_end_matches(['/', '\\']);
        match text.strip_prefix(home) {
            Some(rest) if rest.is_empty() || rest.starts_with(['/', '\\']) => format!("~{rest}"),
            _ => text.into_owned(),
        }
    }

    pub fn save_layout_rule(&mut self) {
        let pattern = App::short_path(&self.cwd);
        let layout = if self.layout.is_grid() {
            "grid"
        } else {
            "list"
        };
        let direction = match self.sort.1 {
            SortDir::Ascending => "ascending",
            SortDir::Descending => "descending",
        };
        let hidden = if self.show_hidden { "true" } else { "false" };
        let text = format!(
            "folder-rule {{\n    match path={}\n    layout {layout}\n    sort {}\n    direction {direction}\n    show-hidden #{hidden}\n}}\n",
            crate::rules::quote(&pattern),
            SortKey::from_column(&self.sort.0)
                .unwrap_or(SortKey::Name)
                .as_str(),
        );
        self.append_rule(Kind::Folder, &text);
    }

    pub fn pin_entry(&mut self, hidden: bool) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        let Some(entry) = self.entries.iter().find(|entry| entry.id == id) else {
            return;
        };
        let pattern = App::short_path(&entry.path);
        let hidden = if hidden { "true" } else { "false" };
        let text = format!(
            "entry-rule {{\n    match path={}\n    hidden #{hidden}\n}}\n",
            crate::rules::quote(&pattern),
        );
        self.append_rule(Kind::Entry, &text);
    }

    pub(super) fn append_rule(&mut self, kind: Kind, text: &str) -> bool {
        let index = self
            .rules
            .rules
            .iter()
            .rposition(|rule| rule.kind == kind)
            .map_or(self.rules.rules.len(), |at| at + 1);
        let written = match text.parse::<KdlNode>() {
            Ok(node) => self.store.edit(config::CONFIG, |doc| {
                let at = rule_insertion(doc, kind);
                // Break the line first: a last node with no terminator or trailing newline would
                // run into the new rule and stop parsing.
                if at > 0
                    && let Some(format) = doc
                        .nodes_mut()
                        .get_mut(at - 1)
                        .and_then(KdlNode::format_mut)
                    && !format.terminator.contains('\n')
                    && !format.trailing.contains('\n')
                {
                    format.trailing.push('\n');
                }
                doc.nodes_mut().insert(at, node.clone());
            }),
            Err(err) => Err(format!("Couldn’t write the rule: {err}")),
        };
        self.after_rule_change();
        if written.is_ok() && self.settings == Some(SettingsTab::Rules) {
            self.rule_reveal = rule_key(&self.rules.rules, index).map(|key| (key, 1));
            self.rule_query = None;
        }
        written.is_ok()
    }

    pub fn ask_delete_rule(&mut self, index: usize) {
        if self.rule_error.is_some() {
            return;
        }
        self.rule_delete_ask = Some(index);
    }

    pub fn cancel_delete_rule(&mut self) {
        self.rule_delete_ask = None;
    }

    pub fn delete_rule(&mut self, index: usize) {
        self.rule_delete_ask = None;
        if self.rule_error.is_some() {
            return;
        }
        self.close_rule_edit();
        let _ = self.store.edit(config::CONFIG, |doc| {
            if let Some(&at) = rule_positions(doc).get(index) {
                doc.nodes_mut().remove(at);
            }
        });
        self.after_rule_change();
    }

    pub fn move_rule(&mut self, from: usize, to: usize) {
        if from == to || self.rule_error.is_some() {
            return;
        }
        self.close_rule_edit();
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

    pub fn open_rule_picker(&mut self) {
        self.rule_picker = Some(0);
    }

    pub fn close_rule_picker(&mut self) {
        self.rule_picker = None;
    }

    pub fn rule_picker_move(&mut self, delta: isize) {
        let count = self.rules.rules.len();
        if count == 0 {
            return;
        }
        let at = self.rule_picker.unwrap_or(0) as isize;
        self.rule_picker = Some((at + delta).clamp(0, count as isize - 1) as usize);
    }

    pub fn choose_rule(&mut self, index: usize) {
        self.rule_picker = None;
        if index < self.rules.rules.len() {
            self.rule_reveal = rule_key(&self.rules.rules, index).map(|key| (key, 1));
            self.rule_query = None;
            self.settings = Some(SettingsTab::Rules);
            self.open_rule_edit(index);
        }
    }

    pub fn confirm_rule_picker(&mut self) {
        if let Some(index) = self.rule_picker {
            self.choose_rule(index);
        }
    }

    pub fn show_folder_rules(&mut self) {
        let count = self.entries.len();
        let focus = self.rules.first_folder(&self.cwd, count);
        self.rule_reveal = focus
            .and_then(|index| rule_key(&self.rules.rules, index))
            .map(|key| (key, 1));
        self.rule_query = focus
            .is_none()
            .then(|| "No rule matches this folder.".to_owned());
        self.settings = Some(SettingsTab::Rules);
    }

    pub fn show_entry_rules(&mut self) {
        let focus = self.selected.as_ref().and_then(|id| {
            let entry = self.entries.iter().find(|entry| &entry.id == id)?;
            let path = entry.path.to_string_lossy();
            self.rules.first_entry(&EntryRef {
                path: &path,
                name: &entry.name,
                is_dir: entry.is_dir,
            })
        });
        self.rule_reveal = focus
            .and_then(|index| rule_key(&self.rules.rules, index))
            .map(|key| (key, 1));
        self.rule_query = focus
            .is_none()
            .then(|| "No rule matches the selected entry.".to_owned());
        self.settings = Some(SettingsTab::Rules);
    }
}
