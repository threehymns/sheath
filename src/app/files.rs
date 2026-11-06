use crate::app::App;
use crate::vfs as fs;
use rune::Tx;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct Clipboard {
    pub paths: Vec<PathBuf>,
    pub cut: bool,
}

fn free_name_with(dir: &Path, name: &str, suffix: &str) -> String {
    let (stem, extension) = match name.rfind('.') {
        Some(0) | None => (name, ""),
        Some(at) => name.split_at(at),
    };
    let mut candidate = format!("{stem} {suffix}{extension}");
    let mut n = 2;
    while fs::exists(&dir.join(&candidate)) {
        candidate = format!("{stem} {suffix} {n}{extension}");
        n += 1;
    }
    candidate
}

fn free_name(dir: &Path, name: &str) -> String {
    free_name_with(dir, name, "copy")
}

/// A path as it would be typed in a shell: under the home folder it starts at `~`.
///
/// Windows shells don't expand a leading tilde, so there the path stays absolute.
#[cfg(not(target_os = "windows"))]
fn copy_path(path: &Path) -> String {
    App::short_path(path)
}

#[cfg(target_os = "windows")]
fn copy_path(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(unix)]
fn make_link(from: &Path, to: &Path) -> io::Result<()> {
    std::os::unix::fs::symlink(from, to)
}

#[cfg(windows)]
fn make_link(from: &Path, to: &Path) -> io::Result<()> {
    if fs::is_dir(from) {
        std::os::windows::fs::symlink_dir(from, to)
    } else {
        std::os::windows::fs::symlink_file(from, to)
    }
}

#[cfg(not(any(unix, windows)))]
fn make_link(_from: &Path, _to: &Path) -> io::Result<()> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "this platform has no links",
    ))
}

fn copy_entry(from: &Path, to: &Path) -> io::Result<()> {
    if fs::is_dir(from) {
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
    /// Archives are read-only: their rows carry paths no write follows, so a command that
    /// changes files stops with a sentence instead of a failed system call.
    pub(super) fn archive_read_only(&mut self) -> bool {
        if self.in_archive() {
            self.error = Some("An archive is read-only. Extract a copy to change it.".into());
            true
        } else {
            false
        }
    }

    pub fn new_file(&mut self, _tx: &mut Tx) {
        if self.archive_read_only() {
            return;
        }
        let mut name = "New File.txt".to_owned();
        let mut n = 2;
        while fs::exists(&self.cwd.join(&name)) {
            name = format!("New File {n}.txt");
            n += 1;
        }
        match fs::write(self.cwd.join(&name), "") {
            Ok(()) => {
                self.pending_select = Some((vec![format!("f:{name}")], true));
                self.reload();
                self.error = None;
            }
            Err(err) => self.error = Some(format!("Couldn’t create “{name}”: {err}")),
        }
    }

    pub fn copy_names(&mut self, tx: &mut Tx) {
        let names: Vec<String> = self
            .selected_ids()
            .iter()
            .filter_map(|id| self.entry_name(id).map(str::to_owned))
            .collect();
        if names.is_empty() {
            return;
        }
        tx.copy(names.join("\n"));
        self.error = None;
    }

    pub fn copy_paths(&mut self, tx: &mut Tx) {
        let paths: Vec<String> = self
            .selected_ids()
            .iter()
            .filter_map(|id| self.entry_snapshot(id))
            .map(|(path, _, _)| copy_path(&path))
            .collect();
        if paths.is_empty() {
            return;
        }
        tx.copy(paths.join("\n"));
        self.error = None;
    }
    pub fn new_folder(&mut self, _tx: &mut Tx) {
        if self.archive_read_only() {
            return;
        }
        let mut name = "New Folder".to_owned();
        let mut n = 2;
        while fs::exists(&self.cwd.join(&name)) {
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
        if self.archive_read_only() {
            return;
        }
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

    // A single chosen folder takes the paste; anything else lands in the folder on screen.
    fn paste_destination(&self) -> PathBuf {
        let ids = self.selected_ids();
        if let [id] = ids.as_slice()
            && let Some((path, _, true)) = self.entry_snapshot(id)
        {
            return path;
        }
        self.cwd.clone()
    }

    pub fn paste(&mut self) {
        if self.archive_read_only() {
            return;
        }
        let Some(clipboard) = self.clipboard.clone() else {
            return;
        };
        let destination = self.paste_destination();
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
                if fs::exists(&target) {
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

    /// Points new names beside the chosen rows at the rows themselves.
    pub fn make_aliases(&mut self) {
        if self.archive_read_only() {
            return;
        }
        let mut made = Vec::new();
        let mut error = None;
        for id in self.selected_ids() {
            let Some((path, name, _)) = self.entry_snapshot(&id) else {
                continue;
            };
            let new_name = free_name_with(&self.cwd, &name, "alias");
            if let Err(err) = make_link(&path, &self.cwd.join(&new_name)) {
                error = Some(format!("Couldn’t alias “{name}”: {err}"));
                break;
            }
            made.push(new_name);
        }
        self.finish_paste(made, error);
    }

    pub fn duplicate_selection(&mut self) {
        if self.archive_read_only() {
            return;
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_under_home_is_copied_with_a_tilde() {
        let Some(home) = crate::app::home_dir() else {
            return;
        };
        let windows = cfg!(target_os = "windows");
        let expected = |path: &Path| {
            if windows {
                path.display().to_string()
            } else {
                "~/Projects/notes.md".to_string()
            }
        };
        let path = home.join("Projects").join("notes.md");
        assert_eq!(copy_path(&path), expected(&path));
        assert_eq!(
            copy_path(&home),
            if windows {
                home.display().to_string()
            } else {
                "~".to_string()
            }
        );
    }

    #[test]
    fn a_path_outside_home_is_copied_whole() {
        let path = Path::new("/somewhere/else/notes.md");
        assert_eq!(copy_path(path), path.display().to_string());
    }
}
