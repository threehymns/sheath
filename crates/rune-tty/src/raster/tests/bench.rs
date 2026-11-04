//! The compose loop at a fullscreened grid, timed by hand.

use super::palette;
use crate::grid::{Frame, Line, Pen, Run};
use crate::raster::Raster;

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
