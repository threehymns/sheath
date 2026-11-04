//! The palette, keybind editor and settings rows for the `actions` kernel. See `README.md`:
//! generic over the app's action type, and no application state.
pub mod keybind;
pub mod keycaps;
pub mod palette;
pub mod settings;

pub use keybind::{KeybindHost, page as keybind_page};
pub use palette::{Palette, PaletteHost, layers as palette_layers};
