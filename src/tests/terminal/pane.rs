//! The pane's own numbers: what a scroll splits into, the height it may take, and what a
//! drag through the minimum does, all without a running view.

use crate::term::{COLLAPSE_DRAG, MAX_HEIGHT, MIN_HEIGHT, Terminal, split};
use std::path::PathBuf;

#[test]
fn a_scroll_splits_into_whole_rows_and_a_fraction() {
    assert_eq!(split(0.0), (0, 0.0));
    assert_eq!(split(2.25), (2, 0.25));
    // Past the top and behind the bottom both settle on the ends, never on a negative row.
    assert_eq!(split(-3.0), (0, 0.0));
}

#[test]
fn the_fraction_is_the_shift_the_content_is_drawn_at() {
    let mut pane = Terminal::new();
    pane.line = 16.0;
    assert_eq!(pane.offset(), 0.0);
    let (rows, fraction) = split(3.5);
    assert_eq!((rows, fraction * pane.line), (3, 8.0));
}

/// The cursor blinks only while the program asked for it and the pane has the keyboard, so a
/// plain shell never wakes the frame loop for it.
#[test]
fn the_cursor_blinks_only_while_the_program_asked_and_the_pane_is_focused() {
    use std::time::{Duration, Instant};
    let mut pane = Terminal::new();
    pane.toggle(PathBuf::from("/tmp"));
    assert!(pane.cursor_on(), "a solid cursor is always drawn");
    assert_eq!(pane.blink_wait(), None, "and never wakes the frame loop");

    pane.frame.cursor_blink = true;
    let wait = pane
        .blink_wait()
        .expect("a blinking cursor is one wake away");
    assert!(wait <= Duration::from_millis(500), "within the first half");
    let at = Instant::now();
    assert!(pane.cursor_on(), "the first half is drawn");
    assert!(
        !pane.tick_blink(at + Duration::from_millis(200)),
        "nothing flips before the half is up"
    );
    assert!(
        pane.tick_blink(at + Duration::from_millis(600)),
        "then it flips"
    );
    assert!(!pane.cursor_on(), "and the cursor is off for it");
    assert!(
        pane.tick_blink(at + Duration::from_millis(1200)),
        "and back on"
    );
    assert!(pane.cursor_on());

    pane.focus = false;
    assert!(pane.cursor_on(), "an unfocused pane keeps it solid");
    assert_eq!(pane.blink_wait(), None, "and stops waking the loop");
}

#[test]
fn the_pane_toggles_and_keeps_its_height_in_range() {
    let mut pane = Terminal::new();
    assert!(!pane.is_open());
    pane.toggle(PathBuf::from("/tmp"));
    assert!(pane.is_open() && pane.focus, "opening focuses the shell");
    pane.toggle(PathBuf::from("/tmp"));
    assert!(!pane.is_open() && !pane.focus);

    pane.set_height(10.0);
    assert_eq!(pane.height(), MIN_HEIGHT);
    pane.set_height(5_000.0);
    assert_eq!(pane.height(), MAX_HEIGHT);
    pane.set_height(280.6);
    assert_eq!(pane.height(), 281.0);
    pane.set_height(f32::NAN);
    assert_eq!(
        pane.height(),
        281.0,
        "a broken drag leaves the height alone"
    );
}

#[test]
fn the_grid_is_sized_from_the_rect_and_the_cell() {
    let mut pane = Terminal::new();
    pane.line = 16.0;
    pane.layout([0.0, 0.0, 800.0, 320.0], 8.0);
    assert_eq!(pane.grid_size(), (100, 20));
    // A pane too small for one cell still asks for a usable grid.
    pane.layout([0.0, 0.0, 2.0, 2.0], 8.0);
    assert_eq!(pane.grid_size(), (2, 1));
}

#[test]
fn a_drag_at_the_minimum_does_not_flip_the_pane() {
    let dir = PathBuf::from("/tmp");
    let mut pane = Terminal::new();

    // Dragging up from a collapsed pane opens it once the drag reaches the minimum height, and
    // not a point before: the band under it is dead.
    pane.drag(MIN_HEIGHT - 1.0, dir.clone());
    assert!(!pane.is_open(), "a short drag does not open it");
    pane.drag(MIN_HEIGHT, dir.clone());
    assert!(pane.is_open(), "reaching the minimum opens it");
    assert_eq!(pane.height(), MIN_HEIGHT);
    assert!(!pane.focus, "a drag does not take the keyboard");

    // Back inside the band the pane stays as it is: the minimum is where the drag stops, not
    // where the pane collapses, so moving through it cannot flip the pane.
    for wanted in [MIN_HEIGHT - COLLAPSE_DRAG, MIN_HEIGHT - 1.0, MIN_HEIGHT] {
        pane.drag(wanted, dir.clone());
        assert!(pane.is_open(), "still open at {wanted}");
        assert_eq!(pane.height(), MIN_HEIGHT);
    }

    // Past the band it collapses; the same drag brings it back by moving up again, with no
    // new press. The dead band holds it collapsed until the drag reaches the minimum.
    pane.drag(MIN_HEIGHT - COLLAPSE_DRAG - 1.0, dir.clone());
    assert!(!pane.is_open());
    pane.drag(MIN_HEIGHT - 1.0, dir.clone());
    assert!(!pane.is_open(), "the band is dead on the way up too");
    pane.drag(MIN_HEIGHT, dir.clone());
    assert!(pane.is_open(), "the same drag reopens it");
    assert_eq!(pane.height(), MIN_HEIGHT);

    // And it keeps following the pointer once it is back.
    pane.drag(240.0, dir.clone());
    assert_eq!(pane.height(), 240.0);
    assert!(!pane.focus);
}

#[test]
fn a_drag_that_collapses_again_is_not_a_click() {
    let dir = PathBuf::from("/tmp");
    let mut pane = Terminal::new();

    // Down on the collapsed handle, then a drag that opens the pane: from here the gesture is a
    // resize, and its release must not be mistaken for the click that opens the pane.
    pane.begin_drag();
    assert!(!pane.did_open(), "a new gesture starts unshown");
    pane.drag(MIN_HEIGHT + 80.0, dir.clone());
    assert!(pane.is_open() && pane.did_open());

    // Dragged back down through the band it collapses again; the gesture has already shown the
    // pane, so it stays collapsed when the finger comes up.
    pane.drag(MIN_HEIGHT - COLLAPSE_DRAG - 1.0, dir.clone());
    assert!(!pane.is_open());
    assert!(
        pane.did_open(),
        "the gesture is still a resize, not a click"
    );

    // A fresh gesture starts unshown again, which is what makes a click a click.
    pane.begin_drag();
    assert!(!pane.did_open());
}
