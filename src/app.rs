//! The browser's state: the folder on screen, its entries, navigation history, the search
//! filter, and the selected entry's preview. Everything here is plain Rust; `ui` turns it into
//! Rune elements.
//!
//! The behaviour is split by job: `browse` reads and orders the folder, `selection` owns which
//! rows are picked, `drag` the pointer gestures, `files` and `sheets` the operations that change
//! files, `commands` what a shortcut or menu can ask for, `scrolling` the list's momentum, and
//! `input` the keys the window takes.

mod browse;
mod commands;
mod drag;
mod entry;
mod files;
mod input;
mod paths;
mod scroll;
mod scrolling;
mod selection;
mod sheets;

pub use commands::{Palette, SettingsTab};
pub use drag::{FileDrag, Marquee, list_drag};
pub use entry::{Entry, Preview};
pub use files::Clipboard;
pub use input::{list_key, on_key};
pub use paths::{home_dir, root_dir};
pub use scrolling::poll;
pub use sheets::{Sheet, SheetKind};

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use rune::{PointerPress, ScrollPos, TextEdit};
use rune_kit::{PaneState, SortDir};

use crate::keybinds::{Action, Keybinds};
use crate::recorder::Recorder;
use crate::settings::Settings;

use paths::start_location;
use scroll::Scroll;

/// A row's height in the file list, in points. The pointer maths and the view share it.
pub const ROW_HEIGHT: f32 = 30.0;

/// The browser.
pub struct App {
    pub cwd: PathBuf,
    pub entries: Vec<Entry>,
    /// Indices into `entries` that pass the hidden-file and search filters, in table order.
    pub visible: Vec<usize>,
    /// The selected rows' ids. The list shows them in table order whatever order they were
    /// picked in.
    pub selection: BTreeSet<String>,
    /// The keyboard cursor and the row the inspector shows.
    pub selected: Option<String>,
    /// The row a Shift-click, a Shift-arrow or a box a Shift-drag draws starts from.
    pub anchor: Option<String>,
    /// The column the list sorts by, and which way.
    pub sort: (String, SortDir),
    /// The drag box, while the pointer is down in the list's empty space.
    pub marquee: Option<Marquee>,
    /// The rows being dragged toward a folder, while the pointer is down on a row.
    pub file_drag: Option<FileDrag>,
    /// The rows copied or cut, waiting to be pasted.
    pub clipboard: Option<Clipboard>,
    /// What has been typed since the last pause, for jumping to a row by name.
    pub typeahead: String,
    typeahead_at: Option<Instant>,
    pub preview: Option<Preview>,
    pub sidebar: PaneState,
    pub inspector: PaneState,
    pub search: TextEdit,
    pub show_hidden: bool,
    pub history: Vec<PathBuf>,
    pub at: usize,
    pub error: Option<String>,
    pub sheet: Option<Sheet>,
    /// Where the row menu opened, from the right-click that opened it.
    pub menu: Option<PointerPress>,
    /// The settings panel's open category, while the panel is up.
    pub settings: Option<SettingsTab>,
    /// The command bar, while it's up.
    pub palette: Option<Palette>,
    /// The settings saved in `config.kdl`: hidden files and the inspector.
    pub config: Settings,
    pub keybinds: Keybinds,
    /// One recorder per action, holding what its field is listening for and any refusal.
    pub recorders: BTreeMap<Action, Recorder>,
    /// Where the list's viewport is scrolled to and how far it can go, told by its
    /// `El::on_scroll_offset` in the frame that scrolled it. The wheel starts from this rather
    /// than from the rows' rects: those are reported a frame late, and only once they have
    /// moved far enough, so a fast scroll would jump back by what the last frame moved.
    pub scroll_told: Rc<Cell<ScrollPos>>,
    /// The list viewport's rect in window points, from the last layout.
    pub rows_view: [f32; 4],
    /// The wheel's inertia, the overscroll past the ends, and the drag that pushes the list.
    scroll: Scroll,
    /// The sidebar places' rects in window points and their folders, from the last layout,
    /// for a dragged row to be dropped on one.
    pub places_at: BTreeMap<&'static str, ([f32; 4], PathBuf)>,
    /// The Size and Date columns' widths, which header dragging changes.
    pub size_width: f32,
    pub date_width: f32,
}

impl App {
    /// Starts at a folder given on the command line, or at home, with the saved settings.
    pub fn new() -> App {
        let (dir, selected) = start_location();
        let mut app = App::open(dir);
        if let Some(name) = selected {
            let id = format!("f:{name}");
            app.select_id(&id);
        }
        app
    }

    /// Opens `path` with a fresh one-entry history and the saved settings.
    pub fn open(path: PathBuf) -> App {
        App::open_with(path, Settings::load())
    }

    /// Opens `path` with `config`, for a launch that already knows its settings (and for tests
    /// that keep them out of the real config folder).
    pub fn open_with(path: PathBuf, config: Settings) -> App {
        let mut app = App::blank(config);
        app.history.push(path.clone());
        app.at = 0;
        app.enter(path);
        app
    }

    fn blank(config: Settings) -> App {
        let keybinds = Keybinds::load();
        let recorders = Keybinds::recorders();
        let mut inspector = PaneState::new(286.0).range(232.0, 420.0);
        inspector.set_open(config.inspector);
        App {
            cwd: PathBuf::new(),
            entries: Vec::new(),
            visible: Vec::new(),
            selection: BTreeSet::new(),
            selected: None,
            anchor: None,
            sort: ("name".to_owned(), SortDir::Ascending),
            marquee: None,
            file_drag: None,
            clipboard: None,
            typeahead: String::new(),
            typeahead_at: None,
            preview: None,
            sidebar: PaneState::new(212.0).range(176.0, 330.0),
            inspector,
            search: TextEdit::new(""),
            show_hidden: config.show_hidden,
            history: Vec::new(),
            at: 0,
            error: None,
            sheet: None,
            menu: None,
            settings: None,
            palette: None,
            config,
            keybinds,
            recorders,
            scroll_told: Rc::new(Cell::new(ScrollPos::default())),
            rows_view: [0.0; 4],
            scroll: Scroll::default(),
            places_at: BTreeMap::new(),
            size_width: 92.0,
            date_width: 158.0,
        }
    }
}
