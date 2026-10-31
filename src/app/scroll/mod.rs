//! The list's scroll: wheel inertia, overscroll past the ends, and the drag that pushes it. The
//! viewport stays authoritative, and each frame starts from the offset it reported. Effects
//! leave as [`Step`] values for `App` to hand to the runtime.

use std::mem;

use rune::{GesturePhase, WheelEvent};

// Per-second decay of the coast's speed; a larger value stops the rows sooner.
const SCROLL_DECAY: f32 = 3.5;

// Share of a notched wheel's delta that becomes coast speed.
const SCROLL_GAIN: f32 = 0.75;

// Fastest coast, in points per second.
const SCROLL_MAX_VELOCITY: f32 = 6000.0;

// Coast speed, in points per second, at which the rows come to rest.
const SCROLL_MIN_VELOCITY: f32 = 8.0;

// Seconds for a slower wheel speed to pull the coast down, so a flick's last frame still glides.
const SCROLL_SPEED_SMOOTH: f32 = 0.15;

// Per-second decay while the fingers rest, much quicker than the coast's.
const SCROLL_HOLD_DECAY: f32 = 12.0;

// Seconds of stillness after which lifting the fingers starts no glide.
const SCROLL_HOLD_LIMIT: f32 = 0.12;

// Seconds without a gesture end before the app stops believing the fingers are down.
const SCROLL_HELD_FORGET: f32 = 1.0;

// How far, in points, the rows can be pulled past the first or last row.
const MAX_OVERSCROLL: f32 = 72.0;

// Share of a wheel delta past an end that becomes overscroll.
const OVERSCROLL_GIVE: f32 = 0.45;

// Per-second spring rate that pulls overscroll back.
const OVERSCROLL_RETURN: f32 = 16.0;

#[derive(Clone, Copy)]
pub(super) struct Limits {
    pub max: f32,
    pub inertia: bool,
    pub bounce: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Step {
    pub scroll_to: Option<f32>,
    pub redraw: bool,
    pub again: bool,
}

impl Step {
    const NONE: Step = Step {
        scroll_to: None,
        redraw: false,
        again: false,
    };

    fn quiet(&self) -> bool {
        self.scroll_to.is_none() && !self.redraw && !self.again
    }
}

#[derive(Default)]
pub(super) struct Scroll {
    told: f32,
    request: Option<f32>,
    velocity: f32,
    push: f32,
    held: bool,
    hold: f32,
    step: f32,
    overshoot: f32,
    last_t: Option<f64>,
    since: f32,
    dt: f32,
}

impl Scroll {
    pub(super) fn current(&self) -> f32 {
        self.request.unwrap_or(self.told)
    }

    fn worst(&self, max: f32) -> f32 {
        let current = self.current();
        if current < 0.0 {
            -current
        } else if current > max {
            current - max
        } else {
            0.0
        }
    }

    pub(super) fn observe(&mut self, told: f32) {
        self.told = told;
        self.request = None;
    }

    pub(super) fn frame(&mut self, t: f64) -> f32 {
        let since = match self.last_t {
            Some(last) => (t - last).max(0.0) as f32,
            None => 0.0,
        };
        let dt = since.clamp(0.0, 0.05);
        self.since = since;
        self.dt = dt;
        self.last_t = Some(t);
        dt
    }

    pub(super) fn wheel(&mut self, wheel: &WheelEvent, limits: Limits) -> Option<Step> {
        if !limits.inertia && !limits.bounce {
            return None;
        }
        if wheel.momentum && limits.inertia {
            return Some(Step::NONE);
        }
        let mut out = Step::NONE;
        if !wheel.momentum && wheel.phase != GesturePhase::Moved {
            if wheel.phase == GesturePhase::Began {
                self.held = true;
                self.hold = 0.0;
                out = self.halt();
            } else {
                self.held = false;
                out.again = true;
            }
        }
        if wheel.dy == 0.0 || wheel.dx.abs() > wheel.dy.abs() {
            return (!out.quiet()).then_some(out);
        }
        let current = self.current() - self.step;
        self.step = 0.0;
        let max = limits.max;
        let bounce = limits.bounce && max > 0.0;
        let past = self.worst(max);
        let target = current - wheel.dy;
        let (offset, overshoot) = if target < 0.0 && bounce {
            (
                0.0,
                (self.overshoot.max(past) - target * OVERSCROLL_GIVE).min(MAX_OVERSCROLL),
            )
        } else if target > max && bounce {
            (
                max,
                (self.overshoot.min(-past) - (target - max) * OVERSCROLL_GIVE).max(-MAX_OVERSCROLL),
            )
        } else {
            (target.clamp(0.0, max), 0.0)
        };
        self.overshoot = overshoot;
        self.request = Some(offset);
        if offset != current {
            out.scroll_to = Some(offset);
            out.redraw = true;
        }
        if limits.inertia && !wheel.momentum {
            if wheel.precise {
                // Precise deltas are the fingers' own speed, which `push` turns into a coast on
                // lift. Adding the coast to each delta would run the rows ahead of them.
                self.push += wheel.dy;
            } else {
                self.velocity = (self.velocity - wheel.dy * SCROLL_GAIN)
                    .clamp(-SCROLL_MAX_VELOCITY, SCROLL_MAX_VELOCITY);
            }
        }
        out.again = true;
        Some(out)
    }

    pub(super) fn nudge(&mut self, points: f32, max: f32) -> Step {
        if points == 0.0 {
            return Step::NONE;
        }
        let current = self.current();
        let next = (current + points).clamp(0.0, max);
        self.request = Some(next);
        self.velocity = 0.0;
        let mut out = Step::NONE;
        if (next - current).abs() > 0.01 {
            out.scroll_to = Some(next);
            out.redraw = true;
            out.again = true;
        }
        out
    }

    pub(super) fn step(&mut self, limits: Limits) -> Step {
        let mut out = Step::NONE;
        let (since, dt) = (self.since, self.dt);
        self.step = 0.0;
        let current = self.current();
        let pushed = mem::take(&mut self.push);
        if dt > 0.0 && pushed != 0.0 {
            let speed = (-pushed / dt).clamp(-SCROLL_MAX_VELOCITY, SCROLL_MAX_VELOCITY);
            let rising =
                speed.abs() > self.velocity.abs() || speed.signum() != self.velocity.signum();
            if rising {
                self.velocity = speed;
            } else {
                let blend = 1.0 - (-dt / SCROLL_SPEED_SMOOTH).exp();
                self.velocity += (speed - self.velocity) * blend;
            }
        }
        if dt > 0.0 && self.held {
            self.hold = if pushed == 0.0 {
                self.hold + since
            } else {
                0.0
            };
            if pushed == 0.0 {
                self.velocity *= (-SCROLL_HOLD_DECAY * since).exp();
                if self.velocity.abs() < SCROLL_MIN_VELOCITY {
                    self.velocity = 0.0;
                }
            }
            if self.hold >= SCROLL_HOLD_LIMIT {
                self.velocity = 0.0;
            }
            if self.hold >= SCROLL_HELD_FORGET {
                self.held = false;
            }
        }
        if dt > 0.0 && self.overshoot != 0.0 {
            self.overshoot *= (-OVERSCROLL_RETURN * dt).exp();
            if self.overshoot.abs() < 0.5 {
                self.overshoot = 0.0;
            }
            out.redraw = true;
            out.again = true;
        }
        if dt > 0.0 && !self.held && self.velocity.abs() >= SCROLL_MIN_VELOCITY {
            let max = limits.max;
            let next = (current + self.velocity * dt).clamp(0.0, max);
            if (next - current).abs() > 0.01 {
                out.scroll_to = Some(next);
                self.request = Some(next);
                self.step = next - current;
                out.redraw = true;
            }
            let at_end =
                (next <= 0.0 && self.velocity < 0.0) || (next >= max && self.velocity > 0.0);
            if pushed == 0.0 {
                self.velocity *= (-SCROLL_DECAY * dt).exp();
            }
            if at_end || self.velocity.abs() < SCROLL_MIN_VELOCITY {
                self.velocity = 0.0;
            }
            out.again = true;
        } else if dt > 0.0 && pushed == 0.0 && !self.held {
            self.velocity *= (-SCROLL_DECAY * since).exp();
            if self.velocity.abs() < SCROLL_MIN_VELOCITY {
                self.velocity = 0.0;
            }
        }
        out
    }

    fn halt(&mut self) -> Step {
        self.velocity = 0.0;
        self.push = 0.0;
        let mut out = Step::NONE;
        if self.step != 0.0 {
            let back = self.current() - self.step;
            self.request = Some(back);
            self.step = 0.0;
            out.scroll_to = Some(back);
        }
        out
    }

    pub(super) fn stop(&mut self) {
        self.velocity = 0.0;
        self.push = 0.0;
        self.held = false;
        self.hold = 0.0;
        self.step = 0.0;
        self.overshoot = 0.0;
        self.request = None;
    }

    pub(super) fn stop_coast(&mut self) {
        self.velocity = 0.0;
        self.push = 0.0;
    }

    pub(super) fn stop_bounce(&mut self) {
        self.overshoot = 0.0;
    }

    pub(super) fn overshoot(&self) -> f32 {
        self.overshoot
    }
}

#[cfg(test)]
mod tests;
