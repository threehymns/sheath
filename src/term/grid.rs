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
    pub underline: bool,
    pub strike: bool,
}

impl Default for Pen {
    fn default() -> Self {
        Pen {
            fg: Color::WHITE,
            bg: None,
            bold: false,
            italic: false,
            faint: false,
            underline: false,
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
    pub title: String,
}
