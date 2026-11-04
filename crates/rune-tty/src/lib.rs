//! A terminal for Rune apps: Ghostty's VT core for the screen, a PTY for the shell, and one
//! streamed texture (or, behind the `text-renderer` feature, Rune text) for the grid.
//!
//! [`Terminal`] is the pane's state, [`surface`] builds the grid's elements from the frame it
//! holds, and a caller wires whatever input it has to those. The engine is native only, so the
//! web build keeps the pane's types and says where the terminal lives.
//!
//! ```no_run
//! use rune_tty::{Terminal, surface};
//! # fn view(t: &Terminal, k: &'static rune_kit::Kit) -> rune::El {
//! let screen = surface::screen_of(t, 1.0, k);
//! # screen
//! # }
//! ```

pub mod fonts;
pub mod grid;
mod pane;
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub mod shapes;

#[cfg(not(target_arch = "wasm32"))]
pub mod engine;
#[cfg(all(not(target_arch = "wasm32"), not(feature = "text-renderer")))]
pub mod raster;
pub mod surface;

pub use surface::{layout_screen, screen_of, terminal_cell, terminal_drag};

#[cfg(any(feature = "text-renderer", target_arch = "wasm32"))]
mod rows;

#[cfg(not(target_arch = "wasm32"))]
use std::process::Command;

#[cfg(not(target_arch = "wasm32"))]
use ghostty_vt::style::RgbColor;

pub use pane::{COLLAPSE_DRAG, MAX_HEIGHT, MIN_HEIGHT, Terminal, split};
pub use pane::{LINE_HEIGHT, MouseAction};

/// The faces the pane draws with: Inter (variable, so every weight) and IBM Plex Mono Regular
/// for the grid, at the weight the theme's `mono` style asks for. The theme also ships Plex Mono
/// Medium, which nothing asks for, and carrying it costs the web build about 157 KB, 55 KB
/// compressed. The order is `rune_theme_opencode::FONTS`: Inter, Plex Mono Regular, Plex Mono
/// Medium, pinned by the Rune revision in Cargo.toml. A Nerd Font installed on the system joins
/// this list at launch as the terminal's fallback for the icons only it draws; see [`fonts`].
pub const FONTS: &[&[u8]] = &[rune_theme_opencode::FONTS[0], rune_theme_opencode::FONTS[2]];

/// The screen's horizontal padding: the pointer's local points include it, a cell does not.
/// Kept small: a terminal wants its columns, and the pane's own edge already separates it.
pub(crate) const PAD: f32 = 6.0;

/// Progress a program reported for its own work (OSC 9;4). The pane's header draws it, so a
/// long build or download shows how far along it is without a prompt of its own.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub kind: ProgressKind,
    /// The percentage the program reported, when it reported one.
    pub percent: Option<u8>,
}

/// Which state a [`Progress`] report is in.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgressKind {
    /// Determinate work.
    Set,
    /// Work with no percentage to show.
    Indeterminate,
    /// Work the program paused.
    Pause,
    /// Work that failed.
    Error,
}

/// The colours the terminal hands libghostty as its defaults: what an unstyled cell draws in,
/// and what `OSC 10`/`11` report. The view paints the same pair, so the grid and the pane agree.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Theme {
    #[cfg(not(target_arch = "wasm32"))]
    pub fg: RgbColor,
    #[cfg(not(target_arch = "wasm32"))]
    pub bg: RgbColor,
    #[cfg(not(target_arch = "wasm32"))]
    pub cursor: RgbColor,
}

/// The theme's terminal colours, taken from the same kit the pane draws with.
#[cfg(not(target_arch = "wasm32"))]
pub fn theme(dark: bool) -> Theme {
    let kit = rune_theme_opencode::kit(dark);
    Theme {
        fg: rgb(kit.c.text_base),
        bg: rgb(kit.c.bg_base),
        cursor: rgb(kit.c.text_accent),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn rgb(color: rune::Color) -> RgbColor {
    let encode = rune::color::srgb_encode;
    let byte = |value: f32| (encode(value) * 255.0).round().clamp(0.0, 255.0) as u8;
    RgbColor {
        r: byte(color.r),
        g: byte(color.g),
        b: byte(color.b),
    }
}

/// Opens a link a program printed (OSC 8). A URL is handed to the platform's launcher as
/// written, but only for the schemes a terminal's links actually use: a program must not be able
/// to talk the viewer into launching an arbitrary handler.
#[cfg(target_arch = "wasm32")]
pub fn open_url(_url: &str) -> Result<(), String> {
    Err("A browser tab can’t open links from the demo folder.".into())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn open_url(url: &str) -> Result<(), String> {
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
    spawn_detached(command, url)
}

/// Starts a program without waiting for it, detached from the viewer's own console.
#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_detached(mut command: Command, label: &str) -> Result<(), String> {
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
