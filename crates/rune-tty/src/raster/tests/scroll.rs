//! The streamed image across a scroll: the bands it moves are the pixels a whole compose would
//! paint in their place.

use crate::grid::{Frame, Line, Pen};
use crate::raster::{Raster, picture, streamed_for_test};

use super::{palette, run};

/// A line of plain text over every column of `cols`.
fn text_line(text: &str, cols: u16) -> Line {
    let mut line = Line {
        text: text.to_owned(),
        ..Line::default()
    };
    line.runs
        .push(run(Pen::default(), (0, cols), 0, text.len()));
    line
}

/// A frame whose placement did not change keeps the image's pixels: only the band whose text
/// changed is painted again, and the image is composited under or over that band the way a whole
/// compose would. This is what a video that only redraws its own overlay costs.
#[test]
fn an_unchanged_image_is_composited_under_the_band_that_changed() {
    use crate::grid::{ImageLayer, ImagePixels, Placement};
    use std::sync::Arc;

    let pixels = Arc::new(ImagePixels {
        width: 2,
        height: 2,
        rgba: [0u8, 255, 0, 255].repeat(4),
    });
    let placement = Placement {
        image: 1,
        data: Arc::clone(&pixels),
        layer: ImageLayer::AboveText,
        z: 0,
        x: 0.0,
        y: 0.0,
        w: 40.0,
        h: 32.0,
        source: [0, 0, 2, 2],
    };
    let (cols, rows) = (5u16, 2u16);
    let mut frame = Frame {
        cols,
        rows,
        generation: 1,
        ..Frame::default()
    };
    frame.lines.push(text_line("aaaa", cols));
    frame.lines.push(text_line("bbbb", cols));
    frame.images.push(placement);
    picture(&frame, 8.0, 16.0, 1.0, palette()).expect("an image");

    // The next frame carries the same placement, built from the same pixels, with one row of
    // text rewritten: the other rows keep the pixels they were painted with.
    frame.lines[1] = text_line("cccc", cols);
    frame.generation = frame.generation.wrapping_add(1);
    picture(&frame, 8.0, 16.0, 1.0, palette()).expect("an image");
    let streamed = streamed_for_test();
    let (_, _, whole) = Raster::new()
        .compose(&frame, 8.0, 16.0, 1.0, palette())
        .expect("an image");
    assert_eq!(streamed, whole, "the band is painted over the same image");
}

/// A scrolled frame streams the pixels a whole compose would paint. A scroll keeps every row it
/// was showing, one band over: the image moves those pixels instead of painting them again, and
/// this is the test that the ones it moves are the ones that belong there.
#[test]
fn a_scrolled_frame_streams_the_pixels_a_whole_compose_would() {
    let (cols, rows) = (20u16, 6u16);
    let mut frame = Frame {
        cols,
        rows,
        ..Frame::default()
    };
    for row in 0..rows {
        frame.lines.push(text_line(&format!("row {row}"), cols));
    }
    picture(&frame, 8.0, 16.0, 1.0, palette()).expect("an image");
    for step in 0..4 {
        // The wheel crosses a row in both directions: down the history, then back toward the
        // shell's newest line, with a fresh line entering at the edge each time.
        if step % 2 == 0 {
            frame.lines.remove(0);
            frame.lines.push(text_line(&format!("new {step}"), cols));
        } else {
            frame.lines.pop();
            frame
                .lines
                .insert(0, text_line(&format!("new {step}"), cols));
        }
        frame.generation = frame.generation.wrapping_add(1);
        let streamed = picture(&frame, 8.0, 16.0, 1.0, palette()).expect("an image");
        let (width, height) = streamed.size();
        assert_eq!(
            (width, height),
            (u32::from(cols) * 8, (u32::from(rows) + 2) * 16),
            "step {step}: the grid and its overscan rows"
        );
        let pixels = streamed_for_test();
        let (_, _, whole) = Raster::new()
            .compose(&frame, 8.0, 16.0, 1.0, palette())
            .expect("an image");
        assert_eq!(
            pixels, whole,
            "step {step}: the shifted image is the composed one"
        );
    }
}
