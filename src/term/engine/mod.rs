//! The native half of the pane: a shell on a PTY, libghostty-vt holding the screen, and the
//! effects that answer the program. Everything here runs on the app thread except the reader,
//! which only copies bytes into a queue and wakes the window.
//!
//! The modules beside this one divide by whose side of the pipe they are on: `pty` runs the shell
//! and keeps its bytes coming, `screen` reads libghostty's render state into the lines the view
//! draws, and `input` encodes keys, mouse and paste into what the program reads.

mod find;
mod input;
mod link;
mod parse;
mod pty;
mod screen;
mod selection;
mod viewport;

use std::cell::Cell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use find::SEARCH_BUDGET;
use pty::Incoming;

use ghostty_vt::key::{Encoder as KeyEncoder, Event as EncoderEvent};
use ghostty_vt::mouse;
use ghostty_vt::terminal::{Event, Mode};
use ghostty_vt::{RenderState, Terminal as Vt, TerminalOptions};
use portable_pty::MasterPty;
use rune::Waker;

use crate::term::grid::Line;
use crate::term::shapes::Metrics;
use crate::term::{Progress, Theme};

/// How much history the terminal keeps above the viewport. libghostty measures this in bytes,
/// not rows, so this is roughly a quarter million lines of plain text.
const MAX_SCROLLBACK: usize = 32 * 1024 * 1024;

/// Bytes one parse step hands libghostty, so the frame's time budget can be checked between
/// steps instead of after everything queued has been parsed.
const PARSE_CHUNK: usize = 64 * 1024;

/// How long one frame may spend parsing. Bytes past this wait for the next frame, so a burst
/// costs more frames rather than longer ones: the screen stays at the display's rate and falls
/// behind the shell by at most [`pty::QUEUE_LIMIT`], instead of every frame growing with the
/// backlog and the two chasing each other down.
const PARSE_BUDGET: Duration = Duration::from_millis(6);

/// How long mode 2026 may hold the screen before the pane draws anyway, the way Ghostty caps a
/// synchronized-output hold. The terminal has no clock of its own, so a program that starts a
/// hold and never ends it would otherwise freeze the pane for good.
const HOLD_LIMIT: Duration = Duration::from_secs(1);

pub struct Engine {
    vt: Vt,
    render: RenderState,
    keys: KeyEncoder,
    mouse: mouse::Encoder,
    /// The shell's process handles. They are options because [`kill`](Engine::kill) hands them
    /// to a helper thread, leaving the engine without a process to talk to.
    master: Option<Box<dyn MasterPty + Send>>,
    writer: Option<Box<dyn Write + Send>>,
    child: Option<Box<dyn portable_pty::Child + Send + Sync>>,
    /// Bytes the reader thread has taken off the PTY and not yet parsed.
    incoming: Arc<Incoming>,
    /// Bytes parsed since the shell started, for the frame-rate and lag measurement.
    parsed: u64,
    /// Time in the parser and in the readback, so a frame can be split by phase.
    parse_time: Duration,
    readback_time: Duration,
    bell: Rc<Cell<bool>>,
    /// How many rows back from the bottom the viewport is drawn, as last told to libghostty.
    applied: usize,
    cols: u16,
    rows: u16,
    cell: (u32, u32),
    /// The cell box and display scale the shapes are computed in, so the cell-filling characters
    /// are drawn one device pixel thick at whatever size the pane is showing.
    metrics: Metrics,
    title: String,
    /// The shell has exited; its status is read once, so the drain stops asking for it.
    exited: bool,
    /// The cell the current selection started at, in viewport coordinates.
    anchor: Option<(u16, u16)>,
    /// How many presses in a row started the selection: a double click selects a word, a
    /// triple click its line, and a drag extends the same way.
    clicks: u8,
    /// Whether the selection is a block over the columns between its ends, which is what
    /// Alt+drag takes, rather than a run of characters.
    block: bool,
    /// The colours the terminal is drawing in, so a theme switch reaches the cells and the
    /// light/dark answer a program can query.
    theme: Theme,
    /// Reused between frames: the grid rebuild writes into these instead of allocating a string
    /// per cell.
    scratch: String,
    key_event: EncoderEvent,
    keybuf: Vec<u8>,
    /// The rows just above and just below the viewport, drawn outside the first and last lines
    /// while a fractional scroll shows part of a row at an edge. Without them the strips there
    /// are background that fills in all at once when the viewport steps, which reads as a jump.
    lead: Option<Line>,
    trail: Option<Line>,
    edges_at: Option<usize>,
    /// Set when the viewport, the selection or the size moved: every row is read again.
    force_full: bool,
    /// Cached libghostty answers, which are otherwise a call per frame, per drag or per key.
    history: usize,
    tracking: bool,
    keys_stale: bool,
    /// One wake per batch of output: the reader asks for a frame only when none is pending, and
    /// a drain that left bytes queued asks again.
    pending: Arc<AtomicBool>,
    wake: Waker,
    /// Events libghostty raised for the chunk being parsed, kept between chunks so the parse
    /// loop does not allocate a vector per chunk.
    events: Vec<Event>,
    /// Text the program put on the clipboard (OSC 52), waiting for the app to take it.
    clipboard: Vec<String>,
    /// Progress the running program reported (OSC 9;4), or `None` when it reports none.
    progress: Option<Progress>,
    /// Notifications the running program addressed to the desktop (OSC 9, OSC 777), waiting for
    /// the app to take them.
    notes: Vec<(String, String)>,
    /// When the running program started holding the screen for synchronized output (mode 2026),
    /// or `None` when it is not holding it.
    hold: Option<Instant>,
    /// The directory the running program reported (OSC 7), for the header to fall back to when
    /// the program sets no title.
    pwd: Option<PathBuf>,
    /// The search over the screen and its history, while the pane's search field is open.
    find: Option<ghostty_vt::search::Search>,
    /// Whether that search has more of the scrollback to walk through.
    search_pending: bool,
}

/// What the pane is doing behind the frame it just drew. `lag` is the honest latency number: a
/// frame showing a screen whose oldest byte has waited 20 seconds is twenty seconds stale, no
/// matter what frame rate the program reports about its own writes.
#[derive(Clone, Copy, Debug)]
pub struct Stats {
    /// Bytes read from the pty and not parsed yet.
    pub queued: usize,
    /// How long the oldest of those has waited.
    pub lag: Duration,
    /// Bytes parsed since the shell started.
    pub parsed: u64,
    /// Time spent in the parser.
    pub parse: Duration,
    /// Time spent reading the screen back out of libghostty into the frame.
    pub readback: Duration,
}

impl Drop for Engine {
    fn drop(&mut self) {
        pty::close(&self.incoming);
    }
}

impl Engine {
    pub fn stats(&self) -> Stats {
        Stats {
            queued: self.incoming.len(),
            lag: self.incoming.lag(),
            parsed: self.parsed,
            parse: self.parse_time,
            readback: self.readback_time,
        }
    }

    pub fn spawn(
        dir: &Path,
        cols: u16,
        rows: u16,
        cell: (f32, f32),
        scale: f32,
        theme: Theme,
        wake: Waker,
    ) -> Result<Engine, String> {
        let cols = cols.max(2);
        let rows = rows.max(1);
        let scale = if scale > 0.0 { scale } else { 1.0 };
        let mut vt = Vt::new(TerminalOptions {
            cols,
            rows,
            max_scrollback: MAX_SCROLLBACK,
        })
        .map_err(|err| err.to_string())?;
        let _ = vt
            .set_default_fg_color(Some(theme.fg))
            .and_then(|vt| vt.set_default_bg_color(Some(theme.bg)))
            .and_then(|vt| vt.set_default_cursor_color(Some(theme.cursor)));
        // Programs ask which side of the theme they are drawing on (CSI ? 996 n) and pick their
        // own colours from the answer, as they do in Ghostty.
        vt.set_color_scheme(Some(parse::scheme(theme)));

        let bell = Rc::new(Cell::new(false));
        let session = pty::spawn(dir, cols, rows, cell, wake.clone())?;

        let history = vt.scrollback_rows().unwrap_or(0);
        let tracking = vt.is_mouse_tracking().unwrap_or(false);
        Ok(Engine {
            vt,
            render: RenderState::new().map_err(|err| err.to_string())?,
            keys: KeyEncoder::new().map_err(|err| err.to_string())?,
            mouse: mouse::Encoder::new().map_err(|err| err.to_string())?,
            master: Some(session.master),
            writer: Some(session.writer),
            child: Some(session.child),
            incoming: session.incoming,
            parsed: 0,
            parse_time: Duration::ZERO,
            readback_time: Duration::ZERO,
            bell,
            applied: 0,
            cols,
            rows,
            cell: (cell.0.max(1.0) as u32, cell.1.max(1.0) as u32),
            metrics: Metrics {
                cell: cell.0.max(1.0),
                line: cell.1.max(1.0),
                scale,
            },
            title: String::new(),
            exited: false,
            anchor: None,
            clicks: 0,
            block: false,
            theme,
            scratch: String::new(),
            key_event: EncoderEvent::new().map_err(|err| err.to_string())?,
            keybuf: Vec::new(),
            lead: None,
            trail: None,
            edges_at: None,
            force_full: true,
            history,
            tracking,
            keys_stale: true,
            pending: session.pending,
            wake,
            events: Vec::new(),
            clipboard: Vec::new(),
            progress: None,
            notes: Vec::new(),
            hold: None,
            pwd: None,
            find: None,
            search_pending: false,
        })
    }

    /// Parses what the reader brought, sends the replies the program asked for, and says whether
    /// anything the pane draws changed.
    pub fn drain(&mut self) -> bool {
        self.pending.store(false, Ordering::Release);
        let mut changed = false;
        // A search that is still walking the scrollback takes another bounded step, and the
        // rows it has found so far are already in the frame.
        if self.search_pending {
            self.search_step(SEARCH_BUDGET);
            changed = true;
        }
        // A hold that ran past its limit is let go before anything else: a frame never drawn is
        // worse than one drawn mid-sequence, and the program has had its second.
        if let Some(started) = self.hold
            && started.elapsed() >= HOLD_LIMIT
        {
            self.hold = None;
            let _ = self.vt.set_mode(Mode::SYNC_OUTPUT, false);
            self.force_full = true;
            changed = true;
        }
        let at = Instant::now();
        let mut replies = Vec::new();
        changed |= self.parse(&mut replies, at + PARSE_BUDGET);
        if let Some(writer) = self.writer.as_mut() {
            if !replies.is_empty() {
                let _ = writer.write_all(&replies);
            }
            let _ = writer.flush();
        }
        if let Ok(title) = self.vt.title()
            && title != self.title
        {
            self.title = title.to_owned();
            changed = true;
        }
        if !self.exited
            && let Some(child) = self.child.as_mut()
            && let Ok(Some(_)) = child.try_wait()
        {
            self.exited = true;
            changed = true;
        }
        if changed {
            // The modes and the page list only move when bytes did.
            self.keys_stale = true;
            self.history = self.vt.scrollback_rows().unwrap_or(self.history);
            self.tracking = self.vt.is_mouse_tracking().unwrap_or(false);
        }
        // Bytes left over from the budget, and bytes that arrived while this drain ran, get the
        // next frame, not the one after. The wake is not recorded as pending: one sent from
        // inside a frame is not one the next frame is guaranteed to receive, and leaving the
        // flag set would swallow every wake the reader sends after it.
        if self.incoming.len() > 0 {
            self.pending.store(false, Ordering::Release);
            (self.wake)();
        }

        self.parse_time += at.elapsed();
        changed
    }

    /// Ends the shell behind the pane, for a close rather than a collapse that leaves it
    /// running. Dropping the PTY hangs the child up, but a kill is what stops a program that
    /// ignores that.
    ///
    /// The press does not wait for any of it: the PTY handles and the child go to a helper
    /// thread, which signals the shell and closes the descriptors there. Waiting for a process
    /// to go is what a close would otherwise block the press on.
    pub fn kill(&mut self) {
        pty::close(&self.incoming);
        let master = self.master.take();
        let writer = self.writer.take();
        let child = self.child.take();
        if master.is_none() && writer.is_none() && child.is_none() {
            return;
        }
        std::thread::spawn(move || {
            if let Some(mut child) = child {
                let _ = child.kill();
            }
            drop(writer);
            drop(master);
        });
    }

    pub fn take_bell(&self) -> bool {
        self.bell.replace(false)
    }

    /// Text the program put on the clipboard (OSC 52).
    pub fn take_clipboard(&mut self) -> Vec<String> {
        std::mem::take(&mut self.clipboard)
    }

    /// Progress the running program reported (OSC 9;4), or `None` once it reports none left.
    pub fn progress(&self) -> Option<Progress> {
        self.progress
    }

    /// Notifications the running program sent (OSC 9, OSC 777), oldest first.
    pub fn take_notifications(&mut self) -> Vec<(String, String)> {
        std::mem::take(&mut self.notes)
    }

    /// Whether the running program is holding the screen for synchronized output (mode 2026).
    /// While it is, the pane keeps parsing and leaves the last complete frame on screen.
    pub fn holding(&self) -> bool {
        self.hold.is_some()
    }

    /// Whether the shell behind the pane has exited, so the pane can close itself the way
    /// Ghostty does when its command ends.
    pub fn shell_exited(&self) -> bool {
        self.exited
    }

    /// The directory the running program reported (OSC 7), if it reported one.
    pub fn pwd(&self) -> Option<&Path> {
        self.pwd.as_deref()
    }

    /// Writes VT bytes straight into the parser, without the PTY, and applies what they raised
    /// the way the parse loop would. Tests use it to put an exact screen in front of the render
    /// path, and an exact clipboard, title or progress report in front of the app.
    #[cfg(test)]
    pub fn feed(&mut self, bytes: &[u8]) {
        self.vt.vt_write(bytes);
        let mut replies = Vec::new();
        self.absorb_all(&mut replies);
    }

    pub fn write(&mut self, bytes: &[u8]) {
        if let Some(writer) = self.writer.as_mut() {
            let _ = writer.write_all(bytes);
            let _ = writer.flush();
        }
    }

    /// Applies the theme the app is drawing in, for a switch between light and dark while the
    /// shell is running: the cells resolve against these defaults, and the light/dark answer
    /// programs query moves with them.
    pub fn set_theme(&mut self, theme: Theme) {
        if self.theme == theme {
            return;
        }
        self.theme = theme;
        let _ = self
            .vt
            .set_default_fg_color(Some(theme.fg))
            .and_then(|vt| vt.set_default_bg_color(Some(theme.bg)))
            .and_then(|vt| vt.set_default_cursor_color(Some(theme.cursor)));
        self.vt.set_color_scheme(Some(parse::scheme(theme)));
        self.force_full = true;
    }

    pub fn mouse_tracking(&self) -> bool {
        self.tracking
    }

    /// A fresh frame, for tests. The pane reuses one across frames instead.
    #[cfg(test)]
    pub fn frame(&mut self) -> crate::term::grid::Frame {
        let mut out = crate::term::grid::Frame::default();
        self.fill(&mut out);
        out
    }
}

// The report parsers live with the parse loop; the tests reach them through here.
#[cfg(test)]
pub(crate) use parse::{directory, progress_of, scheme, trim_trailing};
