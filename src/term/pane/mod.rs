//! The terminal pane's state: whether it shows, how tall it is, the last frame, and the
//! fractional scroll that makes the wheel smooth. The engine behind it is native only; the web
//! build keeps this type so the app compiles and the pane can say where the terminal lives.

mod protocol;

use std::path::PathBuf;

use rune::{KeyEvent, Waker};

use crate::term::Theme;
#[cfg(not(target_arch = "wasm32"))]
use crate::term::engine::Engine;
use crate::term::grid::Frame;

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
pub const LINE_HEIGHT: f32 = 16.5;

/// A row's place in the scroll: whole rows back from the bottom, and the fraction of a row the
/// content is shifted by. Keeping the fraction out of libghostty's viewport is what makes a
/// trackpad's small deltas move pixels without re-wrapping the screen.
pub fn split(scroll: f32) -> (usize, f32) {
    let scroll = scroll.max(0.0);
    let whole = scroll.floor();
    (whole as usize, scroll - whole)
}

pub struct Terminal {
    open: bool,
    height: f32,
    pub frame: Frame,
    scroll: f32,
    pub focus: bool,
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
    /// The press that opened an OSC 8 link: the rest of its gesture is the link's, not a
    /// selection's.
    pub link_press: bool,
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
            link_press: false,
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

    /// Reads the cell from the mono font itself, the way a terminal does: the advance is the
    /// cell's width and the font's own line box its height, both rounded to whole device pixels.
    /// The theme's line height is a paragraph measure, taller than the glyphs, and a fractional
    /// cell puts every glyph edge on a fractional pixel, which leaves a hairline wherever two
    /// cell-filling glyphs (a block, a box rule) meet.
    ///
    /// It runs on every poll, over the style the rows are drawn with, so changing the font (or a
    /// window moving to a display at another scale) takes effect without a restart.
    pub fn measure(&mut self, style: rune::TextStyle, scale: f32) {
        if self.fixed {
            return;
        }
        let natural = style.lh(0.0);
        let advance = rune::text::advance("M", natural);
        let (_, box_height) = rune::text::measure("M", &[], natural, None);
        if advance <= 1.0 || box_height <= 4.0 {
            return;
        }
        let scale = if scale > 0.0 { scale } else { 1.0 };
        self.scale = scale;
        self.cell = (advance * scale).round().max(1.0) / scale;
        self.line = (box_height * scale).round().max(1.0) / scale;
        // The shaper advances by the font's own width; tracking it to the pixel-aligned cell is
        // how the glyphs land on the same grid the pane draws and points at. Only the text
        // renderer shapes again, so the raster build does not carry it.
        #[cfg(any(feature = "text-renderer", target_arch = "wasm32"))]
        {
            self.tracking = self.cell - advance;
        }
    }

    /// The style the rows are drawn in: the font's own line box, tracked to the pixel-aligned
    /// cell so a glyph's advance and a cell are the same width.
    #[cfg(any(feature = "text-renderer", target_arch = "wasm32"))]
    pub fn text_style(&self, style: rune::TextStyle) -> rune::TextStyle {
        style.lh(self.line).tracking(self.tracking)
    }

    pub fn is_open(&self) -> bool {
        self.open
    }

    pub fn height(&self) -> f32 {
        self.height
    }

    pub fn set_height(&mut self, height: f32) {
        if height.is_finite() {
            self.height = height.round().clamp(MIN_HEIGHT, MAX_HEIGHT);
        }
    }

    /// Hides the pane and leaves the shell running, so collapsing and opening it again is the
    /// same session. [`close`](Terminal::close) is what ends the shell.
    pub fn collapse(&mut self) {
        self.open = false;
        self.focus = false;
        self.pending = None;
        self.clear_selection();
    }

    /// Hides the pane and ends its shell, so opening it again starts a fresh session in the
    /// folder on screen.
    pub fn close(&mut self) {
        self.collapse();
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(mut engine) = self.engine.take() {
            engine.kill();
        }
        self.frame = Frame::default();
        self.scroll = 0.0;
        self.fresh = true;
        self.copies.clear();
    }

    /// Shows the pane in `dir`, starting a shell there the first time and keeping the one it
    /// already has.
    pub fn open(&mut self, dir: PathBuf) {
        if !self.open {
            self.open = true;
            self.fresh = true;
            #[cfg(not(target_arch = "wasm32"))]
            if self.engine.is_none() {
                self.pending = Some(dir);
            }
            #[cfg(target_arch = "wasm32")]
            {
                let _ = dir;
                self.notice = Some(
                    "The terminal runs in Sheath's desktop app; the browser tab has no shell."
                        .to_owned(),
                );
            }
        }
        self.focus = true;
    }

    pub fn toggle(&mut self, dir: PathBuf) {
        if self.open {
            self.collapse();
        } else {
            self.open(dir);
        }
    }

    /// Shows the pane at `height`, for a drag up from the bottom edge: a collapsed pane brings
    /// its shell back, a closed one starts a fresh shell in `dir`, and neither takes the
    /// keyboard from the browser.
    pub fn expand(&mut self, dir: PathBuf, height: f32) {
        self.open(dir);
        self.opened = true;
        self.focus = false;
        self.set_height(height);
    }

    /// Starts a drag gesture: nothing has been shown yet, so releasing it while the pane is
    /// collapsed is a click. A drag that opens the pane marks itself with [`did_open`].
    pub fn begin_drag(&mut self) {
        self.opened = false;
    }

    /// Whether the drag in flight has shown the pane at least once.
    pub fn did_open(&self) -> bool {
        self.opened
    }

    /// Applies a drag's target height while the pointer moves. An open pane resizes to it and
    /// collapses once it is pushed well past the minimum; a collapsed one opens when the drag
    /// reaches the minimum. The gap between those two points is a dead band: a drag sitting in
    /// it keeps whatever the pane was, so the handle cannot flip the pane in and out as the
    /// pointer moves a point at a time.
    pub fn drag(&mut self, wanted: f32, dir: PathBuf) {
        if self.open {
            if wanted < MIN_HEIGHT - COLLAPSE_DRAG {
                self.collapse();
            } else {
                self.set_height(wanted);
            }
        } else if wanted >= MIN_HEIGHT {
            self.expand(dir, wanted);
        }
    }

    /// Reads the PTY, resizes the grid to the rect it was laid out in, and rebuilds the frame
    /// when anything moved.
    pub fn poll(&mut self, wake: Option<&Waker>, theme: Theme) -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = (wake, theme);
            false
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if !self.open {
                return false;
            }
            if let Some(dir) = self.pending.take() {
                self.start(dir, wake, theme);
            }
            let (cols, rows) = self.grid_size();
            let Some(engine) = self.engine.as_mut() else {
                return false;
            };
            let mut redraw = engine.drain();
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
            if redraw || self.fresh || split(self.scroll).1 > 0.0 {
                self.refresh();
            }
            if redraw {
                self.clamp_scroll();
            }
            redraw
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

    #[cfg(test)]
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
    #[cfg(all(test, feature = "text-renderer"))]
    pub fn scroll_for_test(&mut self, scroll: f32) {
        self.scroll = scroll.max(0.0);
    }

    /// Shows a prepared frame without a shell, so a benchmark can time the pane's own view.
    #[cfg(test)]
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

    /// The screen's rect, from its layout. The grid is sized from it, so a pane drag reflows
    /// the shell as it moves.
    pub fn layout(&mut self, rect: [f32; 4], cell: f32) {
        self.rect = rect;
        if cell > 1.0 {
            self.cell = cell;
        }
    }

    pub fn grid_size(&self) -> (u16, u16) {
        let cols = (self.rect[2] / self.cell).floor();
        let rows = (self.rect[3] / self.line).floor();
        (cols.max(2.0) as u16, rows.max(1.0) as u16)
    }

    /// What the pane is behind by: bytes queued, the oldest one's wait, and bytes parsed.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn stats(&self) -> Option<crate::term::engine::Stats> {
        self.engine.as_ref().map(|engine| engine.stats())
    }

    pub fn copy_selection(&mut self) -> Option<String> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut() {
            return engine.selected();
        }
        None
    }

    pub fn begin_selection(&mut self, at: (u16, u16)) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut() {
            return engine.begin_selection(at);
        }
        let _ = at;
        false
    }

    pub fn extend_selection(&mut self, at: (u16, u16)) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut() {
            return engine.select_to(at);
        }
        let _ = at;
        false
    }

    pub fn clear_selection(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut() {
            engine.clear_selection();
        }
    }

    pub fn mouse_tracking(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = &self.engine {
            return engine.mouse_tracking();
        }
        false
    }

    pub fn mouse(&mut self, action: MouseAction, at: (u16, u16), shift: bool) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut() {
            let (action, button) = match action {
                MouseAction::Press => (
                    ghostty_vt::mouse::Action::Press,
                    Some(ghostty_vt::mouse::Button::Left),
                ),
                MouseAction::Drag => (
                    ghostty_vt::mouse::Action::Motion,
                    Some(ghostty_vt::mouse::Button::Left),
                ),
                MouseAction::Release => (ghostty_vt::mouse::Action::Release, None),
            };
            let bytes = engine.mouse(action, button, at, shift);
            if !bytes.is_empty() {
                engine.write(&bytes);
                return true;
            }
        }
        let _ = (action, at, shift);
        false
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

/// Which mouse gesture the pane is reporting.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseAction {
    Press,
    Drag,
    Release,
}
