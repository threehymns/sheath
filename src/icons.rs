//! The handful of glyphs the browser needs beyond Rune Kit's built-in set.

use rune::Icon;

pub static HOUSE: Icon = Icon {
    name: "sheath-house",
    view: 16.0,
    body: r#"<path d="M2.5 7.25 8 2.5l5.5 4.75V13.5H2.5V7.25Z" stroke="currentColor" stroke-linejoin="round"/><path d="M6 13.5V9.25h4v4.25" stroke="currentColor" stroke-linejoin="round"/>"#,
};

pub static DESKTOP: Icon = Icon {
    name: "sheath-desktop",
    view: 16.0,
    body: r#"<rect x="1.75" y="3" width="12.5" height="8" rx="1" stroke="currentColor"/><path d="M6 14h4M8 11v3" stroke="currentColor" stroke-linecap="round"/>"#,
};

pub static DOCUMENTS: Icon = Icon {
    name: "sheath-documents",
    view: 16.0,
    body: r#"<path d="M4.5 1.5h5l3 3v10h-8v-13Z" stroke="currentColor" stroke-linejoin="round"/><path d="M9.5 1.5v3h3M6.5 8h4M6.5 10.5h4" stroke="currentColor" stroke-linecap="round"/> "#,
};

pub static DOWNLOADS: Icon = Icon {
    name: "sheath-downloads",
    view: 16.0,
    body: r#"<path d="M8 2.5v7.5M5.25 7.5 8 10.25 10.75 7.5M2.5 13.5h11" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static PICTURES: Icon = Icon {
    name: "sheath-pictures",
    view: 16.0,
    body: r#"<rect x="2" y="2.5" width="12" height="11" rx="1.5" stroke="currentColor"/><circle cx="5.75" cy="6" r="1" fill="currentColor"/><path d="M2.5 11.5 6 8l2 2 2-2 3.5 3.5" stroke="currentColor" stroke-linejoin="round"/>"#,
};

pub static DRIVE: Icon = Icon {
    name: "sheath-drive",
    view: 16.0,
    body: r#"<rect x="2" y="3" width="12" height="10" rx="1.5" stroke="currentColor"/><path d="M2 9.5h12" stroke="currentColor"/><circle cx="11.25" cy="6.25" r=".75" fill="currentColor"/>"#,
};

pub static UNDO: Icon = Icon {
    name: "sheath-undo",
    view: 16.0,
    body: r#"<path d="M13.5 12.5a5.5 5.5 0 0 0-5.5-5.5H3M6 4 3 7l3 3" stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static CHEVRON_LEFT: Icon = Icon {
    name: "sheath-chevron-left",
    view: 16.0,
    body: r#"<path d="M10 3.5 5.5 8l4.5 4.5" stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static CHEVRON_RIGHT: Icon = Icon {
    name: "sheath-chevron-right",
    view: 16.0,
    body: r#"<path d="M6 3.5 10.5 8 6 12.5" stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static ARROW_UP: Icon = Icon {
    name: "sheath-arrow-up",
    view: 16.0,
    body: r#"<path d="M8 13V3M4 7l4-4 4 4" stroke="currentColor" stroke-width="1" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static REFRESH: Icon = Icon {
    name: "sheath-refresh",
    view: 16.0,
    body: r#"<path d="M13.25 8A5.25 5.25 0 1 1 11.7 4.3M13.5 2.5v3h-3" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static INSPECTOR: Icon = Icon {
    name: "sheath-inspector",
    view: 16.0,
    body: r#"<rect x="2" y="3" width="12" height="10" rx="2" stroke="currentColor"/><path d="M9.75 3.5v9" stroke="currentColor"/>"#,
};

pub static LINK: Icon = Icon {
    name: "sheath-link",
    view: 16.0,
    body: r#"<path d="M6.75 9.25 9.25 6.75M7 4.5 8.25 3.25a2.47 2.47 0 0 1 3.5 3.5L10.5 8M9 11.5l-1.25 1.25a2.47 2.47 0 0 1-3.5-3.5L5.5 8" stroke="currentColor" stroke-linecap="round" stroke-linejoin="round"/>"#,
};

pub static SCISSORS: Icon = Icon {
    name: "sheath-scissors",
    view: 16.0,
    body: r#"<circle cx="4" cy="12" r="1.9" stroke="currentColor"/><circle cx="4" cy="4" r="1.9" stroke="currentColor"/><path d="M5.5 5.2 13 12.5M13 3.5 5.5 10.8" stroke="currentColor" stroke-linecap="round"/>"#,
};

pub static CLIPBOARD: Icon = Icon {
    name: "sheath-clipboard",
    view: 16.0,
    body: r#"<path d="M5.5 3H3.75v11h8.5V3H10.5" stroke="currentColor" stroke-linejoin="round"/><rect x="5.5" y="1.75" width="5" height="2.5" rx=".75" stroke="currentColor"/><path d="M6 8h4M6 10.5h4" stroke="currentColor" stroke-linecap="round"/>"#,
};

pub static FOLDER_PLUS: Icon = Icon {
    name: "sheath-folder-plus",
    view: 16.0,
    body: r#"<path d="M1.75 13.25v-10h4.5l1.5 1.5h6.5v8.5h-12.5Z" stroke="currentColor" stroke-linejoin="round"/><path d="M8 6.75v3.5M6.25 8.5h3.5" stroke="currentColor" stroke-linecap="round"/>"#,
};
