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
//! A wide character is one glyph over two columns: the row carries which columns those are
//! ([`Line::wide`](crate::term::grid::Line::wide)), so the glyph after it starts in its own cell
//! rather than in the one the wide glyph also covers.
//!
//! The image is kept between frames: a row whose [`Line`] did not change keeps the pixels it was
//! painted with, so an echo or a fractional scroll costs the bands that actually changed instead
//! of the whole screen. The cursor is not part of it — the view draws that over the image — so
//! its blink and its moves cost no upload at all.

mod atlas;
mod image;
mod paint;

#[cfg(test)]
mod perf;
#[cfg(test)]
mod tests;

use std::cell::RefCell;

use rune::Pixels;
use rune::color::srgb_encode;

use crate::term::grid::{Frame, ImageLayer, Line};

use atlas::Atlas;
use image::paint_image;
use paint::{Surface, glyph_size, paint_line};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) struct Palette {
    pub bg: [u8; 3],
    pub selection: [u8; 3],
    /// The wash under a search match, stronger on the one the search is on.
    pub search: [u8; 3],
}

/// A colour as sRGB bytes, the way the texture holds pixels.
pub(crate) fn rgb(color: rune::Color) -> [u8; 3] {
    let byte = |value: f32| (srgb_encode(value) * 255.0).round().clamp(0.0, 255.0) as u8;
    [byte(color.r), byte(color.g), byte(color.b)]
}

struct Raster {
    atlas: Atlas,
}

/// What the cached image was made in. A view build that changed nothing about the grid (a moved
/// element, a hover underline) leaves all of this equal and reuses the pixels.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Stamp {
    cols: u16,
    rows: u16,
    cell_px: u32,
    line_px: u32,
    palette: Palette,
}

/// The raster and the image it last produced, which the pane streams until the grid changes.
struct Cache {
    raster: Raster,
    pixels: Pixels,
    /// The pixels of the image last handed to [`Pixels`], kept so the rows that did not change
    /// keep theirs: only the bands that changed are painted again.
    rgba: Vec<u8>,
    width: u32,
    height: u32,
    /// The geometry and palette the pixels are in. `None` until an image is drawn.
    drawn: Option<Stamp>,
    /// The rows as the image holds them, for telling which ones changed.
    rows: Vec<Line>,
    lead: Option<Line>,
    trail: Option<Line>,
    /// Whether the image holds kitty graphics placements, which span rows and are painted whole.
    images: bool,
    /// The frame's generation the image was painted at: the engine bumps it on every readback,
    /// so an equal one means no row can have changed since.
    generation: u64,
    /// The font generation the atlas was built in. The Nerd Font fallback is found on a thread
    /// while the window is already drawing, so a cache built before it landed is rebuilt once
    /// with the face, rather than keeping the bundled one for the session.
    face: u64,
}

impl Cache {
    fn new(face: u64) -> Cache {
        Cache {
            raster: Raster::new(),
            pixels: Pixels::new(),
            rgba: Vec::new(),
            width: 0,
            height: 0,
            drawn: None,
            rows: Vec::new(),
            lead: None,
            trail: None,
            images: false,
            generation: 0,
            face,
        }
    }

    /// Sizes the buffer to a whole image, zeroed. A frame that is not the same size as the last
    /// one, or is painted in another palette, is composed whole, so nothing of the old image may
    /// survive in it.
    fn reset(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        let len = (width * height * 4) as usize;
        if self.rgba.len() == len {
            self.rgba.fill(0);
        } else {
            self.rgba = vec![0u8; len];
        }
    }
}

thread_local! {
    static CACHE: RefCell<Option<Cache>> = const { RefCell::new(None) };
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
        let face = crate::term::fonts::generation();
        if slot.as_ref().is_none_or(|cache| cache.face != face) {
            *slot = Some(Cache::new(face));
        }
        let cache = slot.as_mut().expect("the cache was just filled");
        let cell_px = (cell * scale).round().max(1.0) as u32;
        let line_px = (line * scale).round().max(1.0) as u32;
        let width = u32::from(frame.cols) * cell_px;
        let height = (u32::from(frame.rows) + 2) * line_px;
        if width == 0 || height == 0 {
            return None;
        }
        let stamp = Stamp {
            cols: frame.cols,
            rows: frame.rows,
            cell_px,
            line_px,
            palette,
        };
        let images = !frame.images.is_empty();
        let geometry_same = cache.drawn == Some(stamp)
            && cache.width == width
            && cache.height == height
            && cache.rows.len() == usize::from(frame.rows);
        // An equal generation says this is the frame the image was painted from: the engine
        // bumps it on every readback, so no row can have changed. A test's composed frame has
        // generation zero and is always diffed.
        let settled = geometry_same && frame.generation > 0 && cache.generation == frame.generation;
        // A placement spans rows, so a frame that carries one (or one the image still holds) is
        // painted whole; the rest are only ever painted where they changed.
        let full = !geometry_same || images || cache.images;
        let mut bands: Vec<usize> = Vec::new();
        if full {
            cache.reset(width, height);
            bands.extend(0..usize::from(frame.rows) + 2);
        } else if !settled {
            for (index, line) in frame.lines.iter().enumerate() {
                if cache.rows.get(index) != Some(line) {
                    bands.push(index + 1);
                }
            }
            if cache.lead.as_ref() != frame.lead.as_ref() {
                bands.push(0);
            }
            if cache.trail.as_ref() != frame.trail.as_ref() {
                bands.push(usize::from(frame.rows) + 1);
            }
        }
        if bands.is_empty() {
            cache.generation = frame.generation;
            return Some(cache.pixels.clone());
        }
        let row_bytes = width as usize * line_px as usize * 4;
        if !full {
            // A repainted band starts from nothing, the way the whole image does: the pane's fill
            // shows through wherever no glyph, wash or background wrote a pixel.
            for &band in &bands {
                let at = band * row_bytes;
                if let Some(band) = cache.rgba.get_mut(at..at + row_bytes) {
                    band.fill(0);
                }
            }
        }
        let geometry = Geometry {
            width,
            height,
            cell_px,
            line_px,
            scale,
        };
        cache.raster.paint(
            frame,
            geometry,
            Pass {
                palette,
                bands: &bands,
                full,
            },
            &mut cache.rgba,
        );
        // Remember what was painted, so the next frame can tell its rows from these. Only the
        // rows that changed are cloned: the rest are already the line this frame carries, and
        // an echo that rewrote one row should not copy the whole screen's text and allocations.
        for (index, line) in frame.lines.iter().enumerate() {
            match cache.rows.get_mut(index) {
                Some(slot) if slot == line => {}
                Some(slot) => slot.clone_from(line),
                None => cache.rows.push(line.clone()),
            }
        }
        cache.rows.truncate(frame.lines.len());
        if cache.lead.as_ref() != frame.lead.as_ref() {
            cache.lead.clone_from(&frame.lead);
        }
        if cache.trail.as_ref() != frame.trail.as_ref() {
            cache.trail.clone_from(&frame.trail);
        }
        cache.images = images;
        cache.drawn = Some(stamp);
        cache.generation = frame.generation;
        cache.pixels.set(width, height, cache.rgba.clone());
        Some(cache.pixels.clone())
    })
}

/// The line a band holds: the overscan row above the viewport, the viewport's rows, then the one
/// below it.
fn band_line(frame: &Frame, band: usize) -> Option<&Line> {
    if band == 0 {
        frame.lead.as_ref()
    } else if band <= usize::from(frame.rows) {
        frame.lines.get(band - 1)
    } else {
        frame.trail.as_ref()
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
struct Geometry {
    width: u32,
    height: u32,
    cell_px: u32,
    line_px: u32,
    scale: f32,
}

/// What one paint pass covers: every band, or only the bands that changed.
struct Pass<'a> {
    palette: Palette,
    bands: &'a [usize],
    full: bool,
}

impl Raster {
    fn new() -> Raster {
        Raster {
            atlas: Atlas::new(),
        }
    }

    /// Composes the whole frame into a fresh image, for a test that reads the pixels back.
    #[cfg(test)]
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
    fn paint(&mut self, frame: &Frame, geometry: Geometry, pass: Pass, rgba: &mut [u8]) {
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
        let overscan = line_px as i32;
        if pass.full {
            for placement in frame
                .images
                .iter()
                .filter(|placement| placement.layer != ImageLayer::AboveText)
            {
                paint_image(
                    rgba,
                    width,
                    height,
                    placement,
                    scale,
                    overscan,
                    pass.palette.bg,
                );
            }
        }
        let size = glyph_size(line_px);
        self.atlas.preload(frame, size, pass.bands);
        let atlas = &self.atlas;
        let row_bytes = width as usize * 4 * line_px as usize;
        // Only a band per frame is the common case (an echo, a scroll), and spawning workers for
        // it costs more than the row does.
        let workers = if pass.bands.len() < 8 {
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
                paint_image(
                    rgba,
                    width,
                    height,
                    placement,
                    scale,
                    overscan,
                    pass.palette.bg,
                );
            }
        }
    }
}
