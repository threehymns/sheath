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
            folder: None,
        };
        let Ok(text) = std::fs::read_to_string(&path) else {
            return session;
        };
        session.folder = parse(&text);
        session
    }

    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref()
    }

    /// Remembers a folder, skipping the write when the file already says so.
    pub fn remember(&mut self, folder: &Path) {
        if self.folder.as_deref() == Some(folder) {
            return;
        }
        self.folder = Some(folder.to_path_buf());
        let Some(path) = self.path.clone() else {
            return;
        };
        let _ = config::write_atomic(&path, &render(folder));
    }
}

fn render(folder: &Path) -> String {
    let mut doc = KdlDocument::new();
    let mut node = KdlNode::new("folder");
    node.push(KdlEntry::new(folder.to_string_lossy().into_owned()));
    doc.nodes_mut().push(node);
    doc.autoformat();
    format!("// The folder the window was last in. Sheath owns this file.\n{doc}")
}

fn parse(text: &str) -> Option<PathBuf> {
    let doc: KdlDocument = text.parse().ok()?;
    doc.nodes()
        .iter()
        .find(|node| node.name().value() == "folder")
        .and_then(|node| node.get(0))
        .and_then(|entry| entry.as_string())
        .map(PathBuf::from)
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
    fn a_damaged_file_remembers_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(FILE);
        std::fs::write(&path, "folder {").unwrap();
        assert_eq!(Session::at(path).folder(), None);
    }
}
