//! Presses, drags and drops in the file list: the row drag heading for a folder, and the
//! scrolling a drag held at an edge causes. The selection box is `marquee`.

use super::marquee::Marquee;
use crate::app::{App, PLACE_ROW_GAP, PLACE_ROW_HEIGHT};
use crate::places::Section;
use rune::{Drag, Icon, Mods, Phase, Tx};
use rune_kit::glyphs;
use std::path::{Path, PathBuf};

/// How much of a place row's top and bottom is a gap rather than its body: the body takes the
/// drop as a move into the folder, and the edges let it land between places.
const PLACE_INSIDE_BAND: f32 = 6.0;

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
    /// Where the rows would be inserted among the places rather than moved: the section and
    /// the index there. Set between rows, on a row's edge, or on a section's heading.
    pub insert: Option<(Section, usize)>,
}

/// A sidebar row as the last layout placed it: the folder a drop moves into, and the place it
/// holds in its section, for a drag to aim between the rows.
#[derive(Clone)]
pub struct PlaceAt {
    pub rect: [f32; 4],
    pub path: PathBuf,
    pub section: Section,
    pub index: usize,
}

/// What a drag shows following the pointer: the item it carries, and where it would put it.
pub struct DragPreview {
    pub label: String,
    pub icon: &'static Icon,
    pub at: (f32, f32),
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
        drag.insert = None;
        let (x, y) = drag.pointer;
        // The sidebar sits left of the list, and takes the drop before the rows do. A place
        // row's body moves the rows inside its folder; its edges are the gaps around it.
        if let Some((id, at)) = self.place_at(x, y) {
            let edge = PLACE_INSIDE_BAND;
            if (edge..=at.rect[3] - edge).contains(&(y - at.rect[1])) {
                // The open folder takes nothing: the rows are already inside it.
                if at.path != self.cwd {
                    drag.place = Some(at.path.clone());
                    drag.place_id = Some(id.clone());
                }
            } else {
                let index = if y - at.rect[1] < at.rect[3] * 0.5 {
                    at.index
                } else {
                    at.index + 1
                };
                drag.insert = Some((at.section, index));
            }
            return;
        }
        // Between the rows, or on a section's heading: insert the rows there. The sections
        // are part of the sidebar, so a rect left over from before it was collapsed must not
        // catch a drop in the list.
        if x < self.rows_view[0] {
            for (section, rect) in &self.sections_at {
                if inside(*rect, x, y) {
                    drag.insert = Some((*section, self.insert_at(*section, y)));
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
            // A press that has not travelled is a click, not a box. Sweeping it would cover the
            // cell its point sits in, so a click in the gap between tiles took the nearest one.
            if marquee.dragging {
                self.sweep_box(&marquee);
            }
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

    /// The place row under a window point, if one is there.
    fn place_at(&self, x: f32, y: f32) -> Option<(&String, &PlaceAt)> {
        self.places_at.iter().find(|(_, at)| inside(at.rect, x, y))
    }

    /// A section's place rows, in the order they are shown.
    fn place_rows(&self, section: Section) -> Vec<&PlaceAt> {
        let mut rows: Vec<&PlaceAt> = self
            .places_at
            .values()
            .filter(|at| at.section == section)
            .collect();
        rows.sort_by_key(|at| at.index);
        rows
    }

    /// The index a pointer at `y` inserts at: before the first row whose middle is below it.
    fn insert_at(&self, section: Section, y: f32) -> usize {
        self.place_rows(section)
            .iter()
            .filter(|at| at.rect[1] + at.rect[3] * 0.5 < y)
            .count()
    }

    /// The place insertion a file drag is aiming at: the section, and the index the rows would
    /// land at. The rows from there on step aside to open the gap.
    pub fn place_insert(&self) -> Option<(Section, usize)> {
        self.file_drag.as_ref().filter(|drag| drag.dragging)?.insert
    }

    /// The insertion line's rect in window points, while a file drag would insert places here:
    /// the top of the row that stepped aside, or just past the section's last row.
    pub fn place_insert_line(&self) -> Option<[f32; 4]> {
        let (section, index) = self.place_insert()?;
        let rows = self.place_rows(section);
        let (rect, y) = match rows.get(index) {
            Some(at) => (at.rect, at.rect[1]),
            None => match rows.last() {
                Some(at) => (at.rect, at.rect[1] + at.rect[3] + PLACE_ROW_GAP),
                None => {
                    let rect = *self.sections_at.get(&section)?;
                    (rect, rect[1] + PLACE_ROW_HEIGHT + PLACE_ROW_GAP)
                }
            },
        };
        Some([rect[0], y - 1.0, rect[2], 2.0])
    }

    /// The rows being dragged and where the pointer is, once the pointer has moved far enough:
    /// what the view shows following it. A drop between places previews the item itself; one
    /// on a folder says it is moving there.
    pub fn drag_preview(&self) -> Option<DragPreview> {
        let drag = self.file_drag.as_ref().filter(|drag| drag.dragging)?;
        let count = self.selection.len();
        let first = self
            .selected_ids()
            .into_iter()
            .next()
            .and_then(|id| self.entries.iter().find(|entry| entry.id == id));
        let icon = match first {
            Some(entry) if entry.is_dir => &glyphs::FOLDER,
            _ => &glyphs::FILE,
        };
        let name = first.map(|entry| entry.name.clone()).unwrap_or_default();
        let label = match drag.insert {
            Some((section, _)) if count <= 1 => format!("Add “{name}” to {}", section.title()),
            Some((section, _)) => format!("Add {count} items to {}", section.title()),
            None if count == 1 => format!("Move “{name}”"),
            None => format!("Move {count} items"),
        };
        Some(DragPreview {
            label,
            icon,
            at: drag.pointer,
        })
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
