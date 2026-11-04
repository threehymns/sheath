//! The glyphs Avar needs that neither `rune_kit::glyphs` nor the theme draws: a clipboard, a
//! checked square, an undo arrow, and the plus and left arrow the tab actions go by.
use rune::Icon;

/// The new tab button, beside the tab rail.
pub static PLUS: Icon = Icon {
    name: "avar-plus",
    view: 16.0,
    body: r#"<path d="M8 3.5v9M3.5 8h9" stroke="currentColor" stroke-linecap="round"/>"#,
};

/// The tab before this one; the kit draws the arrow the other way.
pub static ARROW_LEFT: Icon = Icon {
    name: "avar-arrow-left",
    view: 16.0,
    body: r#"<path d="M13.5 8H2.5M7 3.5L2.5 8L7 12.5" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static CLIPBOARD: Icon = Icon {
    name: "avar-clipboard",
    view: 16.0,
    body: r#"<path d="M5.5 3H3.75v11h8.5V3H10.5" stroke="currentColor" stroke-linejoin="round"/><rect x="5.5" y="1.75" width="5" height="2.5" rx=".75" stroke="currentColor"/><path d="M6 8h4M6 10.5h4" stroke="currentColor" stroke-linecap="round"/>"#,
};

pub static CHECK_SQUARE: Icon = Icon {
    name: "avar-check-square",
    view: 16.0,
    body: r#"<rect x="2.25" y="2.25" width="11.5" height="11.5" rx="2.5" stroke="currentColor"/><path d="M5.5 8.25 7.25 10l3.25-4" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

/// The split-right button: a window with its trailing pane drawn in, the new block to the
/// right of the one that was divided.
pub static SPLIT_RIGHT: Icon = Icon {
    name: "avar-split-right",
    view: 16.0,
    body: r#"<rect x="2" y="3" width="12" height="10" rx="2" stroke="currentColor" stroke-width="1" fill="none"/><path d="M9.5 3.5V12.5" stroke="currentColor" stroke-width="1"/>"#,
};

/// The split-down button: the same window divided along its middle instead.
pub static SPLIT_DOWN: Icon = Icon {
    name: "avar-split-down",
    view: 16.0,
    body: r#"<rect x="2" y="3" width="12" height="10" rx="2" stroke="currentColor" stroke-width="1" fill="none"/><path d="M2.5 8H13.5" stroke="currentColor" stroke-width="1"/>"#,
};

/// Zoom: a block taking the whole tab, drawn as the four corners it expands into.
pub static ZOOM: Icon = Icon {
    name: "avar-zoom",
    view: 16.0,
    body: r#"<path d="M6 2.5H2.5V6M10 2.5h3.5V6M6 13.5H2.5V10M10 13.5h3.5V10" stroke="currentColor" stroke-width="1.2" stroke-linecap="round" stroke-linejoin="round" fill="none"/>"#,
};

/// The reset button beside a changed shortcut.
pub static UNDO: Icon = Icon {
    name: "avar-undo",
    view: 16.0,
    body: r#"<path d="M13.5 12.5a5.5 5.5 0 0 0-5.5-5.5H3M6 4 3 7l3 3" stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
