use crate::app::App;
use crate::rules::SortKey;
use crate::settings::Setting;
use rune_kit::SortDir;
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use super::scan::{SEARCH_LIMIT, SYNC_SCAN_LIMIT, Scan, contains_folded, sort_rows};

use super::*;

impl App {
    pub(super) fn enter(&mut self, path: PathBuf) {
        self.watch.retarget(&path);
        self.session.remember(&path);
        self.cwd = path;
        self.visit = Default::default();
        self.stop_scrolling();
        self.menu = None;
        self.empty_menu = None;
        self.header_menu = None;
        self.place_menu = None;
        self.marquee = None;
        self.file_drag = None;
        self.clear_typeahead();
        self.search.clear();
        self.clear_selection();
        self.entries.clear();
        self.visible.clear();
        self.unhidden = 0;
        self.name_at.clear();
        self.pending_select = None;
        self.parked = None;
        self.search_truncated = false;
        self.layout_cut = true;
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

    pub fn reload(&mut self) {
        // A read means the folders below may have changed too, so the index starts over.
        self.finder.invalidate();
        // The list is showing matches from below, so a read means walking again, not one folder.
        if self.parked.is_some() {
            self.apply_filter();
            return;
        }
        let path = self.cwd.clone();
        let keep = self.selected.clone();
        let sort = self.sort.clone();
        let folders_first = self.config.folders_first;
        if let Some(scan) = Scan::try_read(
            path.clone(),
            keep.clone(),
            sort.clone(),
            SYNC_SCAN_LIMIT,
            folders_first,
        ) {
            self.scan.cancel();
            self.apply_scan(scan);
            return;
        }
        self.scan.request(path, move |path| {
            Scan::read(path, keep, sort, folders_first)
        });
    }

    pub fn is_scanning(&self) -> bool {
        self.scan.is_pending()
    }

    pub(super) fn land_scan(&mut self) -> bool {
        let Some(scan) = self.scan.take() else {
            return false;
        };
        self.apply_scan(scan)
    }

    /// An index built on the worker has landed: the search runs again over it, so the rows the
    /// keystroke could not answer yet appear.
    pub(super) fn land_find(&mut self) -> bool {
        if self.finder.land().is_none() {
            return false;
        }
        if self.parked.is_some() {
            self.apply_filter();
        }
        true
    }

    /// Whether a search is showing and the folder is still being walked for it.
    pub fn is_indexing(&self) -> bool {
        self.parked.is_some() && self.finder.is_building()
    }

    fn apply_scan(&mut self, scan: Scan) -> bool {
        if scan.path != self.cwd {
            return false;
        }
        self.entries = scan.entries;
        self.error = scan.error;
        self.apply_rules();
        if scan.sort != self.sort || scan.folders_first != self.config.folders_first {
            self.sort_entries();
        }
        self.apply_filter();
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

    fn select_after_read(&mut self, ids: Vec<String>, reveal: bool) {
        let visible: BTreeSet<String> = self
            .visible
            .iter()
            .map(|&i| self.entries[i].id.clone())
            .collect();
        self.selection = ids.into_iter().filter(|id| visible.contains(id)).collect();
        let Some(first) = self.selected_ids().first().cloned() else {
            self.clear_selection();
            return;
        };
        self.anchor = Some(first.clone());
        self.focus_id(&first);
        if reveal {
            self.pending_reveal = Some(first);
        }
    }

    #[cfg(test)]
    pub(crate) fn settle_scan(&mut self) {
        while self.scan.is_pending() {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        self.land_scan();
        self.pending_reveal = None;
    }

    // A search-index walk runs on the pool too, so a test waits for it the same way.
    #[cfg(test)]
    pub(crate) fn settle_find(&mut self) {
        while self.finder.is_building() {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        self.land_find();
    }

    // The read sorted as asked; a sort change since then reorders the rows here.
    pub fn sort_entries(&mut self) {
        let (column, direction) = self.sort.clone();
        sort_rows(
            &mut self.entries,
            &column,
            direction,
            self.config.folders_first,
        );
    }

    /// Opens the pane in the chosen folder, or in the folder on screen when the chosen row is a
    /// file. The pane keeps the shell it already has.
    pub fn open_terminal_in_selection(&mut self) {
        let path = self
            .selected_entry()
            .filter(|entry| entry.is_dir)
            .map(|entry| entry.path.clone())
            .unwrap_or_else(|| self.cwd.clone());
        self.term.open(path);
    }

    pub fn entry_hidden(&self, entry: &Entry) -> bool {
        let path = entry.path.to_string_lossy();
        let rule_hidden = self.rules.hidden(&crate::rules::EntryRef {
            path: &path,
            name: &entry.name,
            is_dir: entry.is_dir,
        });
        rule_hidden.unwrap_or(!self.show_hidden && entry.hidden())
    }

    /// Shows the matches from the folder on screen and the folders below it, and puts the
    /// folder's own rows back when the field is empty.
    fn apply_scope(&mut self, query: &str) {
        if query.is_empty() {
            if let Some(parked) = self.parked.take() {
                self.entries = parked;
            }
            self.search_truncated = false;
            return;
        }
        let found = self
            .finder
            .find(&self.cwd, query, self.show_hidden, SEARCH_LIMIT);
        if self.parked.is_none() {
            self.parked = Some(std::mem::take(&mut self.entries));
        }
        self.search_truncated = found.truncated;
        self.entries = found.entries;
    }

    pub fn apply_filter(&mut self) {
        self.stop_scrolling();
        let query = self.search.text.trim().to_lowercase();
        self.apply_scope(&query);
        // The search has already chosen the matches, so only the folder's own rows need the
        // name check; the path bar keeps counting the folder, not what was found below it.
        let searching = self.parked.is_some();
        let counted = self.parked.as_deref().unwrap_or(&self.entries);
        let unhidden = counted
            .iter()
            .filter(|entry| !self.entry_hidden(entry))
            .count();
        self.unhidden = unhidden;
        let hidden: Vec<bool> = self
            .entries
            .iter()
            .map(|entry| self.entry_hidden(entry))
            .collect();
        self.visible = (0..self.entries.len())
            .filter(|&i| {
                let entry = &self.entries[i];
                !hidden[i]
                    && (query.is_empty() || searching || contains_folded(&entry.name, &query))
            })
            .collect();
        self.keys_version = self.keys_version.wrapping_add(1);
        let visible: HashSet<&str> = self
            .visible
            .iter()
            .map(|&i| self.entries[i].id.as_str())
            .collect();
        self.selection.retain(|id| visible.contains(id.as_str()));
        if self
            .anchor
            .as_ref()
            .is_some_and(|id| !visible.contains(id.as_str()))
        {
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
        if self.rule_presentation().show_hidden.is_some() {
            self.visit.show_hidden = Some(on);
        } else {
            self.config.show_hidden = on;
            self.remember(Setting::ShowHidden, on);
        }
        self.apply_filter();
    }

    pub fn toggle_hidden_setting(&mut self) {
        self.config.show_hidden = !self.config.show_hidden;
        self.remember(Setting::ShowHidden, self.config.show_hidden);
        self.apply_rules();
        self.apply_filter();
    }

    pub fn toggle_layout(&mut self) {
        self.layout = if self.is_grid() {
            Layout::List
        } else {
            Layout::Grid
        };
        self.layout_cut = false;
        self.stop_scrolling();
        if self.rule_presentation().layout.is_some() {
            self.visit.layout = Some(self.layout);
        } else {
            self.remember_presentation();
        }
    }

    pub fn toggle_grid_setting(&mut self) {
        let before = self.layout;
        self.layout_cut = false;
        self.config.grid = !self.config.grid;
        self.remember(Setting::GridLayout, self.config.grid);
        self.apply_rules();
        if self.layout != before {
            self.stop_scrolling();
        }
    }

    pub fn toggle_folders_first(&mut self) {
        self.config.folders_first = !self.config.folders_first;
        self.remember(Setting::FoldersFirst, self.config.folders_first);
        self.sort_entries();
        self.apply_filter();
    }

    pub fn toggle_confirm_trash(&mut self) {
        self.config.confirm_trash = !self.config.confirm_trash;
        self.remember(Setting::ConfirmTrash, self.config.confirm_trash);
    }

    pub fn open_header_menu(&mut self, at: rune::PointerPress) {
        self.header_menu = Some(at);
    }

    pub fn close_header_menu(&mut self) {
        self.header_menu = None;
    }

    pub fn toggle_show_kind(&mut self) {
        self.config.show_kind = !self.config.show_kind;
        self.remember(Setting::ShowKind, self.config.show_kind);
    }

    pub fn toggle_show_size(&mut self) {
        self.config.show_size = !self.config.show_size;
        self.remember(Setting::ShowSize, self.config.show_size);
    }

    pub fn toggle_show_modified(&mut self) {
        self.config.show_modified = !self.config.show_modified;
        self.remember(Setting::ShowModified, self.config.show_modified);
    }

    pub fn toggle_confirm_delete(&mut self) {
        self.config.confirm_delete = !self.config.confirm_delete;
        self.remember(Setting::ConfirmDelete, self.config.confirm_delete);
    }

    pub fn toggle_inspector(&mut self) {
        self.inspector.toggle();
        self.remember(Setting::Inspector, self.inspector.is_open());
    }

    pub fn toggle_scroll_inertia(&mut self) {
        self.config.scroll_inertia = !self.config.scroll_inertia;
        if !self.config.scroll_inertia {
            self.stop_coasting();
        }
        self.remember(Setting::ScrollInertia, self.config.scroll_inertia);
    }

    pub fn toggle_overscroll(&mut self) {
        self.config.overscroll = !self.config.overscroll;
        if !self.config.overscroll {
            self.stop_bouncing();
        }
        self.remember(Setting::Overscroll, self.config.overscroll);
    }

    fn remember(&mut self, changed: Setting, on: bool) {
        self.config.remember(changed, on);
        self.save_setting(changed);
    }

    pub fn reset_setting(&mut self, setting: Setting) {
        self.config.reset(setting);
        let _ = self.store.edit(crate::config::CONFIG, |doc| {
            crate::settings::Settings::clear(doc, setting)
        });
        self.apply_rules();
        self.sort_entries();
        self.apply_filter();
        self.inspector.set_open(self.config.inspector);
        if !self.config.scroll_inertia {
            self.stop_coasting();
        }
        if !self.config.overscroll {
            self.stop_bouncing();
        }
    }

    pub fn sort_column(&self) -> (&str, SortDir) {
        (self.sort.0.as_str(), self.sort.1)
    }

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

    pub fn resize_column(&mut self, column: &str, width: f32) {
        match column {
            "kind" => self.kind_width = width.clamp(80.0, 320.0),
            "size" => self.size_width = width.clamp(64.0, 320.0),
            "modified" => self.date_width = width.clamp(100.0, 420.0),
            _ => {}
        }
    }

    pub fn reset_column(&mut self, column: &str) {
        match column {
            "kind" => self.kind_width = 150.0,
            "size" => self.size_width = 92.0,
            "modified" => self.date_width = 158.0,
            _ => {}
        }
        self.remember_columns();
    }

    pub fn remember_columns(&mut self) {
        self.session
            .remember_columns([self.kind_width, self.size_width, self.date_width]);
    }

    pub fn phrase_for_count(&self) -> String {
        if self.is_scanning() {
            return "Reading…".into();
        }
        if self.is_indexing() {
            return "Searching…".into();
        }
        let shown = self.visible.len();
        // The walk stopped before the folders ran out, so the count is a floor, not a total.
        if self.search_truncated {
            return format!("{shown}+ matches");
        }
        let chosen = self.selection_len();
        if chosen > 1 {
            let size = self.selection_size();
            let suffix = if size > 0 {
                format!(" · {}", crate::format::size(size))
            } else {
                String::new()
            };
            return format!("{chosen} of {shown} selected{suffix}");
        }
        let total = self.unhidden;
        let noun = if shown == 1 { "item" } else { "items" };
        if self.search.text.trim().is_empty() {
            format!("{shown} {noun}")
        } else {
            format!("{shown} of {total} {noun}")
        }
    }
}
