//! The OSC protocols the pane answers beyond the grid, checked where they turn into something
//! the header draws rather than against a shell that has to cooperate.

#[cfg(not(target_arch = "wasm32"))]
use ghostty_vt::terminal::ProgressState;
#[cfg(not(target_arch = "wasm32"))]
use rune_tty::engine::{directory, progress_of};
#[cfg(not(target_arch = "wasm32"))]
use rune_tty::{Progress, ProgressKind};

/// OSC 8 links are stored with the cells they cover, so a click resolves the same link from any
/// cell of it and nothing from the cell past its end.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn an_osc8_link_comes_back_with_the_cells_it_covers() {
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
    assert_eq!(
        engine.hyperlink_span((2, 0)),
        Some((0, 6)),
        "the span a hover underlines covers the link text and stops at its end"
    );
    assert_eq!(
        engine.hyperlink_span((8, 0)),
        None,
        "a cell with no link has no span to underline"
    );
}

/// OSC 12 sets the cursor's own colour and OSC 112 hands it back to the theme, while DECSCUSR
/// picks the shape: the two pieces of cursor state Ghostty's renderer draws from, and the two a
/// program like an editor uses to draw a bar cursor in its insert mode.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn the_program_s_cursor_colour_and_shape_reach_the_frame() {
    use rune_tty::engine::Engine;
    use rune_tty::grid::CursorShape;
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

/// OSC 10 and OSC 11 move the default colours under rows that are already on screen, and
/// libghostty marks no row dirty for it: the pane reads the grid again, so a row it read before
/// the move does not keep drawing the colour it was read in. One frame is reused for both reads,
/// as the pane reuses it; a fresh one would be read in full and hide the difference.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_program_s_default_colours_reach_the_rows_already_drawn() {
    use rune_tty::engine::Engine;
    use rune_tty::grid::Frame;
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
    engine.feed(b"\x1b[2J\x1b[Hplain text");
    let mut frame = Frame::default();
    engine.fill(&mut frame);
    let before = frame.lines[0].runs[0].pen.fg;
    engine.feed(b"\x1b]10;#ff0000\x1b\\");
    engine.fill(&mut frame);
    let after = frame.lines[0].runs[0].pen.fg;
    assert_ne!(
        after, before,
        "a row read before the program moved the default draws the new one"
    );
    let byte = |value: f32| (rune::color::srgb_encode(value) * 255.0).round() as u8;
    assert_eq!(
        (byte(after.r), byte(after.g), byte(after.b)),
        (255, 0, 0),
        "OSC 10's colour"
    );
}

/// A program's erase paints the pen's background into the cells themselves rather than giving
/// them a style: that is how an editor like nvim fills its base colour, and the blank cells it
/// leaves behind are covered by it. The pane reads a cell's background whether or not the cell
/// carries a style, so an erased area draws the program's colour instead of the pane's own.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_program_s_erased_background_reaches_the_cells() {
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
    // The pen sets the base colour and `EL` fills the row with it: no character is written, so
    // the cells carry the colour alone.
    engine.feed(b"\x1b[2J\x1b[H\x1b[48;2;26;27;38m\x1b[K");
    let frame = engine.frame();
    let line = &frame.lines[0];
    let run = line
        .runs
        .first()
        .expect("the erased row keeps the pen's background");
    assert_eq!(run.cols, (0, 40), "the erase covers the whole row");
    assert_eq!(
        line.text.len(),
        40,
        "as spaces, so the strip stays the row's"
    );
    let byte = |value: f32| (rune::color::srgb_encode(value) * 255.0).round() as u8;
    let bg = run.pen.bg.expect("the pen's background");
    assert_eq!(
        (byte(bg.r), byte(bg.g), byte(bg.b)),
        (26, 27, 38),
        "the colour the program erased with"
    );
}

/// The overscan rows above and below a fractionally scrolled viewport are read with the rows, so
/// a default-colour move has to read them again for that frame: dropped instead, they leave the
/// strip the fractional offset shows blank until the next one.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn a_program_s_default_colours_reach_the_overscan_rows() {
    use rune_tty::engine::Engine;
    use rune_tty::grid::Frame;
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
    for line in 0..20 {
        engine.feed(format!("line {line}\r\n").as_bytes());
    }
    assert!(engine.set_scroll(8), "the viewport moves off the tail");
    let mut frame = Frame::default();
    engine.ensure_edges(true);
    engine.fill(&mut frame);
    let above = frame.lead.clone().expect("the row above the viewport");
    engine.feed(b"\x1b]10;#ff0000\x1b\\");
    // What the pane does on the frame after the move: the edges are asked for first, and the
    // fill is what notices the pair changed.
    engine.ensure_edges(true);
    engine.fill(&mut frame);
    let after = frame
        .lead
        .expect("the overscan row is drawn again, not left blank for a frame");
    assert_ne!(
        after.runs[0].pen.fg, above.runs[0].pen.fg,
        "the row read before the move draws the new default"
    );
    let byte = |value: f32| (rune::color::srgb_encode(value) * 255.0).round() as u8;
    assert_eq!(
        (byte(after.runs[0].pen.fg.r), byte(after.runs[0].pen.fg.g)),
        (255, 0),
        "OSC 10's colour, on the row that was already read"
    );
}

/// OSC 52 hands the clipboard what the program copied, with the trailing whitespace Ghostty
/// trims by default (`clipboard-trim-trailing-spaces`): a yank ends in a newline, and keeping it
/// pastes a blank line wherever the copy lands.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn an_osc52_copy_is_trimmed_of_its_trailing_whitespace() {
    use rune_tty::engine::{Engine, trim_trailing};
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
        rune_tty::theme(true),
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
        directory("kitty-shell-cwd://my-host/home/me/src").as_deref(),
        Some(std::path::Path::new("/home/me/src")),
        "the scheme Ghostty's integration scripts report with carries the path after the host"
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

/// A program asks which side of the theme it is drawing on (CSI ? 996 n), and the answer is the
/// one Ghostty gives: the background's luminance decides, not a setting the program can see.
#[cfg(not(target_arch = "wasm32"))]
#[test]
fn the_color_scheme_answer_follows_the_background() {
    use ghostty_vt::style::RgbColor;
    use ghostty_vt::terminal::ColorScheme;
    use rune_tty::Theme;
    use rune_tty::engine::scheme;
    let theme = |bg: u8| Theme {
        fg: RgbColor {
            r: 255 - bg,
            g: 255 - bg,
            b: 255 - bg,
        },
        bg: RgbColor {
            r: bg,
            g: bg,
            b: bg,
        },
        cursor: RgbColor { r: 0, g: 0, b: 0 },
    };
    assert_eq!(scheme(theme(0)), ColorScheme::Dark);
    assert_eq!(scheme(theme(32)), ColorScheme::Dark);
    assert_eq!(scheme(theme(255)), ColorScheme::Light);
    assert_eq!(scheme(theme(230)), ColorScheme::Light);
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
