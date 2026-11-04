//! The three glyphs Avar needs that neither `rune_kit::glyphs` nor the theme draws: a clipboard,
//! a checked square and an undo arrow.
use rune::Icon;

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

/// The reset button beside a changed shortcut.
pub static UNDO: Icon = Icon {
    name: "avar-undo",
    view: 16.0,
    body: r#"<path d="M13.5 12.5a5.5 5.5 0 0 0-5.5-5.5H3M6 4 3 7l3 3" stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round"/>"#,
};
