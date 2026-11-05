//! The desktop's own icons, one implementation per platform.
//!
//! Freedesktop systems read an icon theme, macOS asks the workspace for a file's icon, and
//! Windows asks the shell for the icon registered for its type. A browser tab has no desktop,
//! so its vendor is the stub in `web`.

#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_os = "macos"),
    not(target_os = "windows")
))]
mod freedesktop;
#[cfg(all(
    not(target_arch = "wasm32"),
    not(target_os = "macos"),
    not(target_os = "windows")
))]
pub use freedesktop::SystemIcons;

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::SystemIcons;

#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "windows")]
pub use windows::SystemIcons;

#[cfg(target_arch = "wasm32")]
mod web;
#[cfg(target_arch = "wasm32")]
pub use web::SystemIcons;
