use crate::app::{App, ROW_HEIGHT};
use std::collections::BTreeSet;

pub const TILE_WIDTH: f32 = 104.0;
pub const TILE_HEIGHT: f32 = 96.0;
pub const TILE_GAP: f32 = 8.0;
pub const GRID_PAD: f32 = 12.0;
pub const GRID_ROW_HEIGHT: f32 = TILE_HEIGHT + TILE_GAP;

pub const GRID_TOP_MARGIN: f32 = 4.0;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Layout {
    #[default]
    List,
    Grid,
}

impl Layout {
    pub fn is_grid(self) -> bool {
        matches!(self, Layout::Grid)
    }
}

impl App {
    pub fn is_grid(&self) -> bool {
        self.layout.is_grid()
    }

    pub fn grid_columns(&self) -> usize {
        let width = self.rows_view[2];
        if width <= 0.0 {
            return 1;
        }
        let usable = width - 2.0 * GRID_PAD + TILE_GAP;
        (usable / (TILE_WIDTH + TILE_GAP)).floor().max(1.0) as usize
    }

    pub fn grid_rows(&self) -> usize {
        self.visible.len().div_ceil(self.grid_columns())
    }

    pub fn content_height(&self) -> f32 {
        if self.is_grid() {
            GRID_TOP_MARGIN + self.grid_rows() as f32 * GRID_ROW_HEIGHT
        } else {
            self.visible.len() as f32 * ROW_HEIGHT
        }
    }

    pub fn index_at(&self, at: (f32, f32)) -> Option<usize> {
        if self.visible.is_empty() || at.1 < 0.0 {
            return None;
        }
        if !self.is_grid() {
            let index = (at.1 / ROW_HEIGHT).floor() as usize;
            return (index < self.visible.len()).then_some(index);
        }
        let x = at.0 - GRID_PAD;
        if x < 0.0 || at.1 < GRID_TOP_MARGIN {
            return None;
        }
        let y = at.1 - GRID_TOP_MARGIN;
        let columns = self.grid_columns();
        let column = (x / (TILE_WIDTH + TILE_GAP)).floor() as usize;
        if column >= columns {
            return None;
        }
        let row = (y / GRID_ROW_HEIGHT).floor() as usize;
        let cell_x = x - column as f32 * (TILE_WIDTH + TILE_GAP);
        let cell_y = y - row as f32 * GRID_ROW_HEIGHT;
        if cell_x >= TILE_WIDTH || cell_y >= TILE_HEIGHT {
            return None;
        }
        let index = row * columns + column;
        (index < self.visible.len()).then_some(index)
    }

    fn cell_of(&self, index: usize) -> (usize, usize) {
        let columns = self.grid_columns();
        (index / columns, index % columns)
    }

    fn index_at_cell(&self, row: usize, column: usize) -> Option<usize> {
        if column >= self.grid_columns() {
            return None;
        }
        let index = row * self.grid_columns() + column;
        (index < self.visible.len()).then_some(index)
    }

    pub(super) fn grid_indices_in_rect(
        &self,
        left: f32,
        top: f32,
        right: f32,
        bottom: f32,
    ) -> Vec<usize> {
        if right < left || bottom < top {
            return Vec::new();
        }
        let right = right.max(left + 1.0);
        let bottom = bottom.max(top + 1.0);
        let columns = self.grid_columns();
        let row = |y: f32| ((y - GRID_TOP_MARGIN) / GRID_ROW_HEIGHT).floor().max(0.0) as usize;
        let column = |x: f32| {
            (((x - GRID_PAD) / (TILE_WIDTH + TILE_GAP)).floor().max(0.0) as usize).min(columns - 1)
        };
        let first_row = row(top);
        let last_row = row(bottom - 0.1);
        let first_column = column(left);
        let last_column = column(right - 0.1);
        let mut indices = Vec::new();
        for row in first_row..=last_row {
            for column in first_column..=last_column {
                let x = GRID_PAD + column as f32 * (TILE_WIDTH + TILE_GAP);
                let y = GRID_TOP_MARGIN + row as f32 * GRID_ROW_HEIGHT;
                let covered =
                    left < x + TILE_WIDTH && right > x && top < y + TILE_HEIGHT && bottom > y;
                if covered && let Some(index) = self.index_at_cell(row, column) {
                    indices.push(index);
                }
            }
        }
        indices
    }

    pub fn scroll_key(&self, index: usize) -> Option<String> {
        if !self.is_grid() {
            return self.row_id(index);
        }
        let columns = self.grid_columns();
        self.row_id(index - index % columns)
            .map(|id| format!("grid-row:{id}"))
    }

    pub(super) fn grid_range_to(&self, index: usize, range: &mut BTreeSet<String>) {
        let from = self.anchor_index().unwrap_or(index);
        let (from_row, from_column) = self.cell_of(from);
        let (to_row, to_column) = self.cell_of(index);
        for row in from_row.min(to_row)..=from_row.max(to_row) {
            for column in from_column.min(to_column)..=from_column.max(to_column) {
                if let Some(index) = self.index_at_cell(row, column) {
                    range.insert(self.entries[self.visible[index]].id.clone());
                }
            }
        }
    }
}
