//! A browser tab has no desktop icon theme, so the built-in glyphs draw every row.

use crate::app::Entry;
use rune::{Image, ImageCache};
use std::sync::Arc;

#[derive(Default)]
pub struct SystemIcons;

impl SystemIcons {
    pub fn attach(&mut self, _cache: Arc<ImageCache>) {}

    pub fn image(&self, _entry: &Entry) -> Option<Image> {
        None
    }
}
