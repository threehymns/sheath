//! The browser's state: the folder on screen, its entries, navigation history, the search
//! filter, and the selected entry's preview. Everything here is plain Rust; `ui` turns it into
//! Rune elements.

use std::cell::Cell;
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::time::{Duration, Instant, SystemTime};

use rune::{
    Drag, GesturePhase, Key, KeyEvent, Mods, Phase, PointerPress, ScrollPos, Shortcut,
    ShortcutError, TextEdit, Tx, WheelEvent,
};
use rune_kit::{PaneState, ShortcutRecorderState, SortDir};

use crate::keybinds::{Action, Keybinds};
use crate::settings::Settings;

/// How much of a text file the inspector reads.
const PREVIEW_LIMIT: u64 = 24 * 1024;

/// A row's height in the file list, in points. The pointer maths and the view share it.
pub const ROW_HEIGHT: f32 = 30.0;

/// How far the pointer travels before a press in the list becomes a selection box.
const DRAG_START: f32 = 3.0;

/// How far inside the list's top or bottom edge, in points, a dragged row has to be for the
/// rows to scroll under it.
const DRAG_SCROLL_EDGE: f32 = 44.0;

/// How fast the rows move while a dragged row is held at the very edge, in points per second:
/// pushing the pointer further past the edge scrolls no faster.
const DRAG_SCROLL_MAX: f32 = 1100.0;

/// How quickly the list's own scroll momentum decays, per second: the wheel's inertia.
const SCROLL_DECAY: f32 = 3.5;

/// Points per second of momentum a notch of a notched wheel leaves behind, as a nudge has no
/// speed of its own to carry.
const SCROLL_GAIN: f32 = 0.75;

/// The fastest the list coasts, in points per second. A hard flick is the one gesture that
/// should carry a long way, as a browser's fling does.
const SCROLL_MAX_VELOCITY: f32 = 6000.0;

/// Below this the momentum has stopped, in points per second.
const SCROLL_MIN_VELOCITY: f32 = 8.0;

/// How long, in seconds, a slower wheel speed takes to pull the coast down: long enough that a
/// flick's last light frame doesn't cut its glide short, short enough to follow the fingers.
const SCROLL_SPEED_SMOOTH: f32 = 0.15;

/// How quickly a resting gesture's speed fades, per second: several times the coast's, so a
/// hold before lifting takes the fling away almost at once.
const SCROLL_HOLD_DECAY: f32 = 12.0;

/// How long the fingers can rest on the trackpad, in seconds, before the speed they had is
/// gone: lifting them after that starts no glide, as a browser's fling doesn't.
const SCROLL_HOLD_LIMIT: f32 = 0.12;

/// How long a gesture can go without its end, in seconds, before the app stops believing the
/// fingers are still down: a platform that drops the end mustn't take inertia with it.
const SCROLL_HELD_FORGET: f32 = 1.0;

/// How far the list can be pulled past its first or last row, in points.
const MAX_OVERSCROLL: f32 = 72.0;

/// How much of a wheel delta past an end becomes overscroll.
const OVERSCROLL_GIVE: f32 = 0.45;

/// How quickly overscroll springs back, per second.
const OVERSCROLL_RETURN: f32 = 16.0;

/// One row in the file table.
pub struct Entry {
    /// Stable row key. Names are unique within a folder, and a colon keeps the key out of the
    /// `/`-separated key paths Rune uses elsewhere.
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub is_dir: bool,
    pub is_symlink: bool,
    pub size: u64,
    pub modified: Option<SystemTime>,
    pub created: Option<SystemTime>,
    pub readonly: bool,
    pub permissions: String,
}

impl Entry {
    fn read(dir_entry: &fs::DirEntry) -> Option<Entry> {
        let path = dir_entry.path();
        let name = dir_entry.file_name().to_string_lossy().into_owned();
        // `DirEntry::metadata` doesn't follow links; the target's metadata does.
        let link_meta = dir_entry.metadata().ok()?;
        let is_symlink = link_meta.file_type().is_symlink();
        let target_meta = if is_symlink {
            fs::metadata(&path).ok()
        } else {
            None
        };
        let meta = target_meta.as_ref().unwrap_or(&link_meta);
        let is_dir = meta.is_dir();
        Some(Entry {
            id: format!("f:{name}"),
            name,
            path,
            is_dir,
            is_symlink,
            size: if is_dir { 0 } else { meta.len() },
            modified: meta.modified().or_else(|_| link_meta.modified()).ok(),
            created: meta.created().or_else(|_| link_meta.created()).ok(),
            readonly: meta.permissions().readonly(),
            permissions: crate::format::permissions(meta),
        })
    }

    pub fn hidden(&self) -> bool {
        self.name.starts_with('.')
    }

    pub fn kind(&self) -> String {
        crate::format::kind(&self.name, self.is_dir, self.is_symlink)
    }
}

/// What the inspector shows under the metadata.
pub enum Preview {
    Text(String),
    Folder(usize),
    TooLarge,
    Binary,
}

/// The categories the settings panel's sidebar lists.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsTab {
    General,
    Keybinds,
    About,
}

impl SettingsTab {
    pub const ALL: [SettingsTab; 3] = [
        SettingsTab::General,
        SettingsTab::Keybinds,
        SettingsTab::About,
    ];

    /// The stable name the element keys use.
    pub fn id(self) -> &'static str {
        match self {
            SettingsTab::General => "general",
            SettingsTab::Keybinds => "keybinds",
            SettingsTab::About => "about",
        }
    }

    /// What the sidebar calls it.
    pub fn label(self) -> &'static str {
        match self {
            SettingsTab::General => "General",
            SettingsTab::Keybinds => "Keybinds",
            SettingsTab::About => "About",
        }
    }
}

/// The modal an action opens.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SheetKind {
    Rename,
    Move,
    Trash,
}

/// A rename, move or trash confirmation. `edit` is the field for rename and move, and is
/// empty (and unfocused) for the trash confirmation.
pub struct Sheet {
    pub kind: SheetKind,
    /// The rows the sheet applies to: one for rename, the whole selection for move and trash.
    /// These stay valid while the sheet is open.
    pub targets: Vec<String>,
    /// What the sheet calls them: one name, or a count.
    pub name: String,
    pub edit: TextEdit,
    pub error: Option<String>,
}

impl Sheet {
    /// Whether the sheet covers more than one row.
    pub fn many(&self) -> bool {
        self.targets.len() > 1
    }
}

/// The command bar's state: what's typed and which row of the filtered commands is selected.
pub struct Palette {
    pub query: TextEdit,
    pub selected: usize,
}

/// Rows copied or cut, waiting for a paste: their paths and whether a paste moves them.
#[derive(Clone, Debug)]
pub struct Clipboard {
    pub paths: Vec<PathBuf>,
    pub cut: bool,
}

/// A drag of the selected rows over a folder they can be dropped on, in window points.
pub struct FileDrag {
    pub start: (f32, f32),
    pub pointer: (f32, f32),
    /// The row the press landed on.
    press: String,
    /// The row pressed inside a selection of several: a release without a drag narrows the
    /// selection to it, as a click does.
    pending: Option<String>,
    /// A second press: a release without a drag opens the row, as a double click does.
    pending_open: bool,
    /// False until the pointer has travelled far enough to be a drag rather than a click.
    pub dragging: bool,
    /// The folder row under the pointer, if it can take the drop.
    pub row: Option<usize>,
    /// The sidebar place under the pointer, if it can take the drop.
    pub place: Option<PathBuf>,
    pub place_id: Option<&'static str>,
}

/// A press in the file list and the box it has swept so far, in window points.
pub struct Marquee {
    pub start: (f32, f32),
    pub current: (f32, f32),
    /// The selection and anchor when the press landed, which Shift and Cmd/Ctrl build on.
    base: BTreeSet<String>,
    /// Cmd/Ctrl at the press: rows in the box toggle instead of replacing the selection.
    toggle: bool,
    /// Shift at the press: rows in the box join the selection.
    extend: bool,
    /// False until the pointer has travelled far enough to be a box rather than a click.
    pub dragging: bool,
}

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
    /// One recorder per action, holding what its field shows and records.
    pub recorders: BTreeMap<Action, ShortcutRecorderState>,
    /// Where the list's viewport is scrolled to and how far it can go, told by its
    /// `El::on_scroll_offset` in the frame that scrolled it. The wheel starts from this rather
    /// than from the rows' rects: those are reported a frame late, and only once they have
    /// moved far enough, so a fast scroll would jump back by what the last frame moved.
    pub scroll_told: Rc<Cell<ScrollPos>>,
    /// The list viewport's rect in window points, from the last layout.
    pub rows_view: [f32; 4],
    /// How fast the list is coasting after the wheel, in points per second: positive scrolls on
    /// down the list, as a negative wheel delta does.
    scroll_velocity: f32,
    /// Points of precise wheel delta the rows moved since the last poll: the fingers' speed,
    /// which the rows coast at once they stop.
    wheel_push: f32,
    /// A trackpad gesture has the fingers down: they carry the rows, and only lifting them
    /// hands the speed they had to the coast. Platforms that report no phases never set it.
    wheel_held: bool,
    /// How long the fingers have rested in one place, in seconds, while down.
    wheel_hold: f32,
    /// How far the poll moved the rows this frame, so a wheel delta in the same frame starts
    /// from where the frame did instead of adding the coast to the fingers' own motion.
    coast_step: f32,
    /// How far the rows are drawn past the first or last one, in points: positive pulls the top
    /// down, negative the bottom up.
    pub scroll_overshoot: f32,
    /// The offset the app has asked the viewport for, so several wheel events in one frame add
    /// up instead of each starting from where the frame began. The poll starts every frame from
    /// [`App::scroll`], the offset the viewport was last told to have.
    scroll_at: Option<f32>,
    /// When the momentum was last stepped, for the poll's frame delta.
    last_step: Option<f64>,
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
        let recorders = keybinds.recorders();
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
            scroll_velocity: 0.0,
            wheel_push: 0.0,
            wheel_held: false,
            wheel_hold: 0.0,
            coast_step: 0.0,
            scroll_overshoot: 0.0,
            scroll_at: None,
            last_step: None,
            places_at: BTreeMap::new(),
            size_width: 92.0,
            date_width: 158.0,
        }
    }

    /// Shows `path` without touching history.
    fn enter(&mut self, path: PathBuf) {
        self.cwd = path;
        self.stop_scrolling();
        self.menu = None;
        self.marquee = None;
        self.file_drag = None;
        self.clear_typeahead();
        self.search.clear();
        self.clear_selection();
        self.reload();
    }

    pub fn navigate(&mut self, path: PathBuf) {
        if self.cwd == path {
            self.reload();
            return;
        }
        self.history.truncate(self.at + 1);
        self.history.push(path.clone());
        self.at = self.history.len() - 1;
        self.enter(path);
    }

    pub fn back(&mut self) {
        if self.at == 0 {
            return;
        }
        self.at -= 1;
        let path = self.history[self.at].clone();
        self.enter(path);
    }

    pub fn forward(&mut self) {
        if self.at + 1 >= self.history.len() {
            return;
        }
        self.at += 1;
        let path = self.history[self.at].clone();
        self.enter(path);
    }

    pub fn up(&mut self) {
        let Some(parent) = self.cwd.parent().map(Path::to_path_buf) else {
            return;
        };
        self.navigate(parent);
    }

    pub fn can_back(&self) -> bool {
        self.at > 0
    }

    pub fn can_forward(&self) -> bool {
        self.at + 1 < self.history.len()
    }

    pub fn can_up(&self) -> bool {
        self.cwd.parent().is_some()
    }

    /// Re-reads the folder, keeping the selected rows when they still exist. Rows keep their
    /// ids through a re-read, so a selection of several survives a refresh until something
    /// moves them.
    pub fn reload(&mut self) {
        let keep = self.selected.clone();
        self.read_dir();
        self.sort_entries();
        self.apply_filter();
        match keep {
            Some(id) if self.is_visible(&id) => self.focus_id(&id),
            Some(_) => self.clear_selection(),
            None => {}
        }
    }

    fn read_dir(&mut self) {
        let mut entries = Vec::new();
        let mut error = None;
        match fs::read_dir(&self.cwd) {
            Ok(read) => {
                for item in read.flatten() {
                    if let Some(entry) = Entry::read(&item) {
                        entries.push(entry);
                    }
                }
            }
            Err(err) => {
                error = Some(format!("Couldn’t open {}: {err}", self.cwd.display()));
            }
        }
        self.entries = entries;
        self.error = error;
    }

    /// Folders first, then the list's sort column and direction.
    pub fn sort_entries(&mut self) {
        let (column, direction) = self.sort.clone();
        self.entries.sort_by(|a, b| {
            b.is_dir.cmp(&a.is_dir).then_with(|| {
                let order = match column.as_str() {
                    "size" => a.size.cmp(&b.size),
                    "modified" => compare_time(a.modified, b.modified),
                    _ => rune_kit::natural_cmp(&a.name, &b.name),
                };
                direction.apply(order)
            })
        });
    }

    /// Rebuilds the rows that pass the hidden-file and search filters.
    pub fn apply_filter(&mut self) {
        // Different rows: what the old ones were coasting toward no longer means anything.
        self.stop_scrolling();
        let query = self.search.text.trim().to_lowercase();
        self.visible = (0..self.entries.len())
            .filter(|&i| {
                let entry = &self.entries[i];
                (self.show_hidden || !entry.hidden())
                    && (query.is_empty() || entry.name.to_lowercase().contains(&query))
            })
            .collect();
        // The hidden and filtered rows drop out of the selection with the row itself.
        let visible: BTreeSet<String> = self
            .visible
            .iter()
            .map(|&i| self.entries[i].id.clone())
            .collect();
        self.selection.retain(|id| visible.contains(id));
        if self.anchor.as_ref().is_some_and(|id| !visible.contains(id)) {
            self.anchor = None;
        }
        if self
            .selected
            .as_ref()
            .is_some_and(|id| !self.is_visible(id))
        {
            self.selected = None;
            self.preview = None;
            self.selection.clear();
        }
    }

    fn is_visible(&self, id: &str) -> bool {
        self.visible.iter().any(|&i| self.entries[i].id == id)
    }

    pub fn toggle_hidden(&mut self) {
        self.show_hidden = !self.show_hidden;
        self.apply_filter();
        self.remember();
    }

    /// Shows or hides the inspector, and remembers the choice.
    pub fn toggle_inspector(&mut self) {
        self.inspector.toggle();
        self.remember();
    }

    /// Keeps the list coasting after the wheel, or stops it doing so, and remembers the choice.
    pub fn toggle_scroll_inertia(&mut self) {
        self.config.scroll_inertia = !self.config.scroll_inertia;
        if !self.config.scroll_inertia {
            self.scroll_velocity = 0.0;
            self.wheel_push = 0.0;
        }
        self.remember();
    }

    /// Lets the list be pulled past its ends, or not, and remembers the choice.
    pub fn toggle_overscroll(&mut self) {
        self.config.overscroll = !self.config.overscroll;
        if !self.config.overscroll {
            self.scroll_overshoot = 0.0;
        }
        self.remember();
    }

    /// Saves the browser's view settings into `config.kdl`.
    fn remember(&mut self) {
        self.config.remember(
            self.show_hidden,
            self.inspector.is_open(),
            self.config.scroll_inertia,
            self.config.overscroll,
        );
    }

    /// Selects a row on its own, as a plain click does, and loads its preview.
    pub fn select_id(&mut self, id: &str) {
        self.selection.clear();
        self.selection.insert(id.to_owned());
        self.anchor = Some(id.to_owned());
        self.focus_id(id);
    }

    /// Drops every selected row.
    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.anchor = None;
        self.selected = None;
        self.preview = None;
    }

    /// Moves the cursor to `id` and loads its preview, leaving the selection alone.
    fn focus_id(&mut self, id: &str) {
        self.selected = Some(id.to_owned());
        let found = self
            .entries
            .iter()
            .find(|e| e.id == id)
            .map(|e| (e.path.clone(), e.is_dir));
        self.preview = match found {
            Some((path, is_dir)) => read_preview(&path, is_dir),
            None => None,
        };
    }

    pub fn selected_entry(&self) -> Option<&Entry> {
        let id = self.selected.as_deref()?;
        self.entries.iter().find(|e| e.id == id)
    }

    pub fn entry_name(&self, id: &str) -> Option<&str> {
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| entry.name.as_str())
    }

    pub fn is_selected(&self, id: &str) -> bool {
        self.selection.contains(id)
    }

    /// The rows the selection holds, in table order.
    pub fn selected_ids(&self) -> Vec<String> {
        self.visible
            .iter()
            .map(|&i| self.entries[i].id.clone())
            .filter(|id| self.selection.contains(id))
            .collect()
    }

    pub fn selection_len(&self) -> usize {
        self.selected_ids().len()
    }

    /// How many of the selected rows are folders.
    pub fn selection_folders(&self) -> usize {
        self.selected_ids()
            .iter()
            .filter_map(|id| self.entries.iter().find(|entry| &entry.id == id))
            .filter(|entry| entry.is_dir)
            .count()
    }

    /// The bytes the selected files take up; folders count as nothing, as their size column
    /// does.
    pub fn selection_size(&self) -> u64 {
        self.selected_ids()
            .iter()
            .filter_map(|id| self.entries.iter().find(|entry| &entry.id == id))
            .filter(|entry| !entry.is_dir)
            .map(|entry| entry.size)
            .sum()
    }

    /// Drops a selection of several rows down to the cursor's row, as Escape does.
    pub fn collapse_selection(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        self.selection.clear();
        if self.is_visible(&id) {
            self.selection.insert(id.clone());
        }
        self.anchor = Some(id);
    }

    /// Forgets what was typed for jumping to a name, as changing folder does.
    pub fn clear_typeahead(&mut self) {
        self.typeahead.clear();
        self.typeahead_at = None;
    }

    /// Jumps to the row that starts with what has been typed, and cycles a repeated letter
    /// through the names that share it.
    pub fn typeahead_select(&mut self, typed: &str, tx: &mut Tx) -> bool {
        let now = Instant::now();
        let fresh = self
            .typeahead_at
            .is_none_or(|at| now.duration_since(at) > Duration::from_millis(900));
        if fresh {
            self.typeahead.clear();
        }
        self.typeahead_at = Some(now);
        self.typeahead.push_str(&typed.to_lowercase());
        let buffer = self.typeahead.clone();
        // "aaa" is the third name starting with "a"; "no" narrows from the start.
        let repeated = buffer
            .chars()
            .next()
            .is_some_and(|first| buffer.chars().all(|c| c == first));
        let query = if repeated {
            buffer.chars().next().map(String::from).unwrap_or_default()
        } else {
            buffer.clone()
        };
        let skip = if repeated {
            buffer.chars().count() - 1
        } else {
            0
        };
        let matches: Vec<usize> = (0..self.visible.len())
            .filter(|&i| {
                self.entries[self.visible[i]]
                    .name
                    .to_lowercase()
                    .starts_with(&query)
            })
            .collect();
        let Some(&index) = matches.get(skip % matches.len().max(1)) else {
            return false;
        };
        let Some(id) = self.row_id(index) else {
            return false;
        };
        self.select_id(&id);
        tx.scroll_into_view_in("files-table", id);
        true
    }

    /// Selects every row the filter leaves on screen.
    pub fn select_all(&mut self) {
        self.selection = self
            .visible
            .iter()
            .map(|&i| self.entries[i].id.clone())
            .collect();
        if let Some(id) = self
            .selected
            .clone()
            .filter(|id| self.selection.contains(id))
        {
            self.anchor = Some(id);
        } else {
            self.anchor = self.selection.iter().next().cloned();
        }
    }

    pub fn selected_index(&self) -> Option<usize> {
        let id = self.selected.as_deref()?;
        self.visible.iter().position(|&i| self.entries[i].id == id)
    }

    /// The row under a point in the list's own points, where the first row starts at zero.
    pub fn row_at(&self, y: f32) -> Option<usize> {
        if y < 0.0 || self.visible.is_empty() {
            return None;
        }
        let index = (y / ROW_HEIGHT).floor() as usize;
        (index < self.visible.len()).then_some(index)
    }

    fn row_id(&self, index: usize) -> Option<String> {
        self.visible.get(index).map(|&i| self.entries[i].id.clone())
    }

    /// The rows from the anchor to row `index`, or that row alone when there's no anchor.
    fn range_to(&self, index: usize) -> BTreeSet<String> {
        let from = self
            .anchor
            .as_deref()
            .and_then(|id| self.visible.iter().position(|&i| self.entries[i].id == id))
            .unwrap_or(index);
        let (a, b) = (from.min(index), from.max(index));
        (a..=b)
            .filter_map(|i| self.visible.get(i).map(|&i| self.entries[i].id.clone()))
            .collect()
    }

    /// A click on row `index` with `mods`, the way a file manager reads them: Shift extends
    /// from the anchor, Cmd/Ctrl toggles the row, and Cmd/Ctrl+Shift keeps what is selected and
    /// adds the range.
    pub fn click_row(&mut self, index: usize, mods: Mods, tx: &mut Tx) {
        let Some(id) = self.row_id(index) else {
            if !mods.shift && !(mods.cmd || mods.ctrl) {
                self.clear_selection();
            }
            return;
        };
        let toggle = mods.cmd || mods.ctrl;
        if mods.shift {
            let range = self.range_to(index);
            if toggle {
                self.selection.extend(range);
            } else {
                self.selection = range;
            }
        } else if toggle {
            if !self.selection.remove(&id) {
                self.selection.insert(id.clone());
            }
            self.anchor = Some(id.clone());
        } else {
            self.selection.clear();
            self.selection.insert(id.clone());
            self.anchor = Some(id.clone());
        }
        self.focus_id(&id);
        // A Cmd/Ctrl-click can take the clicked row out of the selection, so the cursor moves
        // to whichever selected row is nearest.
        if !self.selection.contains(&id)
            && let Some(next) = self.nearest_selected(index)
        {
            self.focus_id(&next);
        }
        tx.scroll_into_view_in("files-table", id);
    }

    /// The selected row closest to row `index` in the list.
    fn nearest_selected(&self, index: usize) -> Option<String> {
        self.visible
            .iter()
            .enumerate()
            .filter(|(_, i)| self.selection.contains(&self.entries[**i].id))
            .min_by_key(|(at, _)| at.abs_diff(index))
            .map(|(_, i)| self.entries[*i].id.clone())
    }

    /// Moves the cursor by `delta` rows. Shift grows the range from the anchor, and Cmd/Ctrl
    /// with Shift adds it to what is selected.
    pub fn move_selection(&mut self, delta: isize, mods: Mods, tx: &mut Tx) {
        if self.visible.is_empty() {
            return;
        }
        let last = self.visible.len() - 1;
        let index = match self.selected_index() {
            Some(i) => (i as isize + delta).clamp(0, last as isize) as usize,
            None if delta > 0 => 0,
            None => last,
        };
        self.select_visible(index, mods, tx);
    }

    pub fn select_edge(&mut self, last: bool, mods: Mods, tx: &mut Tx) {
        if self.visible.is_empty() {
            return;
        }
        self.select_visible(if last { self.visible.len() - 1 } else { 0 }, mods, tx);
    }

    /// Selects the row at `index` in the filtered list, revealing it in the viewport.
    fn select_visible(&mut self, index: usize, mods: Mods, tx: &mut Tx) {
        let Some(id) = self.row_id(index) else {
            return;
        };
        if mods.shift {
            let range = self.range_to(index);
            if mods.cmd || mods.ctrl {
                self.selection.extend(range);
            } else {
                self.selection = range;
            }
        } else {
            self.selection.clear();
            self.selection.insert(id.clone());
            self.anchor = Some(id.clone());
        }
        self.focus_id(&id);
        tx.scroll_into_view_in("files-table", id);
    }

    pub fn open_selected(&mut self) {
        let Some(id) = self.selected.clone() else {
            return;
        };
        self.open_id(&id);
    }

    /// Double-click or Return on a row: enter folders, hand files to the system.
    pub fn open_id(&mut self, id: &str) {
        let Some(entry) = self.entries.iter().find(|e| e.id == id) else {
            return;
        };
        let (path, is_dir) = (entry.path.clone(), entry.is_dir);
        if is_dir {
            self.navigate(path);
        } else if let Err(err) = open_external(&path) {
            self.error = Some(err);
        }
    }

    /// Creates `New Folder`, or the next free numbered name, and selects it.
    pub fn new_folder(&mut self, tx: &mut Tx) {
        let mut name = "New Folder".to_owned();
        let mut n = 2;
        while self.cwd.join(&name).exists() {
            name = format!("New Folder {n}");
            n += 1;
        }
        let path = self.cwd.join(&name);
        match fs::create_dir(&path) {
            Ok(()) => {
                self.reload();
                let id = format!("f:{name}");
                self.select_id(&id);
                tx.scroll_into_view_in("files-table", id);
                self.error = None;
            }
            Err(err) => self.error = Some(format!("Couldn’t create “{name}”: {err}")),
        }
    }

    /// Right-clicking row `index`: open the row menu at the press. A row already inside a
    /// selection of several keeps that selection, so the menu acts on all of it; any other row
    /// is selected on its own first.
    pub fn open_row_menu(&mut self, index: usize, at: PointerPress) {
        let Some(id) = self.row_id(index) else {
            self.menu = None;
            return;
        };
        if !(self.selection.len() > 1 && self.selection.contains(&id)) {
            self.selection.clear();
            self.selection.insert(id.clone());
        }
        self.anchor = Some(id.clone());
        self.focus_id(&id);
        self.menu = Some(at);
    }

    /// A right-click anywhere in the list, in window points: the row under it gets the menu.
    pub fn row_menu_at(&mut self, at: PointerPress) {
        match self.row_at_window(at.y) {
            Some(index) if self.rows_view[0] <= at.x => self.open_row_menu(index, at),
            _ => self.menu = None,
        }
    }

    pub fn close_menu(&mut self) {
        self.menu = None;
    }

    /// The columns the list shows, and which one it sorts by.
    pub fn sort_column(&self) -> (&str, SortDir) {
        (self.sort.0.as_str(), self.sort.1)
    }

    /// A header click: the sort column flips, another column starts ascending.
    pub fn sort_by(&mut self, column: &str) {
        self.sort = if self.sort.0 == column {
            (column.to_owned(), self.sort.1.flip())
        } else {
            (column.to_owned(), SortDir::Ascending)
        };
        self.sort_entries();
        self.apply_filter();
    }

    /// Sets a column's width as dragging its header edge does, clamped to a usable range.
    pub fn resize_column(&mut self, column: &str, width: f32) {
        match column {
            "size" => self.size_width = width.clamp(64.0, 320.0),
            "modified" => self.date_width = width.clamp(100.0, 420.0),
            _ => {}
        }
    }

    /// Puts a column back to the width it starts at.
    pub fn reset_column(&mut self, column: &str) {
        match column {
            "size" => self.size_width = 92.0,
            "modified" => self.date_width = 158.0,
            _ => {}
        }
    }

    /// How far the list is scrolled, in points: what the viewport last said it holds.
    fn scroll(&self) -> f32 {
        self.scroll_told.get().offset[1].max(0.0)
    }

    /// How far the list can scroll before its last row is at the bottom, in points.
    fn max_scroll(&self) -> f32 {
        (self.visible.len() as f32 * ROW_HEIGHT - self.rows_view[3]).max(0.0)
    }

    /// Stops the wheel's momentum and any overscroll, as a folder change does.
    fn stop_scrolling(&mut self) {
        self.scroll_velocity = 0.0;
        self.wheel_push = 0.0;
        self.wheel_held = false;
        self.wheel_hold = 0.0;
        self.coast_step = 0.0;
        self.scroll_overshoot = 0.0;
        self.scroll_at = None;
    }

    /// Stops the coast dead, as fingers landing on the trackpad do: the step this frame already
    /// made is taken back, so the rows stay where they were drawn.
    fn halt_coast(&mut self, tx: &mut Tx) {
        self.scroll_velocity = 0.0;
        self.wheel_push = 0.0;
        if self.coast_step != 0.0 {
            let back = self.scroll_at.unwrap_or_else(|| self.scroll()) - self.coast_step;
            self.scroll_at = Some(back);
            self.coast_step = 0.0;
            tx.scroll_to_instant("files-table", 0.0, back);
        }
    }

    /// A wheel over the file list. With inertia or overscroll on the list applies the delta
    /// itself: the rows follow the fingers and coast at their speed when they stop, and a delta
    /// past an end pulls the rows out of place until [`App::poll`] springs them back. False
    /// leaves the wheel to the viewport, so with both settings off the list scrolls as it did.
    pub fn list_wheel(&mut self, wheel: WheelEvent, tx: &mut Tx) -> bool {
        if !self.config.scroll_inertia && !self.config.overscroll {
            return false;
        }
        if wheel.momentum && self.config.scroll_inertia {
            // The platform's coasting would double the list's own.
            return true;
        }
        // A trackpad gesture's phases are the fingers themselves: down stops the rows where
        // they are, even when they never moved, and lifting them hands the speed they had to
        // the coast. Momentum has phases of its own, which aren't a hand.
        if !wheel.momentum && wheel.phase != GesturePhase::Moved {
            if wheel.phase == GesturePhase::Began {
                self.wheel_held = true;
                self.wheel_hold = 0.0;
                self.halt_coast(tx);
            } else {
                self.wheel_held = false;
                tx.wake_after(Duration::ZERO);
            }
        }
        if wheel.dy == 0.0 || wheel.dx.abs() > wheel.dy.abs() {
            return false;
        }
        let current = self.scroll_at.unwrap_or_else(|| self.scroll()) - self.coast_step;
        self.coast_step = 0.0;
        let max = self.max_scroll();
        let bounce = self.config.overscroll && max > 0.0;
        let target = current - wheel.dy;
        let (offset, overshoot) = if target < 0.0 && bounce {
            (
                0.0,
                (self.scroll_overshoot - target * OVERSCROLL_GIVE).min(MAX_OVERSCROLL),
            )
        } else if target > max && bounce {
            (
                max,
                (self.scroll_overshoot - (target - max) * OVERSCROLL_GIVE).max(-MAX_OVERSCROLL),
            )
        } else {
            (target.clamp(0.0, max), 0.0)
        };
        self.scroll_overshoot = overshoot;
        self.scroll_at = Some(offset);
        if offset != current {
            tx.scroll_to_instant("files-table", 0.0, offset);
        }
        if self.config.scroll_inertia && !wheel.momentum {
            if wheel.precise {
                // A trackpad's deltas are the fingers' own motion, so the poll turns them into
                // the speed the rows coast at once the fingers stop. Adding a coast to every
                // delta would run the rows ahead of them while they are still moving.
                self.wheel_push += wheel.dy;
            } else {
                // A notch is a jump with no speed behind it: it leaves a short glide.
                self.scroll_velocity = (self.scroll_velocity - wheel.dy * SCROLL_GAIN)
                    .clamp(-SCROLL_MAX_VELOCITY, SCROLL_MAX_VELOCITY);
            }
        }
        tx.wake_after(Duration::ZERO);
        true
    }

    /// Steps the list's momentum and overscroll once per frame, asking for the next frame while
    /// either still moves. True when the rows moved, so the view draws them where they are.
    pub fn poll(&mut self, t: f64, tx: &mut Tx) -> bool {
        // How much time really passed, and how much this frame steps: a window with nothing to
        // draw runs no frames, so a rest between the last delta and the next frame is one long
        // step. Resting on the trackpad is measured in the real time, not the stepped time.
        let since = match self.last_step {
            Some(last) => (t - last).max(0.0) as f32,
            None => 0.0,
        };
        let dt = since.clamp(0.0, 0.05);
        self.last_step = Some(t);
        self.coast_step = 0.0;
        // Every request the last frame made has been committed and told by now.
        let current = self.scroll();
        self.scroll_at = Some(current);
        let mut changed = false;
        // A drag held against the list's top or bottom edge pulls the rows along with it.
        let current = if dt > 0.0 {
            let scrolled = self.edge_scroll(current, dt, tx);
            changed |= scrolled != current;
            scrolled
        } else {
            current
        };
        // The wheel's deltas of the last frame were the fingers' speed, which the rows coast at
        // once they stop. A flick carries the fastest its fingers went, so a rising speed is
        // taken at once and only a falling one eases in.
        let pushed = std::mem::take(&mut self.wheel_push);
        if dt > 0.0 && pushed != 0.0 {
            let speed = (-pushed / dt).clamp(-SCROLL_MAX_VELOCITY, SCROLL_MAX_VELOCITY);
            let rising = speed.abs() > self.scroll_velocity.abs()
                || speed.signum() != self.scroll_velocity.signum();
            if rising {
                self.scroll_velocity = speed;
            } else {
                let blend = 1.0 - (-dt / SCROLL_SPEED_SMOOTH).exp();
                self.scroll_velocity += (speed - self.scroll_velocity) * blend;
            }
        }
        // Fingers resting on the trackpad are not a fling: once they have been in one place
        // long enough, lifting them starts nothing.
        if dt > 0.0 && self.wheel_held {
            self.wheel_hold = if pushed == 0.0 {
                self.wheel_hold + since
            } else {
                0.0
            };
            if pushed == 0.0 {
                // Resting fingers take the speed away quickly, not at the coast's rate.
                self.scroll_velocity *= (-SCROLL_HOLD_DECAY * since).exp();
                if self.scroll_velocity.abs() < SCROLL_MIN_VELOCITY {
                    self.scroll_velocity = 0.0;
                }
            }
            if self.wheel_hold >= SCROLL_HOLD_LIMIT {
                self.scroll_velocity = 0.0;
            }
            if self.wheel_hold >= SCROLL_HELD_FORGET {
                self.wheel_held = false;
            }
        }
        if dt > 0.0 && self.scroll_overshoot != 0.0 {
            self.scroll_overshoot *= (-OVERSCROLL_RETURN * dt).exp();
            if self.scroll_overshoot.abs() < 0.5 {
                self.scroll_overshoot = 0.0;
            }
            changed = true;
            tx.wake_after(Duration::ZERO);
        }
        // Fingers on the trackpad carry the rows themselves; the speed is only told to the
        // coast when they lift, so holding still stops the list where it is.
        if dt > 0.0 && !self.wheel_held && self.scroll_velocity.abs() >= SCROLL_MIN_VELOCITY {
            let max = self.max_scroll();
            let next = (current + self.scroll_velocity * dt).clamp(0.0, max);
            if (next - current).abs() > 0.01 {
                tx.scroll_to_instant("files-table", 0.0, next);
                self.scroll_at = Some(next);
                self.coast_step = next - current;
                changed = true;
            }
            let at_end = (next <= 0.0 && self.scroll_velocity < 0.0)
                || (next >= max && self.scroll_velocity > 0.0);
            // While the fingers still drive the rows, their speed isn't the coast's to decay.
            if pushed == 0.0 {
                self.scroll_velocity *= (-SCROLL_DECAY * dt).exp();
            }
            if at_end || self.scroll_velocity.abs() < SCROLL_MIN_VELOCITY {
                self.scroll_velocity = 0.0;
            }
            tx.wake_after(Duration::ZERO);
        } else if dt > 0.0 && pushed == 0.0 && !self.wheel_held {
            // No coast to run: the speed left over is the estimate, which fades while nothing
            // moves. A held gesture's estimate fades faster, above.
            self.scroll_velocity *= (-SCROLL_DECAY * since).exp();
            if self.scroll_velocity.abs() < SCROLL_MIN_VELOCITY {
                self.scroll_velocity = 0.0;
            }
        }
        changed
    }

    /// Turns a point in the list viewport's own points into window points.
    pub fn window_point(&self, local: (f32, f32)) -> (f32, f32) {
        (self.rows_view[0] + local.0, self.rows_view[1] + local.1)
    }

    /// The row under a window point inside the list.
    pub fn row_at_window(&self, y: f32) -> Option<usize> {
        // The bounce draws the rows where the scroll doesn't, so the pointer follows them.
        self.row_at(y - self.rows_view[1] + self.scroll() - self.scroll_overshoot)
    }

    /// A press in the list: a press on a row selects it (or keeps a selection it belongs to, so
    /// the selection can be dragged) and a press on the empty space starts a selection box. With
    /// Option (Alt) held the box starts anywhere, over the rows themselves.
    /// Points are the window's.
    pub fn begin_press(&mut self, at: (f32, f32), mods: Mods, tx: &mut Tx) {
        // A box over the rows is the one thing they can't get otherwise: the list only has empty
        // space to start one in when it doesn't scroll, so Option is how a long list gets swept.
        if mods.alt && self.row_at_window(at.1).is_some() {
            self.file_drag = None;
            self.marquee = Some(Marquee {
                start: at,
                current: at,
                base: self.selection.clone(),
                toggle: mods.cmd || mods.ctrl,
                extend: mods.shift,
                dragging: false,
            });
            return;
        }
        let Some(index) = self.row_at_window(at.1) else {
            self.file_drag = None;
            self.marquee = Some(Marquee {
                start: at,
                current: at,
                base: self.selection.clone(),
                toggle: mods.cmd || mods.ctrl,
                extend: mods.shift,
                dragging: false,
            });
            // A press on the empty space below the rows clears them, unless it adds.
            if !mods.shift && !(mods.cmd || mods.ctrl) {
                self.clear_selection();
            }
            return;
        };
        let plain = !mods.shift && !(mods.cmd || mods.ctrl);
        let inside = self
            .row_id(index)
            .is_some_and(|id| self.selection.len() > 1 && self.selection.contains(&id));
        // Pressing a row inside a selection of several keeps the selection for the drag; a
        // release without a drag narrows it to that row.
        let pending = inside && plain;
        self.marquee = None;
        self.file_drag = self.row_id(index).map(|id| FileDrag {
            start: at,
            pointer: at,
            press: id.clone(),
            pending: pending.then_some(id),
            pending_open: false,
            dragging: false,
            row: None,
            place: None,
            place_id: None,
        });
        if !pending {
            self.click_row(index, mods, tx);
        }
    }

    /// The pointer moved with the button down: a row drag heads for a folder, a press on empty
    /// space grows a selection box.
    pub fn drag_to(&mut self, at: (f32, f32)) {
        if let Some(mut drag) = self.file_drag.take() {
            drag.pointer = at;
            let travel = (at.0 - drag.start.0).abs().max((at.1 - drag.start.1).abs());
            if !drag.dragging && travel < DRAG_START {
                self.file_drag = Some(drag);
                return;
            }
            drag.dragging = true;
            self.aim_drag(&mut drag);
            self.file_drag = Some(drag);
            return;
        }
        let Some(mut marquee) = self.marquee.take() else {
            return;
        };
        marquee.current = at;
        let travel = (at.0 - marquee.start.0)
            .abs()
            .max((at.1 - marquee.start.1).abs());
        if !marquee.dragging && travel < DRAG_START {
            self.marquee = Some(marquee);
            return;
        }
        marquee.dragging = true;
        self.sweep_box(&marquee);
        self.marquee = Some(marquee);
    }

    /// Selects the rows a box covers, built on the selection it started from: Shift adds the
    /// box, Cmd/Ctrl toggles it, and a plain box replaces the selection with it.
    fn sweep_box(&mut self, marquee: &Marquee) {
        let swept = self.rows_in_box(marquee.start, marquee.current);
        self.selection = if marquee.extend {
            marquee.base.union(&swept).cloned().collect()
        } else if marquee.toggle {
            marquee.base.symmetric_difference(&swept).cloned().collect()
        } else {
            swept
        };
        if self
            .selected
            .as_ref()
            .is_none_or(|id| !self.selection.contains(id))
        {
            match self.selected_ids().first().cloned() {
                Some(id) => self.focus_id(&id),
                None => {
                    self.selected = None;
                    self.preview = None;
                }
            }
        }
    }

    /// The button came up: drop a dragged selection on its target, narrow a click inside a
    /// selection to its row, or put the old selection back when the box was cancelled.
    pub fn end_drag(&mut self, cancelled: bool) {
        if let Some(drag) = self.file_drag.take() {
            if cancelled {
                return;
            }
            if drag.dragging {
                if let Some(destination) = self.drop_target(&drag) {
                    self.move_selection_into(&destination);
                }
            } else if drag.pending_open {
                self.open_id(&drag.press);
            } else if let Some(id) = drag.pending {
                self.select_id(&id);
            }
            return;
        }
        let Some(marquee) = self.marquee.take() else {
            return;
        };
        if !cancelled {
            return;
        }
        self.selection = marquee.base;
        match self
            .selected
            .clone()
            .filter(|id| self.selection.contains(id))
        {
            Some(id) => self.focus_id(&id),
            None => match self.selected_ids().first().cloned() {
                Some(id) => self.focus_id(&id),
                None => {
                    self.selected = None;
                    self.preview = None;
                }
            },
        }
    }

    /// The folder a row drag is over, if it can take the drop: a sidebar place or a folder row.
    /// Aims at the rows where this frame is putting them (see [`App::scroll_at`]), so a drag
    /// scrolling the list takes the row the pointer has just been carried over.
    fn aim_drag(&self, drag: &mut FileDrag) {
        let scroll = self.scroll_at.unwrap_or_else(|| self.scroll());
        drag.row = None;
        drag.place = None;
        drag.place_id = None;
        let (x, y) = drag.pointer;
        // The sidebar sits left of the list, and takes the drop before the rows do.
        for (id, (rect, path)) in &self.places_at {
            if inside(*rect, x, y) && *path != self.cwd {
                drag.place = Some(path.clone());
                drag.place_id = Some(id);
                return;
            }
        }
        if !inside(self.rows_view, x, y) {
            return;
        }
        // The bounce draws the rows where the scroll doesn't, so the pointer follows them.
        let Some(index) = self.row_at(y - self.rows_view[1] + scroll - self.scroll_overshoot)
        else {
            return;
        };
        if let Some(&entry) = self.visible.get(index) {
            let entry = &self.entries[entry];
            if entry.is_dir && entry.path != self.cwd && !self.selection.contains(&entry.id) {
                drag.row = Some(index);
            }
        }
    }

    /// Scrolls the list while a drag is held against its top or bottom edge, faster the further
    /// past the edge the pointer is pushed, and applies the drag to the rows that moved under
    /// it. Returns the offset the rows are at afterwards.
    fn edge_scroll(&mut self, offset: f32, dt: f32, tx: &mut Tx) -> f32 {
        // A row drag heads for a folder and a box sweeps rows: both follow their pointer.
        let pointer = match (&self.file_drag, &self.marquee) {
            (Some(drag), _) if drag.dragging => Some(drag.pointer),
            (_, Some(marquee)) if marquee.dragging => Some(marquee.current),
            _ => None,
        };
        let Some((x, y)) = pointer else {
            return offset;
        };
        let view = self.rows_view;
        // The sidebar sits left of the list: a drag over a place there scrolls nothing.
        let depth = if x < view[0] || view[3] <= 0.0 {
            0.0
        } else {
            // Up at the top edge, down at the bottom one, and past either kept at full speed.
            let above = (view[1] + DRAG_SCROLL_EDGE - y) / DRAG_SCROLL_EDGE;
            let below = (y - (view[1] + view[3] - DRAG_SCROLL_EDGE)) / DRAG_SCROLL_EDGE;
            if above > 0.0 {
                -above
            } else if below > 0.0 {
                below
            } else {
                0.0
            }
        };
        let step = depth.clamp(-1.0, 1.0) * DRAG_SCROLL_MAX * dt;
        let next = (offset + step).clamp(0.0, self.max_scroll());
        self.scroll_at = Some(next);
        if (next - offset).abs() > 0.01 {
            // A drag owns the list: nothing coasts under it.
            self.scroll_velocity = 0.0;
            tx.scroll_to_instant("files-table", 0.0, next);
            tx.wake_after(Duration::ZERO);
        }
        self.follow_drag();
        next
    }

    /// Applies a dragging pointer to the rows where the frame has just put them: the folder a
    /// row drag is over, or the rows a box now covers.
    fn follow_drag(&mut self) {
        if let Some(mut drag) = self.file_drag.take() {
            self.aim_drag(&mut drag);
            self.file_drag = Some(drag);
        } else if let Some(marquee) = self.marquee.take() {
            self.sweep_box(&marquee);
            self.marquee = Some(marquee);
        }
    }

    /// Where a drag would drop, for the button's release.
    fn drop_target(&self, drag: &FileDrag) -> Option<PathBuf> {
        if let Some(path) = &drag.place {
            return Some(path.clone());
        }
        let index = drag.row?;
        self.visible
            .get(index)
            .map(|&i| self.entries[i].path.clone())
    }

    /// The folder a drag is over, for the view to highlight it.
    pub fn drop_row(&self) -> Option<usize> {
        self.file_drag.as_ref().and_then(|drag| drag.row)
    }

    /// The sidebar place a drag is over, for the view to highlight it.
    pub fn drop_place(&self) -> Option<&'static str> {
        self.file_drag.as_ref().and_then(|drag| drag.place_id)
    }

    /// The rows being dragged, once the pointer has moved far enough, and where the pointer
    /// is: what the view shows following the pointer.
    pub fn drag_badge(&self) -> Option<(String, (f32, f32))> {
        let drag = self.file_drag.as_ref().filter(|drag| drag.dragging)?;
        let count = self.selection.len();
        let noun = if count == 1 { "item" } else { "items" };
        Some((format!("Move {count} {noun}"), drag.pointer))
    }

    /// Moves the selection into `destination`, the way a drop does. A refusal keeps everything
    /// where it is and says why in the banner.
    pub fn move_selection_into(&mut self, destination: &Path) -> bool {
        let targets = self.selected_ids();
        if targets.is_empty() || destination == self.cwd {
            return false;
        }
        match self.move_targets(&targets, destination) {
            Ok(()) => {
                self.error = None;
                true
            }
            Err(message) => {
                self.error = Some(message);
                false
            }
        }
    }

    /// The rows a box between two window points covers, limited to the rows on screen.
    fn rows_in_box(&self, a: (f32, f32), b: (f32, f32)) -> BTreeSet<String> {
        if self.visible.is_empty() {
            return BTreeSet::new();
        }
        let view = self.rows_view;
        let (top, bottom) = (
            a.1.min(b.1).max(view[1]),
            a.1.max(b.1).min(view[1] + view[3]),
        );
        if bottom < top {
            return BTreeSet::new();
        }
        // The rows are where this frame is putting them, not where the last one drew them.
        let scroll = self.scroll_at.unwrap_or_else(|| self.scroll());
        let row = |y: f32| self.row_at(y - view[1] + scroll - self.scroll_overshoot);
        let first = row(top).unwrap_or(0);
        let last = row(bottom).unwrap_or(self.visible.len().saturating_sub(1));
        (first..=last.min(self.visible.len() - 1))
            .filter_map(|i| self.visible.get(i).map(|&i| self.entries[i].id.clone()))
            .collect()
    }
    /// The box's rect in window points, while a drag is drawing one. A drag straight down (or
    /// across) still draws: the box keeps a hairline on the side it has no extent on.
    pub fn marquee_rect(&self) -> Option<[f32; 4]> {
        let marquee = self.marquee.as_ref().filter(|m| m.dragging)?;
        let view = self.rows_view;
        let left = marquee.start.0.min(marquee.current.0).max(view[0]);
        let right = marquee
            .start
            .0
            .max(marquee.current.0)
            .min(view[0] + view[2]);
        let top = marquee.start.1.min(marquee.current.1).max(view[1]);
        let bottom = marquee
            .start
            .1
            .max(marquee.current.1)
            .min(view[1] + view[3]);
        let (width, height) = (right - left, bottom - top);
        if height <= 0.0 {
            return None;
        }
        Some([left, top, width.max(1.0), height])
    }

    /// Opens the settings panel on General, or leaves it on the category already showing.
    /// Shows the command bar, empty, with its field ready for typing.
    pub fn open_palette(&mut self) {
        let mut query = TextEdit::new("");
        query.focused = true;
        self.palette = Some(Palette { query, selected: 0 });
    }

    pub fn close_palette(&mut self) {
        self.palette = None;
    }

    /// The bar's text field (`TextInput::on_edit`).
    pub fn palette_edit(&mut self) -> &mut TextEdit {
        &mut self
            .palette
            .as_mut()
            .expect("the open command bar has a field")
            .query
    }

    /// The commands the bar lists: every registry action but its own opener, filtered by the
    /// query against the label or the id.
    pub fn palette_actions(&self) -> Vec<Action> {
        let query = self
            .palette
            .as_ref()
            .map(|palette| palette.query.text.trim().to_lowercase())
            .unwrap_or_default();
        Action::ALL
            .into_iter()
            .filter(|action| *action != Action::CommandBar)
            .filter(|action| {
                query.is_empty()
                    || action.label().to_lowercase().contains(&query)
                    || action.id().contains(&query)
            })
            .collect()
    }

    /// The filter changed: the first row is selected again.
    pub fn palette_requery(&mut self) {
        if let Some(palette) = &mut self.palette {
            palette.selected = 0;
        }
    }

    pub fn palette_move(&mut self, delta: isize) {
        let count = self.palette_actions().len();
        if count == 0 {
            self.palette_requery();
            return;
        }
        if let Some(palette) = &mut self.palette {
            let last = count as isize - 1;
            palette.selected = (palette.selected as isize + delta).clamp(0, last) as usize;
        }
    }

    /// Runs the selected command and closes the bar.
    pub fn run_palette_command(&mut self, tx: &mut Tx) {
        let actions = self.palette_actions();
        let selected = self.palette.as_ref().map(|p| p.selected).unwrap_or(0);
        let action = actions.get(selected).copied();
        self.close_palette();
        if let Some(action) = action {
            self.run_action(action, tx);
        }
    }

    /// Runs a command picked with the pointer, and closes the bar.
    pub fn choose_command(&mut self, action: Action, tx: &mut Tx) {
        self.close_palette();
        self.run_action(action, tx);
    }

    pub fn open_settings(&mut self) {
        if self.settings.is_none() {
            self.settings = Some(SettingsTab::General);
        }
    }

    pub fn open_tab(&mut self, tab: SettingsTab) {
        self.settings = Some(tab);
    }

    pub fn close_settings(&mut self) {
        self.settings = None;
    }

    /// A recorder proposed `shortcut` for `action`: refuse a chord another action uses, and
    /// save the change. The error words itself on the recorder's field.
    pub fn set_binding(
        &mut self,
        action: Action,
        shortcut: Option<&Shortcut>,
    ) -> Result<(), ShortcutError> {
        self.keybinds.set(action, shortcut)
    }

    /// Puts every shortcut back to its default and shows that in the fields.
    pub fn reset_bindings(&mut self) {
        self.keybinds.reset();
        self.recorders = self.keybinds.recorders();
    }

    /// Whether `action` has something to do right now (Back needs history, the file operations
    /// need a selection); the command bar dims the rest.
    pub fn action_enabled(&self, action: Action) -> bool {
        match action {
            Action::Back => self.can_back(),
            Action::Forward => self.can_forward(),
            Action::EnclosingFolder => self.can_up(),
            // Only one file can be renamed at a time; the rest take the selection.
            Action::Rename => self.selection_len() == 1,
            Action::Move | Action::Trash | Action::Copy | Action::Cut | Action::Duplicate => {
                self.selection_len() > 0
            }
            Action::Paste => self.clipboard.is_some() && self.selection_len() > 0,
            _ => true,
        }
    }

    /// Runs `action` when it applies, whatever asked: a shortcut, the command bar or a menu.
    pub fn run_action(&mut self, action: Action, tx: &mut Tx) {
        if !self.action_enabled(action) {
            return;
        }
        self.perform(action, tx);
    }

    /// Runs a bound command.
    fn perform(&mut self, action: Action, tx: &mut Tx) {
        match action {
            Action::CommandBar => self.open_palette(),
            Action::OpenSettings => self.open_settings(),
            Action::Back => self.back(),
            Action::Forward => self.forward(),
            Action::EnclosingFolder => self.up(),
            Action::Refresh => self.reload(),
            Action::NewFolder => self.new_folder(tx),
            Action::ToggleHidden => self.toggle_hidden(),
            Action::Rename => self.begin_rename(),
            Action::Move => self.begin_move(),
            Action::Trash => self.begin_trash(),
            Action::Copy => self.copy_selection(false),
            Action::Cut => self.copy_selection(true),
            Action::Paste => self.paste(),
            Action::Duplicate => self.duplicate_selection(),
        }
    }

    /// Puts the selection on the app's clipboard. A cut pastes as a move and marks the rows
    /// dimmed until it does.
    pub fn copy_selection(&mut self, cut: bool) {
        let paths: Vec<PathBuf> = self
            .selected_ids()
            .iter()
            .filter_map(|id| self.entry_snapshot(id))
            .map(|(path, _, _)| path)
            .collect();
        if paths.is_empty() {
            return;
        }
        self.clipboard = Some(Clipboard { paths, cut });
        self.error = None;
    }

    /// Whether `path` was cut and is waiting for a paste, so the list can dim it.
    pub fn is_cut(&self, path: &Path) -> bool {
        self.clipboard
            .as_ref()
            .is_some_and(|clipboard| clipboard.cut && clipboard.paths.iter().any(|p| p == path))
    }

    /// Copies the clipboard into this folder, or moves what was cut. There is no
    /// "replace?" prompt: a copy takes a free name of its own, and a cut that would overwrite
    /// stops and says so.
    pub fn paste(&mut self) {
        let Some(clipboard) = self.clipboard.clone() else {
            return;
        };
        let destination = self.cwd.clone();
        let mut made: Vec<String> = Vec::new();
        let mut error = None;
        if clipboard.cut {
            for path in &clipboard.paths {
                if path.parent() == Some(destination.as_path()) {
                    continue;
                }
                let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                    continue;
                };
                let target = destination.join(&name);
                if target.exists() {
                    error = Some(format!(
                        "An item named “{name}” already exists in “{}”.",
                        destination.display()
                    ));
                    break;
                }
                if let Err(err) = fs::rename(path, &target) {
                    error = Some(format!("Couldn’t move “{name}”: {err}"));
                    break;
                }
                made.push(name);
            }
            if error.is_none() {
                // A cut is pasted once, as it is in Finder.
                self.clipboard = None;
            }
        } else {
            for path in &clipboard.paths {
                let Some(name) = path.file_name().map(|n| n.to_string_lossy().into_owned()) else {
                    continue;
                };
                let new_name = free_name(&destination, &name);
                match copy_entry(path, &destination.join(&new_name)) {
                    Ok(()) => made.push(new_name),
                    Err(err) => {
                        error = Some(format!("Couldn’t copy “{name}”: {err}"));
                        break;
                    }
                }
            }
        }
        self.finish_paste(made, error);
    }

    /// Copies the selection beside itself, as Duplicate does.
    pub fn duplicate_selection(&mut self) {
        let mut made = Vec::new();
        let mut error = None;
        for id in self.selected_ids() {
            let Some((path, name, _)) = self.entry_snapshot(&id) else {
                continue;
            };
            let new_name = free_name(&self.cwd, &name);
            match copy_entry(&path, &self.cwd.join(&new_name)) {
                Ok(()) => made.push(new_name),
                Err(err) => {
                    error = Some(format!("Couldn’t duplicate “{name}”: {err}"));
                    break;
                }
            }
        }
        self.finish_paste(made, error);
    }

    /// Reloads after a paste and selects what it made.
    fn finish_paste(&mut self, made: Vec<String>, error: Option<String>) {
        self.reload();
        self.error = error.clone();
        if !made.is_empty() {
            let visible: BTreeSet<String> = self
                .visible
                .iter()
                .map(|&i| self.entries[i].id.clone())
                .collect();
            self.selection = made
                .iter()
                .map(|name| format!("f:{name}"))
                .filter(|id| visible.contains(id))
                .collect();
            if let Some(first) = self.selected_ids().first().cloned() {
                self.anchor = Some(first.clone());
                self.focus_id(&first);
            }
        }
    }

    /// Opens the rename sheet with the current name selected. Renaming needs exactly one row.
    pub fn begin_rename(&mut self) {
        let ids = self.selected_ids();
        let [id] = ids.as_slice() else {
            return;
        };
        let Some(name) = self.entry_name(id).map(str::to_owned) else {
            return;
        };
        let mut edit = TextEdit::new(name.clone());
        edit.set_selection(0, name.len());
        edit.focused = true;
        self.sheet = Some(Sheet {
            kind: SheetKind::Rename,
            targets: vec![id.clone()],
            name,
            edit,
            error: None,
        });
    }

    /// Opens the move sheet for the whole selection, starting at the current folder.
    pub fn begin_move(&mut self) {
        let targets = self.selected_ids();
        let Some(name) = self.sheet_name(&targets) else {
            return;
        };
        let mut edit = TextEdit::new(self.cwd.display().to_string());
        edit.set_selection(0, edit.text.len());
        edit.focused = true;
        self.sheet = Some(Sheet {
            kind: SheetKind::Move,
            targets,
            name,
            edit,
            error: None,
        });
    }

    /// Asks for confirmation before putting the selection in the system Trash.
    pub fn begin_trash(&mut self) {
        let targets = self.selected_ids();
        let Some(name) = self.sheet_name(&targets) else {
            return;
        };
        self.sheet = Some(Sheet {
            kind: SheetKind::Trash,
            targets,
            name,
            edit: TextEdit::new(""),
            error: None,
        });
    }

    /// What a sheet calls its rows: one name, or a count.
    fn sheet_name(&self, targets: &[String]) -> Option<String> {
        match targets {
            [] => None,
            [id] => self.entry_name(id).map(str::to_owned),
            many => Some(format!("{} items", many.len())),
        }
    }

    pub fn close_sheet(&mut self) {
        self.sheet = None;
    }

    /// Binds the sheet's text field (`TextInput::on_edit`).
    pub fn sheet_edit(&mut self) -> &mut TextEdit {
        &mut self
            .sheet
            .as_mut()
            .expect("the open sheet has a field")
            .edit
    }

    /// Applies the open sheet. A failure keeps the sheet open with its message.
    pub fn submit_sheet(&mut self, tx: &mut Tx) {
        let Some(sheet) = self.sheet.take() else {
            return;
        };
        let result = match sheet.kind {
            SheetKind::Rename => self.rename_entry(&sheet, tx),
            SheetKind::Move => self.move_entries(&sheet),
            SheetKind::Trash => self.trash_entries(&sheet),
        };
        if let Err(message) = result {
            let refocus = sheet.kind != SheetKind::Trash;
            let mut sheet = sheet;
            sheet.error = Some(message);
            sheet.edit.focused = refocus;
            self.sheet = Some(sheet);
        }
    }

    /// The sheet's rows that are still in this folder, as (path, name, is folder).
    fn sheet_entries(&self, sheet: &Sheet) -> Result<Vec<(PathBuf, String, bool)>, String> {
        let items: Vec<_> = sheet
            .targets
            .iter()
            .filter_map(|id| self.entry_snapshot(id))
            .collect();
        if items.is_empty() {
            return Err(if sheet.many() {
                "Those items are no longer in this folder.".into()
            } else {
                "That item is no longer in this folder.".into()
            });
        }
        Ok(items)
    }

    fn rename_entry(&mut self, sheet: &Sheet, tx: &mut Tx) -> Result<(), String> {
        let Some(id) = sheet.targets.first() else {
            return Err("That item is no longer in this folder.".into());
        };
        let Some((old_path, old_name, _)) = self.entry_snapshot(id) else {
            return Err("That item is no longer in this folder.".into());
        };
        let new_name = sheet.edit.text.clone();
        validate_name(&new_name)?;
        if new_name == old_name {
            return Ok(());
        }
        let folder = old_path
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.cwd.clone());
        let new_path = folder.join(&new_name);
        if new_path.exists() {
            let same = fs::canonicalize(&new_path)
                .ok()
                .zip(fs::canonicalize(&old_path).ok())
                .is_some_and(|(a, b)| a == b);
            if !same {
                return Err(format!("An item named “{new_name}” already exists."));
            }
        }
        fs::rename(&old_path, &new_path)
            .map_err(|err| format!("Couldn’t rename “{old_name}”: {err}"))?;
        self.reload();
        let id = format!("f:{new_name}");
        if self.is_visible(&id) {
            self.select_id(&id);
            tx.scroll_into_view_in("files-table", id);
        }
        self.error = None;
        Ok(())
    }

    fn move_entries(&mut self, sheet: &Sheet) -> Result<(), String> {
        self.sheet_entries(sheet)?;
        let targets: Vec<String> = sheet
            .targets
            .iter()
            .filter(|id| self.entry_snapshot(id).is_some())
            .cloned()
            .collect();
        let destination = destination_path(&sheet.edit.text, &self.cwd)?;
        self.move_targets(&targets, &destination)
    }

    /// Moves the rows `targets` into `destination`, for the move sheet and for a drop. Every
    /// row is checked before any moves, so a refusal leaves the whole selection where it is.
    fn move_targets(&mut self, targets: &[String], destination: &Path) -> Result<(), String> {
        let items: Vec<(PathBuf, String, bool)> = targets
            .iter()
            .filter_map(|id| self.entry_snapshot(id))
            .collect();
        if items.is_empty() {
            return Err(if targets.len() > 1 {
                "Those items are no longer in this folder.".into()
            } else {
                "That item is no longer in this folder.".into()
            });
        }
        let destination = fs::canonicalize(destination)
            .map_err(|_| format!("“{}” isn’t a folder.", destination.display()))?;
        if !destination.is_dir() {
            return Err(format!("“{}” isn’t a folder.", destination.display()));
        }
        let many = items.len() > 1;
        for (path, name, is_dir) in &items {
            if *is_dir {
                let source = fs::canonicalize(path).unwrap_or_else(|_| path.clone());
                if destination.starts_with(&source) {
                    return Err(if many {
                        format!("The folder “{name}” can’t move into itself or one of its folders.")
                    } else {
                        "A folder can’t move into itself or one of its folders.".into()
                    });
                }
            }
            if destination.join(name).exists() {
                return Err(format!(
                    "An item named “{name}” already exists in “{}”.",
                    destination.display()
                ));
            }
        }
        for (path, name, _) in &items {
            fs::rename(path, destination.join(name))
                .map_err(|err| format!("Couldn’t move “{name}”: {err}"))?;
        }
        self.reload();
        self.error = None;
        Ok(())
    }

    fn trash_entries(&mut self, sheet: &Sheet) -> Result<(), String> {
        let items = self.sheet_entries(sheet)?;
        for (path, name, _) in &items {
            trash::delete(path)
                .map_err(|err| format!("Couldn’t move “{name}” to the Trash: {err}"))?;
        }
        self.reload();
        self.error = None;
        Ok(())
    }

    fn entry_snapshot(&self, id: &str) -> Option<(PathBuf, String, bool)> {
        self.entries
            .iter()
            .find(|entry| entry.id == id)
            .map(|entry| (entry.path.clone(), entry.name.clone(), entry.is_dir))
    }

    pub fn phrase_for_count(&self) -> String {
        let shown = self.visible.len();
        let chosen = self.selection_len();
        if chosen > 1 {
            return format!("{chosen} of {shown} selected");
        }
        let total = self
            .entries
            .iter()
            .filter(|entry| self.show_hidden || !entry.hidden())
            .count();
        let noun = if shown == 1 { "item" } else { "items" };
        if self.search.text.trim().is_empty() {
            format!("{shown} {noun}")
        } else {
            format!("{shown} of {total} {noun}")
        }
    }
}

/// A free name beside `name` in `dir`: "notes copy.md", then "notes copy 2.md".
fn free_name(dir: &Path, name: &str) -> String {
    let (stem, extension) = match name.rfind('.') {
        Some(0) | None => (name, ""),
        Some(at) => name.split_at(at),
    };
    let mut candidate = format!("{stem} copy{extension}");
    let mut n = 2;
    while dir.join(&candidate).exists() {
        candidate = format!("{stem} copy {n}{extension}");
        n += 1;
    }
    candidate
}

/// Copies a file, or a folder and everything in it.
fn copy_entry(from: &Path, to: &Path) -> io::Result<()> {
    if from.is_dir() {
        fs::create_dir(to)?;
        for entry in fs::read_dir(from)? {
            let entry = entry?;
            copy_entry(&entry.path(), &to.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        fs::copy(from, to).map(|_| ())
    }
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Enter a name.".into());
    }
    if name == "." || name == ".." {
        return Err("That name is reserved.".into());
    }
    if name.contains('/') || name.contains('\\') {
        return Err("Names can’t contain / or \\.".into());
    }
    if name.contains('\0') {
        return Err("That name contains an invalid character.".into());
    }
    Ok(())
}

fn destination_path(raw: &str, cwd: &Path) -> Result<PathBuf, String> {
    let raw = raw.trim();
    if raw.is_empty() {
        return Err("Enter a destination folder.".into());
    }
    let home = || home_dir().ok_or_else(|| "Your home folder isn’t available.".to_owned());
    let expanded = if raw == "~" {
        home()?
    } else if let Some(rest) = raw.strip_prefix("~/").or_else(|| raw.strip_prefix("~\\")) {
        home()?.join(rest)
    } else {
        PathBuf::from(raw)
    };
    Ok(if expanded.is_absolute() {
        expanded
    } else {
        cwd.join(expanded)
    })
}

fn compare_time(a: Option<SystemTime>, b: Option<SystemTime>) -> Ordering {
    match (a, b) {
        (Some(a), Some(b)) => a.cmp(&b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
}

fn read_preview(path: &Path, is_dir: bool) -> Option<Preview> {
    if is_dir {
        return fs::read_dir(path)
            .ok()
            .map(|read| Preview::Folder(read.count()));
    }
    let meta = fs::metadata(path).ok()?;
    if meta.len() > PREVIEW_LIMIT {
        return Some(Preview::TooLarge);
    }
    let bytes = fs::read(path).ok()?;
    if bytes.contains(&0) {
        return Some(Preview::Binary);
    }
    match String::from_utf8(bytes) {
        Ok(text) => Some(Preview::Text(text)),
        Err(_) => Some(Preview::Binary),
    }
}

fn start_location() -> (PathBuf, Option<String>) {
    if let Some(arg) = rune::extra_args().iter().find(|a| !a.starts_with('-')) {
        let path = absolute(PathBuf::from(arg));
        if path.is_file() {
            let parent = path
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(root_dir);
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            return (parent, Some(name));
        }
        if path.is_dir() {
            return (path, None);
        }
    }
    (
        home_dir().unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| root_dir())),
        None,
    )
}

fn absolute(path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        std::env::current_dir()
            .map(|cwd| cwd.join(&path))
            .unwrap_or(path)
    }
}

pub fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .filter(|p| p.is_dir())
}

pub fn root_dir() -> PathBuf {
    #[cfg(unix)]
    {
        PathBuf::from("/")
    }
    #[cfg(not(unix))]
    {
        home_dir()
            .and_then(|home| home.ancestors().last().map(Path::to_path_buf))
            .unwrap_or_else(|| PathBuf::from("C:\\"))
    }
}

fn open_external(path: &Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut c = Command::new("cmd");
        c.args(["/C", "start", ""]);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = Command::new("xdg-open");
    match command.arg(path).spawn() {
        Ok(mut child) => {
            // Reap the launcher so opening many files doesn't accumulate zombies.
            std::thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(())
        }
        Err(err) => Err(format!("Couldn’t open {}: {err}", path.display())),
    }
}

/// The list's pointer gesture: a press selects as a click does, a drag draws a selection box,
/// and a second press opens the row.
pub fn list_drag(s: &mut App, drag: &mut Drag, tx: &mut Tx) {
    // Rune hands the list the pointer in its own points; the app works in window points.
    let at = s.window_point((drag.x, drag.y));
    match drag.phase {
        Phase::Down => {
            let mods = tx.mods();
            s.begin_press(at, mods, tx);
            // A modifier-click keeps its selection meaning: only a plain second click opens,
            // and only once the button comes up — a drag out of it moves the row instead.
            if drag.clicks >= 2
                && !(mods.cmd || mods.ctrl || mods.shift)
                && let Some(file_drag) = &mut s.file_drag
            {
                file_drag.pending_open = true;
            }
        }
        Phase::Move => s.drag_to(at),
        Phase::Up => s.end_drag(drag.cancelled()),
    }
}

/// Whether a window point is inside a rect.
fn inside(rect: [f32; 4], x: f32, y: f32) -> bool {
    x >= rect[0] && x <= rect[0] + rect[2] && y >= rect[1] && y <= rect[1] + rect[3]
}

/// [`App::poll`] as `AppSpec::poll` takes it: the per-frame step of the list's scrolling.
pub fn poll(s: &mut App, t: f64, tx: &mut Tx) -> bool {
    s.poll(t, tx)
}

/// Window-level shortcuts and arrow navigation while no control owns the keyboard.
pub fn on_key(s: &mut App, event: &KeyEvent, tx: &mut Tx) -> bool {
    // The command bar takes the keys its field leaves over: the arrows pick a row, Return runs
    // it, Escape closes it, and its own chord closes it too.
    if s.palette.is_some() {
        if matches!(event.key, Key::Escape) {
            s.close_palette();
            return true;
        }
        if s.keybinds.matches(Action::CommandBar, event) {
            s.close_palette();
            return true;
        }
        match event.key {
            Key::Down => s.palette_move(1),
            Key::Up => s.palette_move(-1),
            Key::Enter => s.run_palette_command(tx),
            _ => return false,
        }
        return true;
    }
    // The sheet, the row menu and the settings own the keyboard while open: their fields and
    // rows handle Return, Escape, the arrows and the focus trap.
    if s.sheet.is_some() || s.menu.is_some() || s.settings.is_some() {
        return false;
    }
    let mods = event.mods;
    let typing = tx.focused().is_some_and(|key| key.ends_with("input"));
    // The commands that change files wait until no field has the keyboard: a shortcut like F2,
    // Cmd+C or Delete should type in the search field, not touch what is selected.
    let guarded = [
        Action::Rename,
        Action::Move,
        Action::Trash,
        Action::Copy,
        Action::Cut,
        Action::Paste,
        Action::Duplicate,
    ];
    for action in Action::ALL {
        if typing && guarded.contains(&action) {
            continue;
        }
        if s.keybinds.matches(action, event) {
            s.perform(action, tx);
            return true;
        }
    }
    // Backspace alone still goes to the enclosing folder, a habit the bindings don't cover.
    if matches!(event.key, Key::Backspace) && !mods.cmd && !mods.ctrl && !mods.alt {
        s.up();
        return true;
    }
    if matches!(event.key, Key::Escape) {
        // As in the table itself, Escape takes a selection of several back to one row first.
        if s.selection_len() > 1 {
            s.collapse_selection();
            return true;
        }
        if !s.search.text.is_empty() {
            s.search.clear();
            s.apply_filter();
            return true;
        }
    }
    // A focused field owns its keys; the list's own handler already saw the ones it takes.
    if tx.focused().is_some() {
        return false;
    }
    // The forward-delete key trashes the selection, as the Delete key does everywhere.
    if matches!(event.key, Key::Delete) && !mods.cmd && !mods.ctrl && !mods.alt {
        if s.selection_len() > 0 {
            s.begin_trash();
            return true;
        }
        return false;
    }
    // Typing a name jumps to it, as it does in every file list.
    if let Key::Text(text) = &event.key
        && !text.is_empty()
        && !mods.cmd
        && !mods.ctrl
        && !mods.alt
        && s.typeahead_select(text, tx)
    {
        return true;
    }
    list_key(s, event, tx)
}

/// The keys the file list owns: arrows, paging, Home/End, Return, Cmd/Ctrl+A and Escape.
pub fn list_key(s: &mut App, event: &KeyEvent, tx: &mut Tx) -> bool {
    let mods = event.mods;
    // Cmd/Ctrl+A selects every row; a chord holding Alt is left to the bindings.
    if event.is_command('a') && !mods.alt {
        s.select_all();
        return true;
    }
    if matches!(event.key, Key::Escape) {
        if s.selection_len() > 1 {
            s.collapse_selection();
            return true;
        }
        return false;
    }
    // Cmd/Ctrl+arrow and Alt+arrow belong to the bindings (enclosing folder) or nothing.
    if mods.cmd || mods.ctrl || mods.alt {
        return false;
    }
    match event.key {
        Key::Up => s.move_selection(-1, mods, tx),
        Key::Down => s.move_selection(1, mods, tx),
        Key::PageUp => s.move_selection(-10, mods, tx),
        Key::PageDown => s.move_selection(10, mods, tx),
        Key::Home => s.select_edge(false, mods, tx),
        Key::End => s.select_edge(true, mods, tx),
        Key::Enter => {
            if s.selected.is_none() || tx.key_repeat() {
                return false;
            }
            s.open_selected();
        }
        _ => return false,
    }
    true
}
