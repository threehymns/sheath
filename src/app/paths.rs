use std::path::{Path, PathBuf};
use std::process::Command;

// The folder and file named on the command line, or nothing when it names no place to open.
pub(super) fn start_location() -> Option<(PathBuf, Option<String>)> {
    let arg = rune::extra_args().iter().find(|a| !a.starts_with('-'))?;
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
        return Some((parent, Some(name)));
    }
    if path.is_dir() {
        return Some((path, None));
    }
    None
}

pub(super) fn default_location() -> PathBuf {
    home_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| root_dir()))
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
    command.arg(path);
    match command.spawn() {
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

/// Shows the item in the platform's file manager, with the item itself chosen where the platform
/// can do that.
pub(super) fn reveal_external(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let mut command = Command::new("open");
        command.arg("-R").arg(path);
        return spawn_reaped(command, &path.display().to_string());
    }
    #[cfg(target_os = "windows")]
    {
        let mut command = Command::new("explorer");
        command.arg(format!("/select,{}", path.display()));
        return spawn_reaped(command, &path.display().to_string());
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        // A folder opens directly; a file opens the folder that holds it.
        let target = if path.is_dir() {
            path.to_path_buf()
        } else {
            path.parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| path.to_path_buf())
        };
        let mut command = Command::new("xdg-open");
        command.arg(&target);
        spawn_reaped(command, &target.display().to_string())
    }
}
