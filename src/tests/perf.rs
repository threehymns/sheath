//! Frame-time benchmarks for scrolling, ignored by default. They drive the real Rune runtime
//! headlessly, so they cover the app's poll, its view build, and Rune's layout and paint.
//!
//! Run with:
//!
//! ```sh
//! cargo test --release perf_ -- --ignored --nocapture --test-threads=1
//! ```

use super::helpers::*;
use crate::app::App;
use rune::text::TextSystem;
use rune::{Env, Runtime, WheelEvent};
use std::time::{Duration, Instant};
use tempfile::TempDir;

struct Report {
    label: String,
    median: f64,
    p95: f64,
    mean: f64,
    rows: usize,
}

// Times whole frames around `each`, after warm-up frames that run the same action so the list
// is already moving when the sample starts.
fn measure(
    label: &str,
    rt: &mut Runtime<App>,
    frames: usize,
    mut each: impl FnMut(&mut Runtime<App>),
) -> Report {
    for _ in 0..20 {
        each(rt);
        rt.frame(1.0 / 60.0);
    }
    let mut samples = Vec::with_capacity(frames);
    for _ in 0..frames {
        each(rt);
        let at = Instant::now();
        rt.frame(1.0 / 60.0);
        samples.push(at.elapsed());
    }
    let mut ms: Vec<f64> = samples
        .iter()
        .map(|d: &Duration| d.as_secs_f64() * 1e3)
        .collect();
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    Report {
        label: label.to_owned(),
        median: ms[ms.len() / 2],
        p95: ms[(ms.len() as f64 * 0.95) as usize],
        mean: ms.iter().sum::<f64>() / ms.len() as f64,
        rows: rt
            .elements()
            .iter()
            .filter(|e| e.key.as_deref().is_some_and(|k| k.starts_with("f:")))
            .count(),
    }
}

fn print(reports: &[Report]) {
    println!(
        "\n{:<34} {:>9} {:>9} {:>9} {:>9} {:>7}",
        "scenario", "median ms", "p95 ms", "mean ms", "fps", "rows"
    );
    for r in reports {
        println!(
            "{:<34} {:>9.2} {:>9.2} {:>9.2} {:>9.0} {:>7}",
            r.label,
            r.median,
            r.p95,
            r.mean,
            1000.0 / r.mean,
            r.rows
        );
    }
    println!();
}

fn list_runtime(count: usize) -> (TempDir, Runtime<App>) {
    let dir = long_folder(count);
    let mut rt = runtime(app(&dir));
    rt.state.config.scroll_inertia = true;
    rt.state.config.overscroll = true;
    rt.frame(0.0);
    (dir, rt)
}

fn grid_runtime(count: usize) -> (TempDir, Runtime<App>) {
    let (dir, mut rt) = list_runtime(count);
    rt.state.toggle_layout();
    rt.frame(0.0);
    (dir, rt)
}

fn sized(app: App, size: (f32, f32), scale: f32) -> Runtime<App> {
    let text = TextSystem::new(rune_theme_opencode::FONTS, rune_theme_opencode::FAMILIES);
    let mut rt = Runtime::with_text(app, crate::ui::view, size, Env::new(true, scale), text);
    rt.on_key = Some(crate::app::on_key);
    rt.poll = Some(crate::app::poll);
    rt
}

fn wheel(rt: &mut Runtime<App>) {
    let (x, y) = rt.find("files-table").expect("the list is on screen");
    for _ in 0..4 {
        rt.wheel_event(WheelEvent::new(0.0, -9.0).precise(true).at(x, y));
    }
}

#[test]
#[ignore]
fn perf_scroll() {
    let mut reports = Vec::new();
    for count in [200usize, 20_000] {
        let (_dir, mut rt) = list_runtime(count);
        reports.push(measure(&format!("list {count} idle"), &mut rt, 120, |_| {}));
        reports.push(measure(&format!("list {count} wheel"), &mut rt, 120, wheel));
        let (_dir, mut rt) = grid_runtime(count);
        reports.push(measure(&format!("grid {count} idle"), &mut rt, 120, |_| {}));
        reports.push(measure(&format!("grid {count} wheel"), &mut rt, 120, wheel));
    }
    print(&reports);
}

// A frame that rebuilds the view without scrolling; it isolates the per-commit cost of the
// whole entry list from the per-frame cost of the window.
#[test]
#[ignore]
fn perf_forced_commit() {
    let mut reports = Vec::new();
    for count in [200usize, 20_000] {
        let (_dir, mut rt) = list_runtime(count);
        reports.push(measure(
            &format!("list {count} commit"),
            &mut rt,
            120,
            Runtime::invalidate,
        ));
        let (_dir, mut rt) = grid_runtime(count);
        reports.push(measure(
            &format!("grid {count} commit"),
            &mut rt,
            120,
            Runtime::invalidate,
        ));
    }
    print(&reports);
}

#[test]
#[ignore]
fn perf_phrase_count() {
    for count in [200usize, 20_000] {
        let (_dir, mut rt) = list_runtime(count);
        rt.frame(0.0);
        let at = Instant::now();
        for _ in 0..100 {
            std::hint::black_box(rt.state.phrase_for_count());
        }
        println!(
            "phrase_for_count {count}: {:.3} ms",
            at.elapsed().as_secs_f64() * 1e3 / 100.0
        );
    }
}

// Rune lays out in points, so a bigger window means more rows in the window and more painting;
// the device scale alone only changes the pixel size of the same points. These run headlessly,
// so they time the view build, layout and paint encoding, not the GPU's rasterizing.
#[test]
#[ignore]
fn perf_resolution() {
    let dir = long_folder(20_000);
    let sizes = [
        (800.0, 600.0, 2.0),
        (1180.0, 760.0, 2.0),
        (1920.0, 1080.0, 1.0),
        (1920.0, 1080.0, 2.0),
        (2560.0, 1440.0, 1.0),
        (3840.0, 2160.0, 1.0),
    ];
    let mut reports = Vec::new();
    for (w, h, scale) in sizes {
        let mut rt = sized(app(&dir), (w, h), scale);
        rt.state.config.scroll_inertia = true;
        rt.state.config.overscroll = true;
        rt.frame(0.0);
        reports.push(measure(
            &format!("list {w}x{h}@{scale}x wheel"),
            &mut rt,
            120,
            wheel,
        ));
        let mut rt = sized(app(&dir), (w, h), scale);
        rt.state.config.scroll_inertia = true;
        rt.state.toggle_layout();
        rt.frame(0.0);
        reports.push(measure(
            &format!("grid {w}x{h}@{scale}x wheel"),
            &mut rt,
            120,
            wheel,
        ));
    }
    print(&reports);
}

// The coast is the most common scroll frame: mostly no input, the offset moves and the virtual
// window slides. A wheel lands every sixth frame so the list keeps moving through the sample.
#[test]
#[ignore]
fn perf_coast() {
    let mut reports = Vec::new();
    for count in [200usize, 20_000] {
        let (_dir, mut rt) = list_runtime(count);
        let tick = std::cell::Cell::new(0usize);
        reports.push(measure(
            &format!("list {count} coast"),
            &mut rt,
            150,
            |rt| {
                if tick.get().is_multiple_of(6) {
                    wheel(rt);
                }
                tick.set(tick.get() + 1);
            },
        ));
    }
    print(&reports);
}
