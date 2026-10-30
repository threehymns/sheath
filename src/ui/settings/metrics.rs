//! The settings panel's measurements: the Hex metrics its layout is drawn from, and the widths
//! a corner cut has to measure from.

/// The settings dialog's width, which the corner cuts measure their panels from.
pub(crate) const SETTINGS_DIALOG_WIDTH: f32 = 880.0;

/// Hex's settings metrics: a 220 pt navigation rail, a 32 pt page gutter, cards hanging 16 pt
/// past it so their rows' text lands on the gutter, and 48 pt rows.
pub(crate) const SETTINGS_RAIL: f32 = 220.0;

pub(crate) const SETTINGS_ROW_INSET: f32 = 16.0;

/// The width of a page's card: the dialog less the rail, the panel's inset and its own edge,
/// and the page's gutter. A cut in a card measures its right edge from this.
pub(crate) const SETTINGS_CARD_WIDTH: f32 = SETTINGS_DIALOG_WIDTH
    - SETTINGS_RAIL
    - SETTINGS_INSET
    - 2.0
    - 2.0 * (SETTINGS_GUTTER - SETTINGS_ROW_INSET);

pub(crate) const SETTINGS_ROW_HEIGHT: f32 = 48.0;

pub(crate) const SETTINGS_GUTTER: f32 = 32.0;

/// The dialog's radius, rounder than the kit's own so the panel inside it keeps a visible curve.
pub(crate) const SETTINGS_RADIUS: f32 = 16.0;

/// How far the panel is set in from the dialog's edges, which is how much comes off the radius so
/// the two corners stay concentric.
pub(crate) const SETTINGS_INSET: f32 = 8.0;

/// Done, in the dialog's top-right corner: the panel is cut away around it, so it is on the
/// dialog's surface rather than in the pane.
pub(crate) const DONE_SIZE: (f32, f32) = (76.0, 26.0);

/// Panel left around Done's left and bottom edges.
pub(crate) const DONE_GAP: f32 = 8.0;
