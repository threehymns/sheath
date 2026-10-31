use super::helpers::*;
use rune::{Mods, PhysicalInput};

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
fn a_box_held_at_the_edge_stays_on_the_rows_it_started_on() {
    let dir = long_folder(60);
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    let [x, top, _, height] = rt.presented("files-table").expect("the list").0;
    let (x, from) = (x + 40.0, top + 120.0);
    let to = top + height - 4.0;
    let at_press = scroll_offset(&rt);
    rt.physical_input(PhysicalInput::Modifiers(Mods {
        alt: true,
        ..Mods::default()
    }));
    rt.pointer_down(x, from);
    rt.frame(0.05);
    rt.pointer_move(x, to);
    rt.frame(0.05);

    let before = scroll_offset(&rt);
    for _ in 0..2 {
        rt.frame(1.0 / 60.0);
    }
    let scrolled = scroll_offset(&rt) - at_press;
    assert!(
        scroll_offset(&rt) > before + 10.0,
        "the edge scrolled the rows: {before} -> {}",
        scroll_offset(&rt)
    );
    let [_, box_top, _, _] = rt.state.marquee_rect().expect("the box is drawn");
    let row_top = rt.presented("f:file04.txt").expect("the row").0[1];
    assert!(
        (box_top - row_top).abs() < 2.0,
        "the box's corner stays on the row it started on: {box_top} vs {row_top} at {scrolled} points scrolled"
    );
    assert!(
        rt.state.selection.contains("f:file04.txt"),
        "the row the box started on is still swept"
    );

    rt.pointer_up(x, to);
    rt.physical_input(PhysicalInput::Modifiers(Mods::default()));
    rt.frame(0.1);
}

#[test]
fn a_box_that_scrolls_a_screen_keeps_the_rows_that_left_it() {
    let dir = long_folder(60);
    let mut rt = runtime(app(&dir));
    rt.frame(0.0);

    let [x, top, _, height] = rt.presented("files-table").expect("the list").0;
    let (x, from) = (x + 40.0, top + 120.0);
    let to = top + height - 4.0;
    rt.physical_input(PhysicalInput::Modifiers(Mods {
        alt: true,
        ..Mods::default()
    }));
    rt.pointer_down(x, from);
    rt.frame(0.05);
    rt.pointer_move(x, to);
    rt.frame(0.05);

    for _ in 0..40 {
        rt.frame(1.0 / 60.0);
    }
    assert!(
        scroll_offset(&rt) > height,
        "the rows scrolled past a screen: {}",
        scroll_offset(&rt)
    );
    assert!(
        rt.state.selection.contains("f:file04.txt"),
        "the box still holds the row it started on, off screen"
    );
    assert!(
        !rt.state.selection.contains("f:file03.txt"),
        "the row above where the box started is not swept"
    );
    assert!(
        rt.state.selection_len() > 30,
        "the box swept more than one screen of rows: {}",
        rt.state.selection_len()
    );
    let [_, _, _, box_height] = rt.state.marquee_rect().expect("the box is drawn");
    assert!(
        box_height > height,
        "the box is as tall as the rows it spans, not the viewport it is clipped to: \
         {box_height} vs {height}"
    );

    rt.pointer_up(x, to);
    rt.physical_input(PhysicalInput::Modifiers(Mods::default()));
    rt.frame(0.1);
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
