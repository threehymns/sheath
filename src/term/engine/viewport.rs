//! Where the viewport sits: the rows of history it is drawn at, the overscan rows a fractional
//! scroll shows at the edges, and the grid's size. The pane owns the fractional part; this owns
//! the whole rows libghostty is told about.

use ghostty_vt::terminal::ScrollViewport;
use portable_pty::PtySize;

use super::Engine;
use crate::term::shapes::Metrics;

impl Engine {
    /// The rows just above and just below the viewport, fetched when a fractional scroll shows
    /// part of a row at an edge: the viewport is moved one row each way, the edge row read, and
    /// put back.
    pub fn ensure_edges(&mut self, needed: bool) {
        if !needed {
            let had = self.lead.take().is_some() | self.trail.take().is_some();
            self.edges_at = None;
            self.force_full |= had;
            return;
        }
        if self.edges_at == Some(self.applied) {
            return;
        }
        self.edges_at = Some(self.applied);
        self.fetch_edges();
    }

    /// Reads the two rows, with the viewport moved one row each way and put back. `edges_at` is
    /// the caller's to set: it says which offset these two belong to.
    pub(super) fn fetch_edges(&mut self) {
        self.lead = None;
        self.trail = None;
        self.vt.scroll_viewport(ScrollViewport::Delta(-1));
        self.lead = self.read_edge(false);
        self.vt.scroll_viewport(ScrollViewport::Delta(1));
        // The row below the last line, which is only there when the viewport is scrolled back.
        if self.applied > 0 {
            self.vt.scroll_viewport(ScrollViewport::Delta(1));
            self.trail = self.read_edge(true);
            self.vt.scroll_viewport(ScrollViewport::Delta(-1));
        }
        // The render state was updated a row away from the viewport, so the next fill reads it
        // all rather than trusting the dirty rows it kept from the wrong offset.
        self.force_full = true;
    }

    pub fn resize(&mut self, cols: u16, rows: u16, cell: (f32, f32), scale: f32) -> bool {
        let cols = cols.max(2);
        let rows = rows.max(1);
        let scale = if scale > 0.0 { scale } else { 1.0 };
        let metrics = Metrics {
            cell: cell.0.max(1.0),
            line: cell.1.max(1.0),
            scale,
        };
        if cols == self.cols && rows == self.rows && self.metrics == metrics {
            return false;
        }
        self.cols = cols;
        self.rows = rows;
        self.cell = (metrics.cell as u32, metrics.line as u32);
        // A cell that moved by less than a point, or a display at another scale, changes the
        // shapes even though the grid's size is the same, so every row is read again.
        self.metrics = metrics;
        let _ = self.vt.resize(cols, rows, self.cell.0, self.cell.1);
        if let Some(master) = self.master.as_mut() {
            let _ = master.resize(PtySize {
                rows,
                cols,
                pixel_width: (cols as f32 * cell.0) as u16,
                pixel_height: (rows as f32 * cell.1) as u16,
            });
        }
        self.applied = 0;
        self.anchor = None;
        self.lead = None;
        self.trail = None;
        self.edges_at = None;
        self.force_full = true;
        self.keys_stale = true;
        self.history = self.vt.scrollback_rows().unwrap_or(0);
        true
    }

    /// Rows of history above the viewport.
    pub fn scrollback(&self) -> usize {
        self.history
    }

    /// Rows of history the viewport is scrolled back by, which a search moves from its side.
    pub fn scroll_position(&self) -> usize {
        self.applied
    }

    /// Puts the viewport `back` rows above the bottom, leaving the fraction of a row to the
    /// pane's own offset.
    pub fn set_scroll(&mut self, back: usize) -> bool {
        let back = back.min(self.scrollback());
        if back == self.applied {
            return false;
        }
        let delta = self.applied as isize - back as isize;
        self.vt.scroll_viewport(ScrollViewport::Delta(delta));
        self.applied = back;
        self.edges_at = None;
        self.force_full = true;
        true
    }
}
