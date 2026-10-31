use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::rc::Rc;
use std::time::Duration;

use rune::{ScrollPos, Tx, WheelEvent};

use super::scroll::{Limits, Scroll, Step};

use super::*;

// The file table and the grid share this viewport, and the drag maths works from its offset.
pub const FILES: &str = "files-table";

// Every scroll area's physics, and the offset each viewport last reported. A view registers an
// area when it builds the viewport, so one that is not on screen costs nothing.
#[derive(Default)]
pub(super) struct Scrolls {
    states: BTreeMap<&'static str, Scroll>,
    told: RefCell<BTreeMap<&'static str, Rc<Cell<ScrollPos>>>>,
}

impl Scrolls {
    fn told(&self, key: &'static str) -> Rc<Cell<ScrollPos>> {
        if let Some(cell) = self.told.borrow().get(key) {
            return cell.clone();
        }
        let cell = Rc::new(Cell::new(ScrollPos::default()));
        self.told.borrow_mut().insert(key, cell.clone());
        cell
    }

    fn state(&mut self, key: &'static str) -> &mut Scroll {
        self.states.entry(key).or_default()
    }

    fn seen(&self) -> Vec<&'static str> {
        self.told.borrow().keys().copied().collect()
    }

    fn current(&self, key: &str) -> f32 {
        self.states.get(key).map_or(0.0, Scroll::current)
    }

    fn overshoot(&self, key: &str) -> f32 {
        self.states.get(key).map_or(0.0, Scroll::overshoot)
    }

    fn each(&mut self, f: impl Fn(&mut Scroll)) {
        for state in self.states.values_mut() {
            f(state);
        }
    }
}

pub fn poll(s: &mut App, t: f64, tx: &mut Tx) -> bool {
    s.poll(t, tx)
}

impl App {
    // The offset cell a viewport writes its position into; the view keeps it across frames.
    pub fn told(&self, key: &'static str) -> Rc<Cell<ScrollPos>> {
        self.scrolls.told(key)
    }

    fn told_offset_of(&self, key: &'static str) -> f32 {
        self.told(key).get().offset[1].max(0.0)
    }

    pub(super) fn told_offset(&self) -> f32 {
        self.told_offset_of(FILES)
    }

    // The viewport reports its own extent, so an area needs no content maths of its own.
    fn limits(&self, key: &'static str) -> Limits {
        Limits {
            max: self.told(key).get().max[1].max(0.0),
            inertia: self.config.scroll_inertia,
            bounce: self.config.overscroll,
        }
    }

    pub fn overshoot(&self, key: &str) -> f32 {
        self.scrolls.overshoot(key)
    }

    // The list's offset, which the drag and marquee maths work from.
    pub(super) fn list_scroll(&self) -> f32 {
        self.scrolls.current(FILES)
    }

    fn apply(&self, key: &'static str, step: Step, tx: &mut Tx) {
        if let Some(at) = step.scroll_to {
            tx.scroll_to_instant(key, 0.0, at);
        }
        if step.again {
            tx.wake_after(Duration::ZERO);
        }
    }

    // A viewport's wheel: takes the delta when a setting asks for it, and returns false so Rune
    // scrolls natively when both are off.
    pub fn area_wheel(&mut self, key: &'static str, wheel: WheelEvent, tx: &mut Tx) -> bool {
        let limits = self.limits(key);
        let step = self.scrolls.state(key).wheel(&wheel, limits);
        match step {
            Some(step) => {
                self.apply(key, step, tx);
                true
            }
            None => false,
        }
    }

    pub(super) fn stop_scrolling(&mut self) {
        self.scrolls.each(Scroll::stop);
    }

    pub(super) fn stop_coasting(&mut self) {
        self.scrolls.each(Scroll::stop_coast);
    }

    pub(super) fn stop_bouncing(&mut self) {
        self.scrolls.each(Scroll::stop_bounce);
    }

    pub fn poll(&mut self, t: f64, tx: &mut Tx) -> bool {
        #[cfg(test)]
        self.settle_scan();
        let mut redraw = self.land_scan();
        if let Some(id) = self.pending_reveal.take() {
            tx.scroll_into_view_in(FILES, id);
        }
        if self.store.poll() {
            self.apply_config();
            redraw = true;
        }
        if self.watch.poll() {
            self.reload();
            redraw = true;
        }
        if !self.notices.is_empty() {
            self.notices.retain(|notice| !notice.expired());
            redraw = true;
        }
        if let Some(edit) = self.rule_edit.as_mut() {
            let dirty = edit.dirty();
            if edit.shown_dirty != dirty {
                edit.shown_dirty = dirty;
                redraw = true;
            }
        }
        if let Some((key, wait)) = self.rule_reveal.clone() {
            if wait == 0 {
                self.rule_reveal = None;
                if self.settings == Some(SettingsTab::Rules) {
                    tx.scroll_key_to(key, 8.0);
                }
            } else {
                self.rule_reveal = Some((key, wait - 1));
                redraw = true;
            }
        }
        for key in self.scrolls.seen() {
            let told = self.told_offset_of(key);
            let limits = self.limits(key);
            let dt = {
                let state = self.scrolls.state(key);
                state.observe(told);
                state.frame(t)
            };
            // Only the file list pushes its rows from a drag; the other areas have no drag.
            let push = if key == FILES {
                self.drag_push(dt)
            } else {
                0.0
            };
            let (nudged, stepped) = {
                let state = self.scrolls.state(key);
                (state.nudge(push, limits.max), state.step(limits))
            };
            redraw = redraw || nudged.redraw || stepped.redraw;
            self.apply(key, nudged, tx);
            self.apply(key, stepped, tx);
        }
        self.follow_drag();
        redraw
    }
}
