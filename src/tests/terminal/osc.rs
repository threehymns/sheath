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
