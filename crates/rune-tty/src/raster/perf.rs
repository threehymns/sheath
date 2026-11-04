//! The pane's real image path, as a benchmark: `picture()` diffs the frame against the image it
//! last handed over, so a one-row change is what a keystroke echo costs before the view sees it.

use std::hint::black_box;
use std::time::Instant;

use crate::grid::{Frame, Line, Pen, Run};
use crate::raster::{Palette, picture};

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

/// A video frame: a full-pane placement whose pixels are new, which is what the demo
/// sends through the kitty graphics protocol. Every iteration is a whole compose of the image
/// over the grid, so this is the cost that decides whether the video can run at the display's
/// rate. Run with `cargo test --release perf_texture_video -- --ignored --nocapture`.
#[test]
#[ignore = "manual benchmark; reports measurements, not a timing threshold"]
fn perf_texture_video() {
    use crate::grid::{ImageLayer, ImagePixels, Placement};
    use std::sync::Arc;

    let (cols, rows) = (90u16, 24u16);
    let (source_w, source_h) = (660u32, 372u32);
    let mut frame = Frame {
        cols,
        rows,
        ..Frame::default()
    };
    for row in 0..rows {
        let text: String = format!("{row:>4} the quick brown fox jumps over the lazy dog")
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
    let pixels = |tint: u8| {
        Arc::new(ImagePixels {
            width: source_w,
            height: source_h,
            rgba: (0..source_w * source_h)
                .flat_map(|at| {
                    let x = (at % source_w) as u8;
                    let y = (at / source_w) as u8;
                    [x, y, tint, 255]
                })
                .collect(),
        })
    };
    frame.images.push(Placement {
        image: 1,
        data: pixels(0),
        layer: ImageLayer::BelowText,
        z: 0,
        x: 0.0,
        y: 0.0,
        w: 720.0,
        h: 384.0,
        source: [0, 0, source_w, source_h],
    });
    let image = picture(&frame, 8.0, 16.0, 2.0, palette()).expect("an image");
    let (width, height) = image.size();
    let mut samples = Vec::new();
    for tint in 0..60u8 {
        frame.images[0].data = pixels(tint);
        frame.generation = frame.generation.wrapping_add(1);
        let at = Instant::now();
        black_box(picture(&frame, 8.0, 16.0, 2.0, palette()));
        samples.push(at.elapsed().as_secs_f64() * 1e3);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "video frame {cols}x{rows} at 2x, {}x{} px: median {:.1} ms, p95 {:.1} ms",
        source_w,
        source_h,
        samples[samples.len() / 2],
        samples[(samples.len() as f64 * 0.95) as usize]
    );
    println!(
        "  {}x{} device pixels, {:.1} MiB image",
        width,
        height,
        f64::from(width * height * 4) / (1024.0 * 1024.0)
    );
    // Where that goes: the same grid with no placement (rows alone), the placement over blank
    // rows (the scaler alone), and the placement unchanged while one row changes (the path a
    // video that keeps its pixels takes).
    let stage = |label: &str, frame: &mut Frame, times: usize, fresh: bool| {
        let mut samples = Vec::new();
        for index in 0..times {
            if fresh && let Some(placement) = frame.images.first_mut() {
                placement.data = pixels(index as u8);
            }
            if let Some(line) = frame.lines.last_mut() {
                let text = format!("{index:>4} the quick brown fox jumps");
                if let Some(run) = line.runs.first_mut() {
                    run.end = text.len();
                }
                line.text = text;
            }
            frame.generation = frame.generation.wrapping_add(1);
            let at = Instant::now();
            black_box(picture(frame, 8.0, 16.0, 2.0, palette()));
            samples.push(at.elapsed().as_secs_f64() * 1e3);
        }
        samples.sort_by(f64::total_cmp);
        println!("  {label}: median {:.1} ms", samples[samples.len() / 2]);
    };
    let placement = frame.images.pop().expect("the benchmark placed one image");
    let text = std::mem::take(&mut frame.lines);
    stage("rows alone", &mut frame, 40, false);
    frame.images.push(Placement {
        data: Arc::clone(&placement.data),
        ..placement.clone()
    });
    frame.lines = (0..rows).map(|_| Line::default()).collect();
    stage("scaler, blank rows, new image", &mut frame, 40, true);
    frame.lines = text;
    stage("scaler and rows, new image", &mut frame, 40, true);
    stage("image kept, one row changed", &mut frame, 40, false);
    // The scaler on its own, out of `picture()`: one placement over the whole canvas.
    use super::image::{Canvas, paint_image};
    let canvas = Canvas {
        width,
        height,
        scale: 2.0,
        line_px: 32,
    };
    let mut dst = vec![0u8; (width * height * 4) as usize];
    let mut samples = Vec::new();
    for _ in 0..20 {
        dst.fill(0);
        let at = Instant::now();
        paint_image(&mut dst, canvas, &placement, [0; 3], (0, height as i32));
        samples.push(at.elapsed().as_secs_f64() * 1e3);
    }
    samples.sort_by(f64::total_cmp);
    println!(
        "  scaler alone, {}x{} px: median {:.1} ms",
        width,
        height,
        samples[samples.len() / 2]
    );
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
    // A scroll: every row moves up one band and a line enters at the bottom, which is what the
    // wheel costs once per line it crosses and what a line of a program's output costs.
    let mut scrolled = Vec::new();
    for index in 0..200 {
        frame.lines.remove(0);
        let text = format!("{index:>4} compiled sheath in 1.2s");
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
        frame.generation = frame.generation.wrapping_add(1);
        let at = Instant::now();
        black_box(picture(&frame, 7.6, 16.5, 2.0, palette()));
        scrolled.push(at.elapsed().as_secs_f64() * 1e3);
    }
    scrolled.sort_by(f64::total_cmp);
    println!(
        "picture {cols}x{rows} at 2x, scrolled one row: median {:.3} ms, p95 {:.3} ms",
        scrolled[scrolled.len() / 2],
        scrolled[(scrolled.len() as f64 * 0.95) as usize]
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
