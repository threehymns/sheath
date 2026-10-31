use super::sheets::validate_name;
use super::{App, Sheet, SheetKind};
use crate::places::Section;
use rune::{PointerPress, TextEdit};
use std::path::{Path, PathBuf};

pub struct PlaceMenu {
    pub id: String,
    pub press: PointerPress,
}

pub struct PlaceRename {
    pub id: String,
    pub edit: TextEdit,
}

// `SidebarRow::renaming` takes a plain function, so the field is reached through the app.
pub fn place_rename_edit(s: &mut App) -> &mut TextEdit {
    &mut s
        .place_rename
        .as_mut()
        .expect("only the renaming row binds the field")
        .edit
}

impl App {
    pub fn move_place(&mut self, section: Section, from: usize, to: usize) {
        if self.places.reorder(section, from, to) {
            self.places.save();
        }
    }

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

    pub fn open_place(&mut self, path: PathBuf) {
        self.commit_place_rename();
        if path.is_dir() {
            self.navigate(path);
        } else if path.exists() {
            self.reveal_in_folder(path);
        }
    }

    pub fn take_place_drag(&mut self) -> bool {
        std::mem::take(&mut self.place_dragged)
    }

    pub fn open_place_menu(&mut self, id: String, press: PointerPress) {
        self.commit_place_rename();
        self.place_menu = Some(PlaceMenu { id, press });
    }

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

    pub fn begin_menu_place_rename_on_disk(&mut self) {
        let Some(menu) = self.place_menu.take() else {
            return;
        };
        let Some(place) = self.places.get(&menu.id) else {
            return;
        };
        let Some(name) = place
            .path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
        else {
            return;
        };
        let mut edit = TextEdit::new(name.clone());
        edit.set_selection(0, name.len());
        edit.focused = true;
        self.sheet = Some(Sheet {
            kind: SheetKind::RenamePlace,
            targets: vec![menu.id],
            name,
            edit,
            error: None,
        });
    }

    pub fn rename_place_on_disk(&mut self, sheet: &Sheet) -> Result<(), String> {
        let Some(id) = sheet.targets.first() else {
            return Err("That place is no longer in the sidebar.".into());
        };
        let old_path = self
            .places
            .get(id)
            .map(|place| place.path.clone())
            .ok_or_else(|| "That place is no longer in the sidebar.".to_owned())?;
        let missing = || "That place can’t be renamed on disk.".to_owned();
        let old_name = old_path
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .ok_or_else(missing)?;
        let folder = old_path
            .parent()
            .map(Path::to_path_buf)
            .ok_or_else(missing)?;
        let new_name = sheet.edit.text.trim().to_owned();
        validate_name(&new_name)?;
        if new_name == old_name {
            return Ok(());
        }
        let new_path = folder.join(&new_name);
        if new_path.exists() {
            let same = std::fs::canonicalize(&new_path)
                .ok()
                .zip(std::fs::canonicalize(&old_path).ok())
                .is_some_and(|(a, b)| a == b);
            if !same {
                return Err(format!("An item named “{new_name}” already exists here."));
            }
        }
        std::fs::rename(&old_path, &new_path)
            .map_err(|err| format!("Couldn’t rename “{old_name}”: {err}"))?;
        let old_id = id.clone();
        self.places.move_path(&old_path, &new_path);
        self.places.save();
        self.places_at.remove(&old_id);
        let under_old = self.cwd.strip_prefix(&old_path).map(Path::to_path_buf);
        match under_old {
            Ok(rest) => self.navigate(new_path.join(rest)),
            Err(_) if folder == self.cwd => {
                self.pending_select = Some((vec![format!("f:{new_name}")], true));
                if !self.is_scanning() {
                    self.reload();
                }
            }
            Err(_) => {}
        }
        for step in &mut self.history {
            if let Ok(rest) = step.strip_prefix(&old_path) {
                *step = new_path.join(rest);
            }
        }
        self.error = None;
        Ok(())
    }

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

    pub fn open_menu_place(&mut self) {
        let Some(menu) = self.place_menu.take() else {
            return;
        };
        let Some(path) = self.places.get(&menu.id).map(|place| place.path.clone()) else {
            return;
        };
        self.open_place(path);
    }

    pub fn reveal_menu_place(&mut self) {
        let Some(menu) = self.place_menu.take() else {
            return;
        };
        let Some(path) = self.places.get(&menu.id).map(|place| place.path.clone()) else {
            return;
        };
        self.reveal_in_folder(path);
    }

    pub fn remove_menu_place(&mut self) {
        let Some(menu) = self.place_menu.take() else {
            return;
        };
        if self.places.remove(&menu.id) {
            self.places.save();
            self.places_at.remove(&menu.id);
        }
    }

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
        self.pending_select = Some((vec![format!("f:{name}")], true));
        if !self.is_scanning() {
            self.reload();
        }
    }
}
