//! Whether the pane shows, how tall it is, and what a drag through its minimum does. The grid,
//! the shell and the frame live beside this module; nothing here touches any of them.

use std::path::PathBuf;

use super::{COLLAPSE_DRAG, MAX_HEIGHT, MIN_HEIGHT, Terminal};
use crate::term::grid::Frame;

// The pane's state is built on both targets; only the native build has an engine behind it, so
// the web build's copy has fields and methods nothing reads.
#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
impl Terminal {
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
        self.set_focus(false);
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
        self.search = None;
    }

    /// Whether the shell behind the pane has exited, which closes the pane: a dead shell has no
    /// prompt to type at, and opening the pane again starts a fresh one.
    pub fn shell_exited(&self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = &self.engine {
            return engine.shell_exited();
        }
        false
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
        self.set_focus(true);
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
        self.set_focus(false);
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
}
