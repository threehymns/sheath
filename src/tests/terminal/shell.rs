//! The whole native path: a shell on a PTY, libghostty parsing what it writes, and a frame the
//! pane can draw. It is the one test that proves the pieces are wired together.

// The whole native path: a shell on a PTY, libghostty parsing what it writes, and a frame the
// pane can draw. It is the one test that proves the pieces are wired together.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_shell_starts_on_a_pty_and_its_output_reaches_the_grid() {
    use crate::term::engine::Engine;
    let dir = tempfile::tempdir().unwrap();
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    let mut engine = Engine::spawn(
        dir.path(),
        40,
        10,
        (8.0, 16.0),
        2.0,
        crate::term::theme(true),
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
