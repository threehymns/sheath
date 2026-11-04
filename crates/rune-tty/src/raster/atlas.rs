//! The glyph atlas: swash rasterizes the pane's faces and caches images by glyph, size and
//! weight. Composition asks for a cell's cluster and never sees the font, the shaper or the
//! scaler.
//!
//! A cell holds a whole grapheme cluster: `é` may be two codepoints, `👍🏽` two, a family emoji
//! several. A cluster is shaped rather than walked one character at a time, because shaping is
//! what picks the emoji font's ligature for `👍🏽` and places a combining mark on its base; one
//! character at a time put the modifier in a column of its own and drew the cell after it a
//! column too far along. A character the mono cannot draw comes from a system fallback: the
//! colour emoji font first, since emoji draw in their own colours (swash's COLR layers or
//! embedded bitmaps, the same sources Rune uses), then the Nerd Font for a prompt's private-use
//! icons.

use std::collections::HashMap;

use harfrust::{FontRef as ShapeFace, ShapeOptions, ShaperData, UnicodeBuffer};
use swash::FontRef;
use swash::scale::image::Content;
use swash::scale::{Render, ScaleContext, Scaler, Source, StrikeWith};
use unicode_segmentation::UnicodeSegmentation;

use crate::grid::Frame;

/// The face the pane registers as its mono font, so a glyph here is the glyph the text path
/// would have drawn: the theme's mono, which draws every ordinary cell.
const PRIMARY: &[u8] = crate::FONTS[1];

/// Whether a run is printable ASCII: every cell is one narrow glyph, so it needs neither
/// segmentation nor shaping, and its glyphs can be read by the byte.
pub fn ascii_run(text: &str) -> bool {
    text.bytes()
        .all(|byte| byte.is_ascii_graphic() || byte == b' ')
}

/// One glyph's image, placed around the pen.
pub struct Glyph {
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    /// One alpha byte per pixel, or four (RGBA) when [`Self::color`].
    pub pixels: Vec<u8>,
    /// A colour glyph, drawn in its own colours instead of the pen's: an emoji from the system's
    /// emoji font.
    pub color: bool,
}

/// One glyph of a shaped cluster: its pen offset across the cell, in device pixels, and the
/// image it draws.
pub struct Placed {
    pub x: i32,
    /// The image, as a slot in the atlas's own list: the painter reads it by index instead of
    /// hashing a font, a glyph, a size and a weight for every cell it draws.
    pub slot: usize,
}

/// A face the atlas draws from: swash's view of it, and the bytes the shaper reads.
struct Face {
    swash: FontRef<'static>,
    bytes: &'static [u8],
}

/// A cluster as one size and weight drew it: the size, the weight, and the glyphs it placed.
struct Drawn {
    size: u32,
    bold: bool,
    placed: Vec<Placed>,
}

/// A cluster shaped once, whatever size it is drawn at: the face that draws it, that face's
/// units per em, and each glyph with the pen it starts at in font units.
#[derive(Clone)]
struct Shape {
    face: usize,
    upem: u16,
    glyphs: Vec<(u16, i32)>,
}

/// The glyph slot of each printable ASCII character, at one size and weight.
struct AsciiGlyphs {
    size: u32,
    bold: bool,
    slots: [u32; 128],
}

/// A slot never asked for, and one asked for that found no glyph: remembering the second is
/// what keeps a character no face draws from being rasterized again on every frame.
const ASCII_UNLOADED: u32 = u32::MAX;
const ASCII_MISSING: u32 = u32::MAX - 1;

pub struct Atlas {
    /// The theme's mono first, then the fallbacks.
    faces: Vec<Face>,
    /// The colour emoji face among `faces`, when the system has one.
    emoji: Option<usize>,
    /// The Nerd Font among `faces`, when the system has one.
    fallback: Option<usize>,
    context: ScaleContext,
    /// Shaped clusters, by their text.
    shapes: HashMap<Box<str>, Option<Shape>>,
    /// The slot of each rasterized image, by face, glyph, size and weight.
    images: HashMap<(usize, u16, u32, bool), Option<usize>>,
    /// The images themselves, in the order they were first drawn.
    slots: Vec<Option<Glyph>>,
    /// Each cluster's glyphs, by cluster text, then by the size and weight they were drawn at:
    /// one string hash per cell and a scan over one or two entries, which is what keeps
    /// ordinary text as cheap as it was before clusters.
    placed: HashMap<Box<str>, Vec<Drawn>>,
    /// Printable ASCII glyphs by byte, at the sizes and weights they were drawn at: ordinary
    /// terminal text, read with an index instead of a hash of a one-character string.
    ascii: Vec<AsciiGlyphs>,
}

impl Atlas {
    pub fn new() -> Atlas {
        let mut atlas = Atlas {
            faces: vec![face(PRIMARY).expect("the pane's mono face")],
            emoji: None,
            fallback: None,
            context: ScaleContext::new(),
            shapes: HashMap::new(),
            images: HashMap::new(),
            slots: Vec::new(),
            placed: HashMap::new(),
            ascii: Vec::new(),
        };
        // The colour emoji face before the Nerd Font: an emoji is drawn by the font made for
        // it, and the private-use icons a prompt prints by the font patched for them.
        if let Some(emoji) = crate::fonts::emoji_face().and_then(face) {
            atlas.emoji = Some(atlas.faces.len());
            atlas.faces.push(emoji);
        }
        if let Some(fallback) = crate::fonts::fallback_face().and_then(face) {
            atlas.fallback = Some(atlas.faces.len());
            atlas.faces.push(fallback);
        }
        atlas
    }

    /// Shapes and rasterizes every cluster the bands being painted need before the row workers
    /// read the atlas, and only those: a frame that repaints one row does not walk the screen. A
    /// blank cell has no glyph, and a cluster of only format characters rasterizes to nothing
    /// anyway.
    pub fn preload(&mut self, frame: &Frame, size: u32, bands: &[usize]) {
        for &band in bands {
            let Some(line) = super::band_line(frame, band) else {
                continue;
            };
            for run in &line.runs {
                let text = line.text.get(run.start..run.end).unwrap_or_default();
                if ascii_run(text) {
                    self.preload_ascii(text, size, run.pen.bold);
                    continue;
                }
                for cluster in text.graphemes(true) {
                    self.load(cluster, size, run.pen.bold);
                }
            }
        }
    }

    /// The glyphs a cell's cluster draws, as preloaded. Empty when no face has a glyph for it.
    pub fn cluster(&self, cluster: &str, size: u32, bold: bool) -> &[Placed] {
        let Some(drawn) = self.placed.get(cluster) else {
            return &[];
        };
        drawn
            .iter()
            .find(|drawn| drawn.size == size && drawn.bold == bold)
            .map_or(&[], |drawn| drawn.placed.as_slice())
    }

    /// A cluster's glyph image, as preloaded.
    pub fn glyph(&self, slot: usize) -> Option<&Glyph> {
        self.slots.get(slot).and_then(Option::as_ref)
    }

    /// A printable ASCII cell's glyph slot, as [`preload`](Self::preload) filled it.
    pub fn ascii_glyph(&self, byte: u8, size: u32, bold: bool) -> Option<usize> {
        let set = self
            .ascii
            .iter()
            .find(|set| set.size == size && set.bold == bold)?;
        match set.slots.get(usize::from(byte)).copied()? {
            ASCII_UNLOADED | ASCII_MISSING => None,
            slot => Some(slot as usize),
        }
    }

    /// Rasterizes the glyphs an ASCII run needs, before the row workers read the atlas.
    fn preload_ascii(&mut self, text: &str, size: u32, bold: bool) {
        for byte in text.bytes() {
            if byte != b' ' {
                self.ascii_slot(byte, size, bold);
            }
        }
    }

    /// `byte`'s slot at `size` and `bold`, rasterized the first time. The set is found by a
    /// scan of one or two entries, which is cheaper than hashing a one-character string for
    /// every cell of every row.
    fn ascii_slot(&mut self, byte: u8, size: u32, bold: bool) -> u32 {
        if !byte.is_ascii() {
            return ASCII_MISSING;
        }
        let at = match self
            .ascii
            .iter()
            .position(|set| set.size == size && set.bold == bold)
        {
            Some(at) => at,
            None => {
                self.ascii.push(AsciiGlyphs {
                    size,
                    bold,
                    slots: [ASCII_UNLOADED; 128],
                });
                self.ascii.len() - 1
            }
        };
        let slot = self.ascii[at].slots[usize::from(byte)];
        if slot != ASCII_UNLOADED {
            return slot;
        }
        let cluster = char::from(byte).to_string();
        // Remember it where shaping would have put it as well: a frame that marks the cell's
        // column wide draws the character through the cluster path, which reads `placed`.
        let placed = self.place(&cluster, size, bold);
        let slot = placed
            .first()
            .map_or(ASCII_MISSING, |placed| placed.slot as u32);
        let drawn = self.placed.entry(cluster.into()).or_default();
        if !drawn
            .iter()
            .any(|drawn| drawn.size == size && drawn.bold == bold)
        {
            drawn.push(Drawn { size, bold, placed });
        }
        self.ascii[at].slots[usize::from(byte)] = slot;
        slot
    }

    fn load(&mut self, cluster: &str, size: u32, bold: bool) {
        let drawn = self.placed.get(cluster).is_some_and(|drawn| {
            drawn
                .iter()
                .any(|drawn| drawn.size == size && drawn.bold == bold)
        });
        if drawn {
            return;
        }
        let placed = self.place(cluster, size, bold);
        self.placed
            .entry(cluster.into())
            .or_default()
            .push(Drawn { size, bold, placed });
    }

    fn place(&mut self, cluster: &str, size: u32, bold: bool) -> Vec<Placed> {
        let Some(shape) = self.shape(cluster) else {
            return Vec::new();
        };
        let scale = size as f32 / f32::from(shape.upem);
        let mut placed = Vec::with_capacity(shape.glyphs.len());
        for (id, pen) in shape.glyphs {
            if let Some(slot) = self.image(shape.face, id, size, bold) {
                placed.push(Placed {
                    x: (pen as f32 * scale).round() as i32,
                    slot,
                });
            }
        }
        placed
    }

    fn shape(&mut self, cluster: &str) -> Option<Shape> {
        if let Some(shape) = self.shapes.get(cluster) {
            return shape.clone();
        }
        let shape = self.shaped(cluster);
        self.shapes.insert(cluster.into(), shape.clone());
        shape
    }

    /// The face and glyphs for `cluster`. A one-character cluster is a charmap lookup, the work
    /// the atlas did before it shaped anything: a single codepoint's glyph is what the charmap
    /// holds, so only a sequence — a combining mark, a skin tone, a ZWJ emoji — needs the shaper.
    fn shaped(&self, cluster: &str) -> Option<Shape> {
        let mut chars = cluster.chars();
        if let (Some(ch), None) = (chars.next(), chars.next()) {
            return self.faces_for(cluster).into_iter().find_map(|face| {
                let font = &self.faces[face];
                let id = font.swash.charmap().map(ch);
                (id != 0).then(|| Shape {
                    face,
                    upem: font.swash.metrics(&[]).units_per_em,
                    glyphs: vec![(id, 0)],
                })
            });
        }
        self.faces_for(cluster).into_iter().find_map(|face| {
            let (glyphs, upem) = shape(&self.faces[face], cluster)?;
            Some(Shape { face, upem, glyphs })
        })
    }

    /// The faces to try for `cluster`, most wanted first. A selector asking for emoji
    /// presentation (`❤️`) goes to the colour emoji font before a text font that also has the
    /// base character; anything else tries the mono first, so text keeps the theme's face.
    fn faces_for(&self, cluster: &str) -> Vec<usize> {
        let emoji_first = cluster.contains('\u{fe0f}');
        let mut faces = Vec::with_capacity(self.faces.len());
        if emoji_first && let Some(emoji) = self.emoji {
            faces.push(emoji);
        }
        faces.push(0);
        if !emoji_first && let Some(emoji) = self.emoji {
            faces.push(emoji);
        }
        if let Some(fallback) = self.fallback
            && !faces.contains(&fallback)
        {
            faces.push(fallback);
        }
        faces
    }

    /// `id`'s image at `size` and `bold`, rasterized the first time and then read by slot.
    fn image(&mut self, face: usize, id: u16, size: u32, bold: bool) -> Option<usize> {
        let key = (face, id, size, bold);
        if let Some(slot) = self.images.get(&key) {
            return *slot;
        }
        let slot = self.rasterize(face, id, size, bold).map(|glyph| {
            self.slots.push(Some(glyph));
            self.slots.len() - 1
        });
        self.images.insert(key, slot);
        slot
    }

    fn rasterize(&mut self, face: usize, id: u16, size: u32, bold: bool) -> Option<Glyph> {
        let font = self.faces[face].swash;
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
}

/// The face's bytes, as swash's view of it and as the shaper's.
fn face(bytes: &'static [u8]) -> Option<Face> {
    Some(Face {
        swash: FontRef::from_index(bytes, 0)?,
        bytes,
    })
}

/// The glyphs `cluster` shapes to in `face`, each with the pen it starts at in font units, and
/// the face's units per em. `None` when the face has no glyph for one of the characters: a
/// `.notdef` is not a fallback's answer to a sequence it cannot draw.
fn shape(face: &Face, cluster: &str) -> Option<(Vec<(u16, i32)>, u16)> {
    let font = ShapeFace::from_index(face.bytes, 0).ok()?;
    let data = ShaperData::new(&font);
    let shaper = data.shaper(&font).build();
    let mut buffer = UnicodeBuffer::new();
    buffer.push_str(cluster);
    buffer.guess_segment_properties();
    let shaped = shaper.shape(buffer, ShapeOptions::new());
    let mut glyphs = Vec::with_capacity(shaped.glyph_infos().len());
    let mut pen = 0;
    for (info, position) in shaped.glyph_infos().iter().zip(shaped.glyph_positions()) {
        let id = u16::try_from(info.glyph_id).ok()?;
        if id == 0 {
            return None;
        }
        glyphs.push((id, pen + position.x_offset));
        pen += position.x_advance;
    }
    let upem = face.swash.metrics(&[]).units_per_em;
    (!glyphs.is_empty()).then_some((glyphs, upem))
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
