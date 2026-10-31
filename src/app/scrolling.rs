use std::time::Duration;

use rune::{Tx, WheelEvent};

use super::scroll::{Limits, Step};

use super::*;

pub fn poll(s: &mut App, t: f64, tx: &mut Tx) -> bool {
    s.poll(t, tx)
}

impl App {
    pub(super) fn told_offset(&self) -> f32 {
        self.scroll_told.get().offset[1].max(0.0)
    }

    fn max_scroll(&self) -> f32 {
        (self.content_height() - self.rows_view[3]).max(0.0)
    }

    fn limits(&self) -> Limits {
        Limits {
            max: self.max_scroll(),
            inertia: self.config.scroll_inertia,
            bounce: self.config.overscroll,
        }
    }

    pub fn overshoot(&self) -> f32 {
        self.scroll.overshoot()
    }

    fn apply(&self, step: Step, tx: &mut Tx) {
        if let Some(at) = step.scroll_to {
            tx.scroll_to_instant("files-table", 0.0, at);
        }
        if step.again {
            tx.wake_after(Duration::ZERO);
        }
    }

    pub fn list_wheel(&mut self, wheel: WheelEvent, tx: &mut Tx) -> bool {
        match self.scroll.wheel(&wheel, self.limits()) {
            Some(step) => {
                self.apply(step, tx);
                true
            }
            None => false,
        }
    }

    pub fn poll(&mut self, t: f64, tx: &mut Tx) -> bool {
        #[cfg(test)]
        self.settle_scan();
        let mut redraw = self.land_scan();
        if let Some(id) = self.pending_reveal.take() {
            tx.scroll_into_view_in("files-table", id);
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
