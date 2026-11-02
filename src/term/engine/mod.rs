//! The native half of the pane: a shell on a PTY, libghostty-vt holding the screen, and the
//! effects that answer the program. Everything here runs on the app thread except the reader,
//! which only copies bytes into a queue and wakes the window.
//!
//! The modules beside this one divide by whose side of the pipe they are on: `pty` runs the shell
//! and keeps its bytes coming, `screen` reads libghostty's render state into the lines the view
//! draws, and `input` encodes keys, mouse and paste into what the program reads.

mod input;
mod link;
mod parse;
mod pty;
mod screen;

use std::cell::Cell;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use pty::Incoming;

use ghostty_vt::key::{Encoder as KeyEncoder, Event as EncoderEvent};
use ghostty_vt::mouse;
use ghostty_vt::selection::{FormatOptions, Selection};
use ghostty_vt::terminal::{Mode, Point, PointCoordinate, ScrollViewport};
use ghostty_vt::{RenderState, Terminal as Vt, TerminalOptions};
use portable_pty::{MasterPty, PtySize};
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
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
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

        let bell = Rc::new(Cell::new(false));
        let session = pty::spawn(dir, cols, rows, cell, wake.clone())?;

        let history = vt.scrollback_rows().unwrap_or(0);
        let tracking = vt.is_mouse_tracking().unwrap_or(false);
        Ok(Engine {
            vt,
            render: RenderState::new().map_err(|err| err.to_string())?,
            keys: KeyEncoder::new().map_err(|err| err.to_string())?,
            mouse: mouse::Encoder::new().map_err(|err| err.to_string())?,
            master: session.master,
            writer: session.writer,
            child: session.child,
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
            clipboard: Vec::new(),
            progress: None,
            notes: Vec::new(),
            hold: None,
            pwd: None,
        })
    }

    /// Parses what the reader brought, sends the replies the program asked for, and says whether
    /// anything the pane draws changed.
    pub fn drain(&mut self) -> bool {
        self.pending.store(false, Ordering::Release);
        let mut changed = false;
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
        if !replies.is_empty() {
            let _ = self.writer.write_all(&replies);
        }
        let _ = self.writer.flush();
        if let Ok(title) = self.vt.title()
            && title != self.title
        {
            self.title = title.to_owned();
            changed = true;
        }
        if !self.exited
            && let Ok(Some(_)) = self.child.try_wait()
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
    pub fn kill(&mut self) {
        pty::close(&self.incoming);
        let _ = self.child.kill();
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

    /// The directory the running program reported (OSC 7), if it reported one.
    pub fn pwd(&self) -> Option<&Path> {
        self.pwd.as_deref()
    }

    /// Writes VT bytes straight into the parser, without the PTY. Tests use it to put an exact
    /// screen in front of the render path.
    #[cfg(test)]
    pub fn feed(&mut self, bytes: &[u8]) {
        self.vt.vt_write(bytes);
        for _ in self.vt.events() {}
    }

    pub fn write(&mut self, bytes: &[u8]) {
        let _ = self.writer.write_all(bytes);
        let _ = self.writer.flush();
    }

    /// The rows just above and just below the viewport, fetched when a fractional scroll shows
    /// part of a row at an edge: the viewport is moved one row each way, the edge row read, and
    /// put back.
    pub fn ensure_edges(&mut self, needed: bool) {
        if !needed {
            let had = self.lead.take().is_some() | self.trail.take().is_some();
            self.edges_at = None;
            self.force_full |= had;
            return;
        }
        if self.edges_at == Some(self.applied) {
            return;
        }
        self.edges_at = Some(self.applied);
        self.lead = None;
        self.trail = None;
        self.vt.scroll_viewport(ScrollViewport::Delta(-1));
        self.lead = self.read_edge(false);
        self.vt.scroll_viewport(ScrollViewport::Delta(1));
        // The row below the last line, which is only there when the viewport is scrolled back.
        if self.applied > 0 {
            self.vt.scroll_viewport(ScrollViewport::Delta(1));
            self.trail = self.read_edge(true);
            self.vt.scroll_viewport(ScrollViewport::Delta(-1));
        }
        // The render state was updated a row away from the viewport, so the next fill reads it
        // all rather than trusting the dirty rows it kept from the wrong offset.
        self.force_full = true;
    }

    pub fn resize(&mut self, cols: u16, rows: u16, cell: (f32, f32), scale: f32) -> bool {
        let cols = cols.max(2);
        let rows = rows.max(1);
        let scale = if scale > 0.0 { scale } else { 1.0 };
        let metrics = Metrics {
            cell: cell.0.max(1.0),
            line: cell.1.max(1.0),
            scale,
        };
        if cols == self.cols && rows == self.rows && self.metrics == metrics {
            return false;
        }
        self.cols = cols;
        self.rows = rows;
        self.cell = (metrics.cell as u32, metrics.line as u32);
        // A cell that moved by less than a point, or a display at another scale, changes the
        // shapes even though the grid's size is the same, so every row is read again.
        self.metrics = metrics;
        let _ = self.vt.resize(cols, rows, self.cell.0, self.cell.1);
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: (cols as f32 * cell.0) as u16,
            pixel_height: (rows as f32 * cell.1) as u16,
        });
        self.applied = 0;
        self.anchor = None;
        self.lead = None;
        self.trail = None;
        self.edges_at = None;
        self.force_full = true;
        self.keys_stale = true;
        self.history = self.vt.scrollback_rows().unwrap_or(0);
        true
    }

    /// Rows of history above the viewport.
    pub fn scrollback(&self) -> usize {
        self.history
    }

    /// Puts the viewport `back` rows above the bottom, leaving the fraction of a row to the
    /// pane's own offset.
    pub fn set_scroll(&mut self, back: usize) -> bool {
        let back = back.min(self.scrollback());
        if back == self.applied {
            return false;
        }
        let delta = self.applied as isize - back as isize;
        self.vt.scroll_viewport(ScrollViewport::Delta(delta));
        self.applied = back;
        self.edges_at = None;
        self.force_full = true;
        true
    }

    pub fn select_to(&mut self, at: (u16, u16)) -> bool {
        let Some(anchor) = self.anchor else {
            return false;
        };
        let point = |(x, y): (u16, u16)| Point::Viewport(PointCoordinate { x, y: y as u32 });
        let (Ok(start), Ok(end)) = (self.vt.grid_ref(point(anchor)), self.vt.grid_ref(point(at)))
        else {
            return false;
        };
        let selection = Selection::new(start, end, false);
        self.vt.set_selection(Some(&selection)).is_ok()
    }

    pub fn begin_selection(&mut self, at: (u16, u16)) -> bool {
        self.anchor = Some(at);
        let point = Point::Viewport(PointCoordinate {
            x: at.0,
            y: at.1 as u32,
        });
        let (Ok(start), Ok(end)) = (self.vt.grid_ref(point), self.vt.grid_ref(point)) else {
            return false;
        };
        let selection = Selection::new(start, end, false);
        self.vt.set_selection(Some(&selection)).is_ok()
    }

    pub fn clear_selection(&mut self) {
        self.anchor = None;
        let _ = self.vt.set_selection(None);
    }

    /// The selected text, with hard-wrapped rows joined and trailing blanks trimmed.
    pub fn selected(&self) -> Option<String> {
        let mut size = 4096;
        loop {
            let mut buf = vec![0u8; size];
            let options = FormatOptions::new().with_trim(true).with_unwrap(true);
            match self.vt.format_selection_buf(options, &mut buf) {
                Ok(Some(len)) => return String::from_utf8(buf[..len].to_vec()).ok(),
                Ok(None) => return None,
                Err(ghostty_vt::Error::OutOfSpace { required }) => {
                    if required <= size {
                        return None;
                    }
                    size = required;
                }
                Err(_) => return None,
            }
        }
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
pub(crate) use parse::{directory, progress_of};
