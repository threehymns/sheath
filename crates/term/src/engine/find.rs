//! Scrollback search: Ghostty's matcher over the screen and its history, the bounded steps that
//! keep a keystroke from waiting on a quarter million rows, and the rows of matches the pane
//! draws. The query and the field live with the pane; this only holds what the matcher found.

use std::time::{Duration, Instant};

use ghostty_vt::screen::GridRef;
use ghostty_vt::search::{Scroll, Search, Status};
use ghostty_vt::terminal::PointSpace;

use super::Engine;
use crate::grid::{Frame, Highlight};

/// How much of a frame one search step may take. The matcher keeps its progress, so a query
/// over the whole scrollback lands over a few frames instead of one long one.
pub const SEARCH_BUDGET: Duration = Duration::from_millis(4);

impl Engine {
    /// Sets the query and starts the search. An empty query ends it, which is what a cleared
    /// field asks for.
    pub fn search(&mut self, needle: &str) -> bool {
        if needle.is_empty() {
            self.end_search();
            return true;
        }
        if self.find.is_none() {
            match Search::new(&self.vt) {
                Ok(find) => self.find = Some(find),
                Err(_) => return false,
            }
        }
        {
            let Some(find) = self.find.as_mut() else {
                return false;
            };
            // The pane owns the scroll: it draws a fractional offset the terminal knows nothing
            // about, so a match is scrolled to from here once the search lands.
            let _ = find.set_select_scroll(Some(Scroll::None));
            if find.set_needle(&self.vt, Some(needle)).is_err() {
                return false;
            }
        }
        self.search_pending = true;
        self.force_full = true;
        self.search_step(SEARCH_BUDGET);
        true
    }

    /// Walks to the next match, toward older content, and shows it.
    pub fn search_next(&mut self) -> bool {
        let moved = match self.find.as_mut() {
            Some(find) => find.select_next(&self.vt).unwrap_or(false),
            None => false,
        };
        if moved {
            // A match already on screen has nowhere to scroll, which is not a failure to move.
            let _ = self.scroll_to_match();
            self.force_full = true;
        }
        moved
    }

    /// Walks to the previous match, toward newer content, and shows it.
    pub fn search_prev(&mut self) -> bool {
        let moved = match self.find.as_mut() {
            Some(find) => find.select_prev(&self.vt).unwrap_or(false),
            None => false,
        };
        if moved {
            let _ = self.scroll_to_match();
            self.force_full = true;
        }
        moved
    }

    /// Whether a search is running or has run since the query was set.
    pub fn searching(&self) -> bool {
        self.find.is_some()
    }

    /// How many matches there are and which one is selected, newest first, for the field's
    /// "3/12". The count grows while a long search is still stepping.
    pub fn search_count(&self) -> Option<(usize, Option<usize>)> {
        let find = self.find.as_ref()?;
        Some((
            find.total_matches().ok()?,
            find.selected_index().ok().flatten(),
        ))
    }

    /// Ends the search: the field, the matches and the highlight go.
    pub fn end_search(&mut self) {
        if self.find.take().is_some() {
            self.search_pending = false;
            self.force_full = true;
        }
    }

    /// Moves the search forward by at most `budget`, so a big scrollback is searched over
    /// several frames. A search with work left asks for another frame.
    pub fn search_step(&mut self, budget: Duration) {
        let until = Instant::now() + budget;
        let mut complete = false;
        {
            let Some(find) = self.find.as_mut() else {
                self.search_pending = false;
                return;
            };
            loop {
                match find.tick() {
                    Ok(Status::Running) => {
                        if Instant::now() >= until {
                            break;
                        }
                    }
                    Ok(Status::FeedRequired) => {
                        if find.feed(&self.vt).is_err() {
                            complete = true;
                            break;
                        }
                    }
                    Ok(Status::Complete) | Err(_) => {
                        complete = true;
                        break;
                    }
                }
            }
            // A fresh query starts on the newest match, the way a terminal's search does.
            if complete
                && find.selected_index().ok().flatten().is_none()
                && let Ok(selected) = find.select_next(&self.vt)
                && selected
            {
                self.force_full = true;
            }
        }
        if complete {
            self.search_pending = false;
            self.force_full = true;
            self.scroll_to_match();
            self.history = self.vt.scrollback_rows().unwrap_or(self.history);
        } else {
            (self.wake)();
        }
    }

    /// Scrolls the viewport so the selected match is visible, placing it at the top of the
    /// viewport the way Ghostty's search does.
    fn scroll_to_match(&mut self) -> bool {
        let back = {
            let Some(find) = self.find.as_ref() else {
                return false;
            };
            let Ok(Some(selection)) = find.selected_match(&self.vt) else {
                return false;
            };
            let start = selection.start();
            let Ok(Some(point)) = self.vt.point_from_grid_ref(&start, PointSpace::Screen) else {
                return false;
            };
            let total = self.vt.total_rows().unwrap_or(0);
            total.saturating_sub(1).saturating_sub(point.y as usize)
        };
        self.set_scroll(back)
    }
}

/// Writes the viewport's matches into the rows the pane draws. A match that wraps covers the
/// rest of its first row, every row between, and the start of its last, which is how a wrapped
/// line's match reads as one selection.
pub fn fill_matches(engine: &Engine, out: &mut Frame) {
    for line in &mut out.lines {
        line.matches.clear();
    }
    let Some(find) = engine.find.as_ref() else {
        return;
    };
    let Ok(matches) = find.viewport_matches(&engine.vt) else {
        return;
    };
    if matches.is_empty() {
        return;
    }
    let total = engine.vt.total_rows().unwrap_or(0);
    let top = total
        .saturating_sub(usize::from(out.rows))
        .saturating_sub(engine.applied);
    let current = find
        .selected_match(&engine.vt)
        .ok()
        .flatten()
        .and_then(|selection| screen_of(engine, &selection.start()));
    let cols = out.cols.saturating_sub(1);
    for selection in matches {
        let (Some(start), Some(end)) = (
            screen_of(engine, &selection.start()),
            screen_of(engine, &selection.end()),
        ) else {
            continue;
        };
        let on = current == Some(start);
        let (first, last) = if (start.0, start.1) <= (end.0, end.1) {
            (start, end)
        } else {
            (end, start)
        };
        if first.0 == last.0 {
            push(out, top, first.0, first.1, last.1, on);
            continue;
        }
        push(out, top, first.0, first.1, cols, on);
        for row in first.0 + 1..last.0 {
            push(out, top, row, 0, cols, on);
        }
        push(out, top, last.0, 0, last.1, on);
    }
}

fn screen_of(engine: &Engine, at: &GridRef<'_>) -> Option<(u32, u16)> {
    let point = engine
        .vt
        .point_from_grid_ref(at, PointSpace::Screen)
        .ok()
        .flatten()?;
    Some((point.y, point.x))
}

fn push(out: &mut Frame, top: usize, row: u32, start: u16, end: u16, current: bool) {
    let Some(index) = (row as usize).checked_sub(top) else {
        return;
    };
    let Some(line) = out.lines.get_mut(index) else {
        return;
    };
    if start > end {
        return;
    }
    line.matches.push(Highlight {
        cols: (start, end),
        current,
    });
}
