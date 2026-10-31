use super::App;
use std::collections::BTreeSet;

pub struct Marquee {
    pub start: (f32, f32),
    pub current: (f32, f32),
    // Rows' distance from the viewport top at the press, so the corner moves with them.
    rows_at: f32,
    pub(super) base: BTreeSet<String>,
    toggle: bool,
    extend: bool,
    pub dragging: bool,
    pub(super) pending_open: Option<String>,
}

impl Marquee {
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

    fn rows_in_box(&self, a: (f32, f32), b: (f32, f32)) -> BTreeSet<String> {
        if self.visible.is_empty() {
            return BTreeSet::new();
        }
        let view = self.rows_view;
        let offset = self.scroll.current() - self.overshoot();
        let height = self.content_height();
        let top = (a.1.min(b.1) - view[1] + offset).clamp(0.0, height);
        let bottom = (a.1.max(b.1) - view[1] + offset).clamp(0.0, height);
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

    fn marquee_start(&self, marquee: &Marquee) -> (f32, f32) {
        let moved = self.rows_at() - marquee.rows_at;
        (marquee.start.0, marquee.start.1 - moved)
    }

    pub(super) fn rows_at(&self) -> f32 {
        self.scroll.current() - self.overshoot()
    }

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
