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

/// Opens a terminal with its working folder set here. Linux tries `$TERMINAL`, then the common
/// emulators; the first one that launches wins.
pub(super) fn open_terminal(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        command.arg("-a").arg("Terminal").arg(path);
        return spawn_reaped(command, "Terminal");
    }
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("cmd");
        command
            .args(["/C", "start", "", "cmd", "/K", "cd", "/d"])
            .arg(path);
        return spawn_reaped(command, "Command Prompt");
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        let mut failures = Vec::new();
        for program in terminals() {
            let mut command = Command::new(&program);
            command.current_dir(path);
            match command.spawn() {
                Ok(mut child) => {
                    std::thread::spawn(move || {
                        let _ = child.wait();
                    });
                    return Ok(());
                }
                Err(err) => failures.push(format!("{program}: {err}")),
            }
        }
        Err(format!(
            "Couldn’t open a terminal here. {}",
            failures.join("; ")
        ))
    }
}

#[cfg(all(unix, not(target_os = "macos")))]
fn terminals() -> Vec<String> {
    let mut out = Vec::new();
    if let Some(program) = std::env::var_os("TERMINAL") {
        let program = program.to_string_lossy().trim().to_owned();
        if !program.is_empty() {
            out.push(program);
        }
    }
    for program in [
        "x-terminal-emulator",
        "gnome-terminal",
        "konsole",
        "xfce4-terminal",
        "alacritty",
        "kitty",
        "foot",
        "xterm",
    ] {
        out.push(program.to_owned());
    }
    out
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn spawn_reaped(mut command: Command, label: &str) -> Result<(), String> {
    match command.spawn() {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(())
        }
        Err(err) => Err(format!("Couldn’t open {label}: {err}")),
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
            // Reap the launcher so opening many files doesn't collect zombies.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(())
        }
        Err(err) => Err(format!("Couldn’t open {}: {err}", path.display())),
    }
}
