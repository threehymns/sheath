//! What one frame of the terminal looks like: a line per row, its text, and the styled byte
//! ranges over it. The engine resolves colours against the terminal's palette here, so the view
//! only shapes text and paints runs.

use rune::Color;

/// How one run of a line is drawn.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pen {
    pub fg: Color,
    /// Only set when the cell's background differs from the terminal's, so most runs carry none.
    pub bg: Option<Color>,
    pub bold: bool,
    pub italic: bool,
    pub faint: bool,
    pub underline: UnderlineKind,
    /// The colour the program gave the underline (SGR 58), when it gave one.
    pub underline_color: Option<Color>,
    pub strike: bool,
}

/// How a run's underline is drawn, the styles SGR 4 selects. Ghostty draws all of them, so a
/// spell checker's squiggle or a diff's doubled rule arrives as what the program asked for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UnderlineKind {
    #[default]
    None,
    Single,
    Double,
    Curly,
    Dotted,
    Dashed,
}

impl Default for Pen {
    fn default() -> Self {
        Pen {
            fg: Color::WHITE,
            bg: None,
            bold: false,
            italic: false,
            faint: false,
            underline: UnderlineKind::None,
            underline_color: None,
            strike: false,
        }
    }
}

/// A byte range of a line drawn in one pen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Run {
    pub start: usize,
    pub end: usize,
    /// The cells this run covers. A background is a rectangle over these cells, not something
    /// the text shaper draws: a shaper pads and rounds its spans, which leaves gaps between rows
    /// and insets the edges of a full-width bar.
    pub cols: (u16, u16),
    pub pen: Pen,
}

/// A rectangle one cell is painted with instead of a glyph, in fractions of the cell: `x`
/// across it, `y` down it, both `0..=1`. Box rules, blocks and quadrants come from these, which
/// is what makes them tile at any font, size or scale: see [`crate::term::shapes`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shape {
    pub col: u16,
    pub rect: [f32; 4],
    /// The cell's own foreground, resolved against the palette by the engine.
    pub fg: Color,
}

/// One match a search found on a row: the columns it covers, and whether it is the match the
/// search is currently on, which draws stronger than the rest.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Highlight {
    /// Inclusive first and last column.
    pub cols: (u16, u16),
    pub current: bool,
}

/// The decoded pixels of one kitty graphics image, shared between the frame that places it and
/// the engine's cache: cloning a frame must not copy a megabyte of image data.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
#[derive(Clone, Debug, PartialEq)]
pub struct ImagePixels {
    pub width: u32,
    pub height: u32,
    /// sRGB, straight alpha, four bytes per pixel, rows top first.
    pub rgba: Vec<u8>,
}

/// Which side of the text a kitty graphics placement draws on, from its z-index.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ImageLayer {
    /// Under the cell backgrounds: a wallpaper behind everything the screen draws.
    BelowBackground,
    /// Over the backgrounds and under the text, so a column of text stays readable on it.
    BelowText,
    /// Over the text, which is where a plain `a=T` image lands.
    AboveText,
}

/// One image placed on the screen. The pixels are shared with the engine's cache and the geometry
/// is in points, in the same units as a cell, so the raster scales the whole thing at draw time.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
#[derive(Clone, Debug, PartialEq)]
pub struct Placement {
    /// The image it draws. The engine keeps decoded pixels only for the images still placed, so
    /// it needs the id on the frame to tell a live copy from one that is gone.
    pub image: u32,
    pub data: std::sync::Arc<ImagePixels>,
    pub layer: ImageLayer,
    /// The z-index that ordered it inside its layer.
    pub z: i32,
    /// Its top-left corner in points, from the viewport's own top-left. Negative when it starts
    /// above or left of the viewport, which is what a partly scrolled image looks like.
    pub x: f32,
    pub y: f32,
    /// The size it is drawn at, in points.
    pub w: f32,
    pub h: f32,
    /// The part of the image to draw, in image pixels: x, y, width, height.
    pub source: [u32; 4],
}

/// One terminal row: its text, the runs over it, the shapes under it, and the columns the
/// selection covers.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Line {
    pub text: String,
    pub runs: Vec<Run>,
    /// Cell-filling characters, painted before the text. The text holds a space in their cells.
    pub shapes: Vec<Shape>,
    /// Inclusive first and last selected column.
    pub selection: Option<(u16, u16)>,
    /// The search matches on this row, under the selection and the backgrounds.
    pub matches: Vec<Highlight>,
}

/// How the cursor is drawn, the shapes DECSCUSR selects and Ghostty's renderer draws.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CursorShape {
    /// A wash over the whole cell.
    #[default]
    Block,
    /// A hollow block: the cell's edges only.
    Hollow,
    /// A bar at the cell's left edge, which is what an insert-mode editor asks for.
    Bar,
    /// A rule along the cell's bottom edge.
    Underline,
}

/// The visible screen at one moment.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub cols: u16,
    pub rows: u16,
    pub lines: Vec<Line>,
    /// The row just above the viewport, for the fraction of a row a smooth scroll shows above
    /// the first line. One row of overscan is what keeps that edge from filling in abruptly
    /// when the viewport steps.
    pub lead: Option<Line>,
    /// The row just below it, for the same fraction of a row at the bottom edge.
    pub trail: Option<Line>,
    /// The cursor's cell within the viewport, when it is on screen.
    pub cursor: Option<(u16, u16)>,
    /// The shape the program selected with DECSCUSR, and the colour it set with OSC 12. Ghostty
    /// draws an editor's insert-mode bar and its tinted cursor the same way.
    pub cursor_shape: CursorShape,
    pub cursor_color: Option<Color>,
    /// Whether the program asked for a blinking cursor (DECSCUSR, or DEC mode 12).
    pub cursor_blink: bool,
    pub title: String,
    /// The terminal's own background, when the program set one with OSC 11. Unset means the
    /// view's theme colour, which is what the shell starts with.
    pub background: Option<Color>,
    /// The images the program placed with the kitty graphics protocol, sorted by layer and then
    /// z-index, ready to be drawn under or over the rows.
    pub images: Vec<Placement>,
    /// Bumped by the engine on every readback, so the streamed image can tell a new frame from
    /// the same one drawn again: a view build that only moved a cursor or a hover underline
    /// reuses the last image instead of compositing the grid again.
    pub generation: u64,
}
