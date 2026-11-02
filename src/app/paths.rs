use crate::vfs;
use std::path::{Path, PathBuf};
#[cfg(not(target_arch = "wasm32"))]
use std::process::Command;

// The folder and file named on the command line, or nothing when it names no place to open.
pub(super) fn start_location() -> Option<(PathBuf, Option<String>)> {
    let arg = rune::extra_args().iter().find(|a| !a.starts_with('-'))?;
    let path = absolute(PathBuf::from(arg));
    if vfs::is_file(&path) {
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
    if vfs::is_dir(&path) {
        return Some((path, None));
    }
    None
}

pub(super) fn default_location() -> PathBuf {
    home_dir().unwrap_or_else(cwd)
}

/// `std::env::current_dir` panics on wasm; the web build's tree hangs from `/`.
fn cwd() -> PathBuf {
    #[cfg(target_arch = "wasm32")]
    {
        root_dir()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        std::env::current_dir().unwrap_or_else(|_| root_dir())
    }
}

fn absolute(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        cwd().join(&path)
    }
}

/// A browser tab's home is the demo folder the web build seeds.
#[cfg(target_arch = "wasm32")]
pub fn home_dir() -> Option<PathBuf> {
    Some(vfs::home())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|p| vfs::is_dir(p))
}

/// A browser tab's tree hangs from `/`.
#[cfg(target_arch = "wasm32")]
pub fn root_dir() -> PathBuf {
    PathBuf::from("/")
}

#[cfg(not(target_arch = "wasm32"))]
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

#[cfg(target_arch = "wasm32")]
pub(super) fn open_external(_path: &Path) -> Result<(), String> {
    Err("A browser tab can’t open files from the demo folder.".into())
}

#[cfg(target_arch = "wasm32")]
pub(super) fn reveal_external(_path: &Path) -> Result<(), String> {
    Err("A browser tab has no file manager to reveal in.".into())
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn open_url(_url: &str) -> Result<(), String> {
    Err("A browser tab can’t open links from the demo folder.".into())
}

/// Opens a link a program printed (OSC 8). A URL is handed to the platform's launcher as
/// written, but only for the schemes a terminal's links actually use: a program must not be
/// able to talk the app into launching an arbitrary handler.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn open_url(url: &str) -> Result<(), String> {
    if !["http://", "https://", "mailto:", "file://"]
        .iter()
        .any(|scheme| url.starts_with(scheme))
    {
        return Err(format!("Not a link this app opens: {url}"));
    }
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
    command.arg(url);
    spawn_reaped(command, url)
}

#[cfg(not(target_arch = "wasm32"))]
fn spawn_reaped(mut command: Command, label: &str) -> Result<(), String> {
    // The launched program, and whatever it forks (a browser's zygote, say), must not write to
    // the app's own console: its log lines would land in the terminal the app was started from,
    // and once that pipe closes a stray write surfaces there as "Broken pipe".
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
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

#[cfg(not(target_arch = "wasm32"))]
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
    spawn_reaped(command, &path.display().to_string())
}

/// Shows the item in the platform's file manager, with the item itself chosen where the platform
/// can do that.
#[cfg(not(target_arch = "wasm32"))]
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
        let target = if vfs::is_dir(path) {
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
    #[cfg(not(any(unix, target_os = "windows")))]
    {
        let _ = path;
        Err("This platform has no file manager to reveal in.".into())
    }
}
