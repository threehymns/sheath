use super::{App, FILES, FileDrag, PLACE_ROW_GAP, PLACE_ROW_HEIGHT};
use crate::places::Section;
use rune::Icon;
use rune_kit::glyphs;
use std::path::{Path, PathBuf};

const PLACE_INSIDE_BAND: f32 = 6.0;

#[derive(Clone)]
pub struct PlaceAt {
    pub rect: [f32; 4],
    pub path: PathBuf,
    pub section: Section,
    pub index: usize,
}

pub struct DragPreview {
    pub label: String,
    pub icon: &'static Icon,
    pub at: (f32, f32),
}

fn inside(rect: [f32; 4], x: f32, y: f32) -> bool {
    x >= rect[0] && x <= rect[0] + rect[2] && y >= rect[1] && y <= rect[1] + rect[3]
}

impl App {
    pub(super) fn aim_drag(&self, drag: &mut FileDrag) {
        let scroll = self.list_scroll();
        drag.row = None;
        drag.place = None;
        drag.place_id = None;
        drag.insert = None;
        let (x, y) = drag.pointer;
        if let Some((id, at)) = self.place_at(x, y) {
            let edge = PLACE_INSIDE_BAND;
            if (edge..=at.rect[3] - edge).contains(&(y - at.rect[1])) {
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
        let local = (
            x - self.rows_view[0],
            y - self.rows_view[1] + scroll - self.overshoot(FILES),
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

    pub(super) fn drop_target(&self, drag: &FileDrag) -> Option<PathBuf> {
        if let Some(path) = &drag.place {
            return Some(path.clone());
        }
        let index = drag.row?;
        self.visible
            .get(index)
            .map(|&i| self.entries[i].path.clone())
    }

    pub fn drop_row(&self) -> Option<usize> {
        self.file_drag.as_ref().and_then(|drag| drag.row)
    }

    pub fn drop_place(&self) -> Option<&str> {
        self.file_drag
            .as_ref()
            .and_then(|drag| drag.place_id.as_deref())
    }

    fn place_at(&self, x: f32, y: f32) -> Option<(&String, &PlaceAt)> {
        self.places_at.iter().find(|(_, at)| inside(at.rect, x, y))
    }

    fn place_rows(&self, section: Section) -> Vec<&PlaceAt> {
        let mut rows: Vec<&PlaceAt> = self
            .places_at
            .values()
            .filter(|at| at.section == section)
            .collect();
        rows.sort_by_key(|at| at.index);
        rows
    }

    fn insert_at(&self, section: Section, y: f32) -> usize {
        self.place_rows(section)
            .iter()
            .filter(|at| at.rect[1] + at.rect[3] * 0.5 < y)
            .count()
    }

    pub fn place_insert(&self) -> Option<(Section, usize)> {
        self.file_drag.as_ref().filter(|drag| drag.dragging)?.insert
    }

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
