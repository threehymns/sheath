//! The buffers the engine reuses between frames: the bytes one parse step hands libghostty, the
//! replies the program is owed, and the pointer event with the bytes it encodes to. A frame is a
//! round trip and a drag is one mouse event per cell crossed, so neither path can afford to
//! allocate a fresh buffer for every step.

use ghostty_vt::mouse;

pub(super) struct Buffers {
    /// The bytes one parse step hands libghostty.
    pub parse: Vec<u8>,
    /// The replies the program asked for while those bytes were parsed.
    pub replies: Vec<u8>,
    /// The pointer event, patched in place for every press, move and wheel notch.
    pub pointer: mouse::Event,
    /// The bytes that event encodes to.
    pub pointer_bytes: Vec<u8>,
}

impl Buffers {
    pub(super) fn new() -> Result<Buffers, ghostty_vt::Error> {
        Ok(Buffers {
            parse: Vec::new(),
            replies: Vec::new(),
            pointer: mouse::Event::new()?,
            pointer_bytes: Vec::new(),
        })
    }
}
