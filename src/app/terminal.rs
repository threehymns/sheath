use crate::app::App;
use rune::Tx;

impl App {
    /// Shows the pane, or collapses it. Opening starts a shell in the folder on screen the first
    /// time; after that the shell keeps its session, so collapsing and opening again is the same
    /// session.
    pub fn toggle_terminal(&mut self) {
        self.term.toggle(self.cwd.clone());
    }

    pub fn terminal_copy(&mut self, tx: &mut Tx) {
        if let Some(text) = self.term.copy_selection()
            && !text.trim().is_empty()
        {
            tx.copy(text);
        }
    }

    pub fn terminal_paste(&mut self) {
        // A browser tab has no clipboard to read; the web build never opens a shell anyway.
        #[cfg(target_arch = "wasm32")]
        let _ = &self.term;
        #[cfg(not(target_arch = "wasm32"))]
        if let Ok(mut clipboard) = arboard::Clipboard::new()
            && let Ok(text) = clipboard.get_text()
        {
            self.term.paste(&text);
        }
    }

    /// Drains the PTY and rebuilds the frame when it changed. Runs in the frame's poll, so the
    /// waker the reader thread holds only has to ask for the next frame.
    pub(super) fn poll_terminal(&mut self, tx: &mut Tx) -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            let _ = tx;
            false
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let dark = self.dark.get();
            let theme = crate::term::theme(dark);
            // The grid is sized in the font's own cell, so the shell is told the columns and rows
            // the rows are actually drawn at.
            self.term
                .measure(rune_theme_opencode::kit(dark).ty.mono, self.scale.get());
            let redraw = self.term.poll(self.wake.as_ref(), theme);
            for text in self.term.take_copies() {
                tx.copy(text);
            }
            for (title, body) in self.term.take_notifications() {
                self.raise_terminal_notice(title, body);
            }
            redraw
        }
    }

    /// A notification the shell sent with OSC 9 or OSC 777 becomes a toast, which is the app's
    /// one place for what a program wants to say.
    fn raise_terminal_notice(&mut self, title: String, body: String) {
        let title = if title.trim().is_empty() {
            "Terminal".to_owned()
        } else {
            title
        };
        let detail = if body.trim().is_empty() {
            "The shell sent a notification.".to_owned()
        } else {
            body
        };
        let report = format!("{title}\n{detail}");
        let notice = crate::notice::Notice::new("terminal", title, detail, report);
        // One toast per shell: a program that notifies repeatedly replaces its own, instead of
        // stacking a column of them.
        self.notices.retain(|up| up.file != "terminal");
        self.notices.push(notice);
    }

    pub fn remember_terminal_height(&mut self) {
        self.session.remember_pane(self.term.height());
    }
}
