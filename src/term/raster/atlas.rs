//! The glyph atlas: swash rasterizes the pane's mono face and caches masks by character, size
//! and weight. Composition asks for a glyph and never sees the font or the scaler.

use std::collections::HashMap;

use swash::FontRef;
use swash::scale::image::Content;
use swash::scale::{Render, ScaleContext, Source};

use crate::term::grid::Frame;

/// The face the pane registers as its mono font, so a glyph here is the glyph the text path
/// would have drawn.
const FONT: &[u8] = crate::FONTS[1];

pub(super) struct Glyph {
    pub(super) left: i32,
    pub(super) top: i32,
    pub(super) width: u32,
    pub(super) height: u32,
    pub(super) mask: Vec<u8>,
}

pub(super) struct Atlas {
    font: FontRef<'static>,
    context: ScaleContext,
    glyphs: HashMap<(char, u32, bool), Option<Glyph>>,
}

impl Atlas {
    pub(super) fn new() -> Atlas {
        Atlas {
            font: FontRef::from_index(FONT, 0).expect("the pane's mono face"),
            context: ScaleContext::new(),
            glyphs: HashMap::new(),
        }
    }

    /// Rasterizes every glyph the frame needs before the row workers read the atlas. A blank
    /// cell has no glyph, and a zero-width format character rasterizes to nothing anyway.
    pub(super) fn preload(&mut self, frame: &Frame, size: u32) {
        for line in std::iter::once(frame.lead.as_ref())
            .chain(frame.lines.iter().take(frame.rows as usize).map(Some))
            .chain(std::iter::once(frame.trail.as_ref()))
            .flatten()
        {
            for run in &line.runs {
                let text = line.text.get(run.start..run.end).unwrap_or_default();
                for ch in text.chars() {
                    if ch.is_control() || ch == ' ' {
                        continue;
                    }
                    let _ = self.load(ch, size, run.pen.bold);
                }
            }
        }
    }

    /// A glyph the frame was preloaded with, or `None` when the face has no outline for it.
    pub(super) fn glyph(&self, ch: char, size: u32, bold: bool) -> Option<&Glyph> {
        self.glyphs.get(&(ch, size, bold)).and_then(Option::as_ref)
    }

    fn load(&mut self, ch: char, size: u32, bold: bool) -> Option<&Glyph> {
        let key = (ch, size, bold);
        if !self.glyphs.contains_key(&key) {
            let glyph = self.rasterize(ch, size, bold);
            self.glyphs.insert(key, glyph);
        }
        self.glyphs.get(&key).and_then(Option::as_ref)
    }

    fn rasterize(&mut self, ch: char, size: u32, bold: bool) -> Option<Glyph> {
        let id = self.font.charmap().map(ch);
        if id == 0 {
            return None;
        }
        let mut scaler = self.context.builder(self.font).size(size as f32).build();
        let mut render = Render::new(&[Source::Outline]);
        if bold {
            // There is no bold mono face registered, so a bold run is the regular outline with
            // a little weight added, the way the shaper fakes it.
            render.embolden(size as f32 * 0.03);
        }
        let image = render.render(&mut scaler, id)?;
        if image.content != Content::Mask
            || image.placement.width == 0
            || image.placement.height == 0
        {
            return None;
        }
        Some(Glyph {
            left: image.placement.left,
            top: image.placement.top,
            width: image.placement.width,
            height: image.placement.height,
            mask: image.data,
        })
    }
}
