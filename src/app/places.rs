//! The sidebar's places as the browser changes them: reordering a section, adding the rows a
//! drop lets go of, renaming a label, and opening a place. The places themselves and the file
//! that remembers them live in `crate::places`.

use super::App;
use crate::places::Section;
use rune::{PointerPress, TextEdit};
use std::path::PathBuf;

/// The sidebar menu a right-click on a place opens: the place it is about, and where it opened.
pub struct PlaceMenu {
    pub id: String,
    pub press: PointerPress,
}

/// A place whose display name is being edited in its sidebar row.
pub struct PlaceRename {
    pub id: String,
    pub edit: TextEdit,
}

/// The field the renaming row binds to. `SidebarRow::renaming` wants a plain function, so the
/// field is reached through the app's state rather than captured.
pub fn place_rename_edit(s: &mut App) -> &mut TextEdit {
    &mut s
        .place_rename
        .as_mut()
        .expect("only the renaming row binds the field")
        .edit
}

impl App {
    /// Moves a place within its section, the way a drop does, and writes the order down. `from`
    /// and `to` count the rows on screen; a place that is not there now keeps its slot in the
    /// file, so a folder that comes back comes back where it was left.
    pub fn move_place(&mut self, section: Section, from: usize, to: usize) {
        if self.places.reorder(section, from, to) {
            self.places.save();
        }
    }

    /// Inserts the rows `ids` at `index` in `section` as places, the way a drop between two
    /// places does. Nothing moves on disk, and a path that is already a place anywhere is left
    /// alone.
    pub fn add_places(&mut self, section: Section, index: usize, ids: &[String]) {
        let paths: Vec<PathBuf> = ids
            .iter()
            .filter_map(|id| self.entries.iter().find(|entry| &entry.id == id))
            .map(|entry| entry.path.clone())
            .collect();
        if self.places.insert(section, index, &paths) {
            self.places.save();
        }
    }

    /// Opens a place: a folder shows it, and a file shows the folder holding it with the file
    /// selected. A place whose path went away does nothing. A display name still being typed
    /// is kept first, the way a click elsewhere ends an edit.
    pub fn open_place(&mut self, path: PathBuf) {
        self.commit_place_rename();
        if path.is_dir() {
            self.navigate(path);
        } else if path.exists() {
            self.reveal_in_folder(path);
        }
    }

    /// Whether the press now arriving ends a place reorder, which must not also open a folder.
    pub fn take_place_drag(&mut self) -> bool {
        std::mem::take(&mut self.place_dragged)
    }

    /// Opens the place menu at a right-click on the row `id`, keeping a display name still
    /// being typed.
    pub fn open_place_menu(&mut self, id: String, press: PointerPress) {
        self.commit_place_rename();
        self.place_menu = Some(PlaceMenu { id, press });
    }

    /// Starts editing a place's display name from its menu: the row's label becomes a field
    /// with the name selected. Only the label changes; the folder keeps its own name.
    pub fn begin_menu_place_rename(&mut self) {
        let Some(menu) = self.place_menu.take() else {
            return;
        };
        let Some(name) = self.places.get(&menu.id).map(|place| place.name.clone()) else {
            return;
        };
        let mut edit = TextEdit::new(name.clone());
        edit.set_selection(0, name.len());
        edit.focused = true;
        self.place_rename = Some(PlaceRename { id: menu.id, edit });
    }

    /// Ends the edit, keeping what was typed as the sidebar's label. An empty field, or the name
    /// it started with, changes nothing.
    pub fn commit_place_rename(&mut self) {
        let Some(rename) = self.place_rename.take() else {
            return;
        };
        let name = rename.edit.text.trim();
        if !name.is_empty() && self.places.rename(&rename.id, name) {
            self.places.save();
        }
    }

    pub fn cancel_place_rename(&mut self) {
        self.place_rename = None;
    }

    pub fn close_place_menu(&mut self) {
        self.place_menu = None;
    }

    /// The menu's Open: the place's folder, or the folder holding its file.
    pub fn open_menu_place(&mut self) {
        let Some(menu) = self.place_menu.take() else {
            return;
        };
        let Some(path) = self.places.get(&menu.id).map(|place| place.path.clone()) else {
            return;
        };
        self.open_place(path);
    }

    /// The menu's Show in Folder: the folder holding the place, with the place selected.
    pub fn reveal_menu_place(&mut self) {
        let Some(menu) = self.place_menu.take() else {
            return;
        };
        let Some(path) = self.places.get(&menu.id).map(|place| place.path.clone()) else {
            return;
        };
        self.reveal_in_folder(path);
    }

    /// The menu's Remove: the place leaves the sidebar, and the file keeps its row hidden, so
    /// adding the folder again brings back the same row.
    pub fn remove_menu_place(&mut self) {
        let Some(menu) = self.place_menu.take() else {
            return;
        };
        if self.places.remove(&menu.id) {
            self.places.save();
            // The row leaves the layout; a rect left behind must not take a drop.
            self.places_at.remove(&menu.id);
        }
    }

    /// Shows the folder holding `path`, with `path` selected and revealed.
    fn reveal_in_folder(&mut self, path: PathBuf) {
        let Some(parent) = path.parent().map(std::path::Path::to_path_buf) else {
            return;
        };
        let Some(name) = path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            return;
        };
        self.navigate(parent);
        // The rows land with the folder's read, so the row is selected by the read that finds
        // it. A read already in flight applies it when it lands; one that finished here needs
        // another.
        self.pending_select = Some((vec![format!("f:{name}")], true));
        if !self.is_scanning() {
            self.reload();
        }
    }
}
