//! What the pane is behind by, for the frame-rate report `SHEATH_TERM_STATS` prints.

use std::time::Duration;

use super::Engine;

/// What the pane is doing behind the frame it just drew. `lag` is the honest latency number: a
/// frame showing a screen whose oldest byte has waited 20 seconds is twenty seconds stale, no
/// matter what frame rate the program reports about its own writes.
#[derive(Clone, Copy, Debug)]
pub struct Stats {
    /// Bytes read from the pty and not parsed yet.
    pub queued: usize,
    /// How long the oldest of those has waited.
    pub lag: Duration,
    /// Bytes parsed since the shell started.
    pub parsed: u64,
    /// Time spent in the parser.
    pub parse: Duration,
    /// Time spent reading the screen back out of libghostty into the frame.
    pub readback: Duration,
}

impl Engine {
    pub fn stats(&self) -> Stats {
        Stats {
            queued: self.incoming.len(),
            lag: self.incoming.lag(),
            parsed: self.parsed,
            parse: self.parse_time,
            readback: self.readback_time,
        }
    }
}
