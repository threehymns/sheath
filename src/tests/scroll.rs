use super::helpers::*;
use crate::app::App;
use rune::{Runtime, WheelEvent};

#[test]
fn a_wheel_leaves_the_list_coasting_when_inertia_is_on() {
    let dir = long_folder(60);
    let mut rt = runtime(app(&dir));
    rt.state.config.scroll_inertia = true;
    rt.frame(0.0);
    let (x, y) = rt.find("files-table").expect("the list is on screen");
    rt.wheel_event(WheelEvent::new(0.0, -120.0).at(x, y));
    rt.frame(1.0 / 60.0);
    let landed = rt.find("f:file08.txt").expect("the eighth row").1;
    for _ in 0..4 {
        rt.frame(1.0 / 60.0);
    }
    let coasted = rt.find("f:file08.txt").expect("the eighth row").1;
    assert!(
        coasted < landed - 0.5,
        "the rows kept moving after the wheel: {landed} -> {coasted}"
    );
}

#[test]
fn a_fast_trackpad_scroll_never_jumps_back() {
    let dir = long_folder(60);
    let mut rt = runtime(app(&dir));
    rt.state.config.overscroll = true;
    rt.frame(0.0);
    let at = trackpad_burst(&mut rt, 8);
    for pair in at.windows(2) {
        let moved = pair[1] - pair[0];
        assert!(
            (moved - 36.0).abs() < 0.5,
            "a frame moved {moved}, not the wheel's 36: {at:?}"
        );
    }
}

#[test]
fn inertia_carries_the_fingers_speed_without_outrunning_them() {
    let dir = long_folder(60);
    let mut rt = runtime(app(&dir));
    rt.state.config.scroll_inertia = true;
    rt.frame(0.0);
    let at = trackpad_burst(&mut rt, 8);
    for pair in at.windows(2) {
        let moved = pair[1] - pair[0];
        assert!(
            (moved - 36.0).abs() < 0.5,
            "a frame moved {moved}, not the wheel's 36: {at:?}"
        );
    }

    let lifted = at[at.len() - 1];
    let mut coast = Vec::new();
    for _ in 0..120 {
        rt.frame(1.0 / 60.0);
        coast.push(scroll_offset(&rt));
    }
    let steps: Vec<f32> = coast.windows(2).map(|pair| pair[1] - pair[0]).collect();
    assert!(
        steps[0] > 15.0,
        "the rows keep the finger's speed: {steps:?}"
    );
    for step in &steps {
        assert!(
            *step <= 36.1,
            "the coast never runs ahead of the fingers: {steps:?}"
        );
    }
    for pair in steps.windows(2) {
        assert!(pair[1] <= pair[0] + 0.01, "the coast only slows: {steps:?}");
    }
    let coasted = coast[coast.len() - 1] - lifted;
    assert!(coasted > 100.0, "the rows coast on for {coasted} points");
    assert!(
        steps[steps.len() - 1] < 0.5,
        "the rows come to rest: {steps:?}"
    );
}

#[test]
fn without_inertia_the_list_stops_with_the_wheel() {
    let dir = long_folder(60);
    let mut rt = runtime(app(&dir));
    rt.state.config.scroll_inertia = false;
    rt.frame(0.0);
    let (x, y) = rt.find("files-table").expect("the list is on screen");
    rt.wheel_event(WheelEvent::new(0.0, -120.0).at(x, y));
    rt.frame(1.0 / 60.0);
    let landed = rt.find("f:file08.txt").expect("the eighth row").1;
    for _ in 0..4 {
        rt.frame(1.0 / 60.0);
    }
    let stayed = rt.find("f:file08.txt").expect("the eighth row").1;
    assert_eq!(landed, stayed, "with the setting off the list stops");
}

#[test]
fn overscroll_pulls_the_rows_past_the_top_and_springs_them_back() {
    let dir = long_folder(60);
    let mut rt = runtime(app(&dir));
    rt.state.config.overscroll = true;
    rt.frame(0.0);
    let row_top = |rt: &Runtime<App>| {
        rt.elements()
            .into_iter()
            .find(|e| e.key.as_deref() == Some("f:file00.txt"))
            .expect("the first row is on screen")
            .rect[1]
    };
    let before = row_top(&rt);
    let (x, y) = rt.find("files-table").expect("the list is on screen");
    rt.wheel_event(WheelEvent::new(0.0, 80.0).at(x, y));
    rt.frame(1.0 / 60.0);
    assert!(
        rt.state.overshoot("files-table") > 1.0,
        "the wheel at the top pulled the rows down: {}",
        rt.state.overshoot("files-table")
    );
    assert!(
        row_top(&rt) > before + 1.0,
        "the rows are drawn out of place, not the viewport"
    );
    for _ in 0..60 {
        rt.frame(1.0 / 60.0);
    }
    assert_eq!(
        rt.state.overshoot("files-table"),
        0.0,
        "the rows spring back"
    );
}

#[test]
fn the_command_bar_overscrolls_like_the_list() {
    let dir = long_folder(10);
    let mut rt = runtime(app(&dir));
    rt.state.config.overscroll = true;
    rt.frame(0.0);
    rt.key(command_bar_event(&rt.state, false));
    rt.frame(0.1);
    let (x, y) = rt
        .find("command-scroll")
        .expect("the commands are on screen");
    rt.wheel_event(WheelEvent::new(0.0, 80.0).at(x, y));
    rt.frame(1.0 / 60.0);
    assert!(
        rt.state.overshoot("command-scroll") > 1.0,
        "the wheel at the top pulls the commands down: {}",
        rt.state.overshoot("command-scroll")
    );
    for _ in 0..60 {
        rt.frame(1.0 / 60.0);
    }
    assert_eq!(
        rt.state.overshoot("command-scroll"),
        0.0,
        "the commands spring back"
    );
}

#[test]
fn without_overscroll_an_end_is_hard() {
    let dir = long_folder(60);
    let mut rt = runtime(app(&dir));
    rt.state.config.overscroll = false;
    rt.frame(0.0);
    let (x, y) = rt.find("files-table").expect("the list is on screen");
    rt.wheel_event(WheelEvent::new(0.0, 80.0).at(x, y));
    rt.frame(1.0 / 60.0);
    assert_eq!(rt.state.overshoot("files-table"), 0.0);
}
