//! The one-shot rule actions: saving the view, pinning an entry, deleting, reordering, and the
//! picker that jumps to a rule's definition. Each of these writes the file at once, which is
//! right for an action the user asked for by name.

use crate::app::{App, SettingsTab};
use crate::config;
use crate::rules::{EntryRef, Kind, SortKey};
use kdl::KdlNode;
use rune_kit::SortDir;
use std::path::Path;

use super::{rule_insertion, rule_key, rule_positions};

impl App {
    /// A path with the home folder written as `~`, so a rule travels between machines.
    pub fn short_path(path: &Path) -> String {
        let text = path.to_string_lossy();
        let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) else {
            return text.into_owned();
        };
        let home = home.to_string_lossy();
        let home = home.trim_end_matches(['/', '\\']);
        match text.strip_prefix(home) {
            Some(rest) if rest.is_empty() || rest.starts_with(['/', '\\']) => format!("~{rest}"),
            _ => text.into_owned(),
        }
    }

    /// "Save this view as a rule": pins the current folder's view, sort, direction, and hidden
    /// policy in a new folder rule, written after the last folder rule so it applies at once.
    /// The rule goes unnamed: its matcher already names it, and a name is the user's to give.
    pub fn save_view_rule(&mut self) {
        let pattern = App::short_path(&self.cwd);
        let view = if self.view.is_grid() { "grid" } else { "list" };
        let direction = match self.sort.1 {
            SortDir::Ascending => "ascending",
            SortDir::Descending => "descending",
        };
        let hidden = if self.show_hidden { "true" } else { "false" };
        let text = format!(
            "folder-rule {{\n    match path={}\n    view {view}\n    sort {}\n    direction {direction}\n    show-hidden #{hidden}\n}}\n",
            crate::rules::quote(&pattern),
            SortKey::from_column(&self.sort.0).unwrap_or(SortKey::Name).as_str(),
        );
        self.append_rule(Kind::Folder, &text);
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
        let hidden = if hidden { "true" } else { "false" };
        let text = format!(
            "entry-rule {{\n    match path={}\n    hidden #{hidden}\n}}\n",
            crate::rules::quote(&pattern),
        );
        self.append_rule(Kind::Entry, &text);
    }

    /// Adds a rule node after the last rule of its kind, so the file keeps one block per kind,
    /// and the new rule still lands after every rule that could speak for the same thing.
    pub(super) fn append_rule(&mut self, kind: Kind, text: &str) {
        // Where the rule lands in the tab's list: after the last rule of its kind, or at the
        // end when it is the first of its kind.
        let index = self
            .rules
            .rules
            .iter()
            .rposition(|rule| rule.kind == kind)
            .map_or(self.rules.rules.len(), |at| at + 1);
        let written = match text.parse::<KdlNode>() {
            Ok(node) => self.store.edit(config::CONFIG, |doc| {
                let at = rule_insertion(doc, kind);
                doc.nodes_mut().insert(at, node.clone());
            }),
            Err(err) => Err(format!("Couldn’t write the rule: {err}")),
        };
        let appended = written.is_ok();
        self.after_rule_change();
        // Set after the reload, which clears it whenever the file reads back cleanly.
        if let Err(err) = written {
            self.rule_error = Some(err);
        } else if appended && self.settings == Some(SettingsTab::Rules) {
            // The tab scrolls to the rule the action just wrote, wherever in the file it landed.
            self.rule_reveal = rule_key(&self.rules.rules, index).map(|key| (key, 1));
            self.rule_query = None;
        }
    }

    /// Asks before removing a rule: the row turns into a question until the second press.
    pub fn ask_delete_rule(&mut self, index: usize) {
        // The list on screen came from the last file that parsed, so its positions mean nothing
        // against a document that has not.
        if self.rule_error.is_some() {
            return;
        }
        self.rule_delete_ask = Some(index);
    }

    pub fn cancel_delete_rule(&mut self) {
        self.rule_delete_ask = None;
    }

    /// Removes the rule at `index` in the tab's order.
    pub fn delete_rule(&mut self, index: usize) {
        // The confirmed press leaves no question behind, whichever way it got here.
        self.rule_delete_ask = None;
        // The list on screen came from the last file that parsed, so its positions mean nothing
        // against a document that has not.
        if self.rule_error.is_some() {
            return;
        }
        // The editor's indices would name the wrong rule after this.
        self.close_rule_edit();
        let _ = self.store.edit(config::CONFIG, |doc| {
            if let Some(&at) = rule_positions(doc).get(index) {
                doc.nodes_mut().remove(at);
            }
        });
        self.after_rule_change();
    }

    /// Moves the rule at `from` to `to`, rewriting the order of the rule nodes only.
    pub fn move_rule(&mut self, from: usize, to: usize) {
        if from == to || self.rule_error.is_some() {
            return;
        }
        // The editor's indices would name the wrong rule after this.
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

    /// Jumps from the picker to a rule: the Rules tab opens with that rule's form up.
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

    /// Opens the Rules tab at the first rule matching the folder on screen.
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

    /// Opens the Rules tab at the first rule matching the selected entry.
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
