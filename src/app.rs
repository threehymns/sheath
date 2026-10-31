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
pub use scrolling::{FILES, poll};
pub use sheets::{Sheet, SheetKind};

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::time::Instant;

use rune::{Latest, PointerPress, TextEdit, Tx, Waker};
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
use scrolling::Scrolls;
use watch::Watch;

pub const ROW_HEIGHT: f32 = 30.0;

pub const PLACE_ROW_GAP: f32 = 2.0;

pub const PLACE_ROW_HEIGHT: f32 = rune_kit::shell::ROW_HEIGHT;

pub struct App {
    pub cwd: PathBuf,
    pub entries: Vec<Entry>,
    pub visible: Vec<usize>,
    // Entries no rule or setting hides, whatever the search shows; the path bar's count.
    pub unhidden: usize,
    // Bumped when `visible` changes, so Rune can skip re-reading every row's key.
    keys_version: u64,
    pub selection: BTreeSet<String>,
    pub selected: Option<String>,
    pub anchor: Option<String>,
    pub sort: (String, SortDir),
    pub layout: Layout,
    pub layout_cut: bool,
    pub marquee: Option<Marquee>,
    pub file_drag: Option<FileDrag>,
    pub clipboard: Option<Clipboard>,
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
    pub menu: Option<PointerPress>,
    pub place_menu: Option<PlaceMenu>,
    pub place_rename: Option<PlaceRename>,
    pub settings: Option<SettingsTab>,
    pub palette: Option<Palette>,
    pub config: Settings,
    pub rules: Rules,
    pub rule_error: Option<String>,
    pub rule_drag: RuleDrag,
    pub rule_picker: Option<usize>,
    pub rule_reveal: Option<(String, u8)>,
    pub rule_query: Option<String>,
    pub rule_edit: Option<RuleEdit>,
    pub rule_delete_ask: Option<usize>,
    pub rule_dragged: bool,
    pub visit: Resolved,
    pub views: Views,
    pub places: Places,
    pub favourites_drag: Reorder,
    pub locations_drag: Reorder,
    pub place_dragged: bool,
    pub notices: Vec<Notice>,
    pub keybinds: Keybinds,
    pub recorders: BTreeMap<Action, Recorder>,
    pub store: Store,
    pub rows_view: [f32; 4],
    scrolls: Scrolls,
    pub places_at: BTreeMap<String, PlaceAt>,
    pub sections_at: BTreeMap<Section, [f32; 4]>,
    pub name_at: BTreeMap<String, f32>,
    pub size_width: f32,
    pub date_width: f32,
    scan: Latest<Scan>,
    watch: Watch,
    pending_select: Option<(Vec<String>, bool)>,
    pending_reveal: Option<String>,
}

impl App {
    pub fn new() -> App {
        let (dir, selected) = start_location();
        let mut app = App::open(dir);
        if let Some(name) = selected {
            app.pending_select = Some((vec![format!("f:{name}")], false));
        }
        app
    }

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
            unhidden: 0,
            keys_version: 0,
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
            rows_view: [0.0; 4],
            scrolls: Scrolls::default(),
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

    pub fn copy_notice(&mut self, index: usize, tx: &mut Tx) {
        let Some(notice) = self.notices.get_mut(index) else {
            return;
        };
        tx.copy(notice.clipboard());
        notice.copied = true;
        notice.shown = Instant::now();
    }

    pub fn dismiss_notice(&mut self, index: usize) {
        if index < self.notices.len() {
            self.notices.remove(index);
        }
    }

    pub fn apply_config(&mut self) {
        if self.rule_edit.is_some() {
            self.rule_edit = None;
            self.rule_query = Some(
                "The config file changed on disk, so the rule editor closed. Open the rule \
                 again to keep editing."
                    .to_owned(),
            );
        }
        self.rule_delete_ask = None;
        self.config = Settings::from_doc(self.store.doc(config::CONFIG));
        self.keybinds = Keybinds::from_doc(self.store.doc(config::KEYBINDS));
        self.recorders = Keybinds::recorders();
        self.inspector.set_open(self.config.inspector);
        self.reload_rules();
        self.reload();
    }

    pub fn rule_presentation(&self) -> Resolved {
        self.rules.resolve(&self.cwd, self.entries.len())
    }

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

    fn save_setting(&mut self, changed: crate::settings::Setting) {
        let _ = self
            .store
            .edit(config::CONFIG, |doc| self.config.write(doc, changed));
    }

    pub fn watch(&mut self, wake: Waker) {
        self.watch.start(&self.cwd, wake);
    }
}

pub fn setup(s: &mut App, wake: Waker) {
    s.scan.set_waker(wake.clone());
    s.store.watch(wake.clone());
    s.watch(wake);
}
