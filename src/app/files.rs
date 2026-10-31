use crate::app::App;
use rune::Tx;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Clipboard {
    pub paths: Vec<PathBuf>,
    pub cut: bool,
}

fn free_name(dir: &Path, name: &str) -> String {
    let (stem, extension) = match name.rfind('.') {
        Some(0) | None => (name, ""),
        Some(at) => name.split_at(at),
    };
    let mut candidate = format!("{stem} copy{extension}");
    let mut n = 2;
    while dir.join(&candidate).exists() {
        candidate = format!("{stem} copy {n}{extension}");
        n += 1;
    }
    candidate
}

fn copy_entry(from: &Path, to: &Path) -> io::Result<()> {
    if from.is_dir() {
        fs::create_dir(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy_entry(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        fs::copy(from, to).map(|_| ())
    }
}

impl App {
    pub fn new_folder(&mut self, _tx: &mut Tx) {
        let mut name = "New Folder".to_owned();
        let mut n = 2;
        while self.cwd.join(&name).exists() {
            name = format!("New Folder {n}");
            n += 1;
        }
        let path = self.cwd.join(&name);
        match fs::create_dir(&path) {
            Ok(()) => {
                self.pending_select = Some((vec![format!("f:{name}")], true));
                self.reload();
                self.error = None;
            }
            Err(err) => self.error = Some(format!("Couldn’t create “{name}”: {err}")),
        }
    }

    pub fn copy_selection(&mut self, cut: bool) {
        let paths: Vec<PathBuf> = self
            .selected_ids()
            .iter()
            .filter_map(|id| self.entry_snapshot(id))
            .map(|(path, _, _)| path)
            .collect();
        if paths.is_empty() {
            return;
        }
        self.clipboard = Some(Clipboard { paths, cut });
        self.error = None;
    }

    pub fn is_cut(&self, path: &Path) -> bool {
        self.clipboard
            .as_ref()
            .is_some_and(|clipboard| clipboard.cut && clipboard.paths.iter().any(|p| p == path))
    }

    pub fn paste(&mut self) {
        let Some(clipboard) = self.clipboard.clone() else {
            return;
        };
        let destination = self.cwd.clone();
        let mut made: Vec<String> = Vec::new();
        let mut error = None;
        if clipboard.cut {
            for path in &clipboard.paths {
                if path.parent() == Some(destination.as_path()) {
                    continue;
                }
                let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                    continue;
                };
                let target = destination.join(&name);
                if target.exists() {
                    error = Some(format!(
                        "An item named “{name}” already exists in “{}”.",
                        destination.display()
                    ));
                    break;
                }
                if let Err(err) = fs::rename(path, &target) {
                    error = Some(format!("Couldn’t move “{name}”: {err}"));
                    break;
                }
                made.push(name);
            }
            if error.is_none() {
                self.clipboard = None;
            }
        } else {
            for path in &clipboard.paths {
                let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                    continue;
                };
                let new_name = free_name(&destination, &name);
                match copy_entry(path, &destination.join(&new_name)) {
                    Ok(()) => made.push(new_name),
                    Err(err) => {
                        error = Some(format!("Couldn’t copy “{name}”: {err}"));
                        break;
                    }
                }
            }
        }
        self.finish_paste(made, error);
    }

    pub fn duplicate_selection(&mut self) {
        let mut made = Vec::new();
        let mut error = None;
        for id in self.selected_ids() {
            let Some((path, name, _)) = self.entry_snapshot(&id) else {
                continue;
            };
            let new_name = free_name(&self.cwd, &name);
            match copy_entry(&path, &self.cwd.join(&new_name)) {
                Ok(()) => made.push(new_name),
                Err(err) => {
                    error = Some(format!("Couldn’t duplicate “{name}”: {err}"));
                    break;
                }
            }
        }
        self.finish_paste(made, error);
    }

    fn finish_paste(&mut self, made: Vec<String>, error: Option<String>) {
        self.error = error;
        if !made.is_empty() {
            self.pending_select =
                Some((made.iter().map(|name| format!("f:{name}")).collect(), false));
        }
        self.reload();
    }
}
