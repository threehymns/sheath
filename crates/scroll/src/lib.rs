//! A scroll area's physics. See `README.md`: behaviour only, `rune` only.
mod area;
mod areas;
mod scroll;

pub use area::{Host, area};
pub use areas::{Areas, Progress};
pub use scroll::{Limits, Scroll, Step};
