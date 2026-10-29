//! Where the app's config files live. Each kind of setting has its own file: keybinds go in
//! `keybinds.kdl`, everything else the app saves goes in `config.kdl`.

use std::path::PathBuf;

/// The folder the config files go in: `SHEATH_CONFIG_DIR` when set, else the platform's config
/// folder.
pub fn dir() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("SHEATH_CONFIG_DIR") {
        return Some(PathBuf::from(dir));
    }
    #[cfg(target_os = "macos")]
    {
        std::env::var_os("HOME")
            .map(|home| PathBuf::from(home).join("Library/Application Support/Sheath"))
    }
    #[cfg(target_os = "windows")]
    {
        std::env::var_os("APPDATA").map(|dir| PathBuf::from(dir).join("Sheath"))
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
            .map(|dir| dir.join("sheath"))
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    {
        None
    }
}

/// The path of `file` in the config folder, when there is one.
pub fn path(file: &str) -> Option<PathBuf> {
    dir().map(|dir| dir.join(file))
}
