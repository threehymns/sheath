//! The selection box a drag draws.

use super::helpers::*;
use rune::Mods;

#[test]
fn dragging_a_box_selects_the_rows_it_covers() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:alpha");
    drag_box(&mut rt, "f:file2.txt", Mods::default());
    assert_eq!(
        rt.state.selected_ids(),
        ["f:file2.txt", "f:file10.txt", "f:notes.md"],
        "a plain box replaces the selection with the rows it swept"
    );
    assert!(rt.state.marquee.is_none(), "the box ends with the button");
}

#[test]
fn a_shift_drag_adds_the_box_to_the_selection() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:alpha");
    drag_box(&mut rt, "f:file2.txt", shift());
    assert_eq!(
        rt.state.selected_ids(),
        ["f:alpha", "f:file2.txt", "f:file10.txt", "f:notes.md"],
        "Shift keeps what was selected and adds the box"
    );
}

#[test]
fn a_ctrl_drag_toggles_the_rows_in_the_box() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:file2.txt");
    drag_box(&mut rt, "f:file2.txt", command());
    assert_eq!(
        rt.state.selected_ids(),
        ["f:file10.txt", "f:notes.md"],
        "Cmd/Ctrl toggles the rows the box covers"
    );
}

#[test]
fn a_ctrl_shift_drag_keeps_the_selection_and_adds_the_box() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    click(&mut rt, "f:alpha");
    drag_box(&mut rt, "f:file2.txt", control_shift());
    assert_eq!(
        rt.state.selected_ids(),
        ["f:alpha", "f:file2.txt", "f:file10.txt", "f:notes.md"],
        "Ctrl+Shift+drag adds the box and keeps the rest"
    );
}

#[test]
fn a_press_without_moving_is_still_a_click() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    let (x, y) = rt.find("f:file2.txt").expect("the row is on screen");
    rt.pointer_down(x, y);
    rt.frame(0.05);
    assert!(
        rt.find("marquee").is_none(),
        "no box is drawn before the pointer moves"
    );
    rt.pointer_up(x, y);
    rt.frame(0.1);
    assert_eq!(rt.state.selected_ids(), ["f:file2.txt"]);
    assert!(rt.state.marquee.is_none());
}

#[test]
fn a_drag_draws_the_box_while_the_button_is_down() {
    let dir = fixture();
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    let (x0, y0) = rt.find("files-table").expect("the list is on screen");
    let (_, y1) = rt.find("f:notes.md").expect("the row is on screen");
    rt.pointer_down(x0, y0);
    rt.frame(0.05);
    rt.pointer_move(x0 + 20.0, y1);
    rt.frame(0.05);
    assert!(
        rt.find("marquee").is_some(),
        "the box is drawn while dragging"
    );
    rt.pointer_up(x0 + 20.0, y1);
    rt.frame(0.1);
    assert!(rt.find("marquee").is_none(), "and goes with the button");
}
