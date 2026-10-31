//! Picture previews, decoded at the size the inspector draws them.
//!
//! Rune's `ImageCache::file` decodes a raster at full resolution and builds a mip chain from
//! that, even when the caller asks for a small preview (the size only applies to SVG). A 12 MP
//! photo therefore costs ~0.4 s: ~120 ms decoding and ~280 ms in the mip chain. Here the decode
//! itself is limited to the preview size (JPEG by DCT scaling, PNG by shrinking after inflate)
//! and the result is handed to Rune as RGBA, where its mip chain only covers the small image.
//! Pictures that already fit in the pane skip all of that and are handed to Rune as they are.
//!
//! Decodes run through `Latest`, like folder scans: a newer selection supersedes a queued one
//! and a short debounce keeps a held arrow key from decoding every row on the way past. The
//! worker also waits for Rune's mips, so a preview lands ready to draw: the frame hook only
//! drains finished work, and the window never wakes for a handle that still paints nothing. The
//! last few previews are kept as image handles, so going back to one is instant.

use image::metadata::Orientation;
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits, RgbaImage};
use rune::{Image, ImageCache, Latest, Waker};
use std::collections::VecDeque;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// The inspector's picture area: 420 pt at 2 device pixels per point.
pub(super) const PREVIEW_POINTS: f32 = 420.0;
pub(super) const PREVIEW_SCALE: f32 = 2.0;
const MAX_PIXELS: u32 = (PREVIEW_POINTS * PREVIEW_SCALE) as u32;
const MAX_SIDE: u32 = 8_192;
const MAX_ALLOC: u64 = 256 * 1024 * 1024;
// Recent renders kept as decoded handles. At preview size each is a few megabytes.
const RECENT: usize = 6;
// A held arrow key selects faster than this; waiting it out decodes only the row it stops on.
const DEBOUNCE: Duration = Duration::from_millis(25);

#[derive(Clone, PartialEq, Eq)]
pub(super) struct Key {
    pub(super) path: PathBuf,
    // An edited file has a new key, so an old preview is never shown for it.
    modified: Option<SystemTime>,
    pixels: u32,
}

/// A decoded preview, ready for a frame to draw. `image` is None when this app could not make
/// one and `error` says why.
pub(super) struct Finished {
    pub(super) key: Key,
    pub(super) image: Option<Image>,
    pub(super) error: Option<String>,
}

enum Decoded {
    Rgba {
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    },
    // SVG, or a format only Rune knows: let its image cache read and decode it.
    ViaRune(PathBuf),
    Failed(String),
}

pub(super) struct Previewer {
    jobs: Latest<Finished>,
    recent: VecDeque<(Key, Image)>,
    // Shared with the app; the worker hands finished RGBA to it off the UI thread.
    cache: Option<Arc<ImageCache>>,
}

impl Default for Previewer {
    fn default() -> Self {
        Previewer {
            jobs: Latest::default().debounce(DEBOUNCE),
            recent: VecDeque::new(),
            cache: None,
        }
    }
}

impl Previewer {
    pub(super) fn set_waker(&mut self, waker: Waker) {
        self.jobs.set_waker(waker);
    }

    pub(super) fn attach(&mut self, cache: Arc<ImageCache>) {
        self.cache = Some(cache);
    }

    /// A kept render for this file, ready to draw now. Otherwise asks for a new one.
    pub(super) fn request(&mut self, path: &Path, modified: Option<SystemTime>) -> Option<Image> {
        let key = Key {
            path: path.to_path_buf(),
            modified,
            pixels: MAX_PIXELS,
        };
        if let Some(image) = self.recall(&key) {
            // The kept handle is newer than anything still decoding.
            self.jobs.cancel();
            return Some(image);
        }
        let cache = self.cache.clone();
        self.jobs
            .request(key, move |key| render(&key, cache.as_deref()));
        None
    }

    pub(super) fn take(&mut self) -> Option<Finished> {
        self.jobs.take()
    }

    pub(super) fn is_pending(&self) -> bool {
        self.jobs.is_pending()
    }

    pub(super) fn cancel(&mut self) {
        self.jobs.cancel();
    }

    pub(super) fn remember(&mut self, key: Key, image: Image) {
        if let Some(at) = self.recent.iter().position(|(kept, _)| *kept == key) {
            self.recent.remove(at);
        }
        self.recent.push_back((key, image));
        while self.recent.len() > RECENT {
            self.recent.pop_front();
        }
    }

    fn recall(&mut self, key: &Key) -> Option<Image> {
        let at = self.recent.iter().position(|(kept, _)| kept == key)?;
        let (key, image) = self.recent.remove(at)?;
        self.recent.push_back((key, image.clone()));
        Some(image)
    }
}

/// Decode a file exactly as a preview would, for the benchmarks. `Ok` carries the RGBA that
/// would be handed to Rune's cache.
#[cfg(test)]
pub(crate) fn bench_decode(path: &Path, max: u32) -> Result<(u32, u32, Vec<u8>), String> {
    match decode(&Key {
        path: path.to_path_buf(),
        modified: None,
        pixels: max,
    }) {
        Decoded::Rgba {
            width,
            height,
            pixels,
        } => Ok((width, height, pixels)),
        Decoded::Failed(error) => Err(error),
        Decoded::ViaRune(_) => Err("not a raster preview".into()),
    }
}

/// Decode and hand the result to Rune's cache, waiting for its mips on this worker. The landed
/// preview is therefore ready to draw, and the UI thread does no image work at all.
fn render(key: &Key, cache: Option<&ImageCache>) -> Finished {
    let finished = |image, error| Finished {
        key: key.clone(),
        image,
        error,
    };
    match decode(key) {
        Decoded::Rgba {
            width,
            height,
            pixels,
        } => match cache {
            Some(cache) => finished(Some(settle(cache.rgba(width, height, pixels))), None),
            // Before setup (a unit test) there is nowhere to put the pixels.
            None => finished(None, None),
        },
        Decoded::ViaRune(path) => match cache {
            Some(cache) => finished(
                Some(settle(cache.file(path, PREVIEW_POINTS, PREVIEW_SCALE))),
                None,
            ),
            None => finished(None, None),
        },
        Decoded::Failed(error) => finished(None, Some(error)),
    }
}

/// Block until Rune has built the handle's mips. On the web there is no blocking; the handle
/// lands pending, which this desktop app never hits.
fn settle(image: Image) -> Image {
    #[cfg(not(target_arch = "wasm32"))]
    let _ = image.wait();
    image
}

fn decode(key: &Key) -> Decoded {
    let extension = key
        .path
        .extension()
        .map(|e| e.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if matches!(extension.as_str(), "svg" | "svgz") {
        return Decoded::ViaRune(key.path.clone());
    }
    let bytes = match std::fs::read(&key.path) {
        Ok(bytes) => bytes,
        Err(err) => return Decoded::Failed(err.to_string()),
    };
    match image::guess_format(&bytes) {
        Ok(ImageFormat::Jpeg) => jpeg(&bytes, key),
        Ok(ImageFormat::Png) => png(&bytes, key),
        // SVG (sniffed by Rune) and anything else Rune's cache either decodes or reports in
        // the inspector as before.
        _ => Decoded::ViaRune(key.path.clone()),
    }
}

/// Decode a JPEG with the smallest DCT scale that still covers the preview, so a large photo
/// never becomes a full-resolution RGBA buffer. A picture too small for a half-size decode is
/// handed to Rune: decoding whole is faster in its decoder, and needs no rewrite.
fn jpeg(bytes: &[u8], key: &Key) -> Decoded {
    match scaled_jpeg(bytes, key.pixels) {
        Ok(Some((width, height, pixels))) => Decoded::Rgba {
            width,
            height,
            pixels,
        },
        Ok(None) => Decoded::ViaRune(key.path.clone()),
        Err(err) => Decoded::Failed(err),
    }
}

fn scaled_jpeg(bytes: &[u8], max: u32) -> Result<Option<(u32, u32, Vec<u8>)>, String> {
    use jpeg_decoder::PixelFormat;
    let mut decoder = jpeg_decoder::Decoder::new(Cursor::new(bytes));
    decoder.set_max_decoding_buffer_size(MAX_ALLOC as usize);
    decoder.read_info().map_err(|err| err.to_string())?;
    let info = decoder.info().ok_or("no JPEG header")?;
    if !matches!(info.pixel_format, PixelFormat::RGB24 | PixelFormat::L8) {
        return Err(format!(
            "unsupported JPEG pixel format {:?}",
            info.pixel_format
        ));
    }
    let format = info.pixel_format;
    let (width, height) = (u32::from(info.width), u32::from(info.height));
    // A DCT scale can shrink by 1/2, 1/4 or 1/8. Below twice the preview there is no scale that
    // still covers it, and decoding whole is slower here than in Rune's decoder, so hand it over.
    if width.max(height) < max.saturating_mul(2) {
        return Ok(None);
    }
    let orientation = orientation_of(bytes);
    let (target_width, target_height) = fit(width, height, max);
    decoder
        .scale(target_width as u16, target_height as u16)
        .map_err(|err| err.to_string())?;
    let decoded = decoder.decode().map_err(|err| err.to_string())?;
    let info = decoder.info().ok_or("no JPEG header")?;
    let (width, height) = (u32::from(info.width), u32::from(info.height));
    let pixels = match format {
        PixelFormat::L8 => decoded.iter().flat_map(|&v| [v, v, v, 255]).collect(),
        _ => decoded
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 255])
            .collect(),
    };
    finish(width, height, pixels, orientation, max).map(Some)
}

/// PNG has no scaled decode: inflate, then shrink before Rune builds its mips. A picture too
/// small to gain from the resize goes to Rune untouched.
fn png(bytes: &[u8], key: &Key) -> Decoded {
    match scaled_png(bytes, key.pixels) {
        Ok(Some((width, height, pixels))) => Decoded::Rgba {
            width,
            height,
            pixels,
        },
        Ok(None) => Decoded::ViaRune(key.path.clone()),
        Err(err) => Decoded::Failed(err),
    }
}

fn scaled_png(bytes: &[u8], max: u32) -> Result<Option<(u32, u32, Vec<u8>)>, String> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|err| err.to_string())?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_ALLOC);
    reader.limits(limits.clone());
    let mut decoder = reader.into_decoder().map_err(|err| err.to_string())?;
    let (width, height) = decoder.dimensions();
    // Under twice the preview a resize saves little; Rune's own path is at least as fast.
    if width.max(height) < max.saturating_mul(2) {
        return Ok(None);
    }
    let orientation = decoder.orientation().unwrap_or(Orientation::NoTransforms);
    limits
        .reserve(decoder.total_bytes())
        .map_err(|err| err.to_string())?;
    decoder.set_limits(limits).map_err(|err| err.to_string())?;
    let mut image = DynamicImage::from_decoder(decoder).map_err(|err| err.to_string())?;
    image.apply_orientation(orientation);
    if image.width() > max || image.height() > max {
        image = image.thumbnail(max, max);
    }
    let image = image.into_rgba8();
    Ok(Some((image.width(), image.height(), image.into_raw())))
}

fn finish(
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    orientation: Orientation,
    max: u32,
) -> Result<(u32, u32, Vec<u8>), String> {
    let image = RgbaImage::from_raw(width, height, pixels)
        .ok_or("decoded pixels do not match the image size")?;
    let mut image = DynamicImage::ImageRgba8(image);
    image.apply_orientation(orientation);
    if image.width() > max || image.height() > max {
        image = image.resize(max, max, image::imageops::FilterType::Triangle);
    }
    let image = image.into_rgba8();
    Ok((image.width(), image.height(), image.into_raw()))
}

/// The biggest box with the picture's aspect ratio that fits in `max` square.
fn fit(width: u32, height: u32, max: u32) -> (u32, u32) {
    let scale = (max as f64 / width.max(height).max(1) as f64).min(1.0);
    (
        ((width as f64 * scale).round() as u32).max(1),
        ((height as f64 * scale).round() as u32).max(1),
    )
}

/// EXIF orientation from the container's header only; the pixels still come from the scaled
/// JPEG decode below, which does not read EXIF itself.
fn orientation_of(bytes: &[u8]) -> Orientation {
    ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .ok()
        .and_then(|reader| reader.into_decoder().ok())
        .and_then(|mut decoder| decoder.orientation().ok())
        .unwrap_or(Orientation::NoTransforms)
}
