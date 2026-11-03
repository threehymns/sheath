//! The kitty graphics protocol as the pane sees it: what a program transmits, where the screen
//! puts it, and what a delete leaves behind. The protocol parsing is libghostty's, so these
//! check the half we own — the placements read out of the storage and the pixels the raster
//! composites.

#![cfg(not(target_arch = "wasm32"))]

use crate::term::engine::Engine;

/// A 1x1 red RGB image, transmitted and displayed at the cursor with `a=T`, sized to one cell
/// (`c=1,r=1`) so its geometry is the same at any font: 0xff, 0x00, 0x00 base64-encodes to
/// `/wAA`.
const RED_PIXEL: &[u8] = b"\x1b[2J\x1b[H\x1b_Ga=T,f=24,s=1,v=1,c=1,r=1,i=1;/wAA\x1b\\";

fn engine(cols: u16, rows: u16) -> (tempfile::TempDir, Engine) {
    let dir = tempfile::tempdir().unwrap();
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    let engine = Engine::spawn(
        dir.path(),
        cols,
        rows,
        (8.0, 16.0),
        2.0,
        crate::term::theme(true),
        wake,
    )
    .expect("a shell on a pty");
    (dir, engine)
}

/// `a=T` transmits an image and places it at the cursor, so the frame carries one placement with
/// the pixels the program sent and a box one cell wide and one row tall.
#[test]
fn an_image_transmitted_at_the_cursor_reaches_the_frame() {
    let (_dir, mut engine) = engine(40, 10);
    engine.feed(RED_PIXEL);
    let frame = engine.frame();
    assert_eq!(frame.images.len(), 1, "one image was placed");
    let placement = &frame.images[0];
    assert_eq!(placement.image, 1, "the frame names the image it draws");
    assert_eq!((placement.data.width, placement.data.height), (1, 1));
    assert_eq!(placement.data.rgba, vec![255, 0, 0, 255]);
    assert_eq!(
        placement.source,
        [0, 0, 1, 1],
        "with no source rectangle the whole image is drawn"
    );
    assert_eq!(
        (placement.x, placement.y),
        (0.0, 0.0),
        "it lands on the cursor's cell"
    );
    assert_eq!(
        (placement.w, placement.h),
        (8.0, 16.0),
        "a one pixel image is drawn one cell wide and one row tall"
    );
    assert_eq!(
        placement.layer,
        crate::term::grid::ImageLayer::AboveText,
        "z-index 0 draws over the text"
    );
}

/// The placement id and a cell offset move the image inside its cell and let the same image be
/// placed twice, which is how a viewer draws a thumbnail and a preview of one transmission.
#[test]
fn a_placement_carries_its_own_position_and_source_rectangle() {
    let (_dir, mut engine) = engine(40, 10);
    engine.feed(RED_PIXEL);
    // Move to row 3, column 5 and place the same image again, offset by two pixels and drawing
    // only part of the source.
    engine.feed(b"\x1b[4;6H\x1b_Ga=p,i=1,X=2,Y=3,x=0,y=0,w=1,h=1\x1b\\");
    let frame = engine.frame();
    assert_eq!(frame.images.len(), 2, "the same image is placed twice");
    // The list is ordered by layer and z, which the two placements share, so find the second by
    // the position it asked for rather than by where the storage happens to list it.
    let placed = frame
        .images
        .iter()
        .find(|placement| placement.x > 0.0)
        .expect("the second placement is off the origin");
    assert_eq!((placed.x, placed.y), (5.0 * 8.0 + 2.0, 3.0 * 16.0 + 3.0));
    assert_eq!(placed.source, [0, 0, 1, 1]);
}

/// A negative z-index draws under the text, and the frame's list is ordered so the raster can
/// paint it there without sorting.
#[test]
fn a_negative_z_index_sorts_under_the_text() {
    let (_dir, mut engine) = engine(40, 10);
    engine.feed(RED_PIXEL);
    engine.feed(b"\x1b_Ga=p,i=1,z=-1\x1b\\");
    let frame = engine.frame();
    assert_eq!(frame.images.len(), 2);
    assert_eq!(
        frame.images[0].layer,
        crate::term::grid::ImageLayer::BelowText
    );
    assert_eq!(frame.images[0].z, -1);
    assert_eq!(
        frame.images[1].layer,
        crate::term::grid::ImageLayer::AboveText
    );
}

/// Deleting the images clears the frame: a viewer that exits puts the screen back, and the pane
/// must not keep drawing what it drew. The delete raises no event, so the drain's generation
/// check is what asks for the redraw.
#[test]
fn deleting_the_images_empties_the_frame() {
    let (_dir, mut engine) = engine(40, 10);
    engine.feed(RED_PIXEL);
    assert_eq!(engine.frame().images.len(), 1);
    engine.feed(b"\x1b_Ga=d,d=a\x1b\\");
    let frame = engine.frame();
    assert!(
        frame.images.is_empty(),
        "a delete-all leaves nothing to draw"
    );
}

/// A screen with no images asks for no redraw: the pane only learns that an image arrived from
/// the bytes that carried it, so an idle terminal is not kept awake by the image storage.
#[test]
fn a_plain_screen_settles_with_nothing_to_redraw() {
    let (_dir, mut engine) = engine(40, 10);
    engine.feed(b"\x1b[2J\x1b[Hhello\r\n");
    // The shell's own prompt can land for a moment after the pane opens; wait for the screen to
    // settle, then check that an idle drain reports nothing.
    let mut settled = false;
    for _ in 0..100 {
        if !engine.drain() {
            settled = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    assert!(settled, "the pane settles");
    assert!(!engine.drain(), "an idle drain reports no change");
    assert!(!engine.drain(), "and keeps reporting none");
}

/// A transmit-only action stores the image without showing it, so nothing reaches the frame
/// until a placement asks for it.
#[test]
fn a_transmit_without_a_placement_draws_nothing() {
    let (_dir, mut engine) = engine(40, 10);
    engine.feed(b"\x1b[2J\x1b[H\x1b_Ga=t,f=24,s=1,v=1,i=7;/wAA\x1b\\");
    assert!(engine.frame().images.is_empty(), "nothing was placed");
    engine.feed(b"\x1b_Ga=p,i=7\x1b\\");
    let frame = engine.frame();
    assert_eq!(frame.images.len(), 1);
    assert_eq!(frame.images[0].data.rgba, vec![255, 0, 0, 255]);
}
