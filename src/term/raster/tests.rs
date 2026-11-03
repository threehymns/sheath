//! The streamed image's composition: overscan rows, cell fills, shapes, washes, glyphs and
//! the size the pane streams. These compose directly and read the buffer, since the public
//! image is a `Pixels` the runtime uploads rather than bytes a test can inspect.

use super::*;
use crate::term::grid::{Pen, Run, Shape, UnderlineKind};

fn palette() -> Palette {
    Palette {
        bg: [0, 0, 0],
        selection: [255, 255, 255],
        search: [255, 255, 0],
    }
}

fn pixel(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let at = ((y * width + x) * 4) as usize;
    rgba[at..at + 4].try_into().unwrap()
}

fn run(pen: Pen, cols: (u16, u16), start: usize, end: usize) -> Run {
    Run {
        start,
        end,
        cols,
        pen,
    }
}

/// The image carries the overscan rows in order: lead, the viewport, trail. A fractional
/// scroll hangs it one row up, so those are the rows that enter at the edges.
#[test]
fn the_texture_carries_a_row_above_and_below_the_viewport() {
    let mut red = Line::default();
    red.runs.push(run(
        Pen {
            bg: Some(rune::Color::WHITE),
            ..Pen::default()
        },
        (0, 1),
        0,
        0,
    ));
    let mut green = Line::default();
    green.runs.push(run(
        Pen {
            bg: Some(rune::Color {
                r: 0.0,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            }),
            ..Pen::default()
        },
        (0, 1),
        0,
        0,
    ));
    let mut blue = Line::default();
    blue.runs.push(run(
        Pen {
            bg: Some(rune::Color {
                r: 0.0,
                g: 0.0,
                b: 1.0,
                a: 1.0,
            }),
            ..Pen::default()
        },
        (0, 1),
        0,
        0,
    ));
    let frame = Frame {
        cols: 2,
        rows: 1,
        lines: vec![green],
        lead: Some(red),
        trail: Some(blue),
        ..Frame::default()
    };
    let (width, height, rgba) = Raster::new()
        .compose(&frame, 10.0, 20.0, 1.0, palette())
        .expect("an image");
    assert_eq!((width, height), (20, 60), "one row of overscan each side");
    assert_eq!(pixel(&rgba, width, 0, 0), [255, 255, 255, 255], "lead");
    assert_eq!(pixel(&rgba, width, 0, 20)[1], 255, "viewport");
    assert_eq!(pixel(&rgba, width, 0, 40)[2], 255, "trail");
}

/// Kitty graphics placements are composited where the frame put them: the source rectangle
/// scaled into the box, clipped at the image's edges, and on the side of the text its layer
/// says — under the cell background for a negative z-index, over it otherwise.
#[test]
fn images_are_drawn_at_their_place_size_and_layer() {
    use crate::term::grid::{ImageLayer, ImagePixels, Placement};
    use std::sync::Arc;

    let green = Arc::new(ImagePixels {
        width: 2,
        height: 2,
        rgba: [0u8, 255, 0, 255].repeat(4),
    });
    let placement = |layer, x, w| Placement {
        image: 1,
        data: Arc::clone(&green),
        layer,
        z: 0,
        x,
        y: 0.0,
        w,
        h: 20.0,
        source: [0, 0, 2, 2],
    };
    let mut frame = Frame {
        cols: 2,
        rows: 1,
        lines: vec![Line::default()],
        ..Frame::default()
    };
    frame
        .images
        .push(placement(ImageLayer::AboveText, 0.0, 10.0));
    let (width, _, rgba) = Raster::new()
        .compose(&frame, 10.0, 20.0, 1.0, palette())
        .expect("an image");
    // One overscan row sits above the viewport, so the placement starts on row 20.
    assert_eq!(
        pixel(&rgba, width, 5, 20),
        [0, 255, 0, 255],
        "the first row"
    );
    assert_eq!(pixel(&rgba, width, 5, 39), [0, 255, 0, 255], "the last row");
    assert_eq!(pixel(&rgba, width, 5, 40), [0, 0, 0, 0], "below it");
    assert_eq!(pixel(&rgba, width, 5, 19), [0, 0, 0, 0], "above it");
    assert_eq!(pixel(&rgba, width, 12, 25), [0, 0, 0, 0], "past its width");

    // Half a cell off the left edge: the raster clips the part outside it instead of wrapping.
    frame.images.clear();
    frame
        .images
        .push(placement(ImageLayer::AboveText, -5.0, 10.0));
    let (width, _, rgba) = Raster::new()
        .compose(&frame, 10.0, 20.0, 1.0, palette())
        .expect("an image");
    assert_eq!(
        pixel(&rgba, width, 0, 25),
        [0, 255, 0, 255],
        "the visible left"
    );
    assert_eq!(
        pixel(&rgba, width, 6, 25),
        [0, 0, 0, 0],
        "past its right edge"
    );

    // The same image under the text: the row's own background paints over it, where an image
    // over the text paints over the background.
    let mut line = Line::default();
    line.runs.push(run(
        Pen {
            bg: Some(rune::Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            }),
            ..Pen::default()
        },
        (0, 2),
        0,
        0,
    ));
    frame.lines = vec![line];
    frame.images.clear();
    frame
        .images
        .push(placement(ImageLayer::BelowText, 0.0, 10.0));
    let (width, _, rgba) = Raster::new()
        .compose(&frame, 10.0, 20.0, 1.0, palette())
        .expect("an image");
    assert_eq!(
        pixel(&rgba, width, 5, 25),
        [255, 0, 0, 255],
        "the background wins"
    );
    frame.images.clear();
    frame
        .images
        .push(placement(ImageLayer::AboveText, 0.0, 10.0));
    let (width, _, rgba) = Raster::new()
        .compose(&frame, 10.0, 20.0, 1.0, palette())
        .expect("an image");
    assert_eq!(
        pixel(&rgba, width, 5, 25),
        [0, 255, 0, 255],
        "the image wins"
    );
}

/// Cell backgrounds and shapes are drawn to the cell's device-pixel edges, so neighbouring
/// cells tile instead of leaving a seam.
#[test]
fn a_shape_and_a_background_cover_their_cells() {
    let mut line = Line {
        text: "  ".to_owned(),
        ..Line::default()
    };
    line.runs.push(run(
        Pen {
            bg: Some(rune::Color {
                r: 0.0,
                g: 1.0,
                b: 0.0,
                a: 1.0,
            }),
            ..Pen::default()
        },
        (0, 2),
        0,
        2,
    ));
    line.shapes.push(Shape {
        col: 1,
        rect: [0.5, 0.5, 1.0, 1.0],
        fg: rune::Color::WHITE,
    });
    let frame = Frame {
        cols: 2,
        rows: 1,
        lines: vec![line],
        ..Frame::default()
    };
    let (width, _, rgba) = Raster::new()
        .compose(&frame, 10.0, 20.0, 1.0, palette())
        .expect("an image");
    assert_eq!(pixel(&rgba, width, 0, 20)[1], 255, "the run's background");
    assert_eq!(
        pixel(&rgba, width, 15, 30),
        [255, 255, 255, 255],
        "the shape's lower-right quadrant"
    );
    assert_eq!(
        pixel(&rgba, width, 5, 25)[1],
        255,
        "and it leaves the rest of the cell to the background"
    );
}

/// The selection is a wash over the cell beneath it. The cursor is an element of the view drawn
/// over the grid, so the image leaves its cell untouched.
#[test]
fn the_selection_is_a_wash_over_the_cell() {
    let mut line = Line {
        text: "  ".to_owned(),
        ..Line::default()
    };
    line.selection = Some((0, 0));
    let frame = Frame {
        cols: 2,
        rows: 1,
        lines: vec![line],
        cursor: Some((1, 0)),
        ..Frame::default()
    };
    let (width, _, rgba) = Raster::new()
        .compose(&frame, 10.0, 20.0, 1.0, palette())
        .expect("an image");
    assert_eq!(
        pixel(&rgba, width, 2, 22),
        [61, 61, 61, 255],
        "the selection's 24% wash over the pane's fill"
    );
    assert_eq!(
        pixel(&rgba, width, 12, 22),
        [0, 0, 0, 0],
        "the cursor's cell is left transparent for the view to draw"
    );
}

/// A glyph is drawn in its run's colour, inside the row it belongs to.
#[test]
fn a_glyph_is_drawn_in_its_run_colour() {
    let mut line = Line {
        text: "M".to_owned(),
        ..Line::default()
    };
    line.runs.push(run(
        Pen {
            fg: rune::Color {
                r: 1.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            ..Pen::default()
        },
        (0, 1),
        0,
        1,
    ));
    let frame = Frame {
        cols: 2,
        rows: 1,
        lines: vec![line],
        ..Frame::default()
    };
    let (width, height, rgba) = Raster::new()
        .compose(&frame, 10.0, 20.0, 1.0, palette())
        .expect("an image");
    let lit = (20..60)
        .flat_map(|y| (0..20).map(move |x| (x, y)))
        .filter(|&(x, y)| pixel(&rgba, width, x, y)[0] > 0)
        .count();
    assert!(lit > 20, "the glyph covers pixels: {lit}");
    assert!(
        (0..20).any(|y| pixel(&rgba, width, 0, y)[0] == 0),
        "but not the overscan row above it"
    );
    assert_eq!(height, 60);
}

/// SGR 4's underline styles draw what they ask for, in the colour SGR 58 gave them: a doubled
/// rule, dashes with gaps, or the saw a spell checker's squiggle is made of.
#[test]
fn underline_styles_are_drawn_as_the_style_and_colour_they_ask_for() {
    let red = rune::Color {
        r: 1.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    let compose = |kind: UnderlineKind| {
        let mut line = Line {
            text: "  ".to_owned(),
            ..Line::default()
        };
        line.runs.push(run(
            Pen {
                underline: kind,
                underline_color: Some(red),
                ..Pen::default()
            },
            (0, 2),
            0,
            2,
        ));
        let frame = Frame {
            cols: 2,
            rows: 1,
            lines: vec![line],
            ..Frame::default()
        };
        Raster::new()
            .compose(&frame, 10.0, 20.0, 1.0, palette())
            .expect("an image")
    };
    // The viewport row's band starts one overscan row down: the baseline is at 16 in the row,
    // and the rule one thickness below that.
    let under = 20 + 17;
    let (width, _, single) = compose(UnderlineKind::Single);
    assert_eq!(
        pixel(&single, width, 5, under),
        [255, 0, 0, 255],
        "a single rule sits under the baseline, in SGR 58's colour"
    );
    let (_, _, double) = compose(UnderlineKind::Double);
    assert_eq!(pixel(&double, width, 5, under), [255, 0, 0, 255]);
    assert_eq!(
        pixel(&double, width, 5, under + 2),
        [255, 0, 0, 255],
        "the second rule is a gap below the first"
    );
    assert_eq!(
        pixel(&double, width, 5, under + 1),
        [0, 0, 0, 0],
        "and the gap stays empty"
    );
    let (_, _, dotted) = compose(UnderlineKind::Dotted);
    assert_eq!(pixel(&dotted, width, 5, under), [255, 0, 0, 255]);
    assert_eq!(
        pixel(&dotted, width, 6, under),
        [0, 0, 0, 0],
        "a dot is followed by a gap"
    );
    let (_, _, curly) = compose(UnderlineKind::Curly);
    assert_eq!(pixel(&curly, width, 0, under), [255, 0, 0, 255]);
    assert_eq!(
        pixel(&curly, width, 1, under + 2),
        [255, 0, 0, 255],
        "the squiggle steps down on the next column"
    );
}

/// The streamed image is sized to the grid and its two overscan rows, and a same-pane
/// frame replaces it in place.
#[test]
fn picture_sets_the_streamed_image_to_the_grid_size() {
    let frame = Frame {
        cols: 4,
        rows: 2,
        lines: vec![Line::default(), Line::default()],
        ..Frame::default()
    };
    let image = picture(&frame, 8.0, 16.0, 2.0, palette()).expect("an image");
    assert_eq!(image.size(), (4 * 16, (2 + 2) * 32));
    let wider = Frame {
        cols: 5,
        rows: 3,
        lines: vec![Line::default(); 3],
        ..Frame::default()
    };
    let image = picture(&wider, 8.0, 16.0, 2.0, palette()).expect("an image");
    assert_eq!(image.size(), (5 * 16, (3 + 2) * 32));
}

/// The compose loop at a fullscreened grid, for before/after numbers. Run with
/// `cargo test --release perf_texture_compose -- --ignored --nocapture`.
#[test]
#[ignore = "manual benchmark; reports measurements, not a timing threshold"]
fn perf_texture_compose() {
    use std::hint::black_box;
    use std::time::Instant;

    let (cols, rows) = (274u16, 72u16);
    let body = "the quick brown fox jumps over the lazy dog compiling sheath v0.1.0 ";
    let mut frame = Frame {
        cols,
        rows,
        ..Frame::default()
    };
    for row in 0..rows {
        let text: String = format!("{row:>4} {}", body.repeat(16))
            .chars()
            .take(cols as usize)
            .collect();
        let mut line = Line {
            text,
            ..Line::default()
        };
        line.runs.push(Run {
            start: 0,
            end: 4,
            cols: (0, 4),
            pen: Pen {
                bold: true,
                ..Pen::default()
            },
        });
        line.runs.push(Run {
            start: 5,
            end: line.text.len(),
            cols: (5, cols),
            pen: Pen::default(),
        });
        frame.lines.push(line);
    }
    frame.cursor = Some((6, rows - 1));
    let mut raster = Raster::new();
    let mut samples = Vec::new();
    let mut bytes = 0;
    for _ in 0..20 {
        let at = Instant::now();
        let image = raster
            .compose(&frame, 7.6, 16.5, 2.0, palette())
            .expect("an image");
        samples.push(at.elapsed().as_secs_f64() * 1e3);
        bytes = image.2.len();
        black_box(image);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "texture compose {cols}x{rows} at 2x: median {:.1} ms, p95 {:.1} ms",
        samples[samples.len() / 2],
        samples[(samples.len() as f64 * 0.95) as usize]
    );
    let mut allocate = Vec::new();
    for _ in 0..20 {
        let at = Instant::now();
        black_box(vec![0u8; bytes]);
        allocate.push(at.elapsed().as_secs_f64() * 1e3);
    }
    allocate.sort_by(f64::total_cmp);
    println!(
        "texture allocate {} MiB: median {:.1} ms",
        bytes / (1024 * 1024),
        allocate[allocate.len() / 2]
    );
    let blank = Frame {
        cols,
        rows,
        ..Frame::default()
    };
    let mut background = Vec::new();
    for _ in 0..20 {
        let at = Instant::now();
        black_box(
            raster
                .compose(&blank, 7.6, 16.5, 2.0, palette())
                .expect("an image"),
        );
        background.push(at.elapsed().as_secs_f64() * 1e3);
    }
    background.sort_by(f64::total_cmp);
    println!(
        "texture empty grid: median {:.1} ms (text and fills cost {:.1} ms)",
        background[background.len() / 2],
        samples[samples.len() / 2] - background[background.len() / 2]
    );
    for (label, scale) in [("1x", 1.0), ("1.5x", 1.5)] {
        let mut scaled = Vec::new();
        for _ in 0..20 {
            let at = Instant::now();
            black_box(
                raster
                    .compose(&frame, 7.6, 16.5, scale, palette())
                    .expect("an image"),
            );
            scaled.push(at.elapsed().as_secs_f64() * 1e3);
        }
        scaled.sort_by(f64::total_cmp);
        println!(
            "texture compose {cols}x{rows} at {label}: median {:.1} ms",
            scaled[scaled.len() / 2]
        );
    }
    println!(
        "texture workers: {:?}",
        std::thread::available_parallelism()
    );
}

/// A wide character covers two columns, so the glyph drawn after it starts two cells along. The
/// frame's own record of the wide columns is what the painter advances by; this row marks a
/// narrow glyph wide on purpose, so the test does not depend on a face that installs a CJK one.
#[test]
fn the_cell_after_a_wide_character_is_drawn_at_its_own_column() {
    let mut line = Line {
        text: "l|".to_owned(),
        ..Line::default()
    };
    line.wide.push(0);
    line.runs.push(run(Pen::default(), (0, 3), 0, 2));
    let frame = Frame {
        cols: 4,
        rows: 1,
        lines: vec![line],
        ..Frame::default()
    };
    let (width, _, rgba) = Raster::new()
        .compose(&frame, 10.0, 20.0, 1.0, palette())
        .expect("an image");
    // The viewport is the band below the overscan row: row one of three.
    let ink = |x0: u32, x1: u32| {
        (20..40)
            .flat_map(|y| (x0..x1).map(move |x| (x, y)))
            .filter(|&(x, y)| pixel(&rgba, width, x, y)[3] > 0)
            .count()
    };
    assert_eq!(
        ink(10, 20),
        0,
        "the wide character's second column stays empty"
    );
    assert!(
        ink(20, 30) > 0,
        "the glyph after it is drawn in the cell two columns along"
    );
}

/// An emoji the mono cannot draw comes from the system's colour emoji font, the way Rune draws
/// one in its text: in its own colours, centred across the two cells it covers.
#[test]
fn a_colour_emoji_is_drawn_in_colour_across_its_two_cells() {
    if crate::term::fonts::emoji_face().is_none() {
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
    if crate::term::fonts::emoji_face().is_none() {
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
