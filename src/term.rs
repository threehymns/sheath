//! The terminal pane: Ghostty's VT core for the screen, a PTY for the shell, and one streamed
//! texture (or, behind the `text-renderer` feature, Rune text) for the grid. The engine is native
//! only, so the web build keeps the pane's types and says where the terminal lives.

pub(crate) mod grid;
mod pane;
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(crate) mod shapes;

#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod engine;
#[cfg(all(not(target_arch = "wasm32"), not(feature = "text-renderer")))]
pub(crate) mod raster;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) mod window;

#[cfg(not(target_arch = "wasm32"))]
use ghostty_vt::style::RgbColor;

#[cfg(test)]
pub use pane::{COLLAPSE_DRAG, MAX_HEIGHT, MIN_HEIGHT, split};
pub use pane::{MouseAction, Terminal};

/// The colours the terminal hands libghostty as its defaults: what an unstyled cell draws in,
/// and what `OSC 10`/`11` report. The view paints the same pair, so the grid and the pane agree.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
#[derive(Clone, Copy, Debug, Default)]
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
