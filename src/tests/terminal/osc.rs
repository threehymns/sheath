//! The OSC protocols the pane answers beyond the grid, checked where they turn into something
//! the header draws rather than against a shell that has to cooperate.

#[cfg(not(target_arch = "wasm32"))]
use crate::term::engine::{directory, progress_of};
#[cfg(not(target_arch = "wasm32"))]
use crate::term::{Progress, ProgressKind};
#[cfg(not(target_arch = "wasm32"))]
use ghostty_vt::terminal::ProgressState;

/// OSC 8 links are stored with the cells they cover, so a click resolves the same link from any
/// cell of it and nothing from the cell past its end.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn an_osc8_link_comes_back_with_the_cells_it_covers() {
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
    // Written straight into the parser, so the screen is exactly this and not whatever the
    // shell's prompt happens to print first.
    engine.feed(b"\x1b[2J\x1b[H\x1b]8;;https://example.com\x1b\\example\x1b]8;;\x1b\\");
    assert_eq!(
        engine.hyperlink_at((0, 0)).as_deref(),
        Some("https://example.com"),
        "the link is on the cell its text starts at"
    );
    assert_eq!(
        engine.hyperlink_at((2, 0)).as_deref(),
        Some("https://example.com"),
        "every cell of the link text carries it"
    );
    assert_eq!(
        engine.hyperlink_at((8, 0)),
        None,
        "the cell past the link text has none"
    );
}

/// OSC 12 sets the cursor's own colour and OSC 112 hands it back to the theme, while DECSCUSR
/// picks the shape: the two pieces of cursor state Ghostty's renderer draws from, and the two a
/// program like an editor uses to draw a bar cursor in its insert mode.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn the_program_s_cursor_colour_and_shape_reach_the_frame() {
    use crate::term::engine::Engine;
    use crate::term::grid::CursorShape;
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
    engine.feed(b"\x1b[2J\x1b[H\x1b]12;#ff0000\x1b\\\x1b[4 q");
    let frame = engine.frame();
    assert_eq!(frame.cursor, Some((0, 0)));
    let color = frame.cursor_color.expect("the program set a cursor colour");
    let byte = |value: f32| (rune::color::srgb_encode(value) * 255.0).round() as u8;
    assert_eq!((byte(color.r), byte(color.g), byte(color.b)), (255, 0, 0));
    assert_eq!(
        frame.cursor_shape,
        CursorShape::Underline,
        "DECSCUSR 4 is the underline shape"
    );
    engine.feed(b"\x1b]112\x1b\\\x1b[5 q");
    let frame = engine.frame();
    assert_eq!(
        frame.cursor_color, None,
        "OSC 112 gives the cursor back to the theme"
    );
    assert_eq!(
        frame.cursor_shape,
        CursorShape::Bar,
        "DECSCUSR 5 is the bar"
    );
}

/// OSC 52 hands the clipboard what the program copied, with the trailing whitespace Ghostty
/// trims by default (`clipboard-trim-trailing-spaces`): a yank ends in a newline, and keeping it
/// pastes a blank line wherever the copy lands.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn an_osc52_copy_is_trimmed_of_its_trailing_whitespace() {
    use crate::term::engine::{Engine, trim_trailing};
    assert_eq!(trim_trailing("echo hi\n".to_owned()), "echo hi");
    assert_eq!(trim_trailing("echo hi \t\r\n\n".to_owned()), "echo hi");
    assert_eq!(
        trim_trailing("two words".to_owned()),
        "two words",
        "a copy with nothing trailing keeps its spaces inside"
    );

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
    // OSC 52 with `c` (the clipboard) and base64 of "hello  \n".
    engine.feed(b"\x1b]52;c;aGVsbG8gIAo=\x1b\\");
    assert_eq!(engine.take_clipboard(), vec!["hello".to_owned()]);
}

/// OSC 7 arrives as a `file://` URI with the path percent-encoded, which is how a folder with a
/// space in its name reaches the header.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_directory_report_decodes_into_the_path_it_names() {
    assert_eq!(
        directory("file:///home/me/src").as_deref(),
        Some(std::path::Path::new("/home/me/src"))
    );
    assert_eq!(
        directory("file://my-host/home/me/my%20project").as_deref(),
        Some(std::path::Path::new("/home/me/my project")),
        "a percent-encoded space decodes, and the host is not part of the path"
    );
    assert_eq!(
        directory("/tmp/plain").as_deref(),
        Some(std::path::Path::new("/tmp/plain")),
        "the protocols that report a bare path are taken as one"
    );
    assert_eq!(
        directory("file://"),
        None,
        "a report with no path names nothing"
    );
    assert_eq!(directory(""), None);
}

/// OSC 9;4 reports which state the program is in and how far along it is; the header has a
/// shape for each, and a removed report leaves none.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_progress_report_maps_onto_the_header_s_states() {
    assert_eq!(
        progress_of(ProgressState::Set, Some(42)),
        Some(Progress {
            kind: ProgressKind::Set,
            percent: Some(42)
        })
    );
    assert_eq!(
        progress_of(ProgressState::Set, None),
        Some(Progress {
            kind: ProgressKind::Set,
            percent: None
        })
    );
    assert_eq!(
        progress_of(ProgressState::Indeterminate, None),
        Some(Progress {
            kind: ProgressKind::Indeterminate,
            percent: None
        })
    );
    assert_eq!(
        progress_of(ProgressState::Pause, Some(70)),
        Some(Progress {
            kind: ProgressKind::Pause,
            percent: Some(70)
        })
    );
    assert_eq!(
        progress_of(ProgressState::Error, Some(12)),
        Some(Progress {
            kind: ProgressKind::Error,
            percent: Some(12)
        })
    );
    assert_eq!(
        progress_of(ProgressState::Remove, None),
        None,
        "a removed report leaves no bar behind"
    );
    assert_eq!(
        progress_of(ProgressState::Set, Some(200)).and_then(|report| report.percent),
        Some(100),
        "a percentage past the track is clamped to it"
    );
}
