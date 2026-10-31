//! The browser's state: the folder on screen, its entries, navigation history, the search
//! filter, and the selected entry's preview. Everything here is plain Rust; `ui` turns it into
//! Rune elements.
//!
//! The behaviour is split by job: `browse` reads and orders the folder, `grid` the tile
//! layout's geometry, `selection` owns which rows are picked, `drag` the pointer gestures,
//! `drop` where those gestures land, `files` and `sheets` the operations that change files,
//! `commands` what a shortcut or menu can ask for, `scrolling` the list's momentum, and
//! `input` the keys the window takes.

mod browse;
mod commands;
mod drag;
mod drop;
mod entry;
mod files;
mod grid;
mod input;
mod marquee;
mod paths;
mod places;
mod rules_view;
mod scan;
mod scroll;
mod scrolling;
mod selection;
mod sheets;
mod watch;

pub use commands::{Palette, SettingsTab};
pub use drag::{FileDrag, list_drag};
pub use drop::PlaceAt;
pub use entry::{Entry, Preview};
pub use files::Clipboard;
pub use grid::{
    GRID_PAD, GRID_ROW_HEIGHT, GRID_TOP_MARGIN, Layout, TILE_GAP, TILE_HEIGHT, TILE_WIDTH,
};
pub use input::{list_key, on_key};
pub use marquee::Marquee;
pub use paths::{home_dir, root_dir};
pub use places::{PlaceMenu, PlaceRename, place_rename_edit};
pub use rules_view::{RULE_ROW_HEIGHT, RuleDrag, RuleEdit, RuleMenu, RuleProp, rule_key};
pub use scrolling::poll;
pub use sheets::{Sheet, SheetKind};

use std::cell::Cell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::time::Instant;

use rune::{Latest, PointerPress, ScrollPos, TextEdit, Tx, Waker};
use rune_kit::shell::Reorder;
use rune_kit::{PaneState, SortDir};

use crate::config::{self, Store};
use crate::keybinds::{Action, Keybinds};
use crate::notice::Notice;
use crate::places::{Places, Section};
use crate::recorder::Recorder;
use crate::rules::{Resolved, Rules, SortKey};
use crate::settings::Settings;
use crate::views::Views;

use paths::start_location;
#[cfg(test)]
pub(crate) use scan::SYNC_SCAN_LIMIT;
use scan::Scan;
use scroll::Scroll;
use watch::Watch;

/// A row's height in the file list, in points. The pointer maths and the view share it.
pub const ROW_HEIGHT: f32 = 30.0;

/// The space between two sidebar rows, from `SidebarSection`'s own gap. The place drag turns a
/// pointer's travel into rows with it.
pub const PLACE_ROW_GAP: f32 = 2.0;

/// A sidebar row's height, from the kit. The insertion preview moves the rows under it by this
/// plus the gap, exactly one row.
pub const PLACE_ROW_HEIGHT: f32 = rune_kit::shell::ROW_HEIGHT;

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
    /// Whether the folder is a table of rows or a grid of tiles.
    pub layout: Layout,
    /// Whether that layout arrived with the folder rather than from the user's toggle. A
    /// layout a folder brought lands at once; one the user asked for keeps its fade.
    pub layout_cut: bool,
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
    /// The sidebar place menu, while it's up, and the press that opened it.
    pub place_menu: Option<PlaceMenu>,
    /// The place whose display name is being edited in its sidebar row, while the field is up.
    pub place_rename: Option<PlaceRename>,
    /// The settings panel's open category, while the panel is up.
    pub settings: Option<SettingsTab>,
    /// The command bar, while it's up.
    pub palette: Option<Palette>,
    /// The global settings from `config.kdl`: hidden files, the inspector, and the scrolling.
    pub config: Settings,
    /// The parsed rules from `config.kdl`, in file order.
    pub rules: Rules,
    /// Why the rules could not be read, for the Rules tab.
    pub rule_error: Option<String>,
    /// The rule drag in the tab's list: the row in hand, the place it would land in, and how
    /// far the rows have moved aside for it.
    pub rule_drag: RuleDrag,
    /// The rule picker's highlighted row, while it is up.
    pub rule_picker: Option<usize>,
    /// The key of the rule the tab should scroll to once its row is laid out, and the frames
    /// left to wait.
    pub rule_reveal: Option<(String, u8)>,
    /// What a command asked about that no rule matches, so the tab can say so.
    pub rule_query: Option<String>,
    /// The rule open in the tab's editor, while the form is up.
    pub rule_edit: Option<RuleEdit>,
    /// The rule whose delete is waiting on a second press, while the row asks.
    pub rule_delete_ask: Option<usize>,
    /// Whether the rule press in flight became a drag, so its release does not open the form.
    pub rule_dragged: bool,
    /// A presentation the user chose for this visit, over what the rules say. Cleared on every
    /// navigation, so a rule wins again the next time the folder opens.
    pub visit: Resolved,
    /// What each folder was left showing, in the state directory.
    pub views: Views,
    /// The sidebar's places, in the order the user left them, from the state directory.
    pub places: Places,
    /// The two place lists' drags: each numbers its own rows, so each keeps its own.
    pub favourites_drag: Reorder,
    pub locations_drag: Reorder,
    /// A drop that moved a place, so the press that ends the gesture does not open a folder.
    /// The drop's callback runs before that press.
    pub place_dragged: bool,
    /// What the corner is saying about a config file the app cannot read.
    pub notices: Vec<Notice>,
    pub keybinds: Keybinds,
    /// One recorder per action, holding what its field is listening for and any refusal.
    pub recorders: BTreeMap<Action, Recorder>,
    /// The config folder's documents and the watcher that keeps them current.
    pub store: Store,
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
    /// keyed by place id, for a dragged row to be dropped on one or between two.
    pub places_at: BTreeMap<String, PlaceAt>,
    /// Each sidebar section's rect in window points, from the last layout. A row dropped
    /// there — its heading, or the space between its rows — is added to it as a place.
    pub sections_at: BTreeMap<Section, [f32; 4]>,
    /// Where each row's name ended, in window points, keyed by path, from the last layout.
    /// Only layout knows how wide the text is, so the press that picks a row up reads its
    /// handle from here.
    pub name_at: BTreeMap<String, f32>,
    /// The Size and Date columns' widths, which header dragging changes.
    pub size_width: f32,
    pub date_width: f32,
    /// The folder read in flight, whose answer the next `poll` applies.
    scan: Latest<Scan>,
    /// The open folder's watcher, which reloads the rows when something else changes it.
    watch: Watch,
    /// The rows a file operation made or renamed, to select once the read that follows lands,
    /// and whether the list should scroll to the first of them.
    pending_select: Option<(Vec<String>, bool)>,
    /// The row the next `poll` scrolls into view, once the read holding it has landed.
    pending_reveal: Option<String>,
}

impl App {
    /// Starts at a folder given on the command line, or at home, with the saved settings.
    pub fn new() -> App {
        let (dir, selected) = start_location();
        let mut app = App::open(dir);
        if let Some(name) = selected {
            // The folder's rows land off the main thread, so the row named on the command
            // line is selected by the read that finds it.
            app.pending_select = Some((vec![format!("f:{name}")], false));
        }
        app
    }

    /// Opens `path` with the saved settings, rules, and remembered state.
    pub fn open(path: PathBuf) -> App {
        let store = Store::open();
        let config = Settings::from_doc(store.doc(config::CONFIG));
        let keybinds = Keybinds::from_doc(store.doc(config::KEYBINDS));
        let mut app = App::blank(config, store, keybinds);
        app.views = Views::load();
        app.places = Places::load();
        app.reload_rules();
        app.history.push(path.clone());
        app.at = 0;
        app.enter(path);
        app
    }

    /// Opens `path` with `config`, for a launch that already knows its settings (and for tests
    /// that keep them out of the real config folder).
    #[cfg(test)]
    pub fn open_with(path: PathBuf, config: Settings) -> App {
        let mut app = App::blank(config, Store::none(), Keybinds::defaults());
        app.history.push(path.clone());
        app.at = 0;
        app.enter(path);
        app
    }

    fn blank(config: Settings, store: Store, keybinds: Keybinds) -> App {
        let recorders = Keybinds::recorders();
        let mut inspector = PaneState::new(286.0).range(232.0, 420.0);
        inspector.set_open(config.inspector);
        let layout = if config.grid {
            Layout::Grid
        } else {
            Layout::List
        };
        App {
            cwd: PathBuf::new(),
            entries: Vec::new(),
            visible: Vec::new(),
            selection: BTreeSet::new(),
            selected: None,
            anchor: None,
            sort: ("name".to_owned(), SortDir::Ascending),
            layout,
            layout_cut: true,
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
            place_menu: None,
            place_rename: None,
            settings: None,
            palette: None,
            config,
            rules: Rules::default(),
            rule_error: None,
            rule_drag: RuleDrag::default(),
            rule_picker: None,
            rule_reveal: None,
            rule_query: None,
            rule_edit: None,
            rule_delete_ask: None,
            rule_dragged: false,
            visit: Resolved::default(),
            views: Views::default(),
            places: Places::default(),
            favourites_drag: Reorder::new().gap(PLACE_ROW_GAP),
            locations_drag: Reorder::new().gap(PLACE_ROW_GAP),
            place_dragged: false,
            notices: Vec::new(),
            keybinds,
            recorders,
            store,
            scroll_told: Rc::new(Cell::new(ScrollPos::default())),
            rows_view: [0.0; 4],
            scroll: Scroll::default(),
            places_at: BTreeMap::new(),
            sections_at: BTreeMap::new(),
            name_at: BTreeMap::new(),
            size_width: 92.0,
            date_width: 158.0,
            scan: Latest::default(),
            watch: Watch::default(),
            pending_select: None,
            pending_reveal: None,
        }
    }

    /// Reads the rules out of the config document, keeping the last good ones when the file no
    /// longer parses, then says in the corner what could not be read.
    pub fn reload_rules(&mut self) {
        if let Some(error) = self.store.error(config::CONFIG) {
            self.rule_error = Some(error.to_owned());
        }
        let outcome = self.store.doc(config::CONFIG).map(Rules::parse);
        match outcome {
            Some(Ok(rules)) => {
                self.rules = rules;
                if self.store.error(config::CONFIG).is_none() {
                    self.rule_error = None;
                }
            }
            Some(Err(error)) => self.rule_error = Some(error),
            None => {}
        }
        self.raise_config_notices();
    }

    /// Says, in the corner, which config file the app cannot read and where it stopped. A file
    /// already up is refreshed rather than stacked, and one that parses again takes its toast
    /// away.
    pub fn raise_config_notices(&mut self) {
        let mut found = Vec::new();
        for name in [config::CONFIG, config::KEYBINDS] {
            if let Some(problem) = self.store.problem(name) {
                found.push(Notice::new(
                    name,
                    format!("{name} does not parse"),
                    problem.detail.clone(),
                    problem.report.clone(),
                ));
            }
        }
        if let Some(error) = self.rule_error.clone() {
            let report = config::schema_problem(
                config::CONFIG,
                self.store.path(config::CONFIG).as_deref(),
                &error,
            );
            found.push(Notice::new(
                config::CONFIG,
                format!("{} has a rule it cannot read", config::CONFIG),
                error,
                report,
            ));
        }
        self.notices
            .retain(|notice| found.iter().any(|problem| problem.file == notice.file));
        for notice in found {
            match self.notices.iter_mut().find(|up| up.file == notice.file) {
                Some(up) if up.report != notice.report => *up = notice,
                Some(_) => {}
                None => self.notices.push(notice),
            }
        }
    }

    /// Puts a notice's report on the clipboard, whole, so the problem can be pasted wherever it
    /// will be fixed, and leaves the toast saying so.
    pub fn copy_notice(&mut self, index: usize, tx: &mut Tx) {
        let Some(notice) = self.notices.get_mut(index) else {
            return;
        };
        tx.copy(notice.clipboard());
        notice.copied = true;
        notice.shown = Instant::now();
    }

    /// Takes one toast away, for its close button.
    pub fn dismiss_notice(&mut self, index: usize) {
        if index < self.notices.len() {
            self.notices.remove(index);
        }
    }

    /// Applies a change made to the config folder from outside: the settings, the keybinds, the
    /// rules, and whatever the open folder now resolves to.
    pub fn apply_config(&mut self) {
        // The form names its rule by position in the file. A file that changed on disk may
        // hold a different rule there, so the draft would land on the wrong node.
        if self.rule_edit.is_some() {
            self.rule_edit = None;
            self.rule_query = Some(
                "The config file changed on disk, so the rule editor closed. Open the rule \
                 again to keep editing."
                    .to_owned(),
            );
        }
        // The question names a rule by the same position, so it is stale for the same reason.
        self.rule_delete_ask = None;
        self.config = Settings::from_doc(self.store.doc(config::CONFIG));
        self.keybinds = Keybinds::from_doc(self.store.doc(config::KEYBINDS));
        self.recorders = Keybinds::recorders();
        self.inspector.set_open(self.config.inspector);
        self.reload_rules();
        self.reload();
    }

    /// The presentation the folder resolves to, before this visit's override.
    pub fn rule_presentation(&self) -> Resolved {
        self.rules.resolve(&self.cwd, self.entries.len())
    }

    /// Applies the visit's override, then the rules, then what the folder was left showing,
    /// and last the global defaults.
    fn apply_rules(&mut self) {
        let rule = self.rule_presentation();
        let remembered = self.views.get(&self.cwd);
        let resolved = self.visit.or(rule).or(remembered);
        self.layout = resolved.layout.unwrap_or(if self.config.grid {
            Layout::Grid
        } else {
            Layout::List
        });
        let sort = resolved.sort.unwrap_or(SortKey::Name);
        self.sort = (
            sort.as_str().to_owned(),
            resolved.direction.unwrap_or(SortDir::Ascending),
        );
        self.show_hidden = resolved.show_hidden.unwrap_or(self.config.show_hidden);
    }

    /// Records what this folder is showing, which is what it opens with next time.
    fn remember_presentation(&mut self) {
        let sort = SortKey::from_column(&self.sort.0).unwrap_or(SortKey::Name);
        self.views.set(
            &self.cwd,
            Resolved {
                layout: Some(self.layout),
                sort: Some(sort),
                direction: Some(self.sort.1),
                show_hidden: None,
            },
        );
    }

    /// Saves one setting, leaving every other node in the file as it was.
    fn save_setting(&mut self, changed: crate::settings::Setting) {
        let _ = self
            .store
            .edit(config::CONFIG, |doc| self.config.write(doc, changed));
    }

    /// Starts watching the open folder, so something else changing it reloads the rows.
    pub fn watch(&mut self, wake: Waker) {
        self.watch.start(&self.cwd, wake);
    }
}

/// `AppSpec::setup`: hands the config folder's watcher the waker the event loop gave the app,
/// so a change made in a text editor reaches the open window.
pub fn setup(s: &mut App, wake: Waker) {
    // A folder read that lands wakes the window so `poll` applies it.
    s.scan.set_waker(wake.clone());
    s.store.watch(wake.clone());
    // A change to the open folder from outside reloads it, without the window being touched.
    s.watch(wake);
}
