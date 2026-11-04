use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use rune::{ScrollPos, WheelEvent};

use crate::scroll::{Limits, Scroll, Step};

/// Every scroll area's physics, the offset each viewport last reported, and where it was drawn.
/// A view registers the areas it builds, so one that is not on screen costs nothing.
#[derive(Default)]
pub struct Areas {
    states: BTreeMap<&'static str, Scroll>,
    told: RefCell<BTreeMap<&'static str, Rc<Cell<ScrollPos>>>>,
    regions: RefCell<BTreeMap<&'static str, [f32; 4]>>,
    shown: RefCell<BTreeSet<&'static str>>,
}

impl Areas {
    fn told(&self, key: &'static str) -> Rc<Cell<ScrollPos>> {
        if let Some(cell) = self.told.borrow().get(key) {
            return cell.clone();
        }
        let cell = Rc::new(Cell::new(ScrollPos::default()));
        self.told.borrow_mut().insert(key, cell.clone());
        cell
    }

    /// The offset cell a viewport writes its position into; the view keeps it across frames.
    pub fn offset_cell(&self, key: &'static str) -> Rc<Cell<ScrollPos>> {
        self.told(key)
    }

    fn state(&mut self, key: &'static str) -> &mut Scroll {
        self.states.entry(key).or_default()
    }

    pub fn known(&self) -> Vec<&'static str> {
        self.told.borrow().keys().copied().collect()
    }

    // The areas this build drew, so a panel that closed stops pushing at once.
    pub fn begin_view(&self) {
        self.shown.borrow_mut().clear();
    }

    pub fn mark_shown(&self, key: &'static str) {
        self.shown.borrow_mut().insert(key);
    }

    pub fn set_region(&mut self, key: &'static str, rect: [f32; 4]) {
        self.regions.borrow_mut().insert(key, rect);
    }

    pub fn visible_regions(&self) -> Vec<(&'static str, [f32; 4])> {
        let shown = self.shown.borrow();
        let regions = self.regions.borrow();
        shown
            .iter()
            .filter_map(|key| regions.get(key).map(|rect| (*key, *rect)))
            .collect()
    }

    pub fn current(&self, key: &str) -> f32 {
        self.states.get(key).map_or(0.0, Scroll::current)
    }

    pub fn overshoot(&self, key: &str) -> f32 {
        self.states.get(key).map_or(0.0, Scroll::overshoot)
    }

    pub fn each(&mut self, f: impl Fn(&mut Scroll)) {
        for state in self.states.values_mut() {
            f(state);
        }
    }

    pub fn wheel(&mut self, key: &'static str, wheel: &WheelEvent, limits: Limits) -> Option<Step> {
        self.state(key).wheel(wheel, limits)
    }

    pub fn nudge(&mut self, key: &'static str, points: f32, max: f32) -> Step {
        self.state(key).nudge(points, max)
    }

    /// Starts one area's frame from the offset the viewport reported, and returns the elapsed
    /// time the drag maths needs.
    pub fn begin_frame(&mut self, key: &'static str, t: f64) -> f32 {
        let told = self.told(key).get().offset[1].max(0.0);
        let state = self.state(key);
        state.observe(told);
        state.frame(t)
    }

    /// The drag push and the physics for this frame, as the steps to apply.
    pub fn step_key(&mut self, key: &'static str, limits: Limits, push: f32) -> (Step, Step) {
        let state = self.state(key);
        let nudged = state.nudge(push, limits.max);
        let stepped = state.step(limits);
        (nudged, stepped)
    }

    pub fn stop(&mut self) {
        self.each(Scroll::stop);
    }

    pub fn stop_coasting(&mut self) {
        self.each(Scroll::stop_coast);
    }

    pub fn stop_bouncing(&mut self) {
        self.each(Scroll::stop_bounce);
    }
}
