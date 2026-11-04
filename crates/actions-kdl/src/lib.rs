//! The KDL codec for the actions kernel. See `README.md`: it parses and writes blocks, and never
//! touches a file.
pub mod children;
pub mod keybinds;
pub mod settings;

pub use children::{remove_child, write_child};
