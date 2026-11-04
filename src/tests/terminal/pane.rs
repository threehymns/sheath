//! The pane's own numbers: what a scroll splits into, the height it may take, and what a
//! drag through the minimum does, all without a running view.

use rune_tty::{COLLAPSE_DRAG, MAX_HEIGHT, MIN_HEIGHT, Terminal, split};
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

/// The search field opens with the keyboard in it, so the query is typed into the field and not
/// the shell, and closing it leaves nothing to draw.
#[test]
fn the_search_field_opens_focused_and_closes_clean() {
    let mut pane = Terminal::new();
    assert!(!pane.searching());
    pane.open_search();
    assert!(pane.searching());
    assert!(pane.search_field_focused());
    pane.search_text_mut().text = "needle".to_owned();
    assert_eq!(pane.search_text().text, "needle");
    pane.close_search();
    assert!(!pane.searching());
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

/// A press on the same cell soon after the one before it is the second click, and the third
/// the third: the pane turns those into a word and a line selection.
#[test]
fn a_press_in_the_same_cell_soon_after_is_a_double_click() {
    let mut pane = Terminal::new();
    assert_eq!(pane.click_count((3, 4)), 1);
    assert_eq!(pane.click_count((3, 4)), 2, "the same cell is the double");
    assert_eq!(pane.click_count((3, 4)), 3, "and the third the line");
    assert_eq!(
        pane.click_count((3, 4)),
        1,
        "a fourth press starts a new gesture"
    );
    assert_eq!(
        pane.click_count((4, 4)),
        1,
        "another cell begins another count"
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

/// The screen's layout box includes its horizontal padding, so the grid is sized from the space
/// the padding leaves: the columns it asks for end where the content ends, not at the pane's edge.
#[test]
fn the_grid_fits_inside_the_screen_s_padding() {
    let mut pane = Terminal::new();
    pane.cell = 8.0;
    pane.line = 16.0;
    crate::ui::terminal::layout_screen(&mut pane, [0.0, 0.0, 800.0, 320.0]);
    assert_eq!(
        pane.grid_size(),
        (98, 20),
        "12 points of padding leave 788 for whole cells"
    );
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

/// A selection made after the grid settled reaches the rows the view draws. A drag extends one
/// every cell the pointer crosses, and none of those moves arrives with a byte from the shell:
/// the wash has to come from the readback that follows the selection, not from new output.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_selection_reaches_the_frame_without_new_output() {
    use rune_tty::engine::Engine;
    use rune_tty::grid::Frame;

    let dir = tempfile::tempdir().unwrap();
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    let mut engine = Engine::spawn(
        dir.path(),
        20,
        4,
        (8.0, 16.0),
        2.0,
        rune_tty::theme(true),
        wake,
    )
    .expect("an engine");
    engine.feed(b"abc\r\ndef");
    let mut frame = Frame::default();
    engine.fill(&mut frame);
    assert_eq!(frame.lines[0].selection, None, "nothing is selected yet");

    assert!(
        !engine.begin_selection((0, 0), 1, false),
        "a click on its own selects nothing"
    );
    engine.fill(&mut frame);
    assert_eq!(
        frame.lines[0].selection, None,
        "the cell under a press is not washed"
    );
    engine.select_to((2, 0));
    engine.fill(&mut frame);
    assert_eq!(
        frame.lines[0].selection,
        Some((0, 2)),
        "the row the drag covers is drawn with its wash"
    );
    assert_eq!(
        frame.lines[1].selection, None,
        "and a row it does not cover is left alone"
    );

    // A press that dismisses the wash changes the rows too, so the frame has to be rebuilt for
    // it just as it is for the drag that made it.
    assert!(
        engine.begin_selection((0, 2), 2, false),
        "a double click selects a word"
    );
    assert!(
        engine.begin_selection((0, 0), 1, false),
        "the click that follows clears it"
    );
    engine.fill(&mut frame);
    assert_eq!(
        frame.lines[0].selection, None,
        "and the row is read again without the wash"
    );
    assert!(
        !engine.begin_selection((0, 0), 1, false),
        "a click with nothing to clear changes nothing"
    );
}

/// The pane's own frame follows the selection without waiting for the shell: a press and the
/// drag that widens it carry no output of their own, so the poll right after them is the only
/// thing that can put the wash on screen.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_selection_reaches_the_pane_frame_without_new_output() {
    let dir = tempfile::tempdir().unwrap();
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    let theme = rune_tty::theme(true);
    let mut pane = Terminal::new();
    // A laid-out screen, so the grid has the columns the drag reaches across.
    rune_tty::layout_screen(&mut pane, [0.0, 0.0, 400.0, 200.0]);
    pane.open(dir.path().to_path_buf());
    // Wait for the prompt's own output to stop: the poll after the selection must have nothing
    // to drain, or the frame it rebuilds could be the shell's doing rather than the gesture's.
    let mut settled = false;
    for _ in 0..200 {
        let drew = pane.poll(Some(&wake), theme);
        if !drew && !pane.frame.lines.is_empty() {
            settled = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(settled, "the shell's first frame settled");
    assert!(
        !pane.begin_selection((0, 0), 1, false),
        "one click selects nothing"
    );
    pane.poll(Some(&wake), theme);
    assert_eq!(
        pane.frame.lines[0].selection, None,
        "the click left the rows as it found them"
    );
    assert!(pane.extend_selection((2, 0)), "the drag selects from it");
    assert!(
        pane.frame_stale(),
        "the rows are behind the grid until a poll reads them"
    );
    // The poll refreshes the rows before the view is built from them, and says so: no output
    // arrived, so a window that only drew what the shell wrote would never draw this frame.
    assert!(
        pane.poll(Some(&wake), theme),
        "the poll asks for the frame the gesture changed"
    );
    assert!(
        !pane.frame_stale(),
        "and the rows it read are the ones it holds"
    );
    assert_eq!(
        pane.frame.lines[0].selection,
        Some((0, 2)),
        "the row the drag covers is read again without a keystroke"
    );

    // Escape dismisses the wash rather than reaching the shell, and the poll after it is what
    // clears the rows the view draws.
    use rune::{Key, KeyEvent, Mods};
    pane.focus = true;
    assert!(
        pane.key(&KeyEvent {
            key: Key::Escape,
            mods: Mods::default(),
        }),
        "the pane takes the key"
    );
    assert!(
        pane.frame_stale(),
        "the clear left rows the view has not drawn"
    );
    assert!(
        pane.poll(Some(&wake), theme),
        "and the clear asks for its frame too"
    );
    assert_eq!(
        pane.frame.lines[0].selection, None,
        "Escape cleared the selection"
    );
}
