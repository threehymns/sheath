//! Where the app launches, and the system paths and launchers it works with.

use std::path::{Path, PathBuf};
use std::process::Command;

pub(super) fn start_location() -> (PathBuf, Option<String>) {
    if let Some(arg) = rune::extra_args().iter().find(|a| !a.starts_with('-')) {
        let path = absolute(PathBuf::from(arg));
        if path.is_file() {
            let parent = path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(root_dir);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            return (parent, Some(name));
        }
        if path.is_dir() {
            return (path, None);
        }
    }
    (
        home_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| root_dir())),
        None,
    )
}

fn absolute(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(&path))
            .unwrap_or(path)
    }
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
}

pub fn root_dir() -> PathBuf {
    #[cfg(unix)]
    {
        PathBuf::from("/")
    }
    #[cfg(not(unix))]
    {
        home_dir()
            .and_then(|home| home.ancestors().last().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("C:\\"))
    }
}

pub(super) fn open_external(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = Command::new("xdg-open");
    match command.arg(path).spawn() {
        Ok(mut child) => {
            // Reap the launcher so opening many files doesn't accumulate zombies.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(())
        }
        Err(err) => Err(format!("Couldn’t open {}: {err}", path.display())),
    }
}
