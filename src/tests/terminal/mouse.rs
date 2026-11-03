//! Mouse reports as the program reads them: cell coordinates in the ordinary modes and device
//! pixels in mode 1016, from the same cell the pointer is over.

#![cfg(not(target_arch = "wasm32"))]

use ghostty_vt::mouse;

use crate::term::engine::Engine;

fn engine() -> (tempfile::TempDir, Engine) {
    let dir = tempfile::tempdir().unwrap();
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    let engine = Engine::spawn(
        dir.path(),
        40,
        10,
        (8.0, 16.0),
        1.0,
        crate::term::theme(true),
        wake,
    )
    .expect("a shell on a pty");
    (dir, engine)
}

fn press(engine: &mut Engine) -> String {
    String::from_utf8_lossy(&engine.encode_mouse(
        mouse::Action::Press,
        Some(mouse::Button::Left),
        (2, 3),
        false,
    ))
    .into_owned()
}

/// SGR reports a cell, and SGR-Pixels reports the same pointer in device pixels: a program that
/// draws in pixels gets the position it would have computed itself, not a cell count.
#[test]
fn pixel_tracking_reports_the_pointer_in_device_pixels() {
    let (_dir, mut engine) = engine();
    engine.feed(b"\x1b[?1000h\x1b[?1006h");
    let cells = press(&mut engine);
    engine.feed(b"\x1b[?1006l\x1b[?1016h");
    let pixels = press(&mut engine);
    // The third column and fourth row are 1-based in the cell report: cell (2, 3) is `3;4`. The
    // pixel report names the same pointer's device pixel, 8 and 16 points into the grid at a
    // scale of one, which libghostty reports as the pixel itself rather than a 1-based cell.
    assert_eq!(cells, "\x1b[<0;3;4M", "a cell report names the cell");
    assert_eq!(
        pixels, "\x1b[<0;16;48M",
        "a pixel report names the device pixel"
    );
}
