//! Avar: a window that is only the terminal. The same crate, rows and scroll as the pane inside
//! Sheath, with none of the file manager around it — a shell, a full-window grid, and the wheel
//! and drag that go with it. On top of it: tabs, each a shell and each splittable into a second
//! pane beside it, the shared command palette, a keybind editor, and the `keybinds.kdl` those
//! overrides live in. The tabs and the split are rune-kit's own shell pieces: a `Sidebar` whose
//! rows drag to reorder, and a `SplitView` whose inspector is the split's pane.
//!
//! ```sh
//! cargo run -p avar            # a shell in the folder you are in
//! cargo run -p avar -- ~/src   # ... or in another one
//! ```

mod action;
mod config;
mod frame;
mod icons;
mod settings;
mod tabs;

use std::cell::Cell;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use action::Action;
use actions::{Keybinds, Outcome, Recorder};
use actions_ui::{KeybindHost, Palette, PaletteHost};
use rune::{El, Key, KeyEvent, PhysicalInput, Shortcut, ShortcutError, Tx, Waker, col};
use rune_kit::shell::PaneState;
use rune_theme_opencode as theme;
use scroll::Host;
use settings::SettingsTab;
use tabs::Tabs;

/// The chords that reach the window while a shell has the keyboard. Everything else is typed:
/// these are the commands that are the window's rather than a program's, and a shell reads the
/// unshifted forms as its own control keys.
const WINDOW_CHORDS: [Action; 7] = [
    Action::CommandBar,
    Action::OpenSettings,
    Action::NewTab,
    Action::CloseTab,
    Action::NextTab,
    Action::PrevTab,
    Action::SplitPane,
];

/// Where the shell starts: the first path on the command line that is a folder, else the folder
/// the window was started in.
fn start_dir() -> PathBuf {
    std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .find(|path| path.is_dir())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("/"))
}

pub struct Window {
    /// The tabs, their shells and which of them has the keyboard.
    tabs: Tabs,
    /// The tab rail's own width, which hides like the app's sidebar.
    rail: PaneState,
    /// The appearance the last frame drew in, so the shell starts in the colours on screen.
    dark: Cell<bool>,
    /// The device scale: a terminal's cell is a whole number of its pixels.
    scale: Cell<f32>,
    /// The window's waker, which the terminal's reader thread holds to ask for a frame.
    wake: Option<Waker>,
    /// The open command bar's field and selection; dropped when the bar closes.
    palette: Option<Palette>,
    /// The chords, the recorders the editor edits them through, and the file behind both.
    keybinds: Keybinds<Action>,
    recorders: BTreeMap<Action, Recorder>,
    store: config::Store,
    /// The settings window, with the tab it was left on, while it is up.
    settings: Option<SettingsTab>,
    /// The scroll areas the palette and the editor build, and the physics for them.
    areas: scroll::Areas,
    /// Frames drawn since the last report, the bytes parsed by then, and when that report was
    /// due. `AVAR_TERM_STATS=1` prints them once a second: the frame rate the window really
    /// drew, how far behind the shell it is, and how much is still queued.
    frames: u64,
    parsed: u64,
    parse: Duration,
    readback: Duration,
    built: Cell<Duration>,
    logged: Instant,
}

impl Window {
    /// Asks for the frame a pane change needs when the shell will not: a selection that came or
    /// went is drawn by the next frame, not by anything the shell writes.
    fn wake_frame(&self, tx: &mut Tx) {
        if self.tabs.focused().is_some_and(|term| term.frame_stale()) {
            tx.wake_after(Duration::ZERO);
        }
    }

    pub fn start() -> Window {
        let store = config::Store::open();
        let keybinds = actions_kdl::keybinds::read::<Action>(store.doc());
        Window {
            tabs: Tabs::new(start_dir()),
            rail: PaneState::new(tabs::RAIL).range(tabs::RAIL_MIN, tabs::RAIL_MAX),
            dark: Cell::new(true),
            scale: Cell::new(1.0),
            wake: None,
            palette: None,
            keybinds,
            recorders: Keybinds::<Action>::recorders(),
            store,
            settings: None,
            areas: scroll::Areas::default(),
            frames: 0,
            parsed: 0,
            parse: Duration::ZERO,
            readback: Duration::ZERO,
            built: Cell::new(Duration::ZERO),
            logged: Instant::now(),
        }
    }

    /// What a command does. There is no `enabled` policy here: every command Avar has can always
    /// run, and one that found no selection simply does nothing.
    fn run(&mut self, action: Action, tx: &mut Tx) {
        match action {
            Action::CommandBar => self.palette = Some(Palette::open()),
            Action::OpenSettings => self.open_settings(),
            Action::NewTab => self.new_tab(),
            Action::CloseTab => {
                if let Some(id) = self.tabs.active_id() {
                    self.tabs.close(id);
                }
            }
            Action::NextTab => self.tabs.cycle(1),
            Action::PrevTab => self.tabs.cycle(-1),
            Action::SplitPane => self.tabs.toggle_split(),
            Action::Copy => {
                let copied = self
                    .tabs
                    .focused_mut()
                    .and_then(|term| term.copy_selection());
                if let Some(text) = copied
                    && !text.trim().is_empty()
                {
                    tx.copy(text);
                }
            }
            Action::Paste => {
                if let Ok(mut clipboard) = arboard::Clipboard::new()
                    && let Ok(text) = clipboard.get_text()
                    && let Some(term) = self.tabs.focused_mut()
                {
                    term.paste(&text);
                }
            }
            Action::SelectAll => {
                if let Some(term) = self.tabs.focused_mut() {
                    term.select_all();
                }
                self.wake_frame(tx);
            }
        }
    }

    /// A new tab, in the folder the shell the user is typing in is in.
    pub(crate) fn new_tab(&mut self) {
        let dir = self.tabs.dir().unwrap_or_else(start_dir);
        self.tabs.open(dir);
    }

    fn open_settings(&mut self) {
        if self.settings.is_none() {
            self.settings = Some(SettingsTab::Shortcuts);
        }
    }

    fn open_tab(&mut self, tab: SettingsTab) {
        self.settings = Some(tab);
    }

    fn close_settings(&mut self) {
        self.settings = None;
    }

    fn set_binding(
        &mut self,
        action: Action,
        shortcut: Option<&Shortcut>,
    ) -> Result<(), ShortcutError> {
        self.keybinds.set(action, shortcut)?;
        self.store
            .edit(|doc| actions_kdl::keybinds::set(doc, &self.keybinds, action));
        Ok(())
    }

    fn record_key(&mut self, action: Action, event: &PhysicalInput) -> bool {
        let Some(recorder) = self.recorders.get_mut(&action) else {
            return false;
        };
        match recorder.input(event) {
            Outcome::Pass => false,
            Outcome::Consumed => true,
            Outcome::Change(next) => {
                let same = next.as_ref() == self.keybinds.chord(action);
                let result = if same {
                    Ok(())
                } else {
                    self.set_binding(action, next.as_ref())
                };
                self.recorders
                    .get_mut(&action)
                    .expect("every action has a recorder")
                    .finish(next, result);
                true
            }
        }
    }

    fn reset_binding(&mut self, action: Action) {
        self.keybinds.reset(action);
        self.store
            .edit(|doc| actions_kdl::keybinds::set(doc, &self.keybinds, action));
        self.recorders.insert(action, Recorder::default());
    }
}

pub fn setup(s: &mut Window, wake: Waker) {
    s.wake = Some(wake);
}

/// Drains every pane's PTY and rebuilds the frame when it changed. Runs in the frame's poll, so
/// the waker a reader thread holds only has to ask for the next frame.
pub fn poll(s: &mut Window, t: f64, tx: &mut Tx) -> bool {
    let dark = s.dark.get();
    let mono = theme::kit(dark).ty.mono;
    // Every tab's shells keep running, on screen or not: a build in a background tab is still a
    // build. The tabs close whatever ended, and the window is its last tab, so the window closes
    // when that one's shell ends, the way Ghostty's window does when its command ends.
    let (mut redraw, blink) = s.tabs.poll(mono, s.scale.get(), s.wake.as_ref(), dark, tx);
    if s.tabs.is_empty() {
        std::process::exit(0);
    }
    if let Some(wait) = blink {
        tx.wake_after(wait);
    }
    // The palette's and the editor's wheels: the same physics the app's list shares, with no
    // drag push, because nothing here is a draggable list.
    for key in s.areas.known() {
        s.areas.begin_frame(key, t);
        let limits = s.scroll_limits(key);
        redraw |= s.areas.step_key(key, limits, 0.0, tx);
    }
    s.frames += 1;
    let elapsed = s.logged.elapsed();
    if elapsed >= Duration::from_secs(1)
        && std::env::var_os("AVAR_TERM_STATS").is_some()
        && let Some(stats) = s.tabs.focused().and_then(|term| term.stats())
    {
        let frames = std::mem::take(&mut s.frames).max(1);
        let per_frame = |time: Duration| time.as_secs_f64() * 1000.0 / frames as f64;
        let frame = elapsed.as_secs_f64() * 1000.0 / frames as f64;
        let parse = per_frame(stats.parse - s.parse);
        let readback = per_frame(stats.readback - s.readback);
        let built = per_frame(s.built.take());
        // What is left of the frame once parsing, readback and the view build are taken out:
        // Rune's layout, paint encoding, the GPU pass and any idle wait.
        let rest = (frame - parse - readback - built).max(0.0);
        let rate = (stats.parsed - s.parsed) as f64 / elapsed.as_secs_f64() / (1024.0 * 1024.0);
        eprintln!(
            "term: {frames} fps ({frame:.0} ms): parse {parse:.1}, readback {readback:.1}, \
             view {built:.1}, rest {rest:.1} ms | {rate:.1} MiB/s, queue {} KiB, lag {} ms",
            stats.queued / 1024,
            stats.lag.as_millis()
        );
        s.parsed = stats.parsed;
        s.parse = stats.parse;
        s.readback = stats.readback;
        s.logged = Instant::now();
    }
    redraw
}

pub fn on_key(s: &mut Window, event: &KeyEvent, tx: &mut Tx) -> bool {
    // The command bar's field takes the keys it needs while it has it, and the palette chord
    // closes it again.
    if s.palette.is_some() {
        return s.palette_key(event, tx);
    }
    if s.settings.is_some() {
        if matches!(event.key, Key::Escape) {
            s.close_settings();
            return true;
        }
        return false;
    }
    // A shell with the keyboard keeps every key it can use, Ctrl-C and Escape included. Only the
    // window's own chords reach it while the shell has focus; everything else is typed.
    let typing = s.tabs.focused().is_some_and(|term| term.focus);
    let chords: &[Action] = if typing { &WINDOW_CHORDS } else { &Action::ALL };
    for action in chords.iter().copied() {
        if s.keybinds.matches(action, event) {
            s.run(action, tx);
            return true;
        }
    }
    let Some(term) = s.tabs.focused_mut() else {
        return false;
    };
    let taken = term.key(event);
    // Escape dismissing a selection moves rows the shell knows nothing about, and the frame's
    // poll has already run by the time this key is routed: without a wake the wash stays on
    // screen until something else draws.
    if taken {
        s.wake_frame(tx);
    }
    taken
}

impl PaletteHost<Action> for Window {
    fn palette(&self) -> &Palette {
        self.palette
            .as_ref()
            .expect("the open command bar has a field")
    }

    fn palette_mut(&mut self) -> &mut Palette {
        self.palette
            .as_mut()
            .expect("the open command bar has a field")
    }

    fn keybinds(&self) -> &Keybinds<Action> {
        &self.keybinds
    }

    fn palette_action(&self) -> Action {
        Action::CommandBar
    }

    fn palette_enabled(&self, _action: Action) -> bool {
        true
    }

    fn palette_run(&mut self, action: Action, tx: &mut Tx) {
        self.run(action, tx);
    }

    fn palette_close(&mut self) {
        self.palette = None;
    }

    fn palette_scroll(&self, content: El) -> El {
        // The margin is what keeps three rows visible around the highlight while it walks the
        // list; without it the selected row can sit under the panel's edge.
        scroll::area(
            self,
            "command-scroll",
            col()
                .w_full()
                .max_h(380.0)
                .scroll_edge_fade(12.0)
                .scroll_margin(
                    actions_ui::palette::CLEARANCE_ROWS
                        * (actions_ui::palette::ROW_HEIGHT + actions_ui::palette::ROW_GAP),
                ),
            content,
        )
    }
}

impl KeybindHost<Action> for Window {
    fn recorder(&self, action: Action) -> Option<&Recorder> {
        self.recorders.get(&action)
    }

    fn begin_recording(&mut self, action: Action) {
        self.recorders.entry(action).or_default().begin();
    }

    fn keybinds(&self) -> &Keybinds<Action> {
        &self.keybinds
    }

    fn record_key(&mut self, action: Action, event: &PhysicalInput) -> bool {
        Window::record_key(self, action, event)
    }

    fn reset_binding(&mut self, action: Action) {
        Window::reset_binding(self, action);
    }

    fn keybinds_saved_in(&self) -> Option<String> {
        Some(self.store.path().display().to_string())
    }

    fn keybinds_error(&self) -> Option<String> {
        self.store.error().map(str::to_owned)
    }
}

impl scroll::Host for Window {
    fn areas(&self) -> &scroll::Areas {
        &self.areas
    }

    fn areas_mut(&mut self) -> &mut scroll::Areas {
        &mut self.areas
    }

    fn scroll_limits(&self, key: &'static str) -> scroll::Limits {
        scroll::Limits {
            max: self.areas.offset_cell(key).get().max[1].max(0.0),
            inertia: true,
            bounce: true,
        }
    }

    /// A viewport's rect is recorded so drags know what they are over; nothing here reads the
    /// size back, so a repeat never forces a rebuild.
    fn record_area(&mut self, key: &'static str, rect: [f32; 4]) -> bool {
        self.areas.set_region(key, rect);
        false
    }
}

fn main() {
    rune::set_app_id("dev.sheath.Avar");
    let (fonts, families) = rune_tty::fonts::spec();
    rune::run(rune::AppSpec {
        min_size: Some((480.0, 240.0)),
        fonts,
        families,
        on_key: Some(on_key),
        poll: Some(poll),
        setup: Some(setup),
        ..rune::AppSpec::new("Avar", (1180.0, 760.0), Window::start, frame::view)
    });
}
