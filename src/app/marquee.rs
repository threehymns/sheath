//! The selection box: the press that starts it, the rows it sweeps, and the rect the view
//! draws. The box is measured on the rows it stands for, so it follows them as the list
//! scrolls or bounces under the pointer.

use super::App;
use std::collections::BTreeSet;

/// A press in the file list and the box it has swept so far, in window points.
pub struct Marquee {
    pub start: (f32, f32),
    pub current: (f32, f32),
    /// How far the rows sat from the viewport's top when the press landed: the scroll less its
    /// bounce. The corner is a point on the rows, so knowing where they were lets it move with
    /// them when the list scrolls under the pointer.
    rows_at: f32,
    /// The selection when the press landed, which Shift and Cmd/Ctrl build on.
    pub(super) base: BTreeSet<String>,
    /// Cmd/Ctrl at the press: rows in the box toggle instead of replacing the selection.
    toggle: bool,
    /// Shift at the press: rows in the box join the selection.
    extend: bool,
    /// False until the pointer has travelled far enough to be a box rather than a click.
    pub dragging: bool,
    /// The row a second press landed on: a release without a drag opens it. A press on a
    /// row's blank space sweeps a box rather than dragging the row, but opening is still the
    /// row's, so the double click that would open it must survive the box it starts.
    pub(super) pending_open: Option<String>,
}

impl Marquee {
    /// A press at `start` over rows that sat `rows_at` from the viewport's top, building on the
    /// selection `base`: Shift joins it and Cmd/Ctrl toggles it.
    pub(super) fn begin(
        start: (f32, f32),
        rows_at: f32,
        base: BTreeSet<String>,
        toggle: bool,
        extend: bool,
    ) -> Marquee {
        Marquee {
            start,
            current: start,
            rows_at,
            base,
            toggle,
            extend,
            dragging: false,
            pending_open: None,
        }
    }
}

impl App {
    /// Selects the rows a box covers, built on the selection it started from: Shift adds the
    /// box, Cmd/Ctrl toggles it, and a plain box replaces the selection with it.
    pub(super) fn sweep_box(&mut self, marquee: &Marquee) {
        let swept = self.rows_in_box(self.marquee_start(marquee), marquee.current);
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

    /// The rows a box between two window points covers, from the row it started on to the one
    /// under the pointer. The box follows the rows rather than the screen, so the rows a drag
    /// has scrolled past stay swept and one box can cover more than a screenful.
    fn rows_in_box(&self, a: (f32, f32), b: (f32, f32)) -> BTreeSet<String> {
        if self.visible.is_empty() {
            return BTreeSet::new();
        }
        let view = self.rows_view;
        // The rows are where this frame is putting them, not where the last one drew them.
        let offset = self.scroll.current() - self.overshoot();
        let height = self.content_height();
        let top = (a.1.min(b.1) - view[1] + offset).clamp(0.0, height);
        let bottom = (a.1.max(b.1) - view[1] + offset).clamp(0.0, height);
        // A box past the last row covers none, however its window points sit.
        if top >= height {
            return BTreeSet::new();
        }
        let indices = if self.is_grid() {
            let (left, right) = (
                a.0.min(b.0).max(view[0]) - view[0],
                a.0.max(b.0).min(view[0] + view[2]) - view[0],
            );
            self.grid_indices_in_rect(left, top, right, bottom)
        } else {
            let first = self.index_at((0.0, top)).unwrap_or(0);
            let last = self
                .index_at((0.0, bottom))
                .unwrap_or(self.visible.len().saturating_sub(1));
            (first..=last.min(self.visible.len() - 1)).collect()
        };
        indices
            .into_iter()
            .filter_map(|i| self.visible.get(i).map(|&i| self.entries[i].id.clone()))
            .collect()
    }

    /// Where the box's corner sits in window points now: the point the press landed on, which
    /// is a point on the rows, so it moves with them as the list scrolls or bounces under the
    /// pointer.
    fn marquee_start(&self, marquee: &Marquee) -> (f32, f32) {
        let moved = self.rows_at() - marquee.rows_at;
        (marquee.start.0, marquee.start.1 - moved)
    }

    /// How far the rows sit from the viewport's top right now: the scroll, less the bounce.
    pub(super) fn rows_at(&self) -> f32 {
        self.scroll.current() - self.overshoot()
    }

    /// The box's rect in window points while a drag is drawing one, from the corner it started
    /// on to the pointer and as tall as the rows it spans. The view clips it to the list, so a
    /// drag that scrolls draws a box taller than the viewport. A drag straight down or across
    /// still draws, because the box keeps a hairline on the side it has no extent on.
    pub fn marquee_rect(&self) -> Option<[f32; 4]> {
        let marquee = self.marquee.as_ref().filter(|m| m.dragging)?;
        let start = self.marquee_start(marquee);
        let (left, right) = (
            start.0.min(marquee.current.0),
            start.0.max(marquee.current.0),
        );
        let (top, bottom) = (
            start.1.min(marquee.current.1),
            start.1.max(marquee.current.1),
        );
        let (width, height) = (right - left, bottom - top);
        if height <= 0.0 {
            return None;
        }
        Some([left, top, width.max(1.0), height])
    }
}
