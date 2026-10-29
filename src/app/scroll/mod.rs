//! The list's scroll: the wheel's inertia, the overscroll past its ends, and the drag that
//! pushes it. The viewport stays authoritative: every frame starts from the offset it told the
//! app, and this module only remembers what it has asked for within that frame. Effects leave
//! as [`Step`] values; `App` hands them to the runtime.

use std::mem;

use rune::{GesturePhase, WheelEvent};

/// How quickly the list's own scroll momentum decays, per second: the wheel's inertia.
const SCROLL_DECAY: f32 = 3.5;

/// Points per second of momentum a notch of a notched wheel leaves behind, as a nudge has no
/// speed of its own to carry.
const SCROLL_GAIN: f32 = 0.75;

/// The fastest the list coasts, in points per second. A hard flick is the one gesture that
/// should carry a long way, as a browser's fling does.
const SCROLL_MAX_VELOCITY: f32 = 6000.0;

/// Below this the momentum has stopped, in points per second.
const SCROLL_MIN_VELOCITY: f32 = 8.0;

/// How long, in seconds, a slower wheel speed takes to pull the coast down: long enough that a
/// flick's last light frame doesn't cut its glide short, short enough to follow the fingers.
const SCROLL_SPEED_SMOOTH: f32 = 0.15;

/// How quickly a resting gesture's speed fades, per second: several times the coast's, so a
/// hold before lifting takes the fling away almost at once.
const SCROLL_HOLD_DECAY: f32 = 12.0;

/// How long the fingers can rest on the trackpad, in seconds, before the speed they had is
/// gone: lifting them after that starts no glide, as a browser's fling doesn't.
const SCROLL_HOLD_LIMIT: f32 = 0.12;

/// How long a gesture can go without its end, in seconds, before the app stops believing the
/// fingers are still down: a platform that drops the end mustn't take inertia with it.
const SCROLL_HELD_FORGET: f32 = 1.0;

/// How far the list can be pulled past its first or last row, in points.
const MAX_OVERSCROLL: f32 = 72.0;

/// How much of a wheel delta past an end becomes overscroll.
const OVERSCROLL_GIVE: f32 = 0.45;

/// How quickly overscroll springs back, per second.
const OVERSCROLL_RETURN: f32 = 16.0;

/// How far the list can go, and which of its two behaviours are on: what the list around the
/// scroll tells it each frame, so the module never sees `App` or `Settings`.
#[derive(Clone, Copy)]
pub(super) struct Limits {
    pub max: f32,
    pub inertia: bool,
    pub bounce: bool,
}

/// What one entry point asks the app to do with the viewport.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Step {
    /// Land the viewport on this absolute offset, if it should move.
    pub scroll_to: Option<f32>,
    /// The rows moved without the viewport moving: draw again.
    pub redraw: bool,
    /// Step again at once.
    pub again: bool,
}

impl Step {
    /// Consumed, but with nothing asked for.
    const NONE: Step = Step {
        scroll_to: None,
        redraw: false,
        again: false,
    };

    /// Whether this step asks for nothing at all.
    fn quiet(&self) -> bool {
        self.scroll_to.is_none() && !self.redraw && !self.again
    }
}

/// The list's scroll state: what the viewport told it, what it has asked the viewport for this
/// frame, and the momentum and overscroll it carries between frames.
#[derive(Default)]
pub(super) struct Scroll {
    /// The viewport's offset, observed this frame.
    told: f32,
    /// What this frame has asked the viewport for.
    request: Option<f32>,
    /// The coast's speed, in points per second.
    velocity: f32,
    /// Precise wheel points since the last step: the fingers' speed.
    push: f32,
    /// A trackpad gesture has the fingers down.
    held: bool,
    /// How long the fingers have rested, in seconds.
    hold: f32,
    /// How far the last step moved, for [`Scroll::halt`]'s take-back.
    step: f32,
    /// How far the rows are drawn out of place.
    overshoot: f32,
    /// When the momentum was last stepped.
    last_t: Option<f64>,
    /// The real time since the last step, in seconds, before the step's clamp.
    since: f32,
    /// The time this frame steps: [`Scroll::since`], clamped to 0.05.
    dt: f32,
}

impl Scroll {
    /// The viewport's offset, or the one this frame has asked it for.
    pub(super) fn current(&self) -> f32 {
        self.request.unwrap_or(self.told)
    }

    /// How far the offset is past `max`'s ends, if at all.
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

    /// Starts a frame from the offset the viewport has told the app.
    pub(super) fn observe(&mut self, told: f32) {
        self.told = told;
        self.request = None;
    }

    /// How much time passed, and how much this frame steps: a window with nothing to draw runs
    /// no frames, so a rest between the last delta and the next frame is one long step. Resting
    /// on the trackpad is measured in the real time, not the stepped time.
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

    /// A wheel over the list. `None` leaves the wheel to the viewport; otherwise the step
    /// carries the delta, the bounce, or the fingers landing and lifting.
    pub(super) fn wheel(&mut self, wheel: &WheelEvent, limits: Limits) -> Option<Step> {
        if !limits.inertia && !limits.bounce {
            return None;
        }
        if wheel.momentum && limits.inertia {
            // The platform's coasting would double the list's own.
            return Some(Step::NONE);
        }
        let mut out = Step::NONE;
        // A trackpad gesture's phases are the fingers themselves: down stops the rows where
        // they are, even when they never moved, and lifting them hands the speed they had to
        // the coast. Momentum has phases of its own, which aren't a hand.
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
            // The phase still happened: fingers landing or lifting take effect even when the
            // delta itself belongs to the viewport.
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
                // A trackpad's deltas are the fingers' own motion, so the step turns them into
                // the speed the rows coast at once the fingers stop. Adding a coast to every
                // delta would run the rows ahead of them while they are still moving.
                self.push += wheel.dy;
            } else {
                // A notch is a jump with no speed behind it: it leaves a short glide.
                self.velocity = (self.velocity - wheel.dy * SCROLL_GAIN)
                    .clamp(-SCROLL_MAX_VELOCITY, SCROLL_MAX_VELOCITY);
            }
        }
        out.again = true;
        Some(out)
    }

    /// A drag held against the list's edge: moves the offset by `points` and takes the coast
    /// away, because a drag owns the list. Quiet when the pointer is not at an edge.
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

    /// Steps the list's momentum and overscroll once per frame, asking for the next frame
    /// while either still moves.
    pub(super) fn step(&mut self, limits: Limits) -> Step {
        let mut out = Step::NONE;
        let (since, dt) = (self.since, self.dt);
        self.step = 0.0;
        let current = self.current();
        // The wheel's deltas of the last frame were the fingers' speed, which the rows coast at
        // once they stop. A flick carries the fastest its fingers went, so a rising speed is
        // taken at once and only a falling one eases in.
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
        // Fingers resting on the trackpad are not a fling: once they have been in one place
        // long enough, lifting them starts nothing.
        if dt > 0.0 && self.held {
            self.hold = if pushed == 0.0 {
                self.hold + since
            } else {
                0.0
            };
            if pushed == 0.0 {
                // Resting fingers take the speed away quickly, not at the coast's rate.
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
        // Fingers on the trackpad carry the rows themselves; the speed is only told to the
        // coast when they lift, so holding still stops the list where it is.
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
            // While the fingers still drive the rows, their speed isn't the coast's to decay.
            if pushed == 0.0 {
                self.velocity *= (-SCROLL_DECAY * dt).exp();
            }
            if at_end || self.velocity.abs() < SCROLL_MIN_VELOCITY {
                self.velocity = 0.0;
            }
            out.again = true;
        } else if dt > 0.0 && pushed == 0.0 && !self.held {
            // No coast to run: the speed left over is the estimate, which fades while nothing
            // moves. A held gesture's estimate fades faster, above.
            self.velocity *= (-SCROLL_DECAY * since).exp();
            if self.velocity.abs() < SCROLL_MIN_VELOCITY {
                self.velocity = 0.0;
            }
        }
        out
    }

    /// Stops the coast dead, as fingers landing on the trackpad do: the step this frame already
    /// made is taken back, so the rows stay where they were drawn.
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

    /// Stops the wheel's momentum and any overscroll, as a folder change does.
    pub(super) fn stop(&mut self) {
        self.velocity = 0.0;
        self.push = 0.0;
        self.held = false;
        self.hold = 0.0;
        self.step = 0.0;
        self.overshoot = 0.0;
        self.request = None;
    }

    /// Stops the coast, as turning inertia off does.
    pub(super) fn stop_coast(&mut self) {
        self.velocity = 0.0;
        self.push = 0.0;
    }

    /// Forgets the overscroll, as turning overscroll off does.
    pub(super) fn stop_bounce(&mut self) {
        self.overshoot = 0.0;
    }

    /// How far the rows are drawn past the list's ends, in points.
    pub(super) fn overshoot(&self) -> f32 {
        self.overshoot
    }
}

#[cfg(test)]
mod tests;
