//! Where a drag would land: the folder row or sidebar place under it, the gap between places
//! it would insert between, and what the drag shows while it is aimed. The press and the move
//! stay in `drag`; this is what they aim at.

use super::{App, FileDrag, PLACE_ROW_GAP, PLACE_ROW_HEIGHT};
use crate::places::Section;
use rune::Icon;
use rune_kit::glyphs;
use std::path::{Path, PathBuf};

/// How much of a place row's top and bottom is a gap rather than its body: the body takes the
/// drop as a move into the folder, and the edges let it land between places.
const PLACE_INSIDE_BAND: f32 = 6.0;

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

/// Whether a window point is inside a rect.
fn inside(rect: [f32; 4], x: f32, y: f32) -> bool {
    x >= rect[0] && x <= rect[0] + rect[2] && y >= rect[1] && y <= rect[1] + rect[3]
}

impl App {
    /// The folder a row drag is over, if it can take the drop: a sidebar place or a folder row.
    /// Aims at the rows where this frame is putting them, so a drag scrolling the list takes
    /// the row the pointer has just been carried over.
    pub(super) fn aim_drag(&self, drag: &mut FileDrag) {
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

    /// Where a drag would drop, for the button's release.
    pub(super) fn drop_target(&self, drag: &FileDrag) -> Option<PathBuf> {
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
