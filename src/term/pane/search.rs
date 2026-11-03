//! The pane's search field: the query, the match count and the two gestures that walk the
//! matches. The matcher itself is the engine's; this is only what the field shows and what it
//! asks the engine for.

use rune::TextEdit;

use super::Terminal;

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
impl Terminal {
    /// Opens the search field, or puts the keyboard back in it when it is already open.
    pub fn open_search(&mut self) {
        if let Some(edit) = self.search.as_mut() {
            edit.focused = true;
            return;
        }
        let mut edit = TextEdit::new("");
        edit.focused = true;
        self.search = Some(edit);
    }

    /// Closes the field and drops the matches, which is what Escape in it does.
    pub fn close_search(&mut self) {
        if self.search.take().is_none() {
            return;
        }
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut() {
            engine.end_search();
        }
        self.fresh = true;
    }

    /// Whether the search field is open.
    pub fn searching(&self) -> bool {
        self.search.is_some()
    }

    /// Whether the keyboard is in the field rather than in the shell.
    pub fn search_field_focused(&self) -> bool {
        self.search.as_ref().is_some_and(|edit| edit.focused)
    }

    pub fn search_text(&self) -> &TextEdit {
        self.search
            .as_ref()
            .expect("the field is read only while it is open")
    }

    pub fn search_text_mut(&mut self) -> &mut TextEdit {
        self.search
            .as_mut()
            .expect("the field is edited only while it is open")
    }

    /// Runs the query the field holds and moves to the newest match. An empty field ends the
    /// search but leaves the field open, so clearing it is one keystroke.
    pub fn apply_search(&mut self) -> bool {
        let needle = self
            .search
            .as_ref()
            .map(|edit| edit.text.trim().to_owned())
            .unwrap_or_default();
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut() {
            let ran = engine.search(&needle);
            // The search moves the terminal's viewport; the pane's own scroll follows it so the
            // wheel starts from the match that is on screen.
            self.scroll = engine.scroll_position() as f32;
            self.fresh = true;
            return ran;
        }
        let _ = needle;
        false
    }

    /// Walks to the next match and shows it.
    pub fn search_next(&mut self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut() {
            let moved = engine.search_next();
            self.scroll = engine.scroll_position() as f32;
            self.fresh = true;
            return moved;
        }
        false
    }

    /// Walks to the previous match and shows it.
    pub fn search_prev(&mut self) -> bool {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_mut() {
            let moved = engine.search_prev();
            self.scroll = engine.scroll_position() as f32;
            self.fresh = true;
            return moved;
        }
        false
    }

    /// How many matches the query has and which one is selected, for the field's "3/12".
    pub fn search_state(&self) -> Option<(usize, Option<usize>)> {
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(engine) = self.engine.as_ref() {
            return engine.search_count();
        }
        None
    }
}
