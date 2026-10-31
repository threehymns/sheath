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

/// One folder read, done off the main thread: the rows of a folder and what went wrong
/// opening it. A folder with tens of thousands of entries takes long enough to read that
/// doing it in the frame that opens it would stall the window.
///
/// `Latest` runs it on Rune's worker pool and drops the answer of a read a later one
/// superseded, so navigating away mid-read never lands the folder left behind.
pub(super) struct Scan {
    path: PathBuf,
    entries: Vec<Entry>,
    error: Option<String>,
    /// The row the read was asked to keep selected, if the folder still has it.
    keep: Option<String>,
}

impl Scan {
    fn read(path: PathBuf, keep: Option<String>) -> Scan {
        let mut entries = Vec::new();
        let mut error = None;
        match fs::read_dir(&path) {
            Ok(read) => {
                for item in read.flatten() {
                    if let Some(entry) = Entry::read(&item) {
                        entries.push(entry);
                    }
                }
            }
            Err(err) => {
                error = Some(format!("Couldn’t open {}: {err}", path.display()));
            }
        }
        Scan {
            path,
            entries,
            error,
            keep,
        }
    }
}

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
        // The rows are read off the main thread, so the last folder's go now rather than
        // showing under the new path. What the path alone decides is resolved here; the rules
        // that need the folder's count settle with the read.
        self.entries.clear();
        self.visible.clear();
        self.pending_select = None;
        self.apply_rules();
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

    /// Asks for a fresh read of the folder, keeping the selected rows when they still exist.
    /// Rows keep their ids through a re-read, so a selection of several survives a refresh
    /// until something moves them. The read runs off the main thread: `poll` applies its
    /// answer, so a folder this big never stalls the frame that asked for it.
    pub fn reload(&mut self) {
        let path = self.cwd.clone();
        let keep = self.selected.clone();
        self.scan
            .request(path.clone(), move |path| Scan::read(path, keep));
    }

    /// Applies the folder read that landed, if one did: its rows, its error, the presentation
    /// the rules and the folder's count decide, and the selection the read asked to keep.
    pub(super) fn land_scan(&mut self) -> bool {
        let Some(scan) = self.scan.take() else {
            return false;
        };
        // `Latest` already drops a superseded answer; the path keeps a read of some other
        // folder off this one all the same.
        if scan.path != self.cwd {
            return false;
        }
        self.entries = scan.entries;
        self.error = scan.error;
        self.apply_rules();
        self.sort_entries();
        self.apply_filter();
        // What a file operation made or renamed outranks what the read kept: those rows are
        // what the user just asked for.
        match self.pending_select.take() {
            Some((ids, reveal)) => self.select_after_read(ids, reveal),
            None => match scan.keep {
                Some(id) if self.is_visible(&id) => self.focus_id(&id),
                Some(_) => self.clear_selection(),
                None => {}
            },
        }
        true
    }

    /// Selects the rows `ids` the folder turned out to have, and leaves the list's scroll to
    /// the next `poll` when the operation asked for it.
    fn select_after_read(&mut self, ids: Vec<String>, reveal: bool) {
        let visible: BTreeSet<String> = self
            .visible
            .iter()
            .map(|&i| self.entries[i].id.clone())
            .collect();
        self.selection = ids
            .into_iter()
            .filter(|id| visible.contains(id))
            .collect();
        let Some(first) = self.selected_ids().first().cloned() else {
            // The folder no longer holds the row the operation made: leave no cursor behind.
            self.clear_selection();
            return;
        };
        self.anchor = Some(first.clone());
        self.focus_id(&first);
        if reveal {
            self.pending_reveal = Some(first);
        }
    }

    /// Waits out a read in flight and applies it, for tests that drive the state without a
    /// window's frames to land it.
    #[cfg(test)]
    pub(crate) fn settle_scan(&mut self) {
        while self.scan.is_pending() {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        self.land_scan();
        // No test drives the list's scroll from here.
        self.pending_reveal = None;
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

    /// Whether the rules or the folder's own policy hide `entry`: an entry rule's `hidden`
    /// first, then the dot-name, which only counts while hidden files are off. The row menu
    /// reads the same answer the filter does.
    pub fn entry_hidden(&self, entry: &Entry) -> bool {
        let path = entry.path.to_string_lossy();
        let rule_hidden = self.rules.hidden(&crate::rules::EntryRef {
            path: &path,
            name: &entry.name,
            is_dir: entry.is_dir,
        });
        rule_hidden.unwrap_or(!self.show_hidden && entry.hidden())
    }

    /// Rebuilds the rows that pass the hidden-file and search filters.
    pub fn apply_filter(&mut self) {
        // Different rows: what the old ones were coasting toward no longer means anything.
        self.scroll.stop();
        let query = self.search.text.trim().to_lowercase();
        let hidden: Vec<bool> = self
            .entries
            .iter()
            .map(|entry| self.entry_hidden(entry))
            .collect();
        self.visible = (0..self.entries.len())
            .filter(|&i| {
                let entry = &self.entries[i];
                !hidden[i] && (query.is_empty() || entry.name.to_lowercase().contains(&query))
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
        if self.rule_presentation().show_hidden.is_some() {
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
        // The switch turns the setting over, not what the folder resolved to: a rule or a
        // remembered visit may be what is on screen, and that is not what gets saved.
        self.config.show_hidden = !self.config.show_hidden;
        self.remember(Setting::ShowHidden, self.config.show_hidden);
        self.apply_rules();
        self.apply_filter();
    }

    /// Switches between the table of rows and the grid of tiles, and remembers the choice.
    pub fn toggle_layout(&mut self) {
        self.layout = if self.is_grid() {
            Layout::List
        } else {
            Layout::Grid
        };
        // The two layouts measure their content differently, so what the old one was coasting
        // toward means nothing in the new one.
        self.scroll.stop();
        // A rule for this folder keeps its say after the visit: the change shows now, and the
        // rule wins again the next time the folder opens.
        if self.rule_presentation().layout.is_some() {
            self.visit.layout = Some(self.layout);
        } else {
            self.remember_presentation();
        }
    }

    /// The settings panel's Grid layout switch: turns the global setting over, then resolves the
    /// open folder again, so a rule or what the folder was left showing keeps its say.
    pub fn toggle_grid_setting(&mut self) {
        let before = self.layout;
        // The switch turns the setting over, not what the folder resolved to: a rule or a
        // remembered visit may be what is on screen, and that is not what gets saved.
        self.config.grid = !self.config.grid;
        self.remember(Setting::GridLayout, self.config.grid);
        // A rule or a remembered visit still outranks the global default.
        self.apply_rules();
        if self.layout != before {
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
        let rule = self.rule_presentation();
        if rule.sort.is_some() || rule.direction.is_some() {
            self.visit.sort = Some(SortKey::from_column(column).unwrap_or(SortKey::Name));
            self.visit.direction = Some(self.sort.1);
        } else {
            self.remember_presentation();
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
            .filter(|entry| !self.entry_hidden(entry))
            .count();
        let noun = if shown == 1 { "item" } else { "items" };
        if self.search.text.trim().is_empty() {
            format!("{shown} {noun}")
        } else {
            format!("{shown} of {total} {noun}")
        }
    }
}
