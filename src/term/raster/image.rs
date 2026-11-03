//! Kitty graphics on the grid: one image placement scaled from its source rectangle into the box
//! the program gave it, alpha-blended over whatever the raster already holds. Kept apart from the
//! row painter because an image spans rows: it is drawn against the whole image, not a band.

use crate::term::grid::{ImagePixels, Placement};

/// Draws one placement. `top` is the overscan row the raster carries above the viewport, since
/// placement rows are counted from the viewport's own top.
pub(super) fn paint_image(
    dst: &mut [u8],
    width: u32,
    height: u32,
    placement: &Placement,
    scale: f32,
    top: i32,
    bg: [u8; 3],
) {
    let x0 = (placement.x * scale).round() as i32;
    let y0 = (placement.y * scale).round() as i32 + top;
    let w = (placement.w * scale).round().max(1.0) as i32;
    let h = (placement.h * scale).round().max(1.0) as i32;
    let [sx, sy, sw, sh] = placement.source;
    let sw = sw.max(1) as f32;
    let sh = sh.max(1) as f32;
    let left = x0.max(0);
    let right = (x0 + w).min(width as i32);
    let start = y0.max(0);
    let end = (y0 + h).min(height as i32);
    if left >= right || start >= end {
        return;
    }
    // An image drawn at exactly its source size, which is what a screenshot of one terminal cell
    // per pixel is, needs no filtering at all.
    let direct = w as u32 == sw as u32 && h as u32 == sh as u32;
    for py in start..end {
        let v = sy as f32 + ((py - y0) as f32 + 0.5) * sh / h as f32;
        let row = py as u32 * width;
        for px in left..right {
            let u = sx as f32 + ((px - x0) as f32 + 0.5) * sw / w as f32;
            let src = if direct {
                sample(&placement.data, u as i32, v as i32)
            } else {
                bilinear(&placement.data, u, v)
            };
            let at = (row + px as u32) as usize * 4;
            if let Some(pixel) = dst
                .get_mut(at..at + 4)
                .and_then(|bytes| bytes.try_into().ok())
            {
                overlay(pixel, src, bg);
            }
        }
    }
}

/// One pixel of an image, clamped to its bounds: the edges of a scaled image sample past the last
/// row and column.
fn sample(data: &ImagePixels, x: i32, y: i32) -> [u8; 4] {
    let x = x.clamp(0, data.width as i32 - 1) as u32;
    let y = y.clamp(0, data.height as i32 - 1) as u32;
    let at = (y * data.width + x) as usize * 4;
    match data.rgba.get(at..at + 4) {
        Some(pixel) => [pixel[0], pixel[1], pixel[2], pixel[3]],
        None => [0, 0, 0, 0],
    }
}

/// The four pixels around a source coordinate, weighted by how close it is to each: enough to
/// keep a photo from looking like a checkerboard when it is scaled down.
fn bilinear(data: &ImagePixels, u: f32, v: f32) -> [u8; 4] {
    let fx = u.floor();
    let fy = v.floor();
    let tx = u - fx;
    let ty = v - fy;
    let (x, y) = (fx as i32, fy as i32);
    let top = [sample(data, x, y), sample(data, x + 1, y)];
    let bottom = [sample(data, x, y + 1), sample(data, x + 1, y + 1)];
    let mut out = [0u8; 4];
    for channel in 0..4 {
        let above = f32::from(top[0][channel]) * (1.0 - tx) + f32::from(top[1][channel]) * tx;
        let below = f32::from(bottom[0][channel]) * (1.0 - tx) + f32::from(bottom[1][channel]) * tx;
        out[channel] = (above * (1.0 - ty) + below * ty).round().clamp(0.0, 255.0) as u8;
    }
    out
}

/// One image pixel over one raster pixel, straight alpha, flattened against the pane's own fill
/// when the raster pixel is still untouched. Written pixels are opaque, which is what lets the
/// rest of the painter stay a pair of multiply-adds per channel.
fn overlay(pixel: &mut [u8; 4], src: [u8; 4], bg: [u8; 3]) {
    let alpha = u32::from(src[3]);
    if alpha == 0 {
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
