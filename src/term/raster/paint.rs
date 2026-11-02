//! Painting one grid row into an RGBA band: selection, backgrounds, cell-filling shapes, glyphs
//! and the rules a pen asks for, plus the pixel fills, blends and mask blits they are made of.
//! The band is one image row, so the painter has no idea where the row sits in the grid.

use super::atlas::{Atlas, Glyph};
use super::{Palette, rgb};
use crate::term::grid::{Line, Pen};

/// The glyph's em box as a fraction of the row, and the baseline's place down the row. Both are
/// the same numbers the text path's font box produces at the sizes the pane measures.
const GLYPH_SIZE: f32 = 0.78;
const BASELINE: f32 = 0.8;

/// How far a faux italic leans the top of a glyph to the right, in pixels per row.
const ITALIC_SHEAR: f32 = 0.22;

/// The size a row's glyphs are rasterized at.
pub(super) fn glyph_size(line_px: u32) -> u32 {
    (line_px as f32 * GLYPH_SIZE).round().max(4.0) as u32
}

/// One row's band of the image, with the geometry every fill needs.
pub(super) struct Surface<'a> {
    dst: &'a mut [u8],
    width: u32,
    height: u32,
    /// What an untouched pixel would show: the pane's fill behind the image. Flattening a
    /// translucent glyph or wash against it keeps every written pixel opaque.
    bg: [u8; 3],
}

/// A rectangle in image pixels.
struct Rect {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}

impl<'a> Surface<'a> {
    pub(super) fn new(dst: &'a mut [u8], width: u32, height: u32, bg: [u8; 3]) -> Surface<'a> {
        Surface {
            dst,
            width,
            height,
            bg,
        }
    }

    /// Draws the cursor cell as a wash over what is already in it.
    pub(super) fn cursor(
        &mut self,
        cell: u16,
        cell_px: u32,
        line_px: u32,
        color: [u8; 3],
        alpha: u8,
    ) {
        self.blend(
            Rect {
                x0: i32::from(cell) * cell_px as i32,
                y0: 0,
                x1: (i32::from(cell) + 1) * cell_px as i32,
                y1: line_px as i32,
            },
            color,
            alpha,
        );
    }

    /// Fills a rectangle with an opaque colour, clamped to the image.
    fn fill(&mut self, rect: Rect, color: [u8; 3]) {
        let x0 = rect.x0.clamp(0, self.width as i32);
        let x1 = rect.x1.clamp(0, self.width as i32);
        let y0 = rect.y0.max(0);
        let y1 = rect.y1.max(0);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        let pixel = [color[0], color[1], color[2], 255];
        let span = (x1 - x0) as usize * 4;
        for y in y0..y1 {
            let at = ((y as u32 * self.width + x0 as u32) * 4) as usize;
            if at + span <= self.dst.len() {
                fill_span(&mut self.dst[at..at + span], pixel);
            }
        }
    }

    /// Blends a colour over a rectangle at `alpha` (0-255), clamped to the image.
    fn blend(&mut self, rect: Rect, color: [u8; 3], alpha: u8) {
        let x0 = rect.x0.clamp(0, self.width as i32);
        let x1 = rect.x1.clamp(0, self.width as i32);
        let y0 = rect.y0.clamp(0, self.height as i32);
        let y1 = rect.y1.clamp(0, self.height as i32);
        if x0 >= x1 || y0 >= y1 {
            return;
        }
        for y in y0..y1 {
            let start = ((y as u32 * self.width + x0 as u32) * 4) as usize;
            let end = ((y as u32 * self.width + x1 as u32) * 4) as usize;
            for pixel in self.dst[start..end].as_chunks_mut::<4>().0 {
                over(pixel, color, alpha, self.bg);
            }
        }
    }

    /// Draws a glyph mask in `color`, sheared to the right at the top for a faux italic. `x`
    /// and `y` are the top-left of the image, already including the placement.
    fn blit(&mut self, x: i32, y: i32, glyph: &Glyph, color: [u8; 3], shear: f32) {
        for row in 0..glyph.height {
            let py = y + row as i32;
            if py < 0 || py >= self.height as i32 {
                continue;
            }
            let dx = if shear == 0.0 {
                0
            } else {
                ((glyph.height - row) as f32 * shear).round() as i32
            };
            let left = x + dx;
            let skip = (-left).clamp(0, glyph.width as i32) as u32;
            let visible = (glyph.width - skip).min((self.width as i32 - left.max(0)).max(0) as u32);
            if visible == 0 {
                continue;
            }
            let mask_start = (row * glyph.width + skip) as usize;
            let mask = &glyph.mask[mask_start..mask_start + visible as usize];
            let at = ((py as u32 * self.width + (left + skip as i32) as u32) * 4) as usize;
            let line = &mut self.dst[at..at + visible as usize * 4];
            for (pixel, &alpha) in line.as_chunks_mut::<4>().0.iter_mut().zip(mask) {
                over(pixel, color, alpha, self.bg);
            }
        }
    }
}

/// Paints one grid row: the selection, backgrounds, cell-filling shapes, then the glyphs and the
/// rules the pen asks for.
pub(super) fn paint_line(
    atlas: &Atlas,
    surface: &mut Surface,
    line: &Line,
    cell_px: u32,
    line_px: u32,
    palette: Palette,
) {
    let bottom = line_px as i32;
    // The selection sits under the cell backgrounds, the order the text path paints them in: a
    // selected cell that also carries a background shows the background.
    if let Some((start, end)) = line.selection {
        surface.blend(
            Rect {
                x0: i32::from(start) * cell_px as i32,
                y0: 0,
                x1: (i32::from(end) + 1) * cell_px as i32,
                y1: bottom,
            },
            palette.selection,
            61,
        );
    }
    for run in &line.runs {
        let Some(bg) = run.pen.bg else { continue };
        surface.fill(
            Rect {
                x0: i32::from(run.cols.0) * cell_px as i32,
                y0: 0,
                x1: i32::from(run.cols.1) * cell_px as i32,
                y1: bottom,
            },
            rgb(bg),
        );
    }
    // Shapes snap to whole device pixels from the cell's own column, so two cells' rules abut
    // instead of leaving the hairline a font's glyph leaves.
    for shape in &line.shapes {
        let [x0, y0, x1, y1] = shape.rect;
        let left = ((f32::from(shape.col) + x0) * cell_px as f32).round() as i32;
        let right = ((f32::from(shape.col) + x1) * cell_px as f32).round() as i32;
        let shape_top = (y0 * line_px as f32).round() as i32;
        let shape_bottom = (y1 * line_px as f32).round() as i32;
        surface.fill(
            Rect {
                x0: left,
                y0: shape_top,
                x1: right,
                y1: shape_bottom,
            },
            rgb(shape.fg),
        );
    }
    let size = glyph_size(line_px);
    let baseline = (line_px as f32 * BASELINE) as i32;
    for run in &line.runs {
        let color = pen_color(run.pen, palette);
        let text = line.text.get(run.start..run.end).unwrap_or_default();
        let mut col = u32::from(run.cols.0);
        for ch in text.chars() {
            if ch.is_control() || ch == ' ' {
                if !format_char(ch) {
                    col = col.saturating_add(1);
                }
                continue;
            }
            if format_char(ch) {
                continue;
            }
            // A combining mark has no advance of its own: it belongs to the cell before it.
            let at = if combining(ch) {
                col.saturating_sub(1)
            } else {
                col
            };
            if let Some(glyph) = atlas.glyph(ch, size, run.pen.bold) {
                surface.blit(
                    at as i32 * cell_px as i32 + glyph.left,
                    baseline - glyph.top,
                    glyph,
                    color,
                    if run.pen.italic { ITALIC_SHEAR } else { 0.0 },
                );
            }
            if !combining(ch) {
                col = col.saturating_add(1);
            }
        }
        let x0 = i32::from(run.cols.0) * cell_px as i32;
        let x1 = i32::from(run.cols.1) * cell_px as i32;
        if run.pen.underline {
            let thickness = (line_px / 14).max(1) as i32;
            surface.fill(
                Rect {
                    x0,
                    y0: baseline + thickness,
                    x1,
                    y1: baseline + thickness * 2,
                },
                color,
            );
        }
        if run.pen.strike {
            let thickness = (line_px / 14).max(1) as i32;
            let middle = line_px as i32 / 2;
            surface.fill(
                Rect {
                    x0,
                    y0: middle,
                    x1,
                    y1: middle + thickness,
                },
                color,
            );
        }
    }
}

/// The run's foreground, dimmed halfway to the background when the pen is faint.
fn pen_color(pen: Pen, palette: Palette) -> [u8; 3] {
    let color = rgb(pen.fg);
    if pen.faint {
        [
            ((u16::from(color[0]) + u16::from(palette.bg[0])) / 2) as u8,
            ((u16::from(color[1]) + u16::from(palette.bg[1])) / 2) as u8,
            ((u16::from(color[2]) + u16::from(palette.bg[2])) / 2) as u8,
        ]
    } else {
        color
    }
}

/// A mark that sits on the cell before it instead of advancing one of its own.
fn combining(c: char) -> bool {
    matches!(
        c as u32,
        0x0300..=0x036F
            | 0x0483..=0x0489
            | 0x0591..=0x05BD
            | 0x05BF
            | 0x05C1..=0x05C2
            | 0x05C4..=0x05C5
            | 0x05C7
            | 0x0610..=0x061A
            | 0x064B..=0x065F
            | 0x0670
            | 0x06D6..=0x06DC
            | 0x06DF..=0x06E4
            | 0x06E7..=0x06E8
            | 0x06EA..=0x06ED
            | 0x0711
            | 0x0730..=0x074A
            | 0x0E31
            | 0x0E34..=0x0E3A
            | 0x0E47..=0x0E4E
            | 0x1AB0..=0x1AFF
            | 0x1DC0..=0x1DFF
            | 0x20D0..=0x20FF
            | 0xFE20..=0xFE2F
    )
}

/// A zero-width character that is not drawn as a mark: joiners, variation selectors and the
/// directional marks. Drawing their glyph would show a tofu box instead of nothing.
fn format_char(c: char) -> bool {
    matches!(
        c as u32,
        0x200B..=0x200F | 0x202A..=0x202E | 0x2060..=0x206F | 0xFE00..=0xFE0F | 0xFEFF
            | 0xE0000..=0xE007F
            | 0xE0100..=0xE01EF
    )
}

/// Blends a colour onto one pixel. An unwritten pixel is transparent, so the pane's fill is
/// used as what is under it; a written one is opaque. Either way the result is opaque, which is
/// what lets the glyph loop stay a pair of multiply-adds per channel instead of the full
/// straight-alpha source-over with a divide.
fn over(pixel: &mut [u8; 4], color: [u8; 3], alpha: u8, bg: [u8; 3]) {
    let src = u32::from(alpha);
    if src == 0 {
        return;
    }
    let inv = 255 - src;
    if pixel[3] == 0 {
        for channel in 0..3 {
            pixel[channel] =
                ((u32::from(color[channel]) * src + u32::from(bg[channel]) * inv) / 255) as u8;
        }
    } else {
        for channel in 0..3 {
            let under = u32::from(pixel[channel]);
            pixel[channel] = ((u32::from(color[channel]) * src + under * inv) / 255) as u8;
        }
    }
    pixel[3] = 255;
}

/// Writes one 4-byte pixel across a span by doubling it: four bytes, then eight, then sixteen,
/// which is a handful of `memcpy`s instead of a loop over every pixel.
fn fill_span(dst: &mut [u8], pixel: [u8; 4]) {
    let len = dst.len();
    if len < 4 {
        return;
    }
    dst[..4].copy_from_slice(&pixel);
    let mut filled = 4;
    while filled < len {
        let take = filled.min(len - filled);
        dst.copy_within(0..take, filled);
        filled += take;
    }
}
