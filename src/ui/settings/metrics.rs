pub(crate) const SETTINGS_DIALOG_WIDTH: f32 = 880.0;

pub(crate) const SETTINGS_RAIL: f32 = 220.0;

pub(crate) const SETTINGS_ROW_INSET: f32 = 16.0;

pub(crate) const SETTINGS_CARD_WIDTH: f32 = SETTINGS_DIALOG_WIDTH
    - SETTINGS_RAIL
    - SETTINGS_INSET
    - 2.0
    - 2.0 * (SETTINGS_GUTTER - SETTINGS_ROW_INSET);

pub(crate) const SETTINGS_ROW_HEIGHT: f32 = 48.0;

pub(crate) const SETTINGS_GUTTER: f32 = 32.0;

pub(crate) const SETTINGS_RADIUS: f32 = 16.0;

// How far the panel sits in from the dialog; the radius loses this much to stay concentric.
pub(crate) const SETTINGS_INSET: f32 = 8.0;

pub(crate) const DONE_SIZE: (f32, f32) = (76.0, 26.0);

pub(crate) const DONE_GAP: f32 = 8.0;
