//! The app's half of the list's scrolling: how far it can go, what this frame allows,
//! and handing the `scroll` module's steps to the runtime.

use std::time::Duration;

use rune::{Tx, WheelEvent};

use super::scroll::{Limits, Step};

use super::*;

/// [`App::poll`] as `AppSpec::poll` takes it: the per-frame step of the list's scrolling.
pub fn poll(s: &mut App, t: f64, tx: &mut Tx) -> bool {
    s.poll(t, tx)
}

impl App {
    /// How far the list is scrolled, in points: what the viewport last said it holds.
    pub(super) fn told_offset(&self) -> f32 {
        self.scroll_told.get().offset[1].max(0.0)
    }

    /// How far the list can scroll before its last row is at the bottom, in points.
    fn max_scroll(&self) -> f32 {
        (self.content_height() - self.rows_view[3]).max(0.0)
    }

    /// What this frame tells the scroll: how far it can go, and which of its behaviours are on.
    fn limits(&self) -> Limits {
        Limits {
            max: self.max_scroll(),
            inertia: self.config.scroll_inertia,
            bounce: self.config.overscroll,
        }
    }

    /// How far the rows are drawn past the list's ends, in points.
    pub fn overshoot(&self) -> f32 {
        self.scroll.overshoot()
    }

    /// Hands a step's effects to the runtime.
    fn apply(&self, step: Step, tx: &mut Tx) {
        if let Some(at) = step.scroll_to {
            tx.scroll_to_instant("files-table", 0.0, at);
        }
        if step.again {
            tx.wake_after(Duration::ZERO);
        }
    }

    /// A wheel over the file list. With inertia or overscroll on the list applies the delta
    /// itself: the rows follow the fingers and coast at their speed when they stop, and a delta
    /// past an end pulls the rows out of place until [`App::poll`] springs them back. False
    /// leaves the wheel to the viewport, so with both settings off the list scrolls as it did.
    pub fn list_wheel(&mut self, wheel: WheelEvent, tx: &mut Tx) -> bool {
        match self.scroll.wheel(&wheel, self.limits()) {
            Some(step) => {
                self.apply(step, tx);
                true
            }
            None => false,
        }
    }

    /// Steps the list's momentum and overscroll once per frame, asking for the next frame while
    /// either still moves. True when the rows moved, so the view draws them where they are.
    pub fn poll(&mut self, t: f64, tx: &mut Tx) -> bool {
        // A file the app did not write changed on disk: re-read it and apply what it says.
        let mut redraw = false;
        if self.store.poll() {
            self.apply_config();
            redraw = true;
        }
        // The rule form's Save follows the draft one frame behind a keystroke: typing lands at
        // once, so the catch-up here is a commit of its own, which can animate. Every other
        // control settles the form in the commit its press causes.
        if let Some(edit) = self.rule_edit.as_mut() {
            let dirty = edit.dirty();
            if edit.shown_dirty != dirty {
                edit.shown_dirty = dirty;
                redraw = true;
            }
        }
        // A command asked for a rule: wait one frame so the tab's rows are laid out, then put
        // the rule just under the viewport's top.
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
        let limits = self.limits();
        self.scroll.observe(self.told_offset());
        let dt = self.scroll.frame(t);
        let push = self.drag_push(dt);
        let nudged = self.scroll.nudge(push, limits.max);
        let stepped = self.scroll.step(limits);
        let redraw = redraw || nudged.redraw || stepped.redraw;
        self.apply(nudged, tx);
        self.apply(stepped, tx);
        self.follow_drag();
        redraw
    }
}
