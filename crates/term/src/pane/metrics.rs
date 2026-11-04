//! The pane's cell: its size, read from the mono font itself, and the grid that size fits in
//! the rect the pane was laid out in. A drag reflows through the same measurement, so the shell
//! is always told the columns and rows the rows are drawn at.

use super::Terminal;

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
impl Terminal {
    /// Reads the cell from the mono font itself, the way a terminal does: the advance is the
    /// cell's width and the font's own line box its height, both rounded to whole device pixels.
    /// The theme's line height is a paragraph measure, taller than the glyphs, and a fractional
    /// cell puts every glyph edge on a fractional pixel, which leaves a hairline wherever two
    /// cell-filling glyphs (a block, a box rule) meet.
    ///
    /// It runs on every poll, over the style the rows are drawn with, so changing the font (or a
    /// window moving to a display at another scale) takes effect without a restart.
    pub fn measure(&mut self, style: rune::TextStyle, scale: f32) {
        if self.fixed {
            return;
        }
        let natural = style.lh(0.0);
        let advance = rune::text::advance("M", natural);
        let (_, box_height) = rune::text::measure("M", &[], natural, None);
        if advance <= 1.0 || box_height <= 4.0 {
            return;
        }
        let scale = if scale > 0.0 { scale } else { 1.0 };
        self.scale = scale;
        self.cell = (advance * scale).round().max(1.0) / scale;
        self.line = (box_height * scale).round().max(1.0) / scale;
        // The shaper advances by the font's own width; tracking it to the pixel-aligned cell is
        // how the glyphs land on the same grid the pane draws and points at. Only the text
        // renderer shapes again, so the raster build does not carry it.
        #[cfg(any(feature = "text-renderer", target_arch = "wasm32"))]
        {
            self.tracking = self.cell - advance;
        }
    }

    /// The style the rows are drawn in: the font's own line box, tracked to the pixel-aligned
    /// cell so a glyph's advance and a cell are the same width.
    #[cfg(any(feature = "text-renderer", target_arch = "wasm32"))]
    pub fn text_style(&self, style: rune::TextStyle) -> rune::TextStyle {
        style.lh(self.line).tracking(self.tracking)
    }

    /// The screen's rect, from its layout. The grid is sized from it, so a pane drag reflows
    /// the shell as it moves.
    pub fn layout(&mut self, rect: [f32; 4], cell: f32) {
        self.rect = rect;
        if cell > 1.0 {
            self.cell = cell;
        }
    }

    pub fn grid_size(&self) -> (u16, u16) {
        let cols = (self.rect[2] / self.cell).floor();
        let rows = (self.rect[3] / self.line).floor();
        (cols.max(2.0) as u16, rows.max(1.0) as u16)
    }
}
