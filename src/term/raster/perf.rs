//! The pane's real image path, as a benchmark: `picture()` diffs the frame against the image it
//! last handed over, so a one-row change is what a keystroke echo costs before the view sees it.

use std::hint::black_box;
use std::time::Instant;

use crate::term::grid::{Frame, Line, Pen, Run};
use crate::term::raster::{Palette, picture};

/// A palette of its own, so the benchmark does not depend on a theme.
fn palette() -> Palette {
    Palette {
        bg: [0; 3],
        selection: [255; 3],
        search: [255; 3],
    }
}

/// A screen of plain text, one run per row: what a prompt and a build log leave behind.
fn screen(rows: u16, cols: u16) -> Frame {
    let mut frame = Frame {
        cols,
        rows,
        ..Frame::default()
    };
    for row in 0..rows {
        let text: String =
            format!("{row:>4} the quick brown fox jumps over the lazy dog, compiling")
                .chars()
                .take(cols as usize)
                .collect();
        frame.lines.push(Line {
            runs: vec![Run {
                start: 0,
                end: text.len(),
                cols: (0, cols),
                pen: Pen::default(),
            }],
            text,
            ..Line::default()
        });
    }
    frame
}

/// A keystroke's echo: one row rewritten on a screen that is otherwise the same.
/// Run with `cargo test --release perf_texture_picture -- --ignored --nocapture`.
#[test]
#[ignore = "manual benchmark; reports measurements, not a timing threshold"]
fn perf_texture_picture() {
    let (cols, rows) = (140u16, 14u16);
    let mut frame = screen(rows, cols);
    // The first picture composes the whole grid; every one after it diffs against this image.
    let image = picture(&frame, 7.6, 16.5, 2.0, palette()).expect("an image");
    let (width, height) = image.size();
    let mut samples = Vec::new();
    for index in 0..200 {
        let last = frame.lines.len() - 1;
        frame.lines[last].text = format!("{index:>4} compiled sheath in 1.2s");
        frame.lines[last].runs[0].end = frame.lines[last].text.len();
        frame.generation = frame.generation.wrapping_add(1);
        let at = Instant::now();
        black_box(picture(&frame, 7.6, 16.5, 2.0, palette()));
        samples.push(at.elapsed().as_secs_f64() * 1e3);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "picture {cols}x{rows} at 2x, one row changed: median {:.3} ms, p95 {:.3} ms ({:.1} MiB image)",
        samples[samples.len() / 2],
        samples[(samples.len() as f64 * 0.95) as usize],
        f64::from(width * height * 4) / (1024.0 * 1024.0)
    );
    // Every row rewritten, which is what a whole screen of bands is painted by: the same work a
    // scroll used to do before the image moved with it.
    let mut repaint = Vec::new();
    for index in 0..200 {
        for line in frame.lines.iter_mut() {
            line.text = format!("{index:>4} compiled sheath in 1.2s");
            line.runs[0].end = line.text.len();
        }
        frame.generation = frame.generation.wrapping_add(1);
        let at = Instant::now();
        black_box(picture(&frame, 7.6, 16.5, 2.0, palette()));
        repaint.push(at.elapsed().as_secs_f64() * 1e3);
    }
    repaint.sort_by(f64::total_cmp);
    println!(
        "picture {cols}x{rows} at 2x, every row changed: median {:.3} ms, p95 {:.3} ms",
        repaint[repaint.len() / 2],
        repaint[(repaint.len() as f64 * 0.95) as usize]
    );
    // The copy the image hands the runtime, on its own: the pixels are kept between frames to
    // diff against, so a repaint copies the whole image into the `Pixels`.
    let len = (width * height * 4) as usize;
    let mut copy = Vec::new();
    let source = vec![0u8; len];
    for _ in 0..200 {
        let at = Instant::now();
        black_box(source.clone());
        copy.push(at.elapsed().as_secs_f64() * 1e3);
    }
    copy.sort_by(f64::total_cmp);
    println!(
        "image copy {:.1} MiB: median {:.3} ms",
        len as f64 / (1024.0 * 1024.0),
        copy[copy.len() / 2]
    );
}
