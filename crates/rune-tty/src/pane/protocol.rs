//! The shell's own reports, as the app sees them: the clipboard it filled (OSC 52), the
//! progress it published (OSC 9;4), the notifications it sent (OSC 9, OSC 777) and the
//! directory it named (OSC 7). The pane hands them on; the engine is where they are parsed.

use super::Terminal;

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
impl Terminal {
    /// Progress the shell reported for its own work (OSC 9;4), for the header to draw.
    pub fn progress(&self) -> Option<crate::Progress> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = &self.engine {
            return engine.progress();
        }
        None
    }

    /// The directory the shell reported (OSC 7), for the header to fall back to when the
    /// program sets no title.
    pub fn pwd(&self) -> Option<std::path::PathBuf> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = &self.engine {
            return engine.pwd().map(std::path::Path::to_path_buf);
        }
        None
    }

    /// Notifications the shell addressed to the desktop (OSC 9, OSC 777), handed to the app's
    /// toasts on the next poll.
    pub fn take_notifications(&mut self) -> Vec<(String, String)> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut() {
            return engine.take_notifications();
        }
        Vec::new()
    }

    /// The hyperlink on the cell at a viewport point (OSC 8), which a Cmd/Ctrl+click opens.
    pub fn hyperlink_at(&self, at: (u16, u16)) -> Option<String> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = &self.engine {
            return engine.hyperlink_at(at);
        }
        let _ = at;
        None
    }

    /// A copy the program asked for (OSC 52), handed to the clipboard on the next poll.
    pub fn take_copies(&mut self) -> Vec<String> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut() {
            self.copies.extend(engine.take_clipboard());
        }
        std::mem::take(&mut self.copies)
    }
}
