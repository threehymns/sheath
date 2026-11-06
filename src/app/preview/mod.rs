//! Picture previews, decoded at the size the inspector draws them.
//!
//! Rune's `ImageCache::file` decodes a raster at full resolution and builds a mip chain from
//! that, even when the caller asks for a small preview (the size only applies to SVG). A 12 MP
//! photo therefore costs ~0.4 s: ~120 ms decoding and ~280 ms in the mip chain. Here the decode
//! itself is limited to the preview size (JPEG by DCT scaling, PNG and the other rasters by
//! shrinking after decode) and the result is handed to Rune as RGBA, where its mip chain only
//! covers the small image. PNGs and JPEGs that already fit in the pane skip all of that and are
//! handed to Rune as they are; the other rasters, and AVIF/HEIC, are decoded here, since Rune
//! cannot read them.
//!
//! Decodes run through `Latest`, like folder scans: a newer selection supersedes a queued one
//! and a short debounce keeps a held arrow key from decoding every row on the way past. The
//! worker also waits for Rune's mips, so a preview lands ready to draw: the frame hook only
//! drains finished work, and the window never wakes for a handle that still paints nothing. The
//! last few previews are kept as image handles, so going back to one is instant.
//!
//! The decoders live in `decode`, and only the native build has them. A shared folder's pictures
//! are decoded by the browser at preview size, and the demo tree's by the browser's own codecs.

#[cfg(not(target_arch = "wasm32"))]
mod apk;
#[cfg(not(target_arch = "wasm32"))]
mod decode;
#[cfg(not(target_arch = "wasm32"))]
use decode::decode;
use rune::{Image, ImageCache, Latest, Waker};
use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

/// The inspector's picture area: 420 pt at 2 device pixels per point.
pub(super) const PREVIEW_POINTS: f32 = 420.0;
pub(super) const PREVIEW_SCALE: f32 = 2.0;
const MAX_PIXELS: u32 = (PREVIEW_POINTS * PREVIEW_SCALE) as u32;
// Recent renders kept as decoded handles. At preview size each is a few megabytes.
const RECENT: usize = 6;
// A held arrow key selects faster than this; waiting it out decodes only the row it stops on.
const DEBOUNCE: Duration = Duration::from_millis(25);

#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) struct Key {
    pub(super) path: PathBuf,
    // An edited file has a new key, so an old preview is never shown for it.
    pub(super) modified: Option<SystemTime>,
    pub(super) pixels: u32,
}

/// A decoded preview, ready for a frame to draw. `image` is None when this app could not make
/// one and `error` says why.
pub(super) struct Finished {
    pub(super) key: Key,
    pub(super) image: Option<Image>,
    pub(super) error: Option<String>,
}

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
pub(super) enum Decoded {
    Rgba {
        width: u32,
        height: u32,
        pixels: Vec<u8>,
    },
    // SVG, or a format only Rune knows: let its image cache read and decode it.
    ViaRune(PathBuf),
    // SVG drawn from a package's vector icon, rasterized at the size it is asked for.
    Svg(Vec<u8>),
    Failed(String),
}

impl Decoded {
    /// Hand a decode to Rune's cache, waiting for its mips on this worker, so the landed image
    /// is ready to draw. `pixels` is the decode's longest side and sizes an SVG handed over.
    /// Before `setup` there is nowhere to put the pixels, so the answer is empty: the caller
    /// asks again once the cache is attached.
    pub(super) fn into_image(
        self,
        cache: Option<&ImageCache>,
        pixels: u32,
    ) -> (Option<Image>, Option<String>) {
        match self {
            Decoded::Rgba {
                width,
                height,
                pixels: data,
            } => match cache {
                Some(cache) => (Some(settle(cache.rgba(width, height, data))), None),
                None => (None, None),
            },
            Decoded::ViaRune(path) => match cache {
                Some(cache) => (
                    Some(settle(cache.file(
                        &path,
                        pixels as f32 / PREVIEW_SCALE,
                        PREVIEW_SCALE,
                    ))),
                    None,
                ),
                None => (None, None),
            },
            Decoded::Svg(svg) => match cache {
                Some(cache) => (
                    Some(settle(cache.svg(
                        svg,
                        pixels as f32 / PREVIEW_SCALE,
                        PREVIEW_SCALE,
                    ))),
                    None,
                ),
                None => (None, None),
            },
            Decoded::Failed(error) => (None, Some(error)),
        }
    }
}

/// Decode a file at a chosen longest side, for the thumbnails, which share the preview's
/// decoders but not its size.
pub(super) fn decode_at(path: &Path, pixels: u32) -> Decoded {
    decode(&Key {
        path: path.to_path_buf(),
        modified: None,
        pixels,
    })
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
        // A shared folder's bytes only reach the page through the browser, which decodes the
        // picture itself; `take` lands its pixels once they arrive.
        #[cfg(target_arch = "wasm32")]
        if crate::vfs::request_preview(path, modified, MAX_PIXELS) {
            self.jobs.cancel();
            return None;
        }
        let cache = self.cache.clone();
        self.jobs
            .request(key, move |key| render(&key, cache.as_deref()));
        None
    }

    pub(super) fn take(&mut self) -> Option<Finished> {
        if let Some(finished) = self.jobs.take() {
            return Some(finished);
        }
        #[cfg(target_arch = "wasm32")]
        if let Some(decoded) = crate::vfs::take_preview() {
            let crate::vfs::Decoded {
                path,
                modified,
                width,
                height,
                pixels,
                error,
            } = decoded;
            let key = Key {
                path,
                modified,
                pixels: MAX_PIXELS,
            };
            let image = match (&error, self.cache.as_ref()) {
                (None, Some(cache)) => Some(settle(cache.rgba(width, height, pixels))),
                _ => None,
            };
            return Some(Finished { key, image, error });
        }
        None
    }

    #[cfg(test)]
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
        Decoded::ViaRune(_) | Decoded::Svg(_) => Err("not a raster preview".into()),
    }
}

/// Decode and hand the result to Rune's cache, waiting for its mips on this worker. The landed
/// preview is therefore ready to draw, and the UI thread does no image work at all.
fn render(key: &Key, cache: Option<&ImageCache>) -> Finished {
    let (image, error) = decode(key).into_image(cache, key.pixels);
    Finished {
        key: key.clone(),
        image,
        error,
    }
}

/// Block until Rune has built the handle's mips. On the web there is no blocking; the handle
/// lands pending, which this desktop app never hits.
pub(super) fn settle(image: Image) -> Image {
    #[cfg(not(target_arch = "wasm32"))]
    let _ = image.wait();
    image
}

#[cfg(target_arch = "wasm32")]
fn decode(key: &Key) -> Decoded {
    // Shared pictures are decoded by the browser, which `Previewer::request` asks first; this
    // only runs when that declined, so the browser is also the one that said no.
    if crate::vfs::is_granted(&key.path) {
        return Decoded::Failed("The browser could not preview that shared picture.".into());
    }
    // The browser fetches the demo tree's pictures itself and decodes them with its own
    // codecs, so they go to Rune's cache as a URL beside the page.
    Decoded::ViaRune(key.path.clone())
}
