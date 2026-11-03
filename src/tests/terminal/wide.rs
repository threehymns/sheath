//! Cells a character covers two of: a CJK glyph or an emoji. The row has to hold the character
//! once, leave the cell it covers without a character of its own, and report the columns after
//! it as the cells they are drawn in.

#![cfg(not(target_arch = "wasm32"))]

/// A wide character covers two cells: the row holds the character once, the cell it covers
/// carries no character of its own, and the columns after it are the cells they are drawn in.
/// The phantom space the renderer used to insert there was what drifted the rest of the row.
#[test]
fn a_wide_character_covers_two_cells() {
    use crate::term::engine::Engine;

    let dir = tempfile::tempdir().unwrap();
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    let mut engine = Engine::spawn(
        dir.path(),
        20,
        4,
        (7.0, 15.0),
        2.0,
        crate::term::theme(true),
        wake,
    )
    .expect("an engine");
    // An emoji, then a cell with a background, then a plain one: the cell after the wide one must
    // start at column two, which a renderer walking the text gets from the row's own widths.
    engine.feed("\x1b[2J\x1b[H😀\x1b[41ma\x1b[0mb".as_bytes());
    let frame = engine.frame();
    let line = &frame.lines[0];
    assert_eq!(
        line.text, "😀ab",
        "the cell after the wide character holds no character of its own"
    );
    assert_eq!(
        line.wide,
        [0],
        "the wide character's column is carried on the row"
    );
    assert_eq!(
        (line.runs[0].start, line.runs[0].end),
        (0, 4),
        "one run over the wide character"
    );
    let painted = line
        .runs
        .iter()
        .find(|run| run.pen.bg.is_some())
        .expect("the painted cell");
    assert_eq!(
        painted.cols,
        (2, 3),
        "the cell after the wide one is drawn at column two"
    );
    assert_eq!(
        line.runs[0].cols,
        (0, 2),
        "and the wide character's run covers both of its columns"
    );
}
