//! The selection the grid is showing: the press that started it, the double and triple clicks
//! that widen it, and the text a copy takes. The engine holds the selection itself; this is the
//! pane's side of the same gesture.

use std::time::Instant;

use super::{CLICK_INTERVAL, Terminal};

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
impl Terminal {
    pub fn copy_selection(&mut self) -> Option<String> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut() {
            return engine.selected();
        }
        None
    }

    /// Whether a selection is on the grid, which is all a key that dismisses one has to know.
    pub fn has_selection(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = &self.engine {
            return engine.has_selection();
        }
        false
    }

    /// Every method that moves the selection marks the frame stale. A drag and a select-all come
    /// from the user, not from the shell: no byte arrives to make the poll rebuild the rows, so
    /// without this the wash stays off screen until the next keystroke echoes. A single press
    /// moves nothing — it only anchors the drag that may follow — so it asks for no frame and
    /// says it selected nothing.
    pub fn begin_selection(&mut self, at: (u16, u16), clicks: u8, block: bool) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut() {
            let selected = engine.begin_selection(at, clicks, block);
            self.fresh |= selected;
            return selected;
        }
        let _ = (at, clicks, block);
        false
    }

    pub fn extend_selection(&mut self, at: (u16, u16)) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut() {
            let selected = engine.select_to(at);
            self.fresh |= selected;
            return selected;
        }
        let _ = at;
        false
    }

    pub fn clear_selection(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut() {
            engine.clear_selection();
        }
        // A wash going away is a change to the rows, and it comes from the user: nothing the
        // shell writes will ask for that frame, so clearing asks for it here.
        self.fresh = true;
    }

    /// Selects the whole screen and its history, for a copy that does not need a drag.
    pub fn select_all(&mut self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_ref() {
            let selected = engine.select_all();
            self.fresh |= selected;
            return selected;
        }
        false
    }

    /// How many presses in a row this one is: a second press on the same cell within
    /// [`CLICK_INTERVAL`] is a double click and a third a triple click; a fourth starts over.
    pub fn click_count(&mut self, at: (u16, u16)) -> u8 {
        let now = Instant::now();
        let count = match self.last_press {
            Some((when, cell, count))
                if cell == at && now.saturating_duration_since(when) <= CLICK_INTERVAL =>
            {
                if count >= 3 { 1 } else { count + 1 }
            }
            _ => 1,
        };
        self.last_press = Some((now, at, count));
        count
    }

    /// The cell the pointer is over, which is where a wheel notch goes when the program
    /// captures the mouse: a wheel event carries no position of its own.
    pub fn hovered_cell(&self) -> Option<(u16, u16)> {
        self.hover_cell
    }
}
