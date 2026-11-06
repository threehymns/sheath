//! Picture thumbnails for the list and the grid.
//!
//! A thumbnail is a preview decoded at thumbnail size, so a folder of pictures asks for only
//! the rows on screen, plus a little overscan for a scroll, rather than every file in it. Each
//! picture keeps its own small job on the shared pool — a held scroll must not supersede the
//! thumbnails already on their way — and finished ones are kept, with the pictures on screen
//! protected from eviction, so coming back to a picture is instant. A picture that will not
//! decode is remembered as failed, so the row keeps its file icon instead of asking again every frame.
//!
//! Every picture in the window is offered at once, so the pool's four workers always have the
//! next decode ready instead of waiting for the next frame to hand them one. Offering only a
//! few made a folder load at the frame rate rather than at the decoders' rate: each landing
//! had to wake the window just to start more work. The window is already small, and a scroll
//! that leaves it drops the jobs it no longer wants, so the queue cannot grow into stale work.
//! The request pass only reruns when the window changes, so an idle scroll does no per-frame
//! work here at all.

use rune::{Image, ImageCache, Latest, Waker};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::Path;
use std::sync::Arc;
use std::time::SystemTime;

use super::preview::{Finished, Key, decode_at};
use super::{App, GRID_ROW_HEIGHT, ROW_HEIGHT};

/// The decode's longest side in device pixels: the grid's tile at twice its point size. The
/// list draws the same pixels scaled down by Rune's mips, so one size serves both layouts.
const THUMB_PIXELS: u32 = 96;
/// Kept offscreen renders and failures, beyond the pictures currently on screen. An edited
/// file's stale entry is evicted by a new one, and the earliest offscreen pictures go first.
const CACHE_LIMIT: usize = 256;
/// Rows around the viewport that decode ahead of the scroll.
const OVERSCAN: usize = 5;
/// Rows asked for before the first layout has sized the viewport.
const FIRST_PAGE: usize = 48;

#[derive(Default)]
pub(super) struct Thumbs {
    pending: HashMap<Key, Latest<Finished>>,
    // A shared folder's picture, waiting for the browser to decode it.
    waiting: HashSet<Key>,
    done: HashMap<Key, Image>,
    failed: HashSet<Key>,
    order: VecDeque<Key>,
    // The window from the last request pass. These pictures are never evicted while they are
    // on screen, even if there are more of them than the offscreen cache keeps.
    pinned: HashSet<Key>,
    // The window the queue was built for, so an unchanged window costs nothing per frame.
    window: Option<(usize, usize, u64)>,
    // An answer arrived before the image cache was attached and must be asked for again.
    retry: bool,
    cache: Option<Arc<ImageCache>>,
    wake: Option<Waker>,
}

impl Thumbs {
    pub(super) fn set_waker(&mut self, waker: Waker) {
        self.wake = Some(waker);
    }

    pub(super) fn attach(&mut self, cache: Arc<ImageCache>) {
        self.cache = Some(cache);
    }

    /// Starts a request pass for one window's pictures: pins them against eviction, drops
    /// pending jobs for pictures that left the window, and starts the missing ones in order.
    pub(super) fn queue(&mut self, pictures: &[Key], sig: (usize, usize, u64)) {
        self.window = Some(sig);
        self.retry = false;
        self.pinned = pictures.iter().cloned().collect();
        self.pending.retain(|key, _| self.pinned.contains(key));
        self.waiting.retain(|key| self.pinned.contains(key));
        // Every picture in the window is offered, not a batch to be topped up by later
        // frames: with the pool's queue kept full, decoding no longer waits on the window's
        // frame rate. A picture that scrolls away is dropped from `pending` by the next pass.
        for key in pictures {
            if self.done.contains_key(key)
                || self.failed.contains(key)
                || self.pending.contains_key(key)
                || self.waiting.contains(key)
            {
                continue;
            }
            self.start(key);
        }
    }

    /// Decodes one picture on the pool, unless it is already kept, failed, or in flight.
    fn start(&mut self, key: &Key) {
        // A shared folder's bytes only reach the page through the browser, which decodes the
        // picture itself; `take` lands its pixels once they arrive. Its answer waits on its
        // own queue, so it cannot steal the inspector's.
        #[cfg(target_arch = "wasm32")]
        if crate::vfs::request_thumbnail(&key.path, key.modified, THUMB_PIXELS) {
            self.waiting.insert(key.clone());
            return;
        }
        let cache = self.cache.clone();
        let job = match &self.wake {
            Some(wake) => Latest::new(wake.clone()),
            None => Latest::default(),
        };
        let key = key.clone();
        job.request(key.clone(), move |key| {
            let (image, error) =
                decode_at(&key.path, THUMB_PIXELS).into_image(cache.as_deref(), THUMB_PIXELS);
            Finished { key, image, error }
        });
        self.pending.insert(key, job);
    }

    /// Drains the answers that landed, keeping the pictures and remembering the failures.
    /// `true` when anything landed, which asks for a frame to draw it.
    pub(super) fn take(&mut self) -> bool {
        let mut landed = false;
        for (key, job) in std::mem::take(&mut self.pending) {
            match job.take() {
                Some(finished) => {
                    // Before `setup` there is nowhere to put the pixels; ask again next pass.
                    if finished.image.is_none() && finished.error.is_none() {
                        self.retry = true;
                        continue;
                    }
                    self.store(finished.key, finished.image);
                    landed = true;
                }
                // Still running: put the job back for a later frame.
                None if job.is_pending() => {
                    self.pending.insert(key, job);
                }
                // Settled without an answer. Dropping a `Latest` cancels it, so it stays gone.
                None => {}
            }
        }
        #[cfg(target_arch = "wasm32")]
        while let Some(decoded) = crate::vfs::take_thumbnail() {
            let key = Key {
                path: decoded.path,
                modified: decoded.modified,
                pixels: THUMB_PIXELS,
            };
            let image = match (&decoded.error, self.cache.as_ref()) {
                (None, Some(cache)) => Some(super::preview::settle(cache.rgba(
                    decoded.width,
                    decoded.height,
                    decoded.pixels,
                ))),
                _ => None,
            };
            self.store(key, image);
            landed = true;
        }
        landed
    }

    #[cfg(test)]
    pub(super) fn is_pending(&self) -> bool {
        self.pending.values().any(Latest::is_pending) || !self.waiting.is_empty()
    }

    /// Drops the in-flight jobs, keeping the kept pictures. The browser's own answers are
    /// left where they are; if one lands later it is kept, and the cache stays bounded.
    pub(super) fn clear_waiting(&mut self) {
        self.pending.clear();
        self.waiting.clear();
        self.pinned.clear();
        self.window = None;
        self.retry = false;
    }

    /// The kept thumbnail for a picture, ready to draw now.
    pub(super) fn image(&self, path: &Path, modified: Option<SystemTime>) -> Option<Image> {
        self.done
            .get(&Key {
                path: path.to_path_buf(),
                modified,
                pixels: THUMB_PIXELS,
            })
            .cloned()
    }

    pub(super) fn has(&self, path: &Path, modified: Option<SystemTime>) -> bool {
        self.done.contains_key(&Key {
            path: path.to_path_buf(),
            modified,
            pixels: THUMB_PIXELS,
        })
    }

    fn store(&mut self, key: Key, image: Option<Image>) {
        if let Some(image) = image {
            self.done.insert(key.clone(), image);
        } else {
            self.failed.insert(key.clone());
        }
        self.order.push_back(key);
        let over = (self.done.len() + self.failed.len()).saturating_sub(CACHE_LIMIT);
        if over == 0 {
            return;
        }
        // Only offscreen pictures are candidates: the window's thumbnails stay put, even
        // when they alone are larger than the offscreen cache.
        let stale: HashSet<Key> = self
            .order
            .iter()
            .filter(|key| !self.pinned.contains(key))
            .take(over)
            .cloned()
            .collect();
        for old in &stale {
            self.done.remove(old);
            self.failed.remove(old);
        }
        self.order.retain(|key| !stale.contains(key));
    }
}

impl App {
    /// The rows the thumbnail window covers: the viewport's slice of the list or grid, plus
    /// overscan. Before the first layout sized the viewport, one page is asked for.
    fn thumb_window(&self) -> (usize, usize) {
        let offset = self.told_offset();
        let height = self.rows_view[3];
        if self.is_grid() {
            let columns = self.grid_columns().max(1);
            let (first, last) = if height <= 0.0 {
                (0, self.grid_rows())
            } else {
                (
                    ((offset / GRID_ROW_HEIGHT).floor() as usize).saturating_sub(OVERSCAN),
                    ((offset + height) / GRID_ROW_HEIGHT).ceil() as usize + OVERSCAN,
                )
            };
            (first * columns, last * columns)
        } else {
            let (first, last) = if height <= 0.0 {
                (0, self.visible.len().min(FIRST_PAGE))
            } else {
                (
                    ((offset / ROW_HEIGHT).floor() as usize).saturating_sub(OVERSCAN),
                    ((offset + height) / ROW_HEIGHT).ceil() as usize + OVERSCAN,
                )
            };
            (first, last.min(self.visible.len()))
        }
    }

    /// Queues the thumbnails the window shows and the rows it is scrolling into, once per
    /// window. An unchanged window is skipped entirely, so an idle scroll does no work here.
    pub(super) fn request_thumbs(&mut self) {
        if !self.config.show_thumbnails {
            // Turning them off drops the work in flight, and forgetting the window makes
            // turning them back on queue it again even when the visible rows never moved.
            self.thumbs.clear_waiting();
            return;
        }
        let (first, last) = self.thumb_window();
        let sig = (first, last, self.keys_version());
        if self.thumbs.window == Some(sig) && !self.thumbs.retry {
            return;
        }
        let pictures: Vec<Key> = (first..last)
            .filter_map(|i| self.visible.get(i))
            .map(|&i| &self.entries[i])
            .filter(|entry| entry.is_image || entry.is_apk)
            .map(|entry| Key {
                path: entry.path.clone(),
                modified: entry.modified,
                pixels: THUMB_PIXELS,
            })
            .collect();
        self.thumbs.queue(&pictures, sig);
    }

    /// A finished thumbnail, ready for a frame to draw. The window never wakes for a handle
    /// that still paints nothing: the worker waits for Rune's mips, like a preview.
    pub(super) fn land_thumbs(&mut self) -> bool {
        self.thumbs.take()
    }

    /// The row's thumbnail, if one has decoded and is kept.
    pub fn thumbnail(&self, entry: &crate::app::Entry) -> Option<Image> {
        self.thumbs.image(&entry.path, entry.modified)
    }

    /// The thumbnail for a file by its path and edit time, for tests that must not hold a row.
    #[cfg(test)]
    pub(crate) fn thumbnail_at(
        &self,
        path: &std::path::Path,
        modified: Option<SystemTime>,
    ) -> Option<Image> {
        self.thumbs.image(path, modified)
    }

    /// Whether the row already has a thumbnail to show.
    pub fn has_thumbnail(&self, entry: &crate::app::Entry) -> bool {
        self.thumbs.has(&entry.path, entry.modified)
    }

    /// Wait for the workers, then take their answers. Tests only: a window lands them in
    /// `poll`. An answer given before the image cache was attached asks again.
    #[cfg(test)]
    pub(crate) fn settle_thumbs(&mut self) -> bool {
        let mut landed = false;
        let until = std::time::Instant::now() + std::time::Duration::from_millis(2000);
        let mut retries = 0;
        for _ in 0..128 {
            self.request_thumbs();
            while self.thumbs.is_pending() && std::time::Instant::now() < until {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            let took = self.land_thumbs();
            landed |= took;
            if took {
                retries = 0;
            } else if self.thumbs.retry {
                retries += 1;
                if retries > 8 {
                    break;
                }
            } else {
                break;
            }
        }
        landed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The pool is offered the whole window in one pass. Offering a dozen at a time made each
    // batch wait for its slowest decode before the next could start, so a folder loaded at the
    // batch rate rather than at the decoders' rate.
    #[test]
    fn the_whole_window_is_offered_at_once() {
        let pictures: Vec<Key> = (0..40)
            .map(|i| Key {
                path: std::env::temp_dir().join(format!("sheath-missing-{i}.png")),
                modified: None,
                pixels: THUMB_PIXELS,
            })
            .collect();
        let mut thumbs = Thumbs::default();
        thumbs.queue(&pictures, (0, pictures.len(), 0));
        assert_eq!(
            thumbs.pending.len() + thumbs.waiting.len(),
            pictures.len(),
            "every picture in the window is in flight before the first frame"
        );
    }
}
