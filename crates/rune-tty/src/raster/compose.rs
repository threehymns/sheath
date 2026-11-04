//! One paint pass over the grid's image: the kitty placements under and over the text, and the
//! rows of the bands that changed, on this thread or split across the cores. The cache in the
//! parent module decides which bands a frame needs; this module only turns a pass into pixels.

use crate::grid::{Frame, ImageLayer, Line};

use super::atlas::Atlas;
use super::image::{Canvas, paint_image};
use super::paint::{Surface, glyph_size, paint_line};
use super::{Palette, band_line};

pub(super) struct Raster {
    atlas: Atlas,
}

/// Composites every placement on one side of the text into the bands being painted again, and
/// only those rows: an unchanged video frame under a changed overlay band costs that band.
fn paint_image_bands(
    rgba: &mut [u8],
    canvas: Canvas,
    frame: &Frame,
    bands: &[usize],
    bg: [u8; 3],
    above: bool,
) {
    for &band in bands {
        let top = band as i32 * canvas.line_px as i32;
        let bottom = top + canvas.line_px as i32;
        for placement in frame
            .images
            .iter()
            .filter(|placement| (placement.layer == ImageLayer::AboveText) == above)
        {
            paint_image(rgba, canvas, placement, bg, (top, bottom));
        }
    }
}

/// Paints one band: one row of the image and the line that belongs on it.
fn paint_band(
    atlas: &Atlas,
    pixels: &mut [u8],
    width: u32,
    line_px: u32,
    cell_px: u32,
    palette: Palette,
    line: &Line,
) {
    let mut surface = Surface::new(pixels, width, line_px, palette.bg);
    paint_line(atlas, &mut surface, line, cell_px, line_px, palette);
}

/// The pixel geometry one image is painted at.
#[derive(Clone, Copy)]
pub(super) struct Geometry {
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) cell_px: u32,
    pub(super) line_px: u32,
    pub(super) scale: f32,
}

/// What one paint pass covers: every band, or only the bands that changed.
pub(super) struct Pass<'a> {
    pub(super) palette: Palette,
    pub(super) bands: &'a [usize],
    pub(super) full: bool,
}

impl Raster {
    pub(super) fn new() -> Raster {
        Raster {
            atlas: Atlas::new(),
        }
    }

    /// Composes the whole frame into a fresh image, for a test that reads the pixels back.
    #[cfg(test)]
    pub(super) fn compose(
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
        let height = (u32::from(frame.rows) + 2) * line_px;
        if width == 0 || height == 0 {
            return None;
        }
        let mut rgba = vec![0u8; (width * height * 4) as usize];
        let bands: Vec<usize> = (0..usize::from(frame.rows) + 2).collect();
        self.paint(
            frame,
            Geometry {
                width,
                height,
                cell_px,
                line_px,
                scale,
            },
            Pass {
                palette,
                bands: &bands,
                full: true,
            },
            &mut rgba,
        );
        Some((width, height, rgba))
    }

    /// Paints `pass.bands` of the frame into `rgba`, which holds the whole image: untouched
    /// bands keep the pixels they were painted with.
    pub(super) fn paint(&mut self, frame: &Frame, geometry: Geometry, pass: Pass, rgba: &mut [u8]) {
        let Geometry {
            width,
            height,
            cell_px,
            line_px,
            scale,
        } = geometry;
        // The images a program placed go under the text when their z-index says so: under the
        // cell backgrounds for the smallest values, under the glyphs for the other negatives.
        // Painting them before the rows is what keeps text over an image readable, and it is
        // the order Ghostty's own renderer uses.
        let canvas = Canvas {
            width,
            height,
            scale,
            line_px,
        };
        if pass.full {
            for placement in frame
                .images
                .iter()
                .filter(|placement| placement.layer != ImageLayer::AboveText)
            {
                paint_image(rgba, canvas, placement, pass.palette.bg, (0, height as i32));
            }
        } else {
            // The placements are the ones already in the image; only the bands painted again
            // need them composited under their text.
            paint_image_bands(rgba, canvas, frame, pass.bands, pass.palette.bg, false);
        }
        let size = glyph_size(line_px);
        self.atlas.preload(frame, size, pass.bands);
        let atlas = &self.atlas;
        let row_bytes = width as usize * 4 * line_px as usize;
        // Spawning a thread costs more than painting a band of a pane-sized grid. On an eight-core
        // machine a 140x14 pane at 2x paints a whole screen in 0.51 ms on one thread and 0.79 ms
        // on eight, because the spawns and the joins outweigh the pixels; a fullscreened 274x72
        // grid still comes out ahead with workers (26.8 ms against 33.7 ms). The two cross at
        // about two megapixels, which is what this counts, so a pane's rows stay on this thread.
        let pixels = pass.bands.len() as u64 * u64::from(width) * u64::from(line_px);
        let workers = if pixels < 2_000_000 {
            1
        } else {
            std::thread::available_parallelism()
                .map(|count| count.get())
                .unwrap_or(1)
                .min(pass.bands.len())
        };
        if workers <= 1 {
            for &band in pass.bands {
                let Some(line) = band_line(frame, band) else {
                    continue;
                };
                let at = band * row_bytes;
                paint_band(
                    atlas,
                    &mut rgba[at..at + row_bytes],
                    width,
                    line_px,
                    cell_px,
                    pass.palette,
                    line,
                );
            }
        } else {
            let bands = pass.bands;
            let palette = pass.palette;
            let per = (usize::from(frame.rows) + 2).div_ceil(workers);
            std::thread::scope(|scope| {
                for (index, pixels) in rgba.chunks_mut(per * row_bytes).enumerate() {
                    let first = index * per;
                    scope.spawn(move || {
                        for (offset, pixels) in pixels.chunks_mut(row_bytes).enumerate() {
                            let band = first + offset;
                            if !bands.contains(&band) {
                                continue;
                            }
                            let Some(line) = band_line(frame, band) else {
                                continue;
                            };
                            paint_band(atlas, pixels, width, line_px, cell_px, palette, line);
                        }
                    });
                }
            });
        }
        // A placement above the text is drawn last, over the glyphs, which is where a plain
        // `a=T` image lands.
        if pass.full {
            for placement in frame
                .images
                .iter()
                .filter(|placement| placement.layer == ImageLayer::AboveText)
            {
                paint_image(rgba, canvas, placement, pass.palette.bg, (0, height as i32));
            }
        } else {
            paint_image_bands(rgba, canvas, frame, pass.bands, pass.palette.bg, true);
        }
    }
}
