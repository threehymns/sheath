//! Presses, drags and drops in the file list: the row drag heading for a folder, and the
//! scrolling a drag held at an edge causes. The selection box is `marquee`.

use super::marquee::Marquee;
use crate::app::App;
use crate::places::Section;
use rune::{Drag, Mods, Phase, Tx};
use std::path::PathBuf;

/// How far the pointer travels before a press in the list becomes a selection box.
const DRAG_START: f32 = 3.0;

/// How far past a row's laid-out name a press still counts as the name: the end of a word
/// should not be a pixel hunt.
const NAME_HANDLE_SLOP: f32 = 6.0;

/// How wide a row's name handle is at least, so a short name is still a target to grab.
const NAME_HANDLE_MIN: f32 = 88.0;

/// How far inside the list's top or bottom edge, in points, a dragged row has to be for the
/// rows to scroll under it.
const DRAG_SCROLL_EDGE: f32 = 44.0;

/// How fast the rows move while a dragged row is held at the very edge, in points per second:
/// pushing the pointer further past the edge scrolls no faster.
const DRAG_SCROLL_MAX: f32 = 1100.0;

/// A drag of the selected rows over a folder they can be dropped on, in window points.
pub struct FileDrag {
    pub start: (f32, f32),
    pub pointer: (f32, f32),
    /// The row the press landed on.
    press: String,
    /// The row pressed inside a selection of several: a release without a drag narrows the
    /// selection to it, as a click does.
    pending: Option<String>,
    /// A second press: a release without a drag opens the row, as a double click does.
    pending_open: bool,
    /// False until the pointer has travelled far enough to be a drag rather than a click.
    pub dragging: bool,
    /// The folder row under the pointer, if it can take the drop.
    pub row: Option<usize>,
    /// The sidebar place under the pointer, if it can take the drop.
    pub place: Option<PathBuf>,
    pub place_id: Option<String>,
    /// Where the rows would be inserted among the places rather than moved: the section and
    /// the index there. Set between rows, on a row's edge, or on a section's heading.
    pub insert: Option<(Section, usize)>,
}

/// The list's pointer gesture: a press selects as a click does, a drag draws a selection box,
/// and a second press opens the row.
pub fn list_drag(s: &mut App, drag: &mut Drag, tx: &mut Tx) {
    // Rune hands the list the pointer in its own points; the app works in window points.
    let at = s.window_point((drag.x, drag.y));
    match drag.phase {
        Phase::Down => {
            let mods = tx.mods();
            s.begin_press(at, mods, tx);
            // A modifier-click keeps its selection meaning: only a plain second click opens,
            // and only once the button comes up — a drag out of it moves the row or sweeps a
            // box instead.
            if drag.clicks >= 2 && !(mods.cmd || mods.ctrl || mods.shift) {
                if let Some(file_drag) = &mut s.file_drag {
                    file_drag.pending_open = true;
                } else {
                    // The press landed on a row's blank space, where a drag sweeps a box.
                    // Opening is a row-wide gesture, so a second click still opens the row.
                    let id = s.index_at_window(at).and_then(|index| s.row_id(index));
                    if let Some(marquee) = &mut s.marquee {
                        marquee.pending_open = id;
                    }
                }
            }
        }
        Phase::Move => s.drag_to(at),
        Phase::Up => s.end_drag(drag.cancelled()),
    }
}

impl App {
    /// Turns a point in the list viewport's own points into window points.
    pub fn window_point(&self, local: (f32, f32)) -> (f32, f32) {
        (self.rows_view[0] + local.0, self.rows_view[1] + local.1)
    }

    /// Whether a window point lies on row `index`'s icon or name, the only part of a row a
    /// drag picks up. The handle ends a little past where the name was laid out and is at
    /// least [`NAME_HANDLE_MIN`] wide, instead of being the whole, much wider, name column:
    /// the blank space beside a short name sweeps a box rather than moving the file. A row
    /// that has not reported its name yet — it was drawn in the last frame — keeps its whole
    /// cell, so a press meant for the name still picks the row up.
    fn on_name_cell(&self, index: usize, at: (f32, f32)) -> bool {
        let Some(&visible) = self.visible.get(index) else {
            return false;
        };
        let entry = &self.entries[visible];
        // The name column takes what the fixed columns leave.
        let cell_right = self.rows_view[0] + self.rows_view[2] - self.size_width - self.date_width;
        let widest = (self.rows_view[0] + NAME_HANDLE_MIN).min(cell_right);
        match self.name_at.get(&entry.path.display().to_string()) {
            Some(&right) => at.0 <= (right + NAME_HANDLE_SLOP).max(widest).min(cell_right),
            None => at.0 <= cell_right,
        }
    }

    /// The tile or row under a point in window points inside the list.
    pub fn index_at_window(&self, at: (f32, f32)) -> Option<usize> {
        // The bounce draws the rows where the scroll doesn't, so the pointer follows them.
        self.index_at((
            at.0 - self.rows_view[0],
            at.1 - self.rows_view[1] + self.told_offset() - self.overshoot(),
        ))
    }

    /// A press in the list: a press on a row selects it (or keeps a selection it belongs to, so
    /// the selection can be dragged) and a press on the empty space starts a selection box. With
    /// Option (Alt) held the box starts anywhere, over the rows themselves. An unselected row is
    /// picked up only by its icon and name, so a drag from the space past the name, or from the
    /// size or date columns, sweeps a box and reading a row never moves the file. A row that is
    /// already selected is picked up wherever the press lands on it. Points are the window's.
    pub fn begin_press(&mut self, at: (f32, f32), mods: Mods, tx: &mut Tx) {
        // A box over the rows is the one thing they can't get otherwise: the list only has empty
        // space to start one in when it doesn't scroll, so Option is how a long list gets swept.
        if mods.alt && self.index_at_window(at).is_some() {
            self.file_drag = None;
            self.marquee = Some(Marquee::begin(
                at,
                self.rows_at(),
                self.selection.clone(),
                mods.cmd || mods.ctrl,
                mods.shift,
            ));
            return;
        }
        let Some(index) = self.index_at_window(at) else {
            self.file_drag = None;
            self.marquee = Some(Marquee::begin(
                at,
                self.rows_at(),
                self.selection.clone(),
                mods.cmd || mods.ctrl,
                mods.shift,
            ));
            // A press on the empty space below the rows clears them, unless it adds.
            if !mods.shift && !(mods.cmd || mods.ctrl) {
                self.clear_selection();
            }
            return;
        };
        // A row that is already selected is picked up wherever the press lands on it: a drag
        // there means the selection, not a box. An unselected row is grabbed only by its icon
        // and name, so the wide space beside a short name still sweeps.
        let selected = self
            .row_id(index)
            .is_some_and(|id| self.selection.contains(&id));
        if !self.is_grid() && !selected && !self.on_name_cell(index, at) {
            // The press still reads as a click on the row; the drag that follows sweeps a
            // box from the selection the press found.
            let base = self.selection.clone();
            let rows_at = self.rows_at();
            self.file_drag = None;
            self.click_row(index, mods, tx);
            self.marquee = Some(Marquee::begin(
                at,
                rows_at,
                base,
                mods.cmd || mods.ctrl,
                mods.shift,
            ));
            return;
        }
        let plain = !mods.shift && !(mods.cmd || mods.ctrl);
        let inside = self
            .row_id(index)
            .is_some_and(|id| self.selection.len() > 1 && self.selection.contains(&id));
        // Pressing a row inside a selection of several keeps the selection for the drag; a
        // release without a drag narrows it to that row.
        let pending = inside && plain;
        self.marquee = None;
        self.file_drag = self.row_id(index).map(|id| FileDrag {
            start: at,
            pointer: at,
            press: id.clone(),
            pending: pending.then_some(id),
            pending_open: false,
            dragging: false,
            row: None,
            place: None,
            place_id: None,
            insert: None,
        });
        if !pending {
            self.click_row(index, mods, tx);
        }
    }

    /// The pointer moved with the button down: a row drag heads for a folder, a press on empty
    /// space grows a selection box.
    pub fn drag_to(&mut self, at: (f32, f32)) {
        if let Some(mut drag) = self.file_drag.take() {
            drag.pointer = at;
            let travel = (at.0 - drag.start.0).abs().max((at.1 - drag.start.1).abs());
            if travel < DRAG_START {
                // A pointer that comes back to where it started releases the press as the
                // click it began as, rather than dropping on whatever is under it. The move
                // never happened, so nothing can land in the wrong folder.
                if drag.dragging {
                    drag.dragging = false;
                    drag.row = None;
                    drag.place = None;
                    drag.place_id = None;
                    drag.insert = None;
                }
                self.file_drag = Some(drag);
                return;
            }
            drag.dragging = true;
            self.aim_drag(&mut drag);
            self.file_drag = Some(drag);
            return;
        }
        let Some(mut marquee) = self.marquee.take() else {
            return;
        };
        marquee.current = at;
        let travel = (at.0 - marquee.start.0)
            .abs()
            .max((at.1 - marquee.start.1).abs());
        if !marquee.dragging && travel < DRAG_START {
            self.marquee = Some(marquee);
            return;
        }
        marquee.dragging = true;
        self.sweep_box(&marquee);
        self.marquee = Some(marquee);
    }

    /// The button came up: drop a dragged selection on its target, narrow a click inside a
    /// selection to its row, or put the old selection back when the box was cancelled.
    pub fn end_drag(&mut self, cancelled: bool) {
        if let Some(drag) = self.file_drag.take() {
            if cancelled {
                return;
            }
            if drag.dragging {
                // A section takes the rows as new places; a place row or a folder row takes
                // them as files to move.
                if let Some((section, index)) = drag.insert {
                    let ids = self.selected_ids();
                    self.add_places(section, index, &ids);
                } else if let Some(destination) = self.drop_target(&drag) {
                    self.move_selection_into(&destination);
                }
            } else if drag.pending_open {
                self.open_id(&drag.press);
            } else if let Some(id) = drag.pending {
                self.select_id(&id);
            }
            return;
        }
        let Some(marquee) = self.marquee.take() else {
            return;
        };
        if !cancelled {
            // A press on a row's blank space clears and sweeps a box; a second one still
            // opens the row, as a double click on it does.
            if !marquee.dragging
                && let Some(id) = marquee.pending_open
            {
                self.open_id(&id);
            }
            return;
        }
        self.selection = marquee.base;
        match self
            .selected
            .clone()
            .filter(|id| self.selection.contains(id))
        {
            Some(id) => self.focus_id(&id),
            None => match self.selected_ids().first().cloned() {
                Some(id) => self.focus_id(&id),
                None => {
                    self.selected = None;
                    self.preview = None;
                }
            },
        }
    }
    /// How far a drag held against the list's top or bottom edge pushes the rows this frame,
    /// faster the further past the edge the pointer is. Zero when no drag is against an edge.
    pub(super) fn drag_push(&self, dt: f32) -> f32 {
        // A row drag heads for a folder and a box sweeps rows: both follow their pointer.
        let pointer = match (&self.file_drag, &self.marquee) {
            (Some(drag), _) if drag.dragging => Some(drag.pointer),
            (_, Some(marquee)) if marquee.dragging => Some(marquee.current),
            _ => None,
        };
        let Some((x, y)) = pointer else {
            return 0.0;
        };
        let view = self.rows_view;
        // The sidebar sits left of the list: a drag over a place there scrolls nothing.
        let depth = if x < view[0] || view[3] <= 0.0 {
            0.0
        } else {
            // Up at the top edge, down at the bottom one, and past either kept at full speed.
            let above = (view[1] + DRAG_SCROLL_EDGE - y) / DRAG_SCROLL_EDGE;
            let below = (y - (view[1] + view[3] - DRAG_SCROLL_EDGE)) / DRAG_SCROLL_EDGE;
            if above > 0.0 {
                -above
            } else if below > 0.0 {
                below
            } else {
                0.0
            }
        };
        depth.clamp(-1.0, 1.0) * DRAG_SCROLL_MAX * dt
    }

    /// Applies a dragging pointer to the rows where the frame has just put them: the folder a
    /// row drag is over, or the rows a box now covers.
    pub(super) fn follow_drag(&mut self) {
        if let Some(mut drag) = self.file_drag.take() {
            self.aim_drag(&mut drag);
            self.file_drag = Some(drag);
        } else if let Some(marquee) = self.marquee.take() {
            // A press that has not travelled is a click, not a box. Sweeping it would cover the
            // cell its point sits in, so a click in the gap between tiles took the nearest one.
            if marquee.dragging {
                self.sweep_box(&marquee);
            }
            self.marquee = Some(marquee);
        }
    }
}
