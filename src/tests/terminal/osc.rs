//! The OSC protocols the pane answers beyond the grid, checked where they turn into something
//! the header draws rather than against a shell that has to cooperate.

#[cfg(not(target_arch = "wasm32"))]
use crate::term::engine::{directory, progress_of};
#[cfg(not(target_arch = "wasm32"))]
use crate::term::{Progress, ProgressKind};
#[cfg(not(target_arch = "wasm32"))]
use ghostty_vt::terminal::ProgressState;

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
