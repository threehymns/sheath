//! Kitty graphics, read back out of libghostty: the placements the program put on the screen and
//! the pixels behind them. The protocol itself (chunked transmits, the file, temporary file and
//! shared memory mediums, PNG and zlib payloads, placement ids, deletes and the replies a
//! program gets) is libghostty's, enabled on the terminal in [`super::Engine::spawn`]. This is
//! the half the pane draws: one [`Placement`] per placement on screen, with the image decoded
//! once and reused until its pixels change.
//!
//! Unicode placeholders (a virtual placement, where the image is anchored to `U+10EEEE` cells
//! instead of a position) are stored and answered by libghostty but not drawn yet: the storage
//! has no position for them, and the pane reads positions from the storage.

use std::collections::HashMap;
use std::sync::Arc;

use ghostty_vt::kitty::graphics::{Image, ImageFormat};

use super::Engine;
use crate::term::grid::{Frame, ImageLayer, ImagePixels, Placement};

/// Decoded pixels kept per image id, with the image generation they were read at.
pub(super) type ImageCache = HashMap<u32, (u64, Arc<ImagePixels>)>;

impl Engine {
    /// Reads every placement on screen into `out`, decoding the images whose pixels moved since
    /// the last frame. The storage's generation covers transmits, replacements and deletes; the
    /// per-image generation covers a new image under the same id, animations included.
    pub(super) fn read_images(&mut self, out: &mut Frame) {
        out.images.clear();
        let Ok(graphics) = self.vt.kitty_graphics() else {
            return;
        };
        let Ok(generation) = graphics.generation() else {
            return;
        };
        if generation == 0 {
            // Nothing was ever transmitted, so nothing is cached and nothing is placed.
            self.images.clear();
            self.graphics = generation;
            return;
        }
        if generation != self.graphics {
            // A transmit or a delete invalidates the whole cache: an image that is gone would
            // otherwise keep its decoded copy for the life of the shell.
            self.images.clear();
        }
        self.graphics = generation;
        let Some(mut place) = self.placements.take() else {
            return;
        };
        let cell = self.metrics.cell;
        let line = self.metrics.line;
        if let Ok(mut places) = place.update(&graphics) {
            while let Some(at) = places.next() {
                let Ok(id) = at.image_id() else { continue };
                let Some(image) = graphics.image(id) else {
                    continue;
                };
                if at.is_virtual().unwrap_or(true) {
                    continue;
                }
                let (Ok(x), Ok(y), Ok(z)) = (at.x_offset(), at.y_offset(), at.z()) else {
                    continue;
                };
                let Ok(info) = at.placement_render_info(&image, &self.vt) else {
                    continue;
                };
                if !info.viewport_visible {
                    continue;
                }
                let Some(data) = pixels(&mut self.images, id, &image) else {
                    continue;
                };
                out.images.push(Placement {
                    data,
                    layer: layer_of(z),
                    z,
                    x: info.viewport_col as f32 * cell + x as f32,
                    y: info.viewport_row as f32 * line + y as f32,
                    w: info.pixel_width as f32,
                    h: info.pixel_height as f32,
                    source: [
                        info.source_x,
                        info.source_y,
                        info.source_width,
                        info.source_height,
                    ],
                });
            }
        }
        self.placements = Some(place);
        out.images
            .sort_by_key(|placement| (placement.layer, placement.z));
    }
}

/// Which side of the text a z-index draws on, in the protocol's terms: the smallest values put
/// the image under the cell backgrounds, the other negatives under the text, and everything else
/// over it.
fn layer_of(z: i32) -> ImageLayer {
    if z <= i32::MIN / 2 {
        ImageLayer::BelowBackground
    } else if z < 0 {
        ImageLayer::BelowText
    } else {
        ImageLayer::AboveText
    }
}

/// The decoded pixels for an image id, read again only when libghostty says its contents moved.
/// The cache is a parameter rather than the engine so the storage's borrow of the terminal does
/// not fight the cache's.
fn pixels(cache: &mut ImageCache, id: u32, image: &Image<'_>) -> Option<Arc<ImagePixels>> {
    let generation = image.generation().ok()?;
    if let Some((cached, pixels)) = cache.get(&id)
        && *cached == generation
    {
        return Some(Arc::clone(pixels));
    }
    let pixels = Arc::new(decode(image)?);
    cache.insert(id, (generation, Arc::clone(&pixels)));
    Some(pixels)
}

/// One image as RGBA. libghostty hands back decoded pixels for every format it accepted, PNG
/// included, so this only widens the narrower ones.
fn decode(image: &Image<'_>) -> Option<ImagePixels> {
    let width = image.width().ok()?;
    let height = image.height().ok()?;
    let format = image.format().ok()?;
    let data = image.data().ok()?;
    if width == 0 || height == 0 {
        return None;
    }
    let count = width as usize * height as usize;
    let rgba = match format {
        ImageFormat::Rgba | ImageFormat::Png => {
            if data.len() != count * 4 {
                return None;
            }
            data.to_vec()
        }
        ImageFormat::Rgb => {
            if data.len() != count * 3 {
                return None;
            }
            let mut rgba = Vec::with_capacity(count * 4);
            for pixel in data.as_chunks::<3>().0 {
                rgba.extend_from_slice(&[pixel[0], pixel[1], pixel[2], 255]);
            }
            rgba
        }
        ImageFormat::GrayAlpha => {
            if data.len() != count * 2 {
                return None;
            }
            let mut rgba = Vec::with_capacity(count * 4);
            for pixel in data.as_chunks::<2>().0 {
                rgba.extend_from_slice(&[pixel[0], pixel[0], pixel[0], pixel[1]]);
            }
            rgba
        }
        ImageFormat::Gray => {
            if data.len() != count {
                return None;
            }
            let mut rgba = Vec::with_capacity(count * 4);
            for &value in data {
                rgba.extend_from_slice(&[value, value, value, 255]);
            }
            rgba
        }
        // A format a later libghostty adds: drawing nothing beats drawing the wrong pixels.
        _ => return None,
    };
    Some(ImagePixels {
        width,
        height,
        rgba,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_z_index_picks_the_layer_it_draws_on() {
        assert_eq!(layer_of(i32::MIN), ImageLayer::BelowBackground);
        assert_eq!(layer_of(-2), ImageLayer::BelowText);
        assert_eq!(layer_of(-1), ImageLayer::BelowText);
        assert_eq!(layer_of(0), ImageLayer::AboveText);
        assert_eq!(layer_of(9), ImageLayer::AboveText);
    }
}
