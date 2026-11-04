//! The selection: where the gesture started, how many clicks deep it is, and the text Ghostty's
//! own formatter hands the clipboard. The anchor and the click count live with the selection so
//! a drag reads the same gesture the press started.

use ghostty_vt::selection::{
    FormatOptions, SelectLineOptions, SelectWordBetweenOptions, SelectWordOptions, Selection,
};
use ghostty_vt::terminal::{Point, PointCoordinate};

use super::Engine;

impl Engine {
    pub fn select_to(&mut self, at: (u16, u16)) -> bool {
        let Some(anchor) = self.anchor else {
            return false;
        };
        // A single click's own cell is not something its gesture selects: the wash starts at the
        // first other cell the pointer reaches, so a click that never leaves the cell it started
        // on selects nothing, and a drag back onto it washes nothing again.
        if self.clicks <= 1 && at == anchor {
            return self.vt.set_selection(None).is_ok();
        }
        let point = |(x, y): (u16, u16)| Point::Viewport(PointCoordinate { x, y: y as u32 });
        let (Ok(start), Ok(end)) = (self.vt.grid_ref(point(anchor)), self.vt.grid_ref(point(at)))
        else {
            return false;
        };
        let selection = match self.clicks {
            // After a double click the drag walks word to word: the pointer's word comes from
            // Ghostty's nearest-word helper, so a pointer in the gap between two words still
            // has one, and the anchor's word bounds the other end.
            2 => {
                let anchor_word = self
                    .vt
                    .select_word(SelectWordOptions::new(start.clone()))
                    .ok()
                    .flatten();
                let pointer_word = self
                    .vt
                    .select_word_between(SelectWordBetweenOptions::new(end.clone(), start.clone()))
                    .ok()
                    .flatten()
                    .or_else(|| {
                        self.vt
                            .select_word_between(SelectWordBetweenOptions::new(
                                start.clone(),
                                end.clone(),
                            ))
                            .ok()
                            .flatten()
                    });
                match (anchor_word, pointer_word) {
                    (Some(anchor_word), Some(pointer_word)) => {
                        let before = (at.1, at.0) < (anchor.1, anchor.0);
                        if before {
                            Selection::new(pointer_word.start(), anchor_word.end(), false)
                        } else {
                            Selection::new(anchor_word.start(), pointer_word.end(), false)
                        }
                    }
                    _ => Selection::new(start, end, false),
                }
            }
            // And after a triple click it covers whole rows, from the anchor's row through the
            // pointer's, however far the drag went.
            3.. => {
                let (first, last) = if anchor.1 <= at.1 {
                    (anchor.1, at.1)
                } else {
                    (at.1, anchor.1)
                };
                let tail = self.cols.saturating_sub(1);
                let (Ok(top), Ok(bottom)) = (
                    self.vt.grid_ref(point((0, first))),
                    self.vt.grid_ref(point((tail, last))),
                ) else {
                    return false;
                };
                Selection::new(top, bottom, false)
            }
            _ => Selection::new(start, end, self.block),
        };
        self.vt.set_selection(Some(&selection)).is_ok()
    }

    /// Starts a selection at a viewport point. A double click takes the word under it and a
    /// triple click the whole line, which are the two gestures Ghostty's selection reads. A
    /// single press only marks where a drag would start: the cell under the pointer is not a
    /// selection of its own, so the wash waits for the pointer to reach another cell and a plain
    /// click selects nothing. It says whether the grid changed, which a press that dismissed a
    /// selection does even though it selects nothing.
    pub fn begin_selection(&mut self, at: (u16, u16), clicks: u8, block: bool) -> bool {
        self.anchor = Some(at);
        self.clicks = clicks;
        self.block = block && clicks == 1;
        if clicks <= 1 {
            // A new gesture starts clean: whatever the last one selected is gone, and this click
            // has not selected anything yet.
            let dismissed = self.has_selection();
            let _ = self.vt.set_selection(None);
            return dismissed;
        }
        let point = Point::Viewport(PointCoordinate {
            x: at.0,
            y: at.1 as u32,
        });
        let Ok(at_ref) = self.vt.grid_ref(point) else {
            return false;
        };
        let derived = match clicks {
            2 => self
                .vt
                .select_word(SelectWordOptions::new(at_ref))
                .ok()
                .flatten(),
            _ => self
                .vt
                .select_line(SelectLineOptions::new(at_ref))
                .ok()
                .flatten(),
        };
        if let Some(selection) = derived {
            return self.vt.set_selection(Some(&selection)).is_ok();
        }
        let (Ok(start), Ok(end)) = (self.vt.grid_ref(point), self.vt.grid_ref(point)) else {
            return false;
        };
        let selection = Selection::new(start, end, false);
        self.vt.set_selection(Some(&selection)).is_ok()
    }

    /// Selects everything the terminal holds, screen and history both, so a copy is one gesture
    /// instead of a drag over a quarter million rows.
    pub fn select_all(&self) -> bool {
        match self.vt.select_all() {
            Ok(Some(selection)) => self.vt.set_selection(Some(&selection)).is_ok(),
            _ => false,
        }
    }

    pub fn clear_selection(&mut self) {
        self.anchor = None;
        self.clicks = 0;
        self.block = false;
        let _ = self.vt.set_selection(None);
    }

    /// Whether the grid holds a selection. A key that dismisses one asks this, so it never pays
    /// for formatting the text a select-all can make a quarter million rows long.
    pub fn has_selection(&self) -> bool {
        matches!(self.vt.selection(), Ok(Some(_)))
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
}
