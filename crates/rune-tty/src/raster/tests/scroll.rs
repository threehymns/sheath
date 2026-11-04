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
