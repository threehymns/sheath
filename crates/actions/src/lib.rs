//! The keymap kernel: what an action is, which chord runs it, and how a shortcut is recorded.
//! No files, no widgets, no app types. See `README.md`.
mod action;
mod declare;
mod keybinds;
mod recorder;
mod search;
mod setting;

pub use action::Action;
pub use keybinds::{Keybinds, default_shortcut};
pub use recorder::{Outcome, Recorder};
pub use search::matching;
pub use setting::Setting;
// Re-exported so a codec can name a chord without depending on the framework itself.
pub use rune::Shortcut;
