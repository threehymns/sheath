//! The rename, move and trash sheets: what they ask, and the file operations they apply
//! once confirmed.

use crate::app::App;
use rune::{TextEdit, Tx};
use std::fs;
use std::path::{Path, PathBuf};

use super::*;

/// The modal an action opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetKind {
    Rename,
    Move,
    Trash,
}

/// A rename, move or trash confirmation. `edit` is the field for rename and move, and is
/// empty (and unfocused) for the trash confirmation.
pub struct Sheet {
    pub kind: SheetKind,
    /// The rows the sheet applies to: one for rename, the whole selection for move and trash.
    /// These stay valid while the sheet is open.
    pub targets: Vec<String>,
    /// What the sheet calls them: one name, or a count.
    pub name: String,
    pub edit: TextEdit,
    pub error: Option<String>,
}

impl Sheet {
    /// Whether the sheet covers more than one row.
    pub fn many(&self) -> bool {
        self.targets.len() > 1
    }
}

fn validate_name(name: &str) -> Result<(), String> {
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
    /// Opens the rename sheet with the current name selected. Renaming needs exactly one row.
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

    /// Opens the move sheet for the whole selection, starting at the current folder.
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

    /// Asks for confirmation before putting the selection in the system Trash.
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

    /// What a sheet calls its rows: one name, or a count.
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

    /// Binds the sheet's text field (`TextInput::on_edit`).
    pub fn sheet_edit(&mut self) -> &mut TextEdit {
        &mut self
            .sheet
            .as_mut()
            .expect("the open sheet has a field")
            .edit
    }

    /// Applies the open sheet. A failure keeps the sheet open with its message.
    pub fn submit_sheet(&mut self, tx: &mut Tx) {
        let Some(sheet) = self.sheet.take() else {
            return;
        };
        let result = match sheet.kind {
            SheetKind::Rename => self.rename_entry(&sheet, tx),
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

    /// The sheet's rows that are still in this folder, as (path, name, is folder).
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

    fn rename_entry(&mut self, sheet: &Sheet, tx: &mut Tx) -> Result<(), String> {
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
        self.reload();
        let id = format!("f:{new_name}");
        if self.is_visible(&id) {
            self.select_id(&id);
            tx.scroll_into_view_in("files-table", id);
        }
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

    /// Moves the rows `targets` into `destination`, for the move sheet and for a drop. Every
    /// row is checked before any moves, so a refusal leaves the whole selection where it is.
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
