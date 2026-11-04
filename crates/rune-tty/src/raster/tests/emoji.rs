//! Glyphs the mono face cannot draw: the system's colour emoji font, in its own colours, and a
//! sequence that stays in its one cell.

use crate::grid::{Frame, Line, Pen};
use crate::raster::Raster;

use super::{palette, pixel, run};

/// An emoji the mono cannot draw comes from the system's colour emoji font, the way Rune draws
/// one in its text: in its own colours, centred across the two cells it covers.
#[test]
fn a_colour_emoji_is_drawn_in_colour_across_its_two_cells() {
    if crate::fonts::emoji_face().is_none() {
        eprintln!("skipping: no colour emoji font installed");
        return;
    }
    let mut line = Line {
        text: "😀".to_owned(),
        ..Line::default()
    };
    line.wide.push(0);
    line.runs.push(run(Pen::default(), (0, 2), 0, 4));
    let frame = Frame {
        cols: 2,
        rows: 1,
        lines: vec![line],
        ..Frame::default()
    };
    let (width, _, rgba) = Raster::new()
        .compose(&frame, 10.0, 20.0, 1.0, palette())
        .expect("an image");
    let lit: Vec<(u32, u32)> = (20..40)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .filter(|&(x, y)| pixel(&rgba, width, x, y)[3] > 0)
        .collect();
    assert!(!lit.is_empty(), "the emoji is drawn");
    assert!(
        lit.iter().any(|&(x, y)| {
            let ink = pixel(&rgba, width, x, y);
            i32::from(ink[0]) - i32::from(ink[2]) > 40
        }),
        "in the emoji font's own colours, not the pen's"
    );
    let first = lit.iter().map(|(x, _)| *x).min().unwrap();
    let last = lit.iter().map(|(x, _)| *x).max().unwrap();
    assert!(
        first < 10 && last >= 10,
        "the emoji covers both of its cells, not one: columns {first}..={last}"
    );
}

/// A cell holds a whole emoji sequence: the skin-tone modifier belongs to `👍`'s cell, not to a
/// column of its own, so the cell after the pair starts two columns along.
#[test]
fn a_combining_emoji_is_one_cell_wide() {
    if crate::fonts::emoji_face().is_none() {
        eprintln!("skipping: no colour emoji font installed");
        return;
    }
    let mut line = Line {
        text: "👍🏽x".to_owned(),
        ..Line::default()
    };
    line.wide.push(0);
    // `👍🏽` is one cell of eight bytes; `x` is the cell two columns along.
    line.runs.push(run(Pen::default(), (0, 3), 0, 9));
    let frame = Frame {
        cols: 4,
        rows: 1,
        lines: vec![line],
        ..Frame::default()
    };
    let (width, _, rgba) = Raster::new()
        .compose(&frame, 10.0, 20.0, 1.0, palette())
        .expect("an image");
    let ink = |x0: u32, x1: u32| {
        (20..40)
            .flat_map(|y| (x0..x1).map(move |x| (x, y)))
            .filter(|&(x, y)| pixel(&rgba, width, x, y)[3] > 0)
            .count()
    };
    assert!(ink(0, 20) > 0, "the emoji is drawn");
    assert!(ink(20, 30) > 0, "and the next cell two columns along");
    assert_eq!(
        ink(30, 40),
        0,
        "with nothing leaking into the cell after that"
    );
}
