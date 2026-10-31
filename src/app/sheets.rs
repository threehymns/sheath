use crate::app::App;
use rune::{TextEdit, Tx};
use std::fs;
use std::path::{Path, PathBuf};

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetKind {
    Rename,
    RenamePlace,
    Move,
    Trash,
}

pub struct Sheet {
    pub kind: SheetKind,
    pub targets: Vec<String>,
    pub name: String,
    pub edit: TextEdit,
    pub error: Option<String>,
}

impl Sheet {
    pub fn many(&self) -> bool {
        self.targets.len() > 1
    }
}

pub(super) fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Enter a name.".into());
    }
    if name == "." || name == ".." {
        return Err("That name is reserved.".into());
    }
    if name.contains('/') || name.contains('\\') {
        return Err("Names can’t contain / or \\.".into());
    }
    if name.contains('\0') {
        return Err("That name contains an invalid character.".into());
    }
    Ok(())
}

fn destination_path(raw: &str, cwd: &Path) -> Result<PathBuf, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("Enter a destination folder.".into());
    }
    let home = || home_dir().ok_or_else(|| "Your home folder isn’t available.".to_owned());
    let expanded = if raw == "~" {
        home()?
    } else if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\")) {
        home()?.join(rest)
    } else {
        PathBuf::from(raw)
    };
    Ok(if expanded.is_absolute() {
        expanded
    } else {
        cwd.join(expanded)
    })
}

impl App {
    pub fn begin_rename(&mut self) {
        let ids = self.selected_ids();
        let [id] = ids.as_slice() else {
            return;
        };
        let Some(name) = self.entry_name(id).map(str::to_owned) else {
            return;
        };
        let mut edit = TextEdit::new(name.clone());
        edit.set_selection(0, name.len());
        edit.focused = true;
        self.sheet = Some(Sheet {
            kind: SheetKind::Rename,
            targets: vec![id.clone()],
            name,
            edit,
            error: None,
        });
    }

    pub fn begin_move(&mut self) {
        let targets = self.selected_ids();
        let Some(name) = self.sheet_name(&targets) else {
            return;
        };
        let mut edit = TextEdit::new(self.cwd.display().to_string());
        edit.set_selection(0, edit.text.len());
        edit.focused = true;
        self.sheet = Some(Sheet {
            kind: SheetKind::Move,
            targets,
            name,
            edit,
            error: None,
        });
    }

    pub fn begin_trash(&mut self) {
        let targets = self.selected_ids();
        let Some(name) = self.sheet_name(&targets) else {
            return;
        };
        self.sheet = Some(Sheet {
            kind: SheetKind::Trash,
            targets,
            name,
            edit: TextEdit::new(""),
            error: None,
        });
    }

    fn sheet_name(&self, targets: &[String]) -> Option<String> {
        match targets {
            [] => None,
            [id] => self.entry_name(id).map(str::to_owned),
            many => Some(format!("{} items", many.len())),
        }
    }

    pub fn close_sheet(&mut self) {
        self.sheet = None;
    }

    pub fn sheet_edit(&mut self) -> &mut TextEdit {
        &mut self
            .sheet
            .as_mut()
            .expect("the open sheet has a field")
            .edit
    }

    pub fn submit_sheet(&mut self, _tx: &mut Tx) {
        let Some(sheet) = self.sheet.take() else {
            return;
        };
        let result = match sheet.kind {
            SheetKind::Rename => self.rename_entry(&sheet),
            SheetKind::RenamePlace => self.rename_place_on_disk(&sheet),
            SheetKind::Move => self.move_entries(&sheet),
            SheetKind::Trash => self.trash_entries(&sheet),
        };
        if let Err(message) = result {
            let refocus = sheet.kind != SheetKind::Trash;
            let mut sheet = sheet;
            sheet.error = Some(message);
            sheet.edit.focused = refocus;
            self.sheet = Some(sheet);
        }
    }

    fn sheet_entries(&self, sheet: &Sheet) -> Result<Vec<(PathBuf, String, bool)>, String> {
        let items: Vec<_> = sheet
            .targets
            .iter()
            .filter_map(|id| self.entry_snapshot(id))
            .collect();
        if items.is_empty() {
            return Err(if sheet.many() {
                "Those items are no longer in this folder.".into()
            } else {
                "That item is no longer in this folder.".into()
            });
        }
        Ok(items)
    }

    fn rename_entry(&mut self, sheet: &Sheet) -> Result<(), String> {
        let Some(id) = sheet.targets.first() else {
            return Err("That item is no longer in this folder.".into());
        };
        let Some((old_path, old_name, _)) = self.entry_snapshot(id) else {
            return Err("That item is no longer in this folder.".into());
        };
        let new_name = sheet.edit.text.clone();
        validate_name(&new_name)?;
        if new_name == old_name {
            return Ok(());
        }
        let folder = old_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.cwd.clone());
        let new_path = folder.join(&new_name);
        if new_path.exists() {
            let same = fs::canonicalize(&new_path)
                .ok()
                .zip(fs::canonicalize(&old_path).ok())
                .is_some_and(|(a, b)| a == b);
            if !same {
                return Err(format!("An item named “{new_name}” already exists."));
            }
        }
        fs::rename(&old_path, &new_path)
            .map_err(|err| format!("Couldn’t rename “{old_name}”: {err}"))?;
        self.pending_select = Some((vec![format!("f:{new_name}")], true));
        self.reload();
        self.error = None;
        Ok(())
    }

    fn move_entries(&mut self, sheet: &Sheet) -> Result<(), String> {
        self.sheet_entries(sheet)?;
        let targets: Vec<String> = sheet
            .targets
            .iter()
            .filter(|id| self.entry_snapshot(id).is_some())
            .cloned()
            .collect();
        let destination = destination_path(&sheet.edit.text, &self.cwd)?;
        self.move_targets(&targets, &destination)
    }

    pub(super) fn move_targets(
        &mut self,
        targets: &[String],
        destination: &Path,
    ) -> Result<(), String> {
        let items: Vec<(PathBuf, String, bool)> = targets
            .iter()
            .filter_map(|id| self.entry_snapshot(id))
            .collect();
        if items.is_empty() {
            return Err(if targets.len() > 1 {
                "Those items are no longer in this folder.".into()
            } else {
                "That item is no longer in this folder.".into()
            });
        }
        let destination = fs::canonicalize(destination)
            .map_err(|_| format!("“{}” isn’t a folder.", destination.display()))?;
        if !destination.is_dir() {
            return Err(format!("“{}” isn’t a folder.", destination.display()));
        }
        let many = items.len() > 1;
        for (path, name, is_dir) in &items {
            if *is_dir {
                let source = fs::canonicalize(path).unwrap_or_else(|_| path.clone());
                if destination.starts_with(&source) {
                    return Err(if many {
                        format!("The folder “{name}” can’t move into itself or one of its folders.")
                    } else {
                        "A folder can’t move into itself or one of its folders.".into()
                    });
                }
            }
            if destination.join(name).exists() {
                return Err(format!(
                    "An item named “{name}” already exists in “{}”.",
                    destination.display()
                ));
            }
        }
        for (path, name, _) in &items {
            fs::rename(path, destination.join(name))
                .map_err(|err| format!("Couldn’t move “{name}”: {err}"))?;
        }
        self.reload();
        self.error = None;
        Ok(())
    }

    fn trash_entries(&mut self, sheet: &Sheet) -> Result<(), String> {
        let items = self.sheet_entries(sheet)?;
        for (path, name, _) in &items {
            trash::delete(path)
                .map_err(|err| format!("Couldn’t move “{name}” to the Trash: {err}"))?;
        }
        self.reload();
        self.error = None;
        Ok(())
    }
}
