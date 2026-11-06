//! The terminal pane's state: whether it shows, how tall it is, the last frame, and the
//! fractional scroll that makes the wheel smooth. The engine behind it is native only; the web
//! build keeps this type so the app compiles and the pane can say where the terminal lives.

mod layout;
mod link;
mod metrics;
mod mouse;
mod protocol;
mod search;
mod select;

pub(crate) use mouse::MouseAction;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use rune::{Key, KeyEvent, TextEdit, Waker};

use crate::Theme;
#[cfg(not(target_arch = "wasm32"))]
use crate::engine::Engine;
use crate::grid::Frame;

/// How long each half of the cursor's blink lasts, the rate xterm and Ghostty blink at. Only a
/// cursor the program asked to blink blinks, so a plain shell costs no frames at all.
const BLINK: Duration = Duration::from_millis(500);

/// How soon a second press on the same cell counts as a double click. Ghostty reads the
/// platform's own interval; the pane has no such answer, so this is the common one.
const CLICK_INTERVAL: Duration = Duration::from_millis(400);

pub const MIN_HEIGHT: f32 = 120.0;
pub const MAX_HEIGHT: f32 = 720.0;
pub const DEFAULT_HEIGHT: f32 = 260.0;

/// How far a drag has to push past [`MIN_HEIGHT`] before the pane collapses. A resize that ends
/// at the minimum stops there; the margin is what tells a deliberate collapse from a drag that
/// just ran out of slope. It is half the minimum, so the dead band a drag has to cross either
/// way is always half the pane's smallest height.
pub const COLLAPSE_DRAG: f32 = MIN_HEIGHT / 2.0;

/// A row's height before the mono font has been measured (see [`Terminal::measure`]): the font's
/// own line box replaces it on the first frame.
const LINE_HEIGHT: f32 = 16.5;

/// A row's place in the scroll: whole rows back from the bottom, and the fraction of a row the
/// content is shifted by. Keeping the fraction out of libghostty's viewport is what makes a
/// trackpad's small deltas move pixels without re-wrapping the screen.
pub fn split(scroll: f32) -> (usize, f32) {
    let scroll = scroll.max(0.0);
    let whole = scroll.floor();
    (whole as usize, scroll - whole)
}

/// Everything one frame's [`Terminal::drain`] produced. The app turns the copies into clipboard
/// writes and the notifications into toasts; the standalone window closes a block whose shell
/// ended. Copies and notifications are empty on the web build.
#[derive(Default)]
pub struct Drain {
    /// Whether the rows the view draws changed, so the frame is worth rebuilding.
    pub redraw: bool,
    /// Clipboard writes the shell asked for (OSC 52).
    pub copies: Vec<String>,
    /// Desktop notifications the shell sent (OSC 9, OSC 777).
    pub notifications: Vec<(String, String)>,
    /// Whether the shell behind the pane ended, so its caller can close it.
    pub exited: bool,
    /// How long until the cursor's next half, when it blinks.
    pub blink: Option<Duration>,
}

impl Drain {
    /// Folds another pane's poll into this one, keeping the soonest blink: a tab polls every
    /// block it holds and reports once.
    pub fn merge(&mut self, other: Drain) {
        self.redraw |= other.redraw;
        self.copies.extend(other.copies);
        self.notifications.extend(other.notifications);
        self.exited |= other.exited;
        self.blink = match (self.blink, other.blink) {
            (Some(soonest), Some(wait)) => Some(soonest.min(wait)),
            (soonest, wait) => soonest.or(wait),
        };
    }
}

pub struct Terminal {
    open: bool,
    height: f32,
    pub frame: Frame,
    scroll: f32,
    pub focus: bool,
    /// The last press the grid saw: when, on which cell, and how many in a row it was. A press
    /// on the same cell soon after is the double click that selects a word, and a third the
    /// line.
    last_press: Option<(Instant, (u16, u16), u8)>,
    /// Whether the cursor is in its drawn half of the blink, and when the half started.
    blink_on: bool,
    blink_at: Instant,
    /// Whether the drag in flight has shown the pane. A gesture that did is a resize, so its
    /// release is not the click that opens a collapsed pane.
    opened: bool,
    /// The screen's rect from its layout, which the grid size comes from.
    pub rect: [f32; 4],
    /// One cell, in points: the mono font's advance and line box, measured from the font.
    pub cell: f32,
    pub line: f32,
    /// Device pixels per point, as the font was measured: a cell-filling character's rule is one
    /// device pixel wide however much the display is scaled.
    scale: f32,
    /// Whether the cell is given (a prepared frame in a test) instead of measured from the font.
    fixed: bool,
    /// Extra space per glyph: the shaper's advance plus this is exactly one cell.
    #[cfg(any(feature = "text-renderer", target_arch = "wasm32"))]
    tracking: f32,
    /// The folder the shell starts in, waiting for the first poll to start it.
    pending: Option<PathBuf>,
    pub error: Option<String>,
    /// Why the pane has no terminal on this build.
    pub notice: Option<String>,
    fresh: bool,
    copies: Vec<String>,
    /// The search field, while the pane is searching its scrollback.
    search: Option<TextEdit>,
    /// The press that opened an OSC 8 link: the rest of its gesture is the link's, not a
    /// selection's.
    pub link_press: bool,
    /// The link the pointer is over, as its row and the columns it covers, and the cell that
    /// answer was read for: a move inside that cell never reaches the engine.
    hover: Option<(u16, u16, u16)>,
    hover_cell: Option<(u16, u16)>,
    /// The grid's streamed image and the rows it was painted from. The cache diffs each frame
    /// against the last one it painted, so it belongs to this terminal: a window with two panes
    /// streams two images, one per shell, rather than one image they take turns writing.
    #[cfg(all(not(target_arch = "wasm32"), not(feature = "text-renderer")))]
    pub(crate) raster: std::cell::RefCell<Option<crate::raster::Cache>>,
    #[cfg(not(target_arch = "wasm32"))]
    engine: Option<Engine>,
}

impl Default for Terminal {
    fn default() -> Self {
        Terminal {
            open: false,
            height: DEFAULT_HEIGHT,
            frame: Frame::default(),
            scroll: 0.0,
            focus: false,
            last_press: None,
            blink_on: true,
            blink_at: Instant::now(),
            opened: false,
            rect: [0.0; 4],
            cell: 7.6,
            line: LINE_HEIGHT,
            scale: 1.0,
            fixed: false,
            #[cfg(any(feature = "text-renderer", target_arch = "wasm32"))]
            tracking: 0.0,
            pending: None,
            error: None,
            notice: None,
            fresh: false,
            copies: Vec::new(),
            search: None,
            link_press: false,
            hover: None,
            hover_cell: None,
            #[cfg(all(not(target_arch = "wasm32"), not(feature = "text-renderer")))]
            raster: std::cell::RefCell::new(None),
            #[cfg(not(target_arch = "wasm32"))]
            engine: None,
        }
    }
}

// The pane's state is built on both targets; only the native build has an engine behind it, so
// the web build's copy has fields and methods nothing reads.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
impl Terminal {
    pub fn new() -> Terminal {
        Terminal::default()
    }

    /// Whether the cursor blinks at all: the program asked for a blinking cursor, the pane is
    /// open, and the keyboard is here, which is where the cursor is what the user is looking at.
    /// Everything else keeps it solid, so an idle pane never asks for a frame.
    fn blinking(&self) -> bool {
        self.open && self.focus && self.frame.cursor_blink
    }

    /// Whether to draw the cursor this frame: solid unless it is blinking and in its off half.
    pub fn cursor_on(&self) -> bool {
        self.blink_on || !self.blinking()
    }

    /// Advances the blink: flips the phase once a half is up and says so, which is the one thing
    /// a view build needs. The frame is not rebuilt, since only the cursor's own wash changes.
    pub fn tick_blink(&mut self, now: Instant) -> bool {
        if !self.blinking() || now.saturating_duration_since(self.blink_at) < BLINK {
            self.blink_on = true;
            return false;
        }
        self.blink_at = now;
        self.blink_on = !self.blink_on;
        true
    }

    /// How long until the next blink phase, for the frame loop to come back for it. `None` when
    /// the cursor is solid, so a pane nobody is typing in costs no frames.
    pub fn blink_wait(&self) -> Option<Duration> {
        self.blinking()
            .then(|| BLINK.saturating_sub(Instant::now().saturating_duration_since(self.blink_at)))
    }

    /// Whether the rows the view draws are behind what the pane holds: a selection, a search or
    /// a clear changed them without the shell writing a byte. A poll clears it by rebuilding the
    /// frame, and nothing in the shell's output asks for that frame, so a window whose key
    /// handler changed the rows asks its loop for the next one itself.
    pub fn frame_stale(&self) -> bool {
        self.fresh
    }

    /// Reads the PTY, resizes the grid to the rect it was laid out in, and rebuilds the frame
    /// when anything moved.
    fn poll(&mut self, wake: Option<&Waker>, theme: Theme) -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (wake, theme);
            false
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if !self.open {
                // A collapsed pane still notices its shell ending, so opening it again starts a
                // fresh shell instead of showing one that already exited.
                if let Some(engine) = self.engine.as_mut() {
                    engine.drain();
                    if engine.shell_exited() {
                        self.engine = None;
                        self.fresh = true;
                    }
                }
                return false;
            }
            if let Some(dir) = self.pending.take() {
                self.start(dir, wake, theme);
            }
            // The cursor's half flips before the engine is borrowed: the view draws the cursor,
            // so the frame is built again but the grid is not read for it.
            let blinked = self.tick_blink(Instant::now());
            let (cols, rows) = self.grid_size();
            let Some(engine) = self.engine.as_mut() else {
                return blinked;
            };
            // A switch between light and dark while the shell runs belongs on the screen too,
            // not just on the next shell that starts.
            engine.set_theme(theme);
            let mut redraw = engine.drain();
            // A search scrolls the viewport from the engine's side; the pane's fractional
            // scroll follows it so the rows drawn are the rows the match is on.
            if engine.searching() {
                self.scroll = engine.scroll_position() as f32;
                redraw = true;
            }
            redraw |= blinked;
            redraw |= engine.resize(cols, rows, (self.cell, self.line), self.scale);
            if engine.take_bell() {
                redraw = true;
            }
            // A program in a synchronized-output hold (mode 2026) is still assembling its frame: drawing
            // now would show the half-written screen the hold exists to hide. The bytes keep being
            // parsed, and the frame is drawn the moment the hold ends.
            if engine.holding() {
                return redraw;
            }
            // A stale frame is a change the shell knows nothing about: a selection, a copy or a
            // search moved the rows without a byte arriving, so the app has to be told to draw
            // them. `refresh` takes the flag, so it is read first.
            let stale = self.fresh;
            if redraw || stale || split(self.scroll).1 > 0.0 {
                self.refresh();
            }
            if redraw {
                self.clamp_scroll();
            }
            redraw |= stale;
            redraw
        }
    }

    /// Reads the PTY and hands back everything one frame's poll produced: the redraw, the
    /// clipboard and notifications the shell sent, whether its shell ended, and the next blink.
    /// The app and the standalone window aggregate this instead of repeating the sequence of
    /// `poll`, `take_copies`, `take_notifications`, `shell_exited` and `blink_wait`.
    pub fn drain(&mut self, wake: Option<&Waker>, theme: Theme) -> Drain {
        let redraw = self.poll(wake, theme);
        Drain {
            redraw,
            copies: self.take_copies(),
            notifications: self.take_notifications(),
            exited: self.shell_exited(),
            blink: self.blink_wait(),
        }
    }

    /// Reads the viewport into the frame at the scroll the pane is showing. The view is built
    /// from this frame, so a wheel refreshes here as well as in the poll: a scroll that lands
    /// between the two would otherwise draw the new offset over the previous row set, which
    /// shows as the content jumping back a row at every boundary it crosses.
    fn refresh(&mut self) {
        let fraction = split(self.scroll).1 > 0.0;
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut() {
            engine.ensure_edges(fraction);
            engine.fill(&mut self.frame);
        }
        #[cfg(target_arch = "wasm32")]
        let _ = fraction;
        self.fresh = false;
    }

    /// Sends the key to the shell. Keys a terminal can't use are still consumed, so nothing
    /// behind the pane acts on them.
    pub fn key(&mut self, event: &KeyEvent) -> bool {
        // The cursor is solid while the user types: chasing a blink in the middle of a keystroke
        // is what makes a blinking cursor feel like it lags the keyboard.
        self.blink_at = Instant::now();
        self.blink_on = true;
        // Escape ends a selection before the shell sees it: the wash is the pane's own state, so
        // the key dismisses it the way it dismisses a dialog, and a shell that got it would read
        // a cancel nobody typed at it. Modifiers keep their meaning and go straight to the shell.
        let m = event.mods;
        if matches!(event.key, Key::Escape)
            && !(m.shift || m.cmd || m.ctrl || m.alt)
            && self.has_selection()
        {
            self.clear_selection();
            return true;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let Some(engine) = self.engine.as_mut() else {
                return false;
            };
            let consumed = engine.send_key(event);
            if consumed {
                self.scroll_to_bottom();
            }
            consumed
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = event;
            false
        }
    }

    pub fn paste(&mut self, text: &str) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut() {
            engine.paste(text);
            self.scroll_to_bottom();
        }
        #[cfg(target_arch = "wasm32")]
        let _ = text;
    }

    /// The wheel's delta in points: a whole line per twenty points, kept fractional so a
    /// trackpad glides and a notched wheel still lands on a line.
    pub fn wheel(&mut self, dy: f32) -> bool {
        if !self.open {
            return false;
        }
        let lines = dy / self.line;
        if lines == 0.0 {
            return false;
        }
        self.scroll = (self.scroll + lines).max(0.0);
        self.clamp_scroll();
        self.sync_scroll();
        self.refresh();
        true
    }

    pub fn scroll_to_bottom(&mut self) {
        if self.scroll == 0.0 {
            return;
        }
        self.scroll = 0.0;
        self.sync_scroll();
        self.refresh();
    }

    /// How far the content is shifted down, in points.
    pub fn offset(&self) -> f32 {
        split(self.scroll).1 * self.line
    }

    pub fn scroll_back(&self) -> usize {
        split(self.scroll).0
    }

    fn clamp_scroll(&mut self) {
        let max = self.max_scroll();
        if self.scroll > max {
            self.scroll = max;
            self.sync_scroll();
        }
    }

    fn max_scroll(&self) -> f32 {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = &self.engine {
            return engine.scrollback() as f32;
        }
        0.0
    }

    fn sync_scroll(&mut self) {
        let (rows, _) = split(self.scroll);
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut()
            && engine.set_scroll(rows)
        {
            self.fresh = true;
        }
        #[cfg(target_arch = "wasm32")]
        let _ = rows;
    }

    /// Sets the scroll without a shell, so a test can check where a fractional offset draws.
    #[cfg(feature = "text-renderer")]
    pub fn scroll_for_test(&mut self, scroll: f32) {
        self.scroll = scroll.max(0.0);
    }

    /// Shows a prepared frame without a shell, so a benchmark can time the pane's own view.
    pub fn show_for_test(&mut self, frame: Frame) {
        self.open = true;
        self.focus = true;
        self.frame = frame;
        self.cell = 8.0;
        self.line = 16.0;
        #[cfg(any(feature = "text-renderer", target_arch = "wasm32"))]
        {
            self.tracking = 0.0;
        }
        // A prepared frame comes with prepared geometry; the live pane measures the font.
        self.fixed = true;
        self.rect = [0.0, 0.0, 1160.0, 224.0];
    }

    /// What the pane is behind by: bytes queued, the oldest one's wait, and bytes parsed.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn stats(&self) -> Option<crate::engine::Stats> {
        self.engine.as_ref().map(|engine| engine.stats())
    }

    /// Gives the keyboard to the shell, or takes it away, telling a program that asked for
    /// focus reports (mode 1004) either way.
    pub fn set_focus(&mut self, focus: bool) {
        if self.focus == focus {
            return;
        }
        self.focus = focus;
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut() {
            engine.report_focus(focus);
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn start(&mut self, dir: PathBuf, wake: Option<&Waker>, theme: Theme) {
        let Some(wake) = wake else {
            self.error = Some("The window is not ready for a terminal yet.".to_owned());
            return;
        };
        let (cols, rows) = self.grid_size();
        match Engine::spawn(
            &dir,
            cols,
            rows,
            (self.cell, self.line),
            self.scale,
            theme,
            wake.clone(),
        ) {
            Ok(engine) => {
                self.engine = Some(engine);
                self.error = None;
                self.notice = None;
                self.fresh = true;
            }
            Err(err) => self.error = Some(err),
        }
    }
}
