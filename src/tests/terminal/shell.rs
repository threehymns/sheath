//! The whole native path: a shell on a PTY, libghostty parsing what it writes, and a frame the
//! pane can draw. It is the one test that proves the pieces are wired together.

// The whole native path: a shell on a PTY, libghostty parsing what it writes, and a frame the
// pane can draw. It is the one test that proves the pieces are wired together.

/// A program that captured the mouse gets the wheel as buttons 4 and 5, and the release of the
/// button it was pressing, in the SGR encoding it asked for: the path `less`, `vim` and `htop`
/// read. Nothing is reported before it asks.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_mouse_capturing_program_gets_the_wheel_and_the_button_that_ended() {
    use ghostty_vt::mouse::{Action, Button};
    use rune_tty::engine::Engine;
    let dir = tempfile::tempdir().unwrap();
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    let mut engine = Engine::spawn(
        dir.path(),
        40,
        10,
        (8.0, 16.0),
        2.0,
        rune_tty::theme(true),
        wake,
    )
    .expect("a shell on a pty");
    assert!(
        engine
            .encode_mouse(Action::Press, Some(Button::Four), (3, 2), false)
            .is_empty(),
        "a program that did not ask for the mouse hears nothing"
    );
    engine.feed(b"\x1b[?1000h\x1b[?1006h");
    assert_eq!(
        engine.encode_mouse(Action::Press, Some(Button::Four), (3, 2), false),
        b"\x1b[<64;4;3M",
        "a notch up is button 4, at the pointer's own cell"
    );
    assert_eq!(
        engine.encode_mouse(Action::Press, Some(Button::Five), (3, 2), false),
        b"\x1b[<65;4;3M",
        "and a notch down is button 5"
    );
    assert_eq!(
        engine.encode_mouse(Action::Release, Some(Button::Left), (3, 2), false),
        b"\x1b[<0;4;3m",
        "the release names the button that ended, not a motion with none"
    );

    // A move with no button held is how a program hears a hover: any-event tracking (1003)
    // carries it as button 35, and the modes that only report motion while a button is down
    // do not ask for it at all.
    assert!(
        engine
            .encode_mouse(Action::Motion, None, (5, 1), false)
            .is_empty(),
        "press/release tracking (1000) has no motion to report"
    );
    engine.feed(b"\x1b[?1003h");
    assert_eq!(
        engine.encode_mouse(Action::Motion, None, (5, 1), false),
        b"\x1b[<35;6;2M",
        "a bare move names no button"
    );
}

/// A shell that exits is reported, which is what closes the pane, and the standalone terminal
/// window with it.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_shell_that_exits_is_reported() {
    use rune_tty::engine::Engine;
    let dir = tempfile::tempdir().unwrap();
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    let mut engine = Engine::spawn(
        dir.path(),
        40,
        10,
        (8.0, 16.0),
        2.0,
        rune_tty::theme(true),
        wake,
    )
    .expect("a shell on a pty");
    assert!(
        !engine.shell_exited(),
        "a shell that just started is running"
    );
    engine.write(b"exit\r");
    let mut seen = false;
    for _ in 0..240 {
        engine.drain();
        if engine.shell_exited() {
            seen = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert!(seen, "the shell's exit reached the pane");
}

/// The shell is started with the terminal Ghostty's core actually is, not the plain xterm the
/// pane used to claim: programs read `TERM` and `TERM_PROGRAM` to decide which of its features
/// they may use.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn the_shell_is_told_it_is_on_ghostty_s_terminfo() {
    use rune_tty::engine::Engine;
    let dir = tempfile::tempdir().unwrap();
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    let mut engine = Engine::spawn(
        dir.path(),
        40,
        10,
        (8.0, 16.0),
        2.0,
        rune_tty::theme(true),
        wake,
    )
    .expect("a shell on a pty");
    engine.write(b"printf '%s %s\\n' \"$TERM\" \"$TERM_PROGRAM\"\r");
    let mut seen = false;
    for _ in 0..240 {
        engine.drain();
        if engine
            .frame()
            .lines
            .iter()
            .any(|line| line.text.contains("xterm-ghostty ghostty"))
        {
            seen = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert!(seen, "the shell sees the terminal the core emulates");
}

/// Search walks Ghostty's matcher over the screen and its history, marks every match it found
/// on the rows the pane draws, and moves to the newest one first.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_search_marks_the_matches_it_finds_and_walks_them() {
    use rune_tty::engine::Engine;
    let dir = tempfile::tempdir().unwrap();
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    let mut engine = Engine::spawn(
        dir.path(),
        40,
        10,
        (8.0, 16.0),
        2.0,
        rune_tty::theme(true),
        wake,
    )
    .expect("a shell on a pty");
    engine.feed(b"\x1b[2J\x1b[Hone two\r\nthree two");

    assert!(engine.search("two"));
    assert_eq!(
        engine.search_count(),
        Some((2, Some(0))),
        "both matches are found, and the newest is selected"
    );
    let frame = engine.frame();
    assert_eq!(
        frame.lines[0].matches.first().map(|mark| mark.cols),
        Some((4, 6)),
        "the match on the first row covers the word"
    );
    assert_eq!(
        frame.lines[1].matches.first().map(|mark| mark.cols),
        Some((6, 8))
    );
    assert!(
        frame
            .lines
            .iter()
            .flat_map(|line| &line.matches)
            .any(|mark| mark.current),
        "the selected match draws stronger"
    );

    assert!(engine.search_next());
    assert_eq!(
        engine.search_count().map(|(_, selected)| selected),
        Some(Some(1)),
        "next walks to the older match"
    );

    engine.end_search();
    assert!(!engine.searching());
    assert!(
        engine
            .frame()
            .lines
            .iter()
            .all(|line| line.matches.is_empty()),
        "closing the search leaves no marks behind"
    );
}

/// Double, triple and select-all read the grid through Ghostty's own selection rules, so the
/// words and lines they take are the ones a terminal selects.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_double_click_takes_a_word_and_a_triple_click_its_line() {
    use rune_tty::engine::Engine;
    let dir = tempfile::tempdir().unwrap();
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    let mut engine = Engine::spawn(
        dir.path(),
        40,
        10,
        (8.0, 16.0),
        2.0,
        rune_tty::theme(true),
        wake,
    )
    .expect("a shell on a pty");
    engine.feed(b"\x1b[2J\x1b[Hfoo bar baz");

    assert!(engine.begin_selection((5, 0), 2, false));
    assert_eq!(engine.selected().as_deref(), Some("bar"));

    assert!(engine.begin_selection((5, 0), 3, false));
    assert_eq!(engine.selected().as_deref(), Some("foo bar baz"));

    // A drag after a double click keeps the word it started on and the one under the pointer.
    assert!(engine.begin_selection((0, 0), 2, false));
    assert!(engine.select_to((10, 0)));
    assert_eq!(engine.selected().as_deref(), Some("foo bar baz"));

    // Alt+drag is a block: the columns between the ends, on every row between them.
    engine.feed(b"\x1b[2J\x1b[Habcdef\r\nghijkl\r\nmnopqr");
    engine.clear_selection();
    assert!(
        !engine.begin_selection((1, 0), 1, true),
        "the press alone selects nothing"
    );
    assert!(engine.select_to((3, 2)));
    assert_eq!(
        engine.selected().as_deref(),
        Some("bcd\nhij\nnop"),
        "a block takes the same columns from each row"
    );

    // Select-all covers the screen and the empty rows below it, trimmed on the way out.
    engine.feed(b"\x1b[2J\x1b[Hfoo bar baz");
    assert!(engine.select_all());
    let all = engine.selected().expect("everything is selected");
    assert_eq!(all.trim_end(), "foo bar baz");

    engine.clear_selection();
    assert_eq!(engine.selected(), None);
}
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_shell_starts_on_a_pty_and_its_output_reaches_the_grid() {
    use rune_tty::engine::Engine;
    let dir = tempfile::tempdir().unwrap();
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    let mut engine = Engine::spawn(
        dir.path(),
        40,
        10,
        (8.0, 16.0),
        2.0,
        rune_tty::theme(true),
        wake,
    )
    .expect("a shell on a pty");
    engine.write(b"printf 'sheath-ok\\n'\r");
    let mut seen = false;
    for _ in 0..240 {
        engine.drain();
        if engine
            .frame()
            .lines
            .iter()
            .any(|line| line.text.contains("sheath-ok"))
        {
            seen = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert!(seen, "the shell's output reached the grid");

    // A resize reflows the terminal, and the next frame is the new size.
    assert!(engine.resize(60, 12, (8.0, 16.0), 2.0));
    engine.drain();
    let frame = engine.frame();
    assert_eq!((frame.cols, frame.rows), (60, 12));

    // More lines than the viewport leaves history to scroll through, and scrolling back shows
    // different rows than the live tail does.
    // Enough short lines to push rows out of a 12-row viewport into history.
    engine.write(b"yes history-line | head -40\r");
    let mut kept = false;
    let mut most = 0;
    for _ in 0..240 {
        engine.drain();
        most = most.max(engine.scrollback());
        if most >= 20 {
            kept = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(25));
    }
    assert!(
        kept,
        "the rows that left the viewport are kept as history: {most}"
    );
    let live: Vec<String> = engine
        .frame()
        .lines
        .iter()
        .map(|l| l.text.clone())
        .collect();
    assert!(engine.set_scroll(8));
    let back: Vec<String> = engine
        .frame()
        .lines
        .iter()
        .map(|l| l.text.clone())
        .collect();
    assert_ne!(live, back, "scrolling back shows other rows");

    // A fractional scroll draws the rows above and below the viewport into the strips at the
    // edges: they are the rows the viewport steps to, so nothing fills in abruptly.
    let above = {
        assert!(engine.set_scroll(9));
        engine.frame().lines[0].text.clone()
    };
    let below = {
        assert!(engine.set_scroll(7));
        engine.frame().lines[0].text.clone()
    };
    assert!(engine.set_scroll(8));
    engine.ensure_edges(true);
    let edges = engine.frame();
    assert_eq!(
        edges.lead.expect("the row above the viewport").text,
        above,
        "the overscan row at the top is the one about to scroll in"
    );
    assert_eq!(
        edges.trail.expect("the row below the viewport").text,
        below,
        "the overscan row at the bottom is the one the viewport steps out to"
    );
    assert!(engine.set_scroll(0), "and the bottom pins again");
    engine.ensure_edges(true);
    assert!(
        engine.frame().trail.is_none(),
        "the live tail has no row below it"
    );
    engine.ensure_edges(false);
    assert!(
        engine.frame().lead.is_none(),
        "and both are dropped at a whole-row offset"
    );
}
