//! Kitty graphics on the grid: one image placement scaled from its source rectangle into the box
//! the program gave it, alpha-blended over whatever the raster already holds. Kept apart from the
//! row painter because an image spans rows: it is drawn against the whole image, not a band.
//!
//! A video played through the protocol is the hot path, so the sampler is built once per
//! placement: every output column and row resolves to two source indices and a fixed-point
//! weight before any pixel is touched. What is left per pixel is integer multiply-adds, and a
//! placement large enough to pay for the threads splits its rows across the machine's cores.

use crate::grid::Placement;

/// The image being painted into: its size in device pixels, the display scale the placement's
/// points are drawn at, and the overscan row above the viewport that placement rows are offset
/// by.
#[derive(Clone, Copy)]
pub(super) struct Canvas {
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub line_px: u32,
}

/// How much of the second source pixel an axis takes, in 1/256ths: a column or a row of the
/// scaled placement, resolved once so the inner loop never converts a coordinate.
#[derive(Clone, Copy)]
struct Tap {
    first: usize,
    second: usize,
    weight: u32,
}

/// One axis of a placement's scale, before its taps are resolved: the output positions to
/// resolve, the source rectangle they draw from, and whether it is drawn at exactly its size.
struct Axis {
    first: i32,
    last: i32,
    /// The placement's own origin on this axis, in device pixels.
    origin: i32,
    source_start: u32,
    source_span: u32,
    source_len: u32,
    output_span: i32,
    direct: bool,
}

/// Resolves one axis of the scale: which two source indices each output position samples, and how
/// much of each. A placement drawn at exactly its source size takes the pixel itself, which is
/// what a screenshot of one terminal cell per pixel asks for.
fn taps(axis: Axis) -> Vec<Tap> {
    let last_source = axis.source_len.max(1) as i32 - 1;
    (axis.first..axis.last)
        .map(|at| {
            let position = if axis.direct {
                axis.source_start as f32 + (at - axis.origin) as f32
            } else {
                axis.source_start as f32
                    + ((at - axis.origin) as f32 + 0.5) * axis.source_span as f32
                        / axis.output_span as f32
            };
            let base = position.floor();
            let weight = ((position - base) * 256.0).round().clamp(0.0, 256.0) as u32;
            let index = (base as i32).clamp(0, last_source);
            Tap {
                first: index as usize,
                second: (index + 1).clamp(0, last_source) as usize,
                weight,
            }
        })
        .collect()
}

/// Draws one placement. `top` is the overscan row the raster carries above the viewport, since
/// placement rows are counted from the viewport's own top; `clip_top`/`clip_bottom` limit the
/// rows painted, which is what repainting a single band of the grid asks for.
pub fn paint_image(
    dst: &mut [u8],
    canvas: Canvas,
    placement: &Placement,
    bg: [u8; 3],
    clip: (i32, i32),
) {
    let Canvas {
        width,
        height,
        scale,
        line_px,
    } = canvas;
    let top = line_px as i32;
    let source = &placement.data;
    if source.width == 0 || source.height == 0 {
        return;
    }
    let x0 = (placement.x * scale).round() as i32;
    let y0 = (placement.y * scale).round() as i32 + top;
    let w = (placement.w * scale).round().max(1.0) as i32;
    let h = (placement.h * scale).round().max(1.0) as i32;
    let [sx, sy, sw, sh] = placement.source;
    let sw = sw.max(1);
    let sh = sh.max(1);
    let left = x0.max(0);
    let right = (x0 + w).min(width as i32);
    let start = y0.max(0).max(clip.0);
    let end = (y0 + h).min(height as i32).min(clip.1);
    if left >= right || start >= end {
        return;
    }
    // An image drawn at exactly its source size, which is what a screenshot of one terminal cell
    // per pixel is, needs no filtering at all.
    let direct = w as u32 == sw && h as u32 == sh;
    let columns = taps(Axis {
        first: left,
        last: right,
        origin: x0,
        source_start: sx,
        source_span: sw,
        source_len: source.width,
        output_span: w,
        direct,
    });
    let rows = taps(Axis {
        first: start,
        last: end,
        origin: y0,
        source_start: sy,
        source_span: sh,
        source_len: source.height,
        output_span: h,
        direct,
    });

    let width_bytes = width as usize * 4;
    let left_bytes = left as usize * 4;
    let span = columns.len() * 4;
    let source_width = source.width as usize;
    // Below this many pixels a thread costs more than it saves: a pane's small placement stays on
    // this thread, while a full-pane video frame is split across the cores.
    let pixels = (right - left) as u64 * (end - start) as u64;
    let workers = if pixels < 300_000 {
        1
    } else {
        std::thread::available_parallelism()
            .map(|count| count.get())
            .unwrap_or(1)
            .min(rows.len().max(1))
    };
    let first = start as usize * width_bytes;
    let last = end as usize * width_bytes;
    let pixels = &source.rgba;
    if workers <= 1 {
        for (index, py) in (start..end).enumerate() {
            let at = py as usize * width_bytes + left_bytes;
            paint_row(
                &mut dst[at..at + span],
                rows[index],
                &columns,
                pixels,
                source_width,
                bg,
            );
        }
        return;
    }
    let per = ((end - start) as usize).div_ceil(workers);
    std::thread::scope(|scope| {
        for (chunk, first_row) in dst[first..last]
            .chunks_mut(per * width_bytes)
            .zip((0..workers).map(|index| index * per))
        {
            let columns = &columns;
            let rows = &rows;
            scope.spawn(move || {
                for (offset, row) in chunk.chunks_mut(width_bytes).enumerate() {
                    paint_row(
                        &mut row[left_bytes..left_bytes + span],
                        rows[first_row + offset],
                        columns,
                        pixels,
                        source_width,
                        bg,
                    );
                }
            });
        }
    });
}

/// Blends one output row of a placement: the four source pixels around each position, weighted
/// on each axis, over what the raster already holds.
fn paint_row(
    dst: &mut [u8],
    tap: Tap,
    columns: &[Tap],
    pixels: &[u8],
    source_width: usize,
    bg: [u8; 3],
) {
    let above = tap.first * source_width * 4;
    let below = tap.second * source_width * 4;
    let down = tap.weight;
    let up = 256 - down;
    for (pixel, column) in dst.as_chunks_mut::<4>().0.iter_mut().zip(columns) {
        let left = column.weight;
        let right = 256 - left;
        let top_left = above + column.first * 4;
        let top_right = above + column.second * 4;
        let bottom_left = below + column.first * 4;
        let bottom_right = below + column.second * 4;
        let mut src = [0u8; 4];
        for channel in 0..4 {
            let top = (u32::from(pixels[top_left + channel]) * right
                + u32::from(pixels[top_right + channel]) * left)
                >> 8;
            let bottom = (u32::from(pixels[bottom_left + channel]) * right
                + u32::from(pixels[bottom_right + channel]) * left)
                >> 8;
            src[channel] = ((top * up + bottom * down) >> 8) as u8;
        }
        overlay(pixel, src, bg);
    }
}

/// One image pixel over one raster pixel, straight alpha, flattened against the pane's own fill
/// when the raster pixel is still untouched. Written pixels are opaque, which is what lets the
/// rest of the painter stay a pair of multiply-adds per channel.
fn overlay(pixel: &mut [u8; 4], src: [u8; 4], bg: [u8; 3]) {
    let alpha = u32::from(src[3]);
    if alpha == 0 {
        return;
    }
    // A video frame is opaque, which is the common case: nothing to blend it with.
    if alpha == 255 {
        pixel.copy_from_slice(&src);
        return;
    }
    let inverse = 255 - alpha;
    if pixel[3] == 0 {
        for channel in 0..3 {
            pixel[channel] =
                ((u32::from(src[channel]) * alpha + u32::from(bg[channel]) * inverse) / 255) as u8;
        }
    } else {
        for channel in 0..3 {
            pixel[channel] = ((u32::from(src[channel]) * alpha
                + u32::from(pixel[channel]) * inverse)
                / 255) as u8;
        }
    }
    pixel[3] = 255;
}
