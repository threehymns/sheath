use rune::{GesturePhase, WheelEvent};

use super::*;

/// The viewport, as the tests drive it: it applies what a step asks for and reports where it
/// landed, the way Rune clamps the real one.
struct Viewport {
    at: f32,
    max: f32,
}

impl Viewport {
    fn new(max: f32) -> Viewport {
        Viewport { at: 0.0, max }
    }

    fn apply(&mut self, step: &Step) {
        if let Some(at) = step.scroll_to {
            self.at = at.clamp(0.0, self.max);
        }
    }
}

/// One simulated frame: the viewport tells the scroll where it is, the scroll steps, and the
/// viewport lands where it was asked to go.
fn frame(v: &mut Viewport, s: &mut Scroll, l: Limits, t: f64) -> Step {
    s.observe(v.at);
    s.frame(t);
    let step = s.step(l);
    v.apply(&step);
    step
}

/// The scroll's two behaviours on, with room for `max` points.
fn limits(max: f32) -> Limits {
    Limits {
        max,
        inertia: true,
        bounce: true,
    }
}

/// A precise wheel delta with the fingers moving, applied to the viewport.
fn wheel(s: &mut Scroll, v: &mut Viewport, l: Limits, dy: f32) {
    let event = WheelEvent::new(0.0, dy)
        .precise(true)
        .phase(GesturePhase::Moved);
    if let Some(step) = s.wheel(&event, l) {
        v.apply(&step);
    }
}

/// A trackpad gesture's phase, with an optional delta, applied to the viewport.
fn gesture(s: &mut Scroll, v: &mut Viewport, l: Limits, dy: f32, phase: GesturePhase) {
    let event = WheelEvent::new(0.0, dy).precise(true).phase(phase);
    if let Some(step) = s.wheel(&event, l) {
        v.apply(&step);
    }
}

/// The next frame's time, a sixtieth of a second on.
fn tick(t: &mut f64) -> f64 {
    *t += 1.0 / 60.0;
    *t
}

#[test]
fn a_hard_flick_carries_a_lot_further() {
    let l = limits(6000.0);
    let mut v = Viewport::new(l.max);
    let mut s = Scroll::default();
    let mut t = 0.0;
    frame(&mut v, &mut s, l, t);
    // A hard flick: 120 points in one frame, the fastest a hand moves the content.
    for _ in 0..4 {
        wheel(&mut s, &mut v, l, -30.0);
    }
    frame(&mut v, &mut s, l, tick(&mut t));
    let lifted = v.at;
    for _ in 0..240 {
        frame(&mut v, &mut s, l, tick(&mut t));
    }
    let coasted = v.at - lifted;
    assert!(
        coasted > 1200.0,
        "a hard flick carries {coasted} points, not a screenful"
    );
}

#[test]
fn fingers_on_the_trackpad_stop_the_coast_where_it_is() {
    let l = limits(6000.0);
    let mut v = Viewport::new(l.max);
    let mut s = Scroll::default();
    let mut t = 0.0;
    frame(&mut v, &mut s, l, t);
    for _ in 0..4 {
        wheel(&mut s, &mut v, l, -20.0);
    }
    frame(&mut v, &mut s, l, tick(&mut t));
    for _ in 0..3 {
        frame(&mut v, &mut s, l, tick(&mut t));
    }
    // Two fingers down, not moving: the rows stop at once, not after the coast runs out.
    let before = v.at;
    let began = WheelEvent::new(0.0, 0.0)
        .precise(true)
        .phase(GesturePhase::Began);
    let step = s
        .wheel(&began, l)
        .expect("fingers landing belong to the scroll");
    let back = step.scroll_to.expect("the coast's last step is taken back");
    assert!(back < before, "the rows go back to where they were drawn");
    v.apply(&step);
    let stopped = v.at;
    for _ in 0..10 {
        frame(&mut v, &mut s, l, tick(&mut t));
    }
    assert!(
        (v.at - stopped).abs() < 0.01,
        "the rows stopped under the fingers: {stopped} -> {}",
        v.at
    );
}

#[test]
fn fingers_that_never_lift_never_coast() {
    let l = limits(6000.0);
    let mut v = Viewport::new(l.max);
    let mut s = Scroll::default();
    let mut t = 0.0;
    frame(&mut v, &mut s, l, t);
    // A gesture whose fingers stay down: the rows go with them, and stop with them.
    for frame_index in 0..4 {
        let phase = if frame_index == 0 {
            GesturePhase::Began
        } else {
            GesturePhase::Moved
        };
        gesture(&mut s, &mut v, l, -20.0, phase);
        frame(&mut v, &mut s, l, tick(&mut t));
    }
    let held = v.at;
    for _ in 0..3 {
        frame(&mut v, &mut s, l, tick(&mut t));
    }
    assert!(
        (v.at - held).abs() < 0.01,
        "a held gesture doesn't coast: {held} -> {}",
        v.at
    );
    // Lifting them right away hands the speed they had to the coast.
    gesture(&mut s, &mut v, l, 0.0, GesturePhase::Ended);
    frame(&mut v, &mut s, l, tick(&mut t));
    for _ in 0..5 {
        frame(&mut v, &mut s, l, tick(&mut t));
    }
    assert!(
        v.at > held + 10.0,
        "lifting the fingers starts the coast: {held} -> {}",
        v.at
    );
}

#[test]
fn resting_the_fingers_clears_the_inertia() {
    let l = limits(6000.0);
    let mut v = Viewport::new(l.max);
    let mut s = Scroll::default();
    let mut t = 0.0;
    frame(&mut v, &mut s, l, t);
    for frame_index in 0..4 {
        let phase = if frame_index == 0 {
            GesturePhase::Began
        } else {
            GesturePhase::Moved
        };
        gesture(&mut s, &mut v, l, -20.0, phase);
        frame(&mut v, &mut s, l, tick(&mut t));
    }
    let held = v.at;
    // A second with the fingers in one place: nothing is left to fling when they lift.
    for _ in 0..60 {
        frame(&mut v, &mut s, l, tick(&mut t));
    }
    gesture(&mut s, &mut v, l, 0.0, GesturePhase::Ended);
    frame(&mut v, &mut s, l, tick(&mut t));
    for _ in 0..10 {
        frame(&mut v, &mut s, l, tick(&mut t));
    }
    assert!(
        (v.at - held).abs() < 0.01,
        "resting the fingers clears the inertia: {held} -> {}",
        v.at
    );
}

/// How far the rows glide after lifting the fingers `hold` frames after the last delta.
fn glide_after_hold(hold: usize) -> f32 {
    let l = limits(6000.0);
    let mut v = Viewport::new(l.max);
    let mut s = Scroll::default();
    let mut t = 0.0;
    frame(&mut v, &mut s, l, t);
    for frame_index in 0..4 {
        let phase = if frame_index == 0 {
            GesturePhase::Began
        } else {
            GesturePhase::Moved
        };
        gesture(&mut s, &mut v, l, -20.0, phase);
        frame(&mut v, &mut s, l, tick(&mut t));
    }
    for _ in 0..hold {
        frame(&mut v, &mut s, l, tick(&mut t));
    }
    gesture(&mut s, &mut v, l, 0.0, GesturePhase::Ended);
    frame(&mut v, &mut s, l, tick(&mut t));
    let lifted = v.at;
    for _ in 0..120 {
        frame(&mut v, &mut s, l, tick(&mut t));
    }
    v.at - lifted
}

#[test]
fn resting_before_lifting_takes_the_speed_away_fast() {
    let quick = glide_after_hold(0);
    // A few frames of stillness, well inside the hold limit: it shouldn't glide nearly as far.
    let resting = glide_after_hold(5);
    assert!(quick > 100.0, "lifting at once carries {quick} points");
    assert!(
        resting < quick * 0.6,
        "resting takes the speed away: {quick} -> {resting}"
    );
}

#[test]
fn a_pause_with_no_frames_still_clears_the_inertia() {
    let l = limits(6000.0);
    let mut v = Viewport::new(l.max);
    let mut s = Scroll::default();
    let mut t = 0.0;
    frame(&mut v, &mut s, l, t);
    for frame_index in 0..4 {
        let phase = if frame_index == 0 {
            GesturePhase::Began
        } else {
            GesturePhase::Moved
        };
        gesture(&mut s, &mut v, l, -20.0, phase);
        frame(&mut v, &mut s, l, tick(&mut t));
    }
    let held = v.at;
    // A window with nothing to draw runs no frames: two seconds of resting arrive as one step.
    t += 2.0;
    frame(&mut v, &mut s, l, t);
    gesture(&mut s, &mut v, l, 0.0, GesturePhase::Ended);
    frame(&mut v, &mut s, l, tick(&mut t));
    for _ in 0..10 {
        frame(&mut v, &mut s, l, tick(&mut t));
    }
    assert!(
        (v.at - held).abs() < 0.01,
        "a long rest clears the inertia: {held} -> {}",
        v.at
    );
}

#[test]
fn a_gesture_that_never_reports_its_end_still_lets_inertia_run() {
    let l = limits(6000.0);
    let mut v = Viewport::new(l.max);
    let mut s = Scroll::default();
    let mut t = 0.0;
    frame(&mut v, &mut s, l, t);
    // A gesture whose end never comes: Wayland doesn't promise one for every device.
    for frame_index in 0..4 {
        let phase = if frame_index == 0 {
            GesturePhase::Began
        } else {
            GesturePhase::Moved
        };
        gesture(&mut s, &mut v, l, -20.0, phase);
        frame(&mut v, &mut s, l, tick(&mut t));
    }
    for _ in 0..90 {
        frame(&mut v, &mut s, l, tick(&mut t));
    }
    // The next delta counts as a fresh gesture, and lifting it starts a glide.
    wheel(&mut s, &mut v, l, -20.0);
    frame(&mut v, &mut s, l, tick(&mut t));
    gesture(&mut s, &mut v, l, 0.0, GesturePhase::Ended);
    frame(&mut v, &mut s, l, tick(&mut t));
    let lifted = v.at;
    for _ in 0..10 {
        frame(&mut v, &mut s, l, tick(&mut t));
    }
    assert!(
        v.at > lifted + 5.0,
        "inertia still runs after a lost gesture end: {lifted} -> {}",
        v.at
    );
}
