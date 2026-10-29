//! Presses, drags and drops in the file list: the selection box, the row drag heading for
//! a folder, and the scrolling a drag held at an edge causes.

use crate::app::App;
use rune::{Drag, Mods, Phase, Tx};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

/// How far the pointer travels before a press in the list becomes a selection box.
const DRAG_START: f32 = 3.0;

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
    pub place_id: Option<&'static str>,
}

/// A press in the file list and the box it has swept so far, in window points.
pub struct Marquee {
    pub start: (f32, f32),
    pub current: (f32, f32),
    /// The selection and anchor when the press landed, which Shift and Cmd/Ctrl build on.
    base: BTreeSet<String>,
    /// Cmd/Ctrl at the press: rows in the box toggle instead of replacing the selection.
    toggle: bool,
    /// Shift at the press: rows in the box join the selection.
    extend: bool,
    /// False until the pointer has travelled far enough to be a box rather than a click.
    pub dragging: bool,
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
            // and only once the button comes up — a drag out of it moves the row instead.
            if drag.clicks >= 2
                && !(mods.cmd || mods.ctrl || mods.shift)
                && let Some(file_drag) = &mut s.file_drag
            {
                file_drag.pending_open = true;
            }
        }
        Phase::Move => s.drag_to(at),
        Phase::Up => s.end_drag(drag.cancelled()),
    }
}

/// Whether a window point is inside a rect.
fn inside(rect: [f32; 4], x: f32, y: f32) -> bool {
    x >= rect[0] && x <= rect[0] + rect[2] && y >= rect[1] && y <= rect[1] + rect[3]
}

impl App {
    /// Turns a point in the list viewport's own points into window points.
    pub fn window_point(&self, local: (f32, f32)) -> (f32, f32) {
        (self.rows_view[0] + local.0, self.rows_view[1] + local.1)
    }

    /// The row under a window point inside the list.
    pub fn row_at_window(&self, y: f32) -> Option<usize> {
        // The bounce draws the rows where the scroll doesn't, so the pointer follows them.
        self.row_at(y - self.rows_view[1] + self.told_offset() - self.overshoot())
    }

    /// A press in the list: a press on a row selects it (or keeps a selection it belongs to, so
    /// the selection can be dragged) and a press on the empty space starts a selection box. With
    /// Option (Alt) held the box starts anywhere, over the rows themselves.
    /// Points are the window's.
    pub fn begin_press(&mut self, at: (f32, f32), mods: Mods, tx: &mut Tx) {
        // A box over the rows is the one thing they can't get otherwise: the list only has empty
        // space to start one in when it doesn't scroll, so Option is how a long list gets swept.
        if mods.alt && self.row_at_window(at.1).is_some() {
            self.file_drag = None;
            self.marquee = Some(Marquee {
                start: at,
                current: at,
                base: self.selection.clone(),
                toggle: mods.cmd || mods.ctrl,
                extend: mods.shift,
                dragging: false,
            });
            return;
        }
        let Some(index) = self.row_at_window(at.1) else {
            self.file_drag = None;
            self.marquee = Some(Marquee {
                start: at,
                current: at,
                base: self.selection.clone(),
                toggle: mods.cmd || mods.ctrl,
                extend: mods.shift,
                dragging: false,
            });
            // A press on the empty space below the rows clears them, unless it adds.
            if !mods.shift && !(mods.cmd || mods.ctrl) {
                self.clear_selection();
            }
            return;
        };
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
            if !drag.dragging && travel < DRAG_START {
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

    /// Selects the rows a box covers, built on the selection it started from: Shift adds the
    /// box, Cmd/Ctrl toggles it, and a plain box replaces the selection with it.
    fn sweep_box(&mut self, marquee: &Marquee) {
        let swept = self.rows_in_box(marquee.start, marquee.current);
        self.selection = if marquee.extend {
            marquee.base.union(&swept).cloned().collect()
        } else if marquee.toggle {
            marquee.base.symmetric_difference(&swept).cloned().collect()
        } else {
            swept
        };
        if self
            .selected
            .as_ref()
            .is_none_or(|id| !self.selection.contains(id))
        {
            match self.selected_ids().first().cloned() {
                Some(id) => self.focus_id(&id),
                None => {
                    self.selected = None;
                    self.preview = None;
                }
            }
        }
    }

    /// The button came up: drop a dragged selection on its target, narrow a click inside a
    /// selection to its row, or put the old selection back when the box was cancelled.
    pub fn end_drag(&mut self, cancelled: bool) {
        if let Some(drag) = self.file_drag.take() {
            if cancelled {
                return;
            }
            if drag.dragging {
                if let Some(destination) = self.drop_target(&drag) {
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

    /// The folder a row drag is over, if it can take the drop: a sidebar place or a folder row.
    /// Aims at the rows where this frame is putting them, so a drag scrolling the list takes
    /// the row the pointer has just been carried over.
    fn aim_drag(&self, drag: &mut FileDrag) {
        let scroll = self.scroll.current();
        drag.row = None;
        drag.place = None;
        drag.place_id = None;
        let (x, y) = drag.pointer;
        // The sidebar sits left of the list, and takes the drop before the rows do.
        for (id, (rect, path)) in &self.places_at {
            if inside(*rect, x, y) && *path != self.cwd {
                drag.place = Some(path.clone());
                drag.place_id = Some(id);
                return;
            }
        }
        if !inside(self.rows_view, x, y) {
            return;
        }
        // The bounce draws the rows where the scroll doesn't, so the pointer follows them.
        let Some(index) = self.row_at(y - self.rows_view[1] + scroll - self.overshoot()) else {
            return;
        };
        if let Some(&entry) = self.visible.get(index) {
            let entry = &self.entries[entry];
            if entry.is_dir && entry.path != self.cwd && !self.selection.contains(&entry.id) {
                drag.row = Some(index);
            }
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
            self.sweep_box(&marquee);
            self.marquee = Some(marquee);
        }
    }

    /// Where a drag would drop, for the button's release.
    fn drop_target(&self, drag: &FileDrag) -> Option<PathBuf> {
        if let Some(path) = &drag.place {
            return Some(path.clone());
        }
        let index = drag.row?;
        self.visible
            .get(index)
            .map(|&i| self.entries[i].path.clone())
    }

    /// The folder a drag is over, for the view to highlight it.
    pub fn drop_row(&self) -> Option<usize> {
        self.file_drag.as_ref().and_then(|drag| drag.row)
    }

    /// The sidebar place a drag is over, for the view to highlight it.
    pub fn drop_place(&self) -> Option<&'static str> {
        self.file_drag.as_ref().and_then(|drag| drag.place_id)
    }

    /// The rows being dragged, once the pointer has moved far enough, and where the pointer
    /// is: what the view shows following the pointer.
    pub fn drag_badge(&self) -> Option<(String, (f32, f32))> {
        let drag = self.file_drag.as_ref().filter(|drag| drag.dragging)?;
        let count = self.selection.len();
        let noun = if count == 1 { "item" } else { "items" };
        Some((format!("Move {count} {noun}"), drag.pointer))
    }

    /// Moves the selection into `destination`, the way a drop does. A refusal keeps everything
    /// where it is and says why in the banner.
    pub fn move_selection_into(&mut self, destination: &Path) -> bool {
        let targets = self.selected_ids();
        if targets.is_empty() || destination == self.cwd {
            return false;
        }
        match self.move_targets(&targets, destination) {
            Ok(()) => {
                self.error = None;
                true
            }
            Err(message) => {
                self.error = Some(message);
                false
            }
        }
    }

    /// The rows a box between two window points covers, limited to the rows on screen.
    fn rows_in_box(&self, a: (f32, f32), b: (f32, f32)) -> BTreeSet<String> {
        if self.visible.is_empty() {
            return BTreeSet::new();
        }
        let view = self.rows_view;
        let (top, bottom) = (
            a.1.min(b.1).max(view[1]),
            a.1.max(b.1).min(view[1] + view[3]),
        );
        if bottom < top {
            return BTreeSet::new();
        }
        // The rows are where this frame is putting them, not where the last one drew them.
        let scroll = self.scroll.current();
        let row = |y: f32| self.row_at(y - view[1] + scroll - self.overshoot());
        let first = row(top).unwrap_or(0);
        let last = row(bottom).unwrap_or(self.visible.len().saturating_sub(1));
        (first..=last.min(self.visible.len() - 1))
            .filter_map(|i| self.visible.get(i).map(|&i| self.entries[i].id.clone()))
            .collect()
    }

    /// The box's rect in window points, while a drag is drawing one. A drag straight down (or
    /// across) still draws: the box keeps a hairline on the side it has no extent on.
    pub fn marquee_rect(&self) -> Option<[f32; 4]> {
        let marquee = self.marquee.as_ref().filter(|m| m.dragging)?;
        let view = self.rows_view;
        let left = marquee.start.0.min(marquee.current.0).max(view[0]);
        let right = marquee
            .start
            .0
            .max(marquee.current.0)
            .min(view[0] + view[2]);
        let top = marquee.start.1.min(marquee.current.1).max(view[1]);
        let bottom = marquee
            .start
            .1
            .max(marquee.current.1)
            .min(view[1] + view[3]);
        let (width, height) = (right - left, bottom - top);
        if height <= 0.0 {
            return None;
        }
        Some([left, top, width.max(1.0), height])
    }
}
