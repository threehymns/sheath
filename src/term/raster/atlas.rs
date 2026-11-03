//! The glyph atlas: swash rasterizes the pane's mono face and caches masks by character, size
//! and weight. Composition asks for a glyph and never sees the font or the scaler.
//!
//! A character the mono cannot draw comes from a system fallback: the colour emoji font first,
//! since emoji draw in their own colours (swash's COLR layers or embedded bitmaps, the same
//! sources Rune uses), then the Nerd Font for a prompt's private-use icons.

use std::collections::HashMap;

use swash::FontRef;
use swash::scale::image::Content;
use swash::scale::{Render, ScaleContext, Scaler, Source, StrikeWith};

use crate::term::grid::Frame;

/// The face the pane registers as its mono font, so a glyph here is the glyph the text path
/// would have drawn: the theme's mono, which draws every ordinary cell.
const PRIMARY: &[u8] = crate::FONTS[1];

pub(super) struct Glyph {
    pub(super) left: i32,
    pub(super) top: i32,
    pub(super) width: u32,
    pub(super) height: u32,
    /// One alpha byte per pixel, or four (RGBA) when [`Self::color`].
    pub(super) pixels: Vec<u8>,
    /// A colour glyph, drawn in its own colours instead of the pen's: an emoji from the system's
    /// emoji font.
    pub(super) color: bool,
}

pub(super) struct Atlas {
    font: FontRef<'static>,
    /// The system's colour emoji font, for the emoji the mono has none of.
    emoji: Option<FontRef<'static>>,
    /// The Nerd Font the system has, for the private-use glyphs the mono has none of.
    fallback: Option<FontRef<'static>>,
    context: ScaleContext,
    glyphs: HashMap<(char, u32, bool), Option<Glyph>>,
}

impl Atlas {
    pub(super) fn new() -> Atlas {
        Atlas {
            font: FontRef::from_index(PRIMARY, 0).expect("the pane's mono face"),
            emoji: crate::term::fonts::emoji_face().and_then(|bytes| FontRef::from_index(bytes, 0)),
            fallback: crate::term::fonts::fallback_face()
                .and_then(|bytes| FontRef::from_index(bytes, 0)),
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
        let (font, id) = self.face_for(ch)?;
        let mut scaler = self.context.builder(font).size(size as f32).build();
        // An emoji font draws the glyph as COLR layers or an embedded bitmap, in colours the pen
        // has no say over; the same two sources Rune rasterizes its emoji with.
        if let Some(glyph) = color(&mut scaler, id) {
            return Some(glyph);
        }
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
            pixels: image.data,
            color: false,
        })
    }

    /// The face that draws `ch`: the theme's mono, and only for a character it has no glyph for,
    /// the system's colour emoji font and then the installed Nerd Font. Ordinary text never
    /// changes face; an emoji comes from the font made for it, and the private-use icons a prompt
    /// prints from the font that was patched for them.
    fn face_for(&self, ch: char) -> Option<(FontRef<'static>, u16)> {
        let id = self.font.charmap().map(ch);
        if id != 0 {
            return Some((self.font, id));
        }
        for font in [self.emoji, self.fallback].into_iter().flatten() {
            let id = font.charmap().map(ch);
            if id != 0 {
                return Some((font, id));
            }
        }
        None
    }
}

/// A colour image of `id`, when the face draws it as COLR layers or an embedded bitmap. `None`
/// for a face with no colour tables, or a glyph that is an ordinary outline there.
fn color(scaler: &mut Scaler, id: u16) -> Option<Glyph> {
    if !scaler.has_color_bitmaps() && !scaler.has_color_outlines() {
        return None;
    }
    let image = Render::new(&[
        Source::ColorOutline(0),
        Source::ColorBitmap(StrikeWith::BestFit),
    ])
    .render(scaler, id)?;
    let (width, height) = (image.placement.width, image.placement.height);
    if image.content != Content::Color
        || width == 0
        || height == 0
        || image.data.len() != (width * height * 4) as usize
    {
        return None;
    }
    Some(Glyph {
        left: image.placement.left,
        top: image.placement.top,
        width,
        height,
        pixels: image.data,
        color: true,
    })
}
