//! The link the pointer is over (OSC 8), which the pane underlines the way Ghostty shows one.
//! What a link covers on its row is the engine's answer, not the pane's: this only remembers
//! which span the pointer was last over, so a pointer moving within one link draws nothing.

use super::Terminal;

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
impl Terminal {
    /// Tracks the link under the pointer, or clears it when `at` is `None` (the pointer left the
    /// grid). Says whether the answer changed, which is the only thing the view has to know: a
    /// pointer moving across one link is a hit test and no frame.
    pub fn hover(&mut self, at: Option<(u16, u16)>) -> bool {
        // Moving inside one cell is the common case, dozens of times a second: answering it from
        // the last cell is what keeps a pointer crossing the grid from hitting the terminal at
        // all. Only a move to another cell scans the row.
        if at == self.hover_cell {
            return false;
        }
        self.hover_cell = at;
        #[cfg(not(target_arch = "wasm32"))]
        let span = at.and_then(|at| {
            self.engine
                .as_ref()
                .and_then(|engine| engine.hyperlink_span(at))
                .map(|(start, end)| (at.1, start, end))
        });
        #[cfg(target_arch = "wasm32")]
        let span = {
            let _ = at;
            None
        };
        if span == self.hover {
            return false;
        }
        self.hover = span;
        true
    }

    /// The link under the pointer as `(row, first column, last column)`, for the underline the
    /// view draws over those cells.
    pub fn hovered(&self) -> Option<(u16, u16, u16)> {
        self.hover
    }
}
