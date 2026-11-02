//! The terminal grid as one streamed RGBA image instead of one text element per row and one
//! rectangle per background run. Cells are composited on the CPU from a glyph atlas and handed
//! to Rune as a [`Pixels`] buffer, which it uploads by dirty rows.
//!
//! It is the native renderer. The text-row renderer is a build-time fallback
//! (`--features text-renderer`) and the only one on the web, so a gap here does not reach the
//! default build. The image always carries one blank overscan row above and below the viewport,
//! which is what lets a fractional scroll show the next row entering at the edge instead of a
//! strip of background.
//!
//! Wide characters still advance one column per code point, so a CJK glyph can overlap the cell
//! after it. The text renderer remains the reference for that until the cell widths are carried
//! on the frame.

mod atlas;
mod paint;

#[cfg(test)]
mod tests;

use std::cell::RefCell;

use rune::Pixels;
use rune::color::srgb_encode;

use crate::term::grid::{Frame, Line};

use atlas::Atlas;
use paint::{Surface, glyph_size, paint_line};

#[derive(Clone, Copy)]
pub(crate) struct Palette {
    pub bg: [u8; 3],
    pub cursor: [u8; 3],
    /// Alpha, 0-255, of the cursor over the cell. A focused pane's cursor is stronger than an
    /// unfocused one's, matching the text path's two fills.
    pub cursor_alpha: u8,
    pub selection: [u8; 3],
}

/// A colour as sRGB bytes, the way the texture holds pixels.
pub(crate) fn rgb(color: rune::Color) -> [u8; 3] {
    let byte = |value: f32| (srgb_encode(value) * 255.0).round().clamp(0.0, 255.0) as u8;
    [byte(color.r), byte(color.g), byte(color.b)]
}

struct Raster {
    atlas: Atlas,
}

thread_local! {
    static CACHE: RefCell<Option<(Raster, Pixels)>> = const { RefCell::new(None) };
}

/// The visible grid as an RGBA image, at device pixels, with one overscan row above and below.
/// Its top is one row above the viewport: the view hangs it there and lets the clip cut it.
/// `None` until a font and a size are known.
pub(crate) fn picture(
    frame: &Frame,
    cell: f32,
    line: f32,
    scale: f32,
    palette: Palette,
) -> Option<Pixels> {
    if frame.cols == 0 || frame.rows == 0 {
        return None;
    }
    CACHE.with(|slot| {
        let mut slot = slot.borrow_mut();
        let (raster, pixels) = slot.get_or_insert_with(|| (Raster::new(), Pixels::new()));
        let (width, height, rgba) = raster.compose(frame, cell, line, scale, palette)?;
        pixels.set(width, height, rgba);
        Some(pixels.clone())
    })
}

impl Raster {
    fn new() -> Raster {
        Raster {
            atlas: Atlas::new(),
        }
    }

    fn compose(
        &mut self,
        frame: &Frame,
        cell: f32,
        line: f32,
        scale: f32,
        palette: Palette,
    ) -> Option<(u32, u32, Vec<u8>)> {
        let cell_px = (cell * scale).round().max(1.0) as u32;
        let line_px = (line * scale).round().max(1.0) as u32;
        let width = u32::from(frame.cols) * cell_px;
        let row_count = u32::from(frame.rows) + 2;
        let height = row_count * line_px;
        if width == 0 || height == 0 {
            return None;
        }
        // Untouched pixels stay transparent: the pane already fills its background, so the
        // image only has to carry what is drawn on top of it. Writing the default background
        // into every pixel would touch all 40 MB of a fullscreened grid each frame, which costs
        // more than all the glyphs together.
        let mut rgba = vec![0u8; (width * height * 4) as usize];
        let size = glyph_size(line_px);
        // The atlas is the only mutable state, so it is filled once and then read by the row
        // workers. Spawning them per frame is cheap next to the blending, and a grid this size
        // is what makes the difference between a 35 ms and a 10 ms compose.
        self.atlas.preload(frame, size);
        let atlas = &self.atlas;
        let mut grid: Vec<Option<&Line>> = Vec::with_capacity(row_count as usize);
        grid.push(frame.lead.as_ref());
        for index in 0..usize::from(frame.rows) {
            grid.push(frame.lines.get(index));
        }
        grid.push(frame.trail.as_ref());
        let row_bytes = width as usize * 4 * line_px as usize;
        let workers = if grid.len() < 8 {
            1
        } else {
            std::thread::available_parallelism()
                .map(|count| count.get())
                .unwrap_or(1)
                .min(grid.len())
        };
        if workers <= 1 {
            for (pixels, line) in rgba.chunks_mut(row_bytes).zip(&grid) {
                let Some(line) = line else { continue };
                let mut surface = Surface::new(pixels, width, line_px, palette.bg);
                paint_line(atlas, &mut surface, line, cell_px, line_px, palette);
            }
        } else {
            let per = grid.len().div_ceil(workers);
            std::thread::scope(|scope| {
                for (band, pixels) in rgba.chunks_mut(per * row_bytes).enumerate() {
                    let first = band * per;
                    let band = &grid[first..(first + per).min(grid.len())];
                    scope.spawn(move || {
                        for (pixels, line) in pixels.chunks_mut(row_bytes).zip(band) {
                            let Some(line) = line else { continue };
                            let mut surface = Surface::new(pixels, width, line_px, palette.bg);
                            paint_line(atlas, &mut surface, line, cell_px, line_px, palette);
                        }
                    });
                }
            });
        }
        if let Some((x, y)) = frame.cursor
            && x < frame.cols
            && y < frame.rows
        {
            let at = (usize::from(y) + 1) * row_bytes;
            let mut surface =
                Surface::new(&mut rgba[at..at + row_bytes], width, line_px, palette.bg);
            // A program that set a cursor colour with OSC 12 draws it in that, not in the
            // theme's; the theme's is only the colour the shell starts with.
            let color = frame.cursor_color.map(rgb).unwrap_or(palette.cursor);
            surface.cursor(
                x,
                cell_px,
                line_px,
                color,
                palette.cursor_alpha,
                frame.cursor_shape,
            );
        }
        Some((width, height, rgba))
    }
}
