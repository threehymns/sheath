//! The folder the window was left in, so the next launch opens there. Machine-owned state in the
//! state directory, ignored when it cannot be read.

use crate::config;
use kdl::{KdlDocument, KdlEntry, KdlNode};
use std::path::{Path, PathBuf};

const FILE: &str = "session.kdl";

#[derive(Default)]
pub struct Session {
    path: Option<PathBuf>,
    folder: Option<PathBuf>,
    // Kind, size and date column widths, in points.
    columns: Option<[f32; 3]>,
    // The terminal pane's height, in points, so it opens at the size it was left.
    pane: Option<f32>,
}

impl Session {
    pub fn load() -> Session {
        let Some(dir) = config::state_dir() else {
            return Session::default();
        };
        Session::at(dir.join(FILE))
    }

    pub fn at(path: PathBuf) -> Session {
        let mut session = Session {
            path: Some(path.clone()),
            ..Session::default()
        };
        let Ok(text) = crate::vfs::read_to_string(&path) else {
            return session;
        };
        let (folder, columns, pane) = parse(&text);
        session.folder = folder;
        session.columns = columns;
        session.pane = pane;
        session
    }

    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref()
    }

    pub fn columns(&self) -> Option<[f32; 3]> {
        self.columns
    }

    pub fn pane(&self) -> Option<f32> {
        self.pane
    }

    /// Remembers the terminal pane's height after a drag, not on every frame of one.
    pub fn remember_pane(&mut self, height: f32) {
        if self.pane == Some(height) {
            return;
        }
        self.pane = Some(height);
        self.save();
    }

    /// Remembers a folder, skipping the write when the file already says so.
    pub fn remember(&mut self, folder: &Path) {
        if self.folder.as_deref() == Some(folder) {
            return;
        }
        self.folder = Some(folder.to_path_buf());
        self.save();
    }

    /// Remembers the column widths after a resize, not on every frame of one.
    pub fn remember_columns(&mut self, columns: [f32; 3]) {
        if self.columns == Some(columns) {
            return;
        }
        self.columns = Some(columns);
        self.save();
    }

    fn save(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        let _ = config::write_atomic(
            &path,
            &render(self.folder.as_deref(), self.columns, self.pane),
        );
    }
}

fn render(folder: Option<&Path>, columns: Option<[f32; 3]>, pane: Option<f32>) -> String {
    let mut doc = KdlDocument::new();
    if let Some(folder) = folder {
        let mut node = KdlNode::new("folder");
        node.push(KdlEntry::new(folder.to_string_lossy().into_owned()));
        doc.nodes_mut().push(node);
    }
    if let Some([kind, size, date]) = columns {
        let mut node = KdlNode::new("columns");
        node.push(KdlEntry::new_prop("kind", kind as f64));
        node.push(KdlEntry::new_prop("size", size as f64));
        node.push(KdlEntry::new_prop("date", date as f64));
        doc.nodes_mut().push(node);
    }
    if let Some(height) = pane {
        let mut node = KdlNode::new("pane");
        node.push(KdlEntry::new(height as f64));
        doc.nodes_mut().push(node);
    }
    doc.autoformat();
    format!("// What the window was last showing. Sheath owns this file.\n{doc}")
}

fn parse(text: &str) -> (Option<PathBuf>, Option<[f32; 3]>, Option<f32>) {
    let Ok(doc) = text.parse::<KdlDocument>() else {
        return (None, None, None);
    };
    let folder = doc
        .nodes()
        .iter()
        .find(|node| node.name().value() == "folder")
        .and_then(|node| node.get(0))
        .and_then(|entry| entry.as_string())
        .map(PathBuf::from);
    let columns = doc
        .nodes()
        .iter()
        .find(|node| node.name().value() == "columns")
        .and_then(|node| {
            let read = |name: &str| node.get(name).and_then(|entry| entry.as_float());
            Some([
                read("kind")? as f32,
                read("size")? as f32,
                read("date")? as f32,
            ])
        });
    let pane = doc
        .nodes()
        .iter()
        .find(|node| node.name().value() == "pane")
        .and_then(|node| node.get(0))
        .and_then(|entry| entry.as_float())
        .map(|height| height as f32);
    (folder, columns, pane)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_folder_survives_a_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        let mut session = Session::at(path.clone());
        assert_eq!(session.folder(), None, "a missing file remembers nothing");

        session.remember(dir.path());
        assert_eq!(Session::at(path).folder(), Some(dir.path()));
    }

    #[test]
    fn the_column_widths_survive_a_reload() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        let mut session = Session::at(path.clone());

        session.remember_columns([150.0, 92.0, 158.0]);
        session.remember(dir.path());

        let loaded = Session::at(path);
        assert_eq!(loaded.columns(), Some([150.0, 92.0, 158.0]));
        assert_eq!(loaded.folder(), Some(dir.path()));
    }

    #[test]
    fn a_damaged_file_remembers_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        std::fs::write(&path, "folder {").unwrap();
        assert_eq!(Session::at(path).folder(), None);
    }
}
