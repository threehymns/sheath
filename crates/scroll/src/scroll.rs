//! A scroll area's scroll: wheel inertia, overscroll past the ends, and the drag that pushes the
//! list. The viewport stays authoritative, and each frame starts from the offset it reported.
//! Effects leave as [`Step`] values for `App` to hand to the runtime.

use std::mem;

use rune::{GesturePhase, WheelEvent};

// Per-second decay of the coast's speed; UIKit's normal deceleration rate (0.998 a
// millisecond) works out to about 2, and a larger value stops the rows sooner.
const SCROLL_DECAY: f32 = 2.0;

// Share of a notched wheel's delta that becomes coast speed.
const SCROLL_GAIN: f32 = 0.75;

// Fastest measured flick speed and notched wheel coast, in points per second; a sustained
// fast flick adds the boost below on top.
const SCROLL_MAX_VELOCITY: f32 = 12000.0;

// Coast speed, in points per second, at which the rows come to rest.
const SCROLL_MIN_VELOCITY: f32 = 8.0;

// Frames a flick's speed must stay fast through for it to gain on release.
const SCROLL_BOOST_FRAMES: usize = 3;

// Speeds, in points per second, between which a sustained flick gains the full boost share.
const SCROLL_BOOST_FROM: f32 = 3000.0;
const SCROLL_BOOST_TO: f32 = 6000.0;

// Share of the release speed a fully sustained fast flick adds to its coast.
const SCROLL_BOOST_SHARE: f32 = 0.5;

// Share of a stroke's peak speed below which its last frames mean the fingers stopped.
const SCROLL_STOP_SHARE: f32 = 0.2;

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
pub struct Limits {
    pub max: f32,
    pub inertia: bool,
    pub bounce: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Step {
    pub scroll_to: Option<f32>,
    pub redraw: bool,
    pub again: bool,
}

impl Step {
    pub const NONE: Step = Step {
        scroll_to: None,
        redraw: false,
        again: false,
    };

    fn quiet(&self) -> bool {
        self.scroll_to.is_none() && !self.redraw && !self.again
    }
}

#[derive(Default)]
pub struct Scroll {
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
    stroke: [f32; SCROLL_BOOST_FRAMES],
    stroke_at: usize,
    stroke_seen: usize,
    stroke_peak: f32,
    pushing: bool,
}

impl Scroll {
    pub fn current(&self) -> f32 {
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

    pub fn observe(&mut self, told: f32) {
        self.told = told;
        self.request = None;
    }

    pub fn frame(&mut self, t: f64) -> f32 {
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

    pub fn wheel(&mut self, wheel: &WheelEvent, limits: Limits) -> Option<Step> {
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
                self.pushing = false;
            }
        }
        out.again = true;
        Some(out)
    }

    pub fn nudge(&mut self, points: f32, max: f32) -> Step {
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

    pub fn step(&mut self, limits: Limits) -> Step {
        let mut out = Step::NONE;
        let (since, dt) = (self.since, self.dt);
        self.step = 0.0;
        let current = self.current();
        let pushed = mem::take(&mut self.push);
        if dt > 0.0 && pushed != 0.0 {
            // The fingers' speed this frame is the speed they would lift at. Keeping an earlier
            // frame's speed would carry momentum through a slowdown the fingers already made.
            let speed = (-pushed / dt).clamp(-SCROLL_MAX_VELOCITY, SCROLL_MAX_VELOCITY);
            if !self.pushing {
                self.stroke = [0.0; SCROLL_BOOST_FRAMES];
                self.stroke_at = 0;
                self.stroke_seen = 0;
                self.stroke_peak = 0.0;
                self.pushing = true;
            }
            self.velocity = speed;
            self.stroke[self.stroke_at] = speed.abs();
            self.stroke_at = (self.stroke_at + 1) % SCROLL_BOOST_FRAMES;
            self.stroke_seen = (self.stroke_seen + 1).min(SCROLL_BOOST_FRAMES);
            self.stroke_peak = self.stroke_peak.max(speed.abs());
        } else if dt > 0.0 && self.pushing {
            // The stroke ended: launch at the speed of its last few frames, so one small final
            // delta doesn't erase a flick but a sustained slowdown does.
            self.pushing = false;
            let sign = self.velocity.signum();
            self.velocity = sign * self.stroke_release();
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
        // A frame with its own precise deltas is still the fingers moving; the coast starts
        // once they stop delivering, at the speed of the last frame that did.
        if dt > 0.0 && pushed == 0.0 && !self.held && self.velocity.abs() >= SCROLL_MIN_VELOCITY {
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

    // The middle of the last few frame speeds: one slow frame doesn't erase a flick, while a
    // slowdown that lasts through the release does.
    fn stroke_recent(&self) -> f32 {
        if self.stroke_seen == 0 {
            return 0.0;
        }
        if self.stroke_seen < SCROLL_BOOST_FRAMES {
            return self.stroke[(self.stroke_at + SCROLL_BOOST_FRAMES - 1) % SCROLL_BOOST_FRAMES];
        }
        let mut speeds = self.stroke;
        speeds.sort_by(f32::total_cmp);
        speeds[SCROLL_BOOST_FRAMES / 2]
    }

    fn stroke_boost(&self, recent: f32) -> f32 {
        if self.stroke_seen < SCROLL_BOOST_FRAMES {
            return 1.0;
        }
        let share =
            ((recent - SCROLL_BOOST_FROM) / (SCROLL_BOOST_TO - SCROLL_BOOST_FROM)).clamp(0.0, 1.0);
        1.0 + share * SCROLL_BOOST_SHARE
    }

    // The speed to launch a coast at: none when the last frames are a small share of the
    // stroke's peak, else the recent speed with the sustained-flick boost on top.
    fn stroke_release(&self) -> f32 {
        let recent = self.stroke_recent();
        if recent < self.stroke_peak * SCROLL_STOP_SHARE {
            return 0.0;
        }
        recent * self.stroke_boost(recent)
    }

    fn halt(&mut self) -> Step {
        self.velocity = 0.0;
        self.push = 0.0;
        self.pushing = false;
        self.stroke_seen = 0;
        self.stroke_peak = 0.0;
        let mut out = Step::NONE;
        if self.step != 0.0 {
            let back = self.current() - self.step;
            self.request = Some(back);
            self.step = 0.0;
            out.scroll_to = Some(back);
        }
        out
    }

    pub fn stop(&mut self) {
        self.velocity = 0.0;
        self.push = 0.0;
        self.pushing = false;
        self.stroke_seen = 0;
        self.stroke_peak = 0.0;
        self.held = false;
        self.hold = 0.0;
        self.step = 0.0;
        self.overshoot = 0.0;
        self.request = None;
    }

    pub fn stop_coast(&mut self) {
        self.velocity = 0.0;
        self.push = 0.0;
        self.pushing = false;
        self.stroke_seen = 0;
        self.stroke_peak = 0.0;
    }

    pub fn stop_bounce(&mut self) {
        self.overshoot = 0.0;
    }

    pub fn overshoot(&self) -> f32 {
        self.overshoot
    }
}
