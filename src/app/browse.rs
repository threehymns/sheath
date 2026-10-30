//! The folder on screen: reading it, sorting it, filtering it, and the history that
//! navigation walks.

use crate::app::App;
use crate::rules::SortKey;
use crate::settings::Setting;
use rune_kit::SortDir;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use super::entry::compare_time;

use super::*;

impl App {
    /// Shows `path` without touching history.
    pub(super) fn enter(&mut self, path: PathBuf) {
        self.cwd = path;
        self.visit = Default::default();
        self.scroll.stop();
        self.menu = None;
        self.marquee = None;
        self.file_drag = None;
        self.clear_typeahead();
        self.search.clear();
        self.clear_selection();
        self.reload();
    }

    pub fn navigate(&mut self, path: PathBuf) {
        if self.cwd == path {
            self.reload();
            return;
        }
        self.history.truncate(self.at + 1);
        self.history.push(path.clone());
        self.at = self.history.len() - 1;
        self.enter(path);
    }

    pub fn back(&mut self) {
        if self.at == 0 {
            return;
        }
        self.at -= 1;
        let path = self.history[self.at].clone();
        self.enter(path);
    }

    pub fn forward(&mut self) {
        if self.at + 1 >= self.history.len() {
            return;
        }
        self.at += 1;
        let path = self.history[self.at].clone();
        self.enter(path);
    }

    pub fn up(&mut self) {
        let Some(parent) = self.cwd.parent().map(Path::to_path_buf) else {
            return;
        };
        self.navigate(parent);
    }

    pub fn can_back(&self) -> bool {
        self.at > 0
    }

    pub fn can_forward(&self) -> bool {
        self.at + 1 < self.history.len()
    }

    pub fn can_up(&self) -> bool {
        self.cwd.parent().is_some()
    }

    /// Re-reads the folder, keeping the selected rows when they still exist. Rows keep their
    /// ids through a re-read, so a selection of several survives a refresh until something
    /// moves them.
    pub fn reload(&mut self) {
        let keep = self.selected.clone();
        self.read_dir();
        self.apply_rules();
        self.sort_entries();
        self.apply_filter();
        match keep {
            Some(id) if self.is_visible(&id) => self.focus_id(&id),
            Some(_) => self.clear_selection(),
            None => {}
        }
    }

    fn read_dir(&mut self) {
        let mut entries = Vec::new();
        let mut error = None;
        match fs::read_dir(&self.cwd) {
            Ok(read) => {
                for item in read.flatten() {
                    if let Some(entry) = Entry::read(&item) {
                        entries.push(entry);
                    }
                }
            }
            Err(err) => {
                error = Some(format!("Couldn’t open {}: {err}", self.cwd.display()));
            }
        }
        self.entries = entries;
        self.error = error;
    }

    /// Folders first, then the list's sort column and direction.
    pub fn sort_entries(&mut self) {
        let (column, direction) = self.sort.clone();
        self.entries.sort_by(|a, b| {
            b.is_dir.cmp(&a.is_dir).then_with(|| {
                let order = match column.as_str() {
                    "size" => a.size.cmp(&b.size),
                    "modified" => compare_time(a.modified, b.modified),
                    _ => rune_kit::natural_cmp(&a.name, &b.name),
                };
                direction.apply(order)
            })
        });
    }

    /// Rebuilds the rows that pass the hidden-file and search filters.
    pub fn apply_filter(&mut self) {
        // Different rows: what the old ones were coasting toward no longer means anything.
        self.scroll.stop();
        let query = self.search.text.trim().to_lowercase();
        self.visible = (0..self.entries.len())
            .filter(|&i| {
                let entry = &self.entries[i];
                let path = entry.path.to_string_lossy();
                let rule_hidden = self.rules.hidden(&crate::rules::EntryRef {
                    path: &path,
                    name: &entry.name,
                    is_dir: entry.is_dir,
                });
                let hidden = rule_hidden.unwrap_or(!self.show_hidden && entry.hidden());
                !hidden && (query.is_empty() || entry.name.to_lowercase().contains(&query))
            })
            .collect();
        // The hidden and filtered rows drop out of the selection with the row itself.
        let visible: BTreeSet<String> = self
            .visible
            .iter()
            .map(|&i| self.entries[i].id.clone())
            .collect();
        self.selection.retain(|id| visible.contains(id));
        if self.anchor.as_ref().is_some_and(|id| !visible.contains(id)) {
            self.anchor = None;
        }
        if self
            .selected
            .as_ref()
            .is_some_and(|id| !self.is_visible(id))
        {
            self.selected = None;
            self.preview = None;
            self.selection.clear();
        }
    }

    pub(super) fn is_visible(&self, id: &str) -> bool {
        self.visible.iter().any(|&i| self.entries[i].id == id)
    }

    pub fn toggle_hidden(&mut self) {
        let on = !self.show_hidden;
        self.show_hidden = on;
        // A rule that speaks for this folder holds it for the visit; otherwise the switch is
        // the global setting it has always been.
        if self.rule_view().show_hidden.is_some() {
            self.visit.show_hidden = Some(on);
        } else {
            self.config.show_hidden = on;
            self.remember(Setting::ShowHidden, on);
        }
        self.apply_filter();
    }

    /// The settings panel's Show hidden files switch: turns the global setting over, then
    /// resolves the open folder again, so a rule or a remembered visit keeps its say.
    pub fn toggle_hidden_setting(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.remember(Setting::ShowHidden, self.show_hidden);
        self.apply_rules();
        self.apply_filter();
    }

    /// Switches between the table of rows and the grid of tiles, and remembers the choice.
    pub fn toggle_view(&mut self) {
        self.view = if self.is_grid() {
            ViewMode::List
        } else {
            ViewMode::Grid
        };
        // The two views measure their content differently, so what the old one was coasting
        // toward means nothing in the new one.
        self.scroll.stop();
        // A rule for this folder keeps its say after the visit: the change shows now, and the
        // rule wins again the next time the folder opens.
        if self.rule_view().view.is_some() {
            self.visit.view = Some(self.view);
        } else {
            self.remember_view();
        }
    }

    /// The settings panel's Grid view switch: turns the global setting over, then resolves the
    /// open folder again, so a rule or what the folder was left showing keeps its say.
    pub fn toggle_grid_setting(&mut self) {
        // The switch shows what the folder resolved to, so that is what it turns over.
        let before = self.view;
        self.view = if before.is_grid() {
            ViewMode::List
        } else {
            ViewMode::Grid
        };
        // The setting is written from the browser's view, so the new mode is what it records.
        self.remember(Setting::GridView, self.is_grid());
        // A rule or a remembered visit still outranks the global default.
        self.apply_rules();
        if self.view != before {
            self.scroll.stop();
        }
    }

    /// Shows or hides the inspector, and remembers the choice.
    pub fn toggle_inspector(&mut self) {
        self.inspector.toggle();
        self.remember(Setting::Inspector, self.inspector.is_open());
    }

    /// Keeps the list coasting after the wheel, or stops it doing so, and remembers the choice.
    pub fn toggle_scroll_inertia(&mut self) {
        self.config.scroll_inertia = !self.config.scroll_inertia;
        if !self.config.scroll_inertia {
            self.scroll.stop_coast();
        }
        self.remember(Setting::ScrollInertia, self.config.scroll_inertia);
    }

    /// Lets the list be pulled past its ends, or not, and remembers the choice.
    pub fn toggle_overscroll(&mut self) {
        self.config.overscroll = !self.config.overscroll;
        if !self.config.overscroll {
            self.scroll.stop_bounce();
        }
        self.remember(Setting::Overscroll, self.config.overscroll);
    }

    /// Saves one setting into `config.kdl`, naming it there from now on. Only that setting is
    /// recorded: what the open folder shows may be a rule's or a remembered visit's doing, and
    /// that must not become the global default for the folders those leave alone.
    fn remember(&mut self, changed: Setting, on: bool) {
        self.config.remember(changed, on);
        self.save_setting(changed);
    }

    /// Takes a setting out of `config.kdl`, so the next launch uses the default, and uses it
    /// now too.
    pub fn reset_setting(&mut self, setting: Setting) {
        self.config.reset(setting);
        let _ = self.store.edit(crate::config::CONFIG, |doc| {
            crate::settings::Settings::clear(doc, setting)
        });
        // A rule or a remembered visit still outranks the global default, so the open folder
        // is resolved again rather than forced back to the setting just reset.
        self.apply_rules();
        self.apply_filter();
        self.inspector.set_open(self.config.inspector);
        if !self.config.scroll_inertia {
            self.scroll.stop_coast();
        }
        if !self.config.overscroll {
            self.scroll.stop_bounce();
        }
    }

    /// The columns the list shows, and which one it sorts by.
    pub fn sort_column(&self) -> (&str, SortDir) {
        (self.sort.0.as_str(), self.sort.1)
    }

    /// A header click: the sort column flips, another column starts ascending.
    pub fn sort_by(&mut self, column: &str) {
        self.sort = if self.sort.0 == column {
            (column.to_owned(), self.sort.1.flip())
        } else {
            (column.to_owned(), SortDir::Ascending)
        };
        let rule = self.rule_view();
        if rule.sort.is_some() || rule.direction.is_some() {
            self.visit.sort = Some(SortKey::from_column(column).unwrap_or(SortKey::Name));
            self.visit.direction = Some(self.sort.1);
        } else {
            self.remember_view();
        }
        self.sort_entries();
        self.apply_filter();
    }

    /// Sets a column's width as dragging its header edge does, clamped to a usable range.
    pub fn resize_column(&mut self, column: &str, width: f32) {
        match column {
            "size" => self.size_width = width.clamp(64.0, 320.0),
            "modified" => self.date_width = width.clamp(100.0, 420.0),
            _ => {}
        }
    }

    /// Puts a column back to the width it starts at.
    pub fn reset_column(&mut self, column: &str) {
        match column {
            "size" => self.size_width = 92.0,
            "modified" => self.date_width = 158.0,
            _ => {}
        }
    }

    pub fn phrase_for_count(&self) -> String {
        let shown = self.visible.len();
        let chosen = self.selection_len();
        if chosen > 1 {
            return format!("{chosen} of {shown} selected");
        }
        let total = self
            .entries
            .iter()
            .filter(|entry| self.show_hidden || !entry.hidden())
            .count();
        let noun = if shown == 1 { "item" } else { "items" };
        if self.search.text.trim().is_empty() {
            format!("{shown} {noun}")
        } else {
            format!("{shown} of {total} {noun}")
        }
    }
}
