use crate::app::App;
use crate::rules::SortKey;
use crate::settings::Setting;
use rune_kit::SortDir;
use std::collections::{BTreeSet, HashSet};
use std::path::{Path, PathBuf};

use super::scan::{SYNC_SCAN_LIMIT, Scan, sort_rows};

use super::*;

impl App {
    pub(super) fn enter(&mut self, path: PathBuf) {
        self.watch.retarget(&path);
        self.cwd = path;
        self.visit = Default::default();
        self.scroll.stop();
        self.menu = None;
        self.place_menu = None;
        self.marquee = None;
        self.file_drag = None;
        self.clear_typeahead();
        self.search.clear();
        self.clear_selection();
        self.entries.clear();
        self.visible.clear();
        self.pending_select = None;
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
        let path = self.cwd.clone();
        let keep = self.selected.clone();
        let sort = self.sort.clone();
        if let Some(scan) =
            Scan::try_read(path.clone(), keep.clone(), sort.clone(), SYNC_SCAN_LIMIT)
        {
            self.scan.cancel();
            self.apply_scan(scan);
            return;
        }
        self.scan
            .request(path, move |path| Scan::read(path, keep, sort));
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

    fn apply_scan(&mut self, scan: Scan) -> bool {
        if scan.path != self.cwd {
            return false;
        }
        self.entries = scan.entries;
        self.error = scan.error;
        self.apply_rules();
        if scan.sort != self.sort {
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

    // The read sorted as asked; a sort change since then reorders the rows here.
    pub fn sort_entries(&mut self) {
        let (column, direction) = self.sort.clone();
        sort_rows(&mut self.entries, &column, direction);
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

    pub fn apply_filter(&mut self) {
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
        self.scroll.stop();
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
            self.scroll.stop();
        }
    }

    pub fn toggle_inspector(&mut self) {
        self.inspector.toggle();
        self.remember(Setting::Inspector, self.inspector.is_open());
    }

    pub fn toggle_scroll_inertia(&mut self) {
        self.config.scroll_inertia = !self.config.scroll_inertia;
        if !self.config.scroll_inertia {
            self.scroll.stop_coast();
        }
        self.remember(Setting::ScrollInertia, self.config.scroll_inertia);
    }

    pub fn toggle_overscroll(&mut self) {
        self.config.overscroll = !self.config.overscroll;
        if !self.config.overscroll {
            self.scroll.stop_bounce();
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
        self.apply_filter();
        self.inspector.set_open(self.config.inspector);
        if !self.config.scroll_inertia {
            self.scroll.stop_coast();
        }
        if !self.config.overscroll {
            self.scroll.stop_bounce();
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
            "size" => self.size_width = width.clamp(64.0, 320.0),
            "modified" => self.date_width = width.clamp(100.0, 420.0),
            _ => {}
        }
    }

    pub fn reset_column(&mut self, column: &str) {
        match column {
            "size" => self.size_width = 92.0,
            "modified" => self.date_width = 158.0,
            _ => {}
        }
    }

    pub fn phrase_for_count(&self) -> String {
        if self.is_scanning() {
            return "Reading…".into();
        }
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
