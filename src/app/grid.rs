//! The grid layout's geometry: how many tiles fit across, which tile a point or cell holds, and
//! how a keyboard move or a box maps to tiles rather than rows.

use crate::app::{App, ROW_HEIGHT};
use std::collections::BTreeSet;

/// A tile's size and the gap after it. The virtual rows scroll by [`GRID_ROW_HEIGHT`].
pub const TILE_WIDTH: f32 = 104.0;
pub const TILE_HEIGHT: f32 = 96.0;
pub const TILE_GAP: f32 = 8.0;
pub const GRID_PAD: f32 = 12.0;
pub const GRID_ROW_HEIGHT: f32 = TILE_HEIGHT + TILE_GAP;

/// The gap between the sort toolbar and the first row of tiles, so a tile's highlight is not
/// flush against the bar above it.
pub const GRID_TOP_MARGIN: f32 = 4.0;

/// How the list shows the folder: one row per item, or a wall of tiles.
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

    /// How many tiles fit across the list's viewport. Before layout has spoken, one does.
    pub fn grid_columns(&self) -> usize {
        let width = self.rows_view[2];
        if width <= 0.0 {
            return 1;
        }
        let usable = width - 2.0 * GRID_PAD + TILE_GAP;
        (usable / (TILE_WIDTH + TILE_GAP)).floor().max(1.0) as usize
    }

    /// The grid rows the filter leaves on screen.
    pub fn grid_rows(&self) -> usize {
        self.visible.len().div_ceil(self.grid_columns())
    }

    /// How tall the list's content is in points: what the scroll's range is measured against.
    pub fn content_height(&self) -> f32 {
        if self.is_grid() {
            self.grid_rows() as f32 * GRID_ROW_HEIGHT
        } else {
            self.visible.len() as f32 * ROW_HEIGHT
        }
    }

    /// The tile or row under a point in the list's own points, where the first starts at zero.
    /// The gaps around the tiles hold neither: a point there is empty space.
    pub fn index_at(&self, at: (f32, f32)) -> Option<usize> {
        if self.visible.is_empty() || at.1 < 0.0 {
            return None;
        }
        if !self.is_grid() {
            let index = (at.1 / ROW_HEIGHT).floor() as usize;
            return (index < self.visible.len()).then_some(index);
        }
        let x = at.0 - GRID_PAD;
        if x < 0.0 {
            return None;
        }
        let columns = self.grid_columns();
        let column = (x / (TILE_WIDTH + TILE_GAP)).floor() as usize;
        if column >= columns {
            return None;
        }
        let row = (at.1 / GRID_ROW_HEIGHT).floor() as usize;
        // A cell is a tile and the gap that follows it. Only the tile counts as one, so a
        // press between tiles starts a selection box as a press below the rows does.
        let cell_x = x - column as f32 * (TILE_WIDTH + TILE_GAP);
        let cell_y = at.1 - row as f32 * GRID_ROW_HEIGHT;
        if cell_x >= TILE_WIDTH || cell_y >= TILE_HEIGHT {
            return None;
        }
        let index = row * columns + column;
        (index < self.visible.len()).then_some(index)
    }

    /// The grid cell a visible index sits in: (row, column).
    fn cell_of(&self, index: usize) -> (usize, usize) {
        let columns = self.grid_columns();
        (index / columns, index % columns)
    }

    /// The visible index in a grid cell, if the cell holds one.
    fn index_at_cell(&self, row: usize, column: usize) -> Option<usize> {
        if column >= self.grid_columns() {
            return None;
        }
        let index = row * self.grid_columns() + column;
        (index < self.visible.len()).then_some(index)
    }

    /// The visible indices a box covers, in the list's own points. A tile the box touches
    /// counts, as a row the box touches does; the gaps around the tiles hold no tile, so a box
    /// drawn only in them takes nothing. A box with no extent on one axis is measured as the
    /// hairline the view draws for it, so a drag straight down still sweeps the tiles it runs
    /// through.
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
        let row = |y: f32| (y / GRID_ROW_HEIGHT).floor().max(0.0) as usize;
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
                // The cell's tile, not the cell: the gap after a tile is not part of it.
                let x = GRID_PAD + column as f32 * (TILE_WIDTH + TILE_GAP);
                let y = row as f32 * GRID_ROW_HEIGHT;
                let covered =
                    left < x + TILE_WIDTH && right > x && top < y + TILE_HEIGHT && bottom > y;
                if covered && let Some(index) = self.index_at_cell(row, column) {
                    indices.push(index);
                }
            }
        }
        indices
    }

    /// The key a scroll should bring into view for a visible index: the row's own id in the
    /// list, and the grid row that holds the tile. Grid rows are keyed apart from the tiles
    /// they hold, so a tile's id always names the tile itself.
    pub fn scroll_key(&self, index: usize) -> Option<String> {
        if !self.is_grid() {
            return self.row_id(index);
        }
        let columns = self.grid_columns();
        self.row_id(index - index % columns)
            .map(|id| format!("grid-row:{id}"))
    }

    /// The tiles between the anchor and `index`, as a rectangle: the grid's Shift range.
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
