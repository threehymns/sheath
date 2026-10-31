use crate::app::App;
use rune::{Mods, PointerPress, Tx};
use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use super::entry::read_preview;
use super::paths::open_external;

use super::*;

impl App {
    pub fn select_id(&mut self, id: &str) {
        self.selection.clear();
        self.selection.insert(id.to_owned());
        self.anchor = Some(id.to_owned());
        self.focus_id(id);
    }

    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.anchor = None;
        self.selected = None;
        self.preview = None;
    }

    pub(super) fn focus_id(&mut self, id: &str) {
        self.selected = Some(id.to_owned());
        let found = self
            .entries
            .iter()
            .find(|e| e.id == id)
            .map(|e| (e.path.clone(), e.is_dir));
        self.preview = match found {
            Some((path, is_dir)) => read_preview(&path, is_dir),
            None => None,
        };
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        let id = self.selected.as_deref()?;
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn entry_name(&self, id: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.name.as_str())
    }

    pub fn is_selected(&self, id: &str) -> bool {
        self.selection.contains(id)
    }

    pub fn selected_ids(&self) -> Vec<String> {
        self.visible
            .iter()
            .map(|&i| self.entries[i].id.clone())
            .filter(|id| self.selection.contains(id))
            .collect()
    }

    pub fn selection_len(&self) -> usize {
        self.selected_ids().len()
    }

    pub fn selection_folders(&self) -> usize {
        self.selected_ids()
            .iter()
            .filter_map(|id| self.entries.iter().find(|entry| &entry.id == id))
            .filter(|entry| entry.is_dir)
            .count()
    }

    pub fn selection_size(&self) -> u64 {
        self.selected_ids()
            .iter()
            .filter_map(|id| self.entries.iter().find(|entry| &entry.id == id))
            .filter(|entry| !entry.is_dir)
            .map(|entry| entry.size)
            .sum()
    }

    pub fn collapse_selection(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        self.selection.clear();
        if self.is_visible(&id) {
            self.selection.insert(id.clone());
        }
        self.anchor = Some(id);
    }

    pub fn clear_typeahead(&mut self) {
        self.typeahead.clear();
        self.typeahead_at = None;
    }

    pub fn typeahead_select(&mut self, typed: &str, tx: &mut Tx) -> bool {
        let now = Instant::now();
        let fresh = self
            .typeahead_at
            .is_none_or(|at| now.duration_since(at) > Duration::from_millis(900));
        if fresh {
            self.typeahead.clear();
        }
        self.typeahead_at = Some(now);
        self.typeahead.push_str(&typed.to_lowercase());
        let buffer = self.typeahead.clone();
        let repeated = buffer
            .chars()
            .next()
            .is_some_and(|first| buffer.chars().all(|c| c == first));
        let query = if repeated {
            buffer.chars().next().map(String::from).unwrap_or_default()
        } else {
            buffer.clone()
        };
        let skip = if repeated {
            buffer.chars().count() - 1
        } else {
            0
        };
        let matches: Vec<usize> = (0..self.visible.len())
            .filter(|&i| {
                self.entries[self.visible[i]]
                    .name
                    .to_lowercase()
                    .starts_with(&query)
            })
            .collect();
        let Some(&index) = matches.get(skip % matches.len().max(1)) else {
            return false;
        };
        let Some(id) = self.row_id(index) else {
            return false;
        };
        self.select_id(&id);
        if let Some(key) = self.scroll_key(index) {
            tx.scroll_into_view_in("files-table", key);
        }
        true
    }

    pub fn select_all(&mut self) {
        self.selection = self
            .visible
            .iter()
            .map(|&i| self.entries[i].id.clone())
            .collect();
        if let Some(id) = self
            .selected
            .clone()
            .filter(|id| self.selection.contains(id))
        {
            self.anchor = Some(id);
        } else {
            self.anchor = self.selection.iter().next().cloned();
        }
    }

    pub fn selected_index(&self) -> Option<usize> {
        let id = self.selected.as_deref()?;
        self.visible.iter().position(|&i| self.entries[i].id == id)
    }

    pub(super) fn row_id(&self, index: usize) -> Option<String> {
        self.visible.get(index).map(|&i| self.entries[i].id.clone())
    }

    pub(super) fn anchor_index(&self) -> Option<usize> {
        let id = self.anchor.as_deref()?;
        self.visible.iter().position(|&i| self.entries[i].id == id)
    }

    fn range_to(&self, index: usize) -> BTreeSet<String> {
        if self.is_grid() {
            let mut range = BTreeSet::new();
            self.grid_range_to(index, &mut range);
            return range;
        }
        let from = self.anchor_index().unwrap_or(index);
        let (a, b) = (from.min(index), from.max(index));
        (a..=b)
            .filter_map(|i| self.visible.get(i).map(|&i| self.entries[i].id.clone()))
            .collect()
    }

    pub fn click_row(&mut self, index: usize, mods: Mods, tx: &mut Tx) {
        let Some(id) = self.row_id(index) else {
            if !mods.shift && !(mods.cmd || mods.ctrl) {
                self.clear_selection();
            }
            return;
        };
        let toggle = mods.cmd || mods.ctrl;
        if mods.shift {
            let range = self.range_to(index);
            if toggle {
                self.selection.extend(range);
            } else {
                self.selection = range;
            }
        } else if toggle {
            if !self.selection.remove(&id) {
                self.selection.insert(id.clone());
            }
            self.anchor = Some(id.clone());
        } else {
            self.selection.clear();
            self.selection.insert(id.clone());
            self.anchor = Some(id.clone());
        }
        self.focus_id(&id);
        if !self.selection.contains(&id)
            && let Some(next) = self.nearest_selected(index)
        {
            self.focus_id(&next);
        }
        if let Some(key) = self.scroll_key(index) {
            tx.scroll_into_view_in("files-table", key);
        }
    }

    fn nearest_selected(&self, index: usize) -> Option<String> {
        self.visible
            .iter()
            .enumerate()
            .filter(|(_, i)| self.selection.contains(&self.entries[**i].id))
            .min_by_key(|(at, _)| at.abs_diff(index))
            .map(|(_, i)| self.entries[*i].id.clone())
    }

    pub fn move_selection(&mut self, delta: isize, mods: Mods, tx: &mut Tx) {
        if self.visible.is_empty() {
            return;
        }
        let last = self.visible.len() - 1;
        let index = match self.selected_index() {
            Some(i) => (i as isize + delta).clamp(0, last as isize) as usize,
            None if delta > 0 => 0,
            None => last,
        };
        self.select_visible(index, mods, tx);
    }

    pub fn select_edge(&mut self, last: bool, mods: Mods, tx: &mut Tx) {
        if self.visible.is_empty() {
            return;
        }
        self.select_visible(if last { self.visible.len() - 1 } else { 0 }, mods, tx);
    }

    fn select_visible(&mut self, index: usize, mods: Mods, tx: &mut Tx) {
        let Some(id) = self.row_id(index) else {
            return;
        };
        if mods.shift {
            let range = self.range_to(index);
            if mods.cmd || mods.ctrl {
                self.selection.extend(range);
            } else {
                self.selection = range;
            }
        } else {
            self.selection.clear();
            self.selection.insert(id.clone());
            self.anchor = Some(id.clone());
        }
        self.focus_id(&id);
        if let Some(key) = self.scroll_key(index) {
            tx.scroll_into_view_in("files-table", key);
        }
    }

    pub fn open_selected(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        self.open_id(&id);
    }

    pub fn open_id(&mut self, id: &str) {
        let Some(entry) = self.entries.iter().find(|e| e.id == id) else {
            return;
        };
        let (path, is_dir) = (entry.path.clone(), entry.is_dir);
        if is_dir {
            self.navigate(path);
        } else if let Err(err) = open_external(&path) {
            self.error = Some(err);
        }
    }

    pub fn open_row_menu(&mut self, index: usize, at: PointerPress) {
        let Some(id) = self.row_id(index) else {
            self.menu = None;
            return;
        };
        if !(self.selection.len() > 1 && self.selection.contains(&id)) {
            self.selection.clear();
            self.selection.insert(id.clone());
        }
        self.anchor = Some(id.clone());
        self.focus_id(&id);
        self.menu = Some(at);
    }

    pub fn row_menu_at(&mut self, at: PointerPress) {
        let view = self.rows_view;
        let inside = view[0] <= at.x && at.x <= view[0] + view[2];
        match self.index_at_window((at.x, at.y)).filter(|_| inside) {
            Some(index) => self.open_row_menu(index, at),
            None => self.menu = None,
        }
    }

    pub fn close_menu(&mut self) {
        self.menu = None;
    }

    pub(super) fn entry_snapshot(&self, id: &str) -> Option<(PathBuf, String, bool)> {
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| (entry.path.clone(), entry.name.clone(), entry.is_dir))
    }
}
