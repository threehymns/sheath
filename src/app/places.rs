//! The sidebar's places as the browser changes them: reordering a section, adding the rows a
//! drop lets go of, and opening a place. The places themselves and the file that remembers
//! them live in `crate::places`.

use super::App;
use crate::places::Section;
use std::path::PathBuf;

impl App {
    /// Moves a place within its section, the way a drop does, and writes the order down. `from`
    /// and `to` count the rows on screen; a place that is not there now keeps its slot in the
    /// file, so a folder that comes back comes back where it was left.
    pub fn move_place(&mut self, section: Section, from: usize, to: usize) {
        if self.places.reorder(section, from, to) {
            self.places.save();
        }
    }

    /// Adds the rows `ids` to `section` as places, the way a drop on its heading does. Nothing
    /// moves on disk, and a path that is already a place anywhere is left alone.
    pub fn add_places(&mut self, section: Section, ids: &[String]) {
        let paths: Vec<PathBuf> = ids
            .iter()
            .filter_map(|id| self.entries.iter().find(|entry| &entry.id == id))
            .map(|entry| entry.path.clone())
            .collect();
        if self.places.add(section, &paths) {
            self.places.save();
        }
    }

    /// Opens a place: a folder shows it, and a file shows the folder holding it with the file
    /// selected. A place whose path went away does nothing.
    pub fn open_place(&mut self, path: PathBuf) {
        if path.is_dir() {
            self.navigate(path);
            return;
        }
        if !path.exists() {
            return;
        }
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
        // The rows land with the folder's read, so the file is selected by the read that finds
        // it. A read already in flight applies it when it lands; one that finished here needs
        // another.
        self.pending_select = Some((vec![format!("f:{name}")], true));
        if !self.is_scanning() {
            self.reload();
        }
    }

    /// Whether the press now arriving ends a place reorder, which must not also open a folder.
    pub fn take_place_drag(&mut self) -> bool {
        std::mem::take(&mut self.place_dragged)
    }
}
