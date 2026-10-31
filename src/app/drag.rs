use super::marquee::Marquee;
use crate::app::App;
use crate::places::Section;
use rune::{Drag, Mods, Phase, Tx};
use std::path::PathBuf;

const DRAG_START: f32 = 3.0;

const NAME_HANDLE_SLOP: f32 = 6.0;

const NAME_HANDLE_MIN: f32 = 88.0;

const DRAG_SCROLL_EDGE: f32 = 44.0;

const DRAG_SCROLL_MAX: f32 = 1100.0;

pub struct FileDrag {
    pub start: (f32, f32),
    pub pointer: (f32, f32),
    press: String,
    pending: Option<String>,
    pending_open: bool,
    pub dragging: bool,
    pub row: Option<usize>,
    pub place: Option<PathBuf>,
    pub place_id: Option<String>,
    pub insert: Option<(Section, usize)>,
}

pub fn list_drag(s: &mut App, drag: &mut Drag, tx: &mut Tx) {
    let at = s.window_point((drag.x, drag.y));
    match drag.phase {
        Phase::Down => {
            let mods = tx.mods();
            s.begin_press(at, mods, tx);
            if drag.clicks >= 2 && !(mods.cmd || mods.ctrl || mods.shift) {
                if let Some(file_drag) = &mut s.file_drag {
                    file_drag.pending_open = true;
                } else {
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
    // Rune reports a pointer in the viewport's own points; the app works in window points.
    pub fn window_point(&self, local: (f32, f32)) -> (f32, f32) {
        (self.rows_view[0] + local.0, self.rows_view[1] + local.1)
    }

    fn on_name_cell(&self, index: usize, at: (f32, f32)) -> bool {
        let Some(&visible) = self.visible.get(index) else {
            return false;
        };
        let entry = &self.entries[visible];
        let cell_right = self.rows_view[0] + self.rows_view[2] - self.size_width - self.date_width;
        let widest = (self.rows_view[0] + NAME_HANDLE_MIN).min(cell_right);
        match self.name_at.get(&entry.path.display().to_string()) {
            Some(&right) => at.0 <= (right + NAME_HANDLE_SLOP).max(widest).min(cell_right),
            None => at.0 <= cell_right,
        }
    }

    pub fn index_at_window(&self, at: (f32, f32)) -> Option<usize> {
        self.index_at((
            at.0 - self.rows_view[0],
            at.1 - self.rows_view[1] + self.told_offset() - self.overshoot(),
        ))
    }

    pub fn begin_press(&mut self, at: (f32, f32), mods: Mods, tx: &mut Tx) {
        // Alt sweeps a box over a row; a scrolling list has no empty space to start one.
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
            if !mods.shift && !(mods.cmd || mods.ctrl) {
                self.clear_selection();
            }
            return;
        };
        let selected = self
            .row_id(index)
            .is_some_and(|id| self.selection.contains(&id));
        if !self.is_grid() && !selected && !self.on_name_cell(index, at) {
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

    pub fn drag_to(&mut self, at: (f32, f32)) {
        if let Some(mut drag) = self.file_drag.take() {
            drag.pointer = at;
            let travel = (at.0 - drag.start.0).abs().max((at.1 - drag.start.1).abs());
            if travel < DRAG_START {
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

    pub fn end_drag(&mut self, cancelled: bool) {
        if let Some(drag) = self.file_drag.take() {
            if cancelled {
                return;
            }
            if drag.dragging {
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
    pub(super) fn drag_push(&self, dt: f32) -> f32 {
        let pointer = match (&self.file_drag, &self.marquee) {
            (Some(drag), _) if drag.dragging => Some(drag.pointer),
            (_, Some(marquee)) if marquee.dragging => Some(marquee.current),
            _ => None,
        };
        let Some((x, y)) = pointer else {
            return 0.0;
        };
        let view = self.rows_view;
        let depth = if x < view[0] || view[3] <= 0.0 {
            0.0
        } else {
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

    pub(super) fn follow_drag(&mut self) {
        if let Some(mut drag) = self.file_drag.take() {
            self.aim_drag(&mut drag);
            self.file_drag = Some(drag);
        } else if let Some(marquee) = self.marquee.take() {
            if marquee.dragging {
                self.sweep_box(&marquee);
            }
            self.marquee = Some(marquee);
        }
    }
}
