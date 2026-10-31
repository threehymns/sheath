//! Presses, drags and drops in the file list: the row drag heading for a folder, and the
//! scrolling a drag held at an edge causes. The selection box is `marquee`.

use super::marquee::Marquee;
use crate::app::App;
use crate::places::Section;
use rune::{Drag, Mods, Phase, Tx};
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
    pub place_id: Option<String>,
    /// The sidebar section under the pointer, when the drop adds the rows to it rather than
    /// moving them: its heading, or the space around its rows.
    pub section: Option<Section>,
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
    /// Option (Alt) held the box starts anywhere, over the rows themselves.
    /// Points are the window's.
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
            section: None,
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
                if let Some(section) = drag.section {
                    let ids = self.selected_ids();
                    self.add_places(section, &ids);
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
        drag.section = None;
        let (x, y) = drag.pointer;
        // The sidebar sits left of the list, and takes the drop before the rows do.
        for (id, (rect, path)) in &self.places_at {
            if !inside(*rect, x, y) {
                continue;
            }
            // A place row is a folder to move into, never the section's "add a place": the
            // press is over the row, not the space around it. The open folder takes nothing.
            if *path != self.cwd {
                drag.place = Some(path.clone());
                drag.place_id = Some(id.clone());
            }
            return;
        }
        // Between the rows, or on a section's heading: add the rows to that section. The
        // sections are part of the sidebar, so a rect left over from before it was collapsed
        // must not catch a drop in the list.
        if x < self.rows_view[0] {
            for (section, rect) in &self.sections_at {
                if inside(*rect, x, y) {
                    drag.section = Some(*section);
                    return;
                }
            }
        }
        if !inside(self.rows_view, x, y) {
            return;
        }
        // The bounce draws the rows where the scroll doesn't, so the pointer follows them.
        let local = (
            x - self.rows_view[0],
            y - self.rows_view[1] + scroll - self.overshoot(),
        );
        let Some(index) = self.index_at(local) else {
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
    pub fn drop_place(&self) -> Option<&str> {
        self.file_drag
            .as_ref()
            .and_then(|drag| drag.place_id.as_deref())
    }

    /// The rows being dragged, once the pointer has moved far enough, and where the pointer
    /// is: what the view shows following the pointer.
    pub fn drag_badge(&self) -> Option<(String, (f32, f32))> {
        let drag = self.file_drag.as_ref().filter(|drag| drag.dragging)?;
        let count = self.selection.len();
        let label = match drag.section {
            Some(section) if count == 1 => format!("Add to {}", section.title()),
            Some(section) => format!("Add {count} to {}", section.title()),
            None => {
                let noun = if count == 1 { "item" } else { "items" };
                format!("Move {count} {noun}")
            }
        };
        Some((label, drag.pointer))
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
}
