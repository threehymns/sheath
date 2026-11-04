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
//! ([`Line::wide`](crate::grid::Line::wide)), so the glyph after it starts in its own cell
//! rather than in the one the wide glyph also covers.
//!
//! The image is kept between frames: a row whose [`Line`] did not change keeps the pixels it was
//! painted with, so an echo or a fractional scroll costs the bands that actually changed instead
//! of the whole screen. The cursor is not part of it — the view draws that over the image — so
//! its blink and its moves cost no upload at all.

mod atlas;
mod compose;
mod image;
mod paint;

#[cfg(test)]
mod perf;
#[cfg(test)]
mod tests;

use std::cell::RefCell;

use rune::Pixels;
use rune::color::srgb_encode;

use crate::grid::{Frame, Line, Placement};

use compose::{Geometry, Pass, Raster};

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub bg: [u8; 3],
    pub selection: [u8; 3],
    /// The wash under a search match, stronger on the one the search is on.
    pub search: [u8; 3],
}

/// A colour as sRGB bytes, the way the texture holds pixels.
pub fn rgb(color: rune::Color) -> [u8; 3] {
    let byte = |value: f32| (srgb_encode(value) * 255.0).round().clamp(0.0, 255.0) as u8;
    [byte(color.r), byte(color.g), byte(color.b)]
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
    /// The kitty graphics placements the image holds. A frame whose placements are these same
    /// images in the same places leaves them in the pixels: only the bands whose text changed are
    /// painted again, with the image composited under or over them.
    placements: Vec<Placement>,
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
            placements: Vec::new(),
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
pub fn picture(
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
        let face = crate::fonts::generation();
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
        let geometry_same = cache.drawn == Some(stamp)
            && cache.width == width
            && cache.height == height
            && cache.rows.len() == usize::from(frame.rows);
        // A placement that did not move and whose pixels did not change is already in the image:
        // the video frame of a program that only redraws its own overlay must not scale and blit
        // the whole picture again. The engine keeps one `Arc` per image generation, so comparing
        // the pointers tells a new frame from the one already composited.
        let placements_same = same_placements(&cache.placements, &frame.images);
        let has_images = !frame.images.is_empty() || !cache.placements.is_empty();
        // An equal generation says this is the frame the image was painted from: the engine
        // bumps it on every readback, so no row can have changed. A test's composed frame has
        // generation zero and is always diffed.
        let settled = geometry_same && frame.generation > 0 && cache.generation == frame.generation;
        // A changed placement spans rows, so that frame is painted whole; the rest are only ever
        // painted where they changed.
        let full = !geometry_same || !placements_same;
        let bands_total = usize::from(frame.rows) + 2;
        let row_bytes = width as usize * line_px as usize * 4;
        let mut bands: Vec<usize> = Vec::new();
        if full {
            cache.reset(width, height);
            bands.extend(0..bands_total);
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
                bands.push(bands_total - 1);
            }
            // A viewport that stepped by whole rows is still showing the rows it was showing,
            // one band over: a wheel notch, or a line of a program's output scrolling the
            // screen. The pixels move with it and only the bands that have no source are
            // painted, which is what a scroll costs instead of the whole grid. A placement is
            // anchored to the viewport, so moving the text's pixels would move the image with
            // it: a frame carrying one is diffed band by band instead.
            if !has_images
                && bands.len() * 2 > bands_total
                && let Some((step, paints)) = stepped(cache, frame, bands_total)
                && paints.len() + 2 < bands_total
            {
                shift_pixels(&mut cache.rgba, row_bytes, bands_total, step);
                bands = paints;
            }
        }
        if bands.is_empty() {
            cache.generation = frame.generation;
            return Some(cache.pixels.clone());
        }
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
        cache.placements.clone_from(&frame.images);
        cache.drawn = Some(stamp);
        cache.generation = frame.generation;
        cache.pixels.set(width, height, cache.rgba.clone());
        Some(cache.pixels.clone())
    })
}

/// The image the last [`picture`] composed, for a test that compares it with a whole compose: the
/// streamed [`Pixels`] belongs to the runtime, and its bytes are not readable from here.
#[cfg(test)]
fn streamed_for_test() -> Vec<u8> {
    CACHE.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|cache| cache.rgba.clone())
            .unwrap_or_default()
    })
}

/// Whether two frames carry the same placements: the same images, in the same places, at the
/// same sizes. An image's pixels are one `Arc` per engine readback, so two placements built from
/// one image generation share it, and a new video frame under the same image id does not.
fn same_placements(old: &[Placement], new: &[Placement]) -> bool {
    old.len() == new.len()
        && old.iter().zip(new).all(|(was, now)| {
            was.image == now.image
                && was.layer == now.layer
                && was.z == now.z
                && was.x == now.x
                && was.y == now.y
                && was.w == now.w
                && was.h == now.h
                && was.source == now.source
                && std::sync::Arc::ptr_eq(&was.data, &now.data)
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

/// How far the viewport stepped between two frames, in bands, with the bands that do not line up
/// with the image it already holds. Positive is the content moving down the image, which is the
/// viewport going back into history; negative is content moving up, which is a program's output
/// scrolling the screen. `None` when no step fits.
///
/// Every band it does not list holds exactly what the band it moves there holds: a line that
/// compares equal is painted equal, which is the rule the per-row diff already trusts.
fn stepped(cache: &Cache, frame: &Frame, total: usize) -> Option<(isize, Vec<usize>)> {
    // A step big enough to be a jump (a scrollbar, a page of history) is cheaper painted whole.
    const MAX_STEP: isize = 8;
    let last = total as isize;
    let old = |band: isize| -> Option<&Line> {
        if band < 0 || band >= last {
            return None;
        }
        let band = band as usize;
        if band == 0 {
            cache.lead.as_ref()
        } else if band + 1 == total {
            cache.trail.as_ref()
        } else {
            cache.rows.get(band - 1)
        }
    };
    let new = |band: usize| -> Option<&Line> { band_line(frame, band) };
    for step in (1..=MAX_STEP).flat_map(|step| [step, -step]) {
        if step.abs() >= last {
            continue;
        }
        let mut paints = Vec::new();
        for band in 0..total {
            let source = band as isize - step;
            let same = source >= 0
                && source < last
                && match (old(source), new(band)) {
                    (None, None) => true,
                    (Some(was), Some(now)) => was == now,
                    _ => false,
                };
            if !same {
                paints.push(band);
            }
        }
        // A scroll paints the bands that entered at an edge; anything past that is not a step.
        if paints.len() as isize <= step.abs() + 2 {
            return Some((step, paints));
        }
    }
    None
}

/// Moves the image by `step` bands in place: what a scroll keeps is the pixels it already has.
fn shift_pixels(rgba: &mut [u8], row_bytes: usize, total: usize, step: isize) {
    if step > 0 {
        let step = step as usize;
        rgba.copy_within(0..(total - step) * row_bytes, step * row_bytes);
    } else {
        let step = (-step) as usize;
        rgba.copy_within(step * row_bytes..total * row_bytes, 0);
    }
}
