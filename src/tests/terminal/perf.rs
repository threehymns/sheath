use std::time::Instant;

use rune::{Key, KeyEvent, Mods, Runtime};

use crate::app::App;
use crate::tests::helpers::{app, fixture, long_folder, runtime};
use rune_tty::engine::Engine;
use rune_tty::grid::{Frame, Line, Pen, Run};

fn stats(mut samples: Vec<f64>) -> (f64, f64) {
    samples.sort_by(f64::total_cmp);
    (
        samples[samples.len() / 2],
        samples[(samples.len() as f64 * 0.95) as usize],
    )
}

/// A screen of mixed weight and colour: what `cargo build` output looks like.
fn vt_screen(rows: usize, cols: usize) -> String {
    let mut out = String::new();
    let body = "the quick brown fox jumps over the lazy dog compiling sheath v0.1.0 ";
    for row in 0..rows {
        let text: String = body.repeat(cols / body.len() + 1);
        out.push_str(&format!(
            "\x1b[3{}m{row:>4}\x1b[0m \x1b[1mcompiling\x1b[0m {}\x1b[0m\r\n",
            row % 8,
            &text[..cols.saturating_sub(24)]
        ));
    }
    out
}

fn styled_frame(rows: usize, cols: usize) -> Frame {
    let mut frame = Frame {
        cols: cols as u16,
        rows: rows as u16,
        ..Frame::default()
    };
    let body = "the quick brown fox jumps over the lazy dog compiling sheath v0.1.0 ";
    for row in 0..rows {
        let text: String = format!("{row:>4} {}", body.repeat(cols / body.len() + 2));
        let text: String = text.chars().take(cols).collect();
        let mut line = Line {
            text,
            ..Line::default()
        };
        if line.text.len() > 6 {
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
                cols: (5, line.text.chars().count() as u16),
                pen: Pen::default(),
            });
        }
        frame.lines.push(line);
    }
    frame.cursor = Some((6, rows as u16 - 1));
    frame
}

/// The grid rebuild on its own: a real parser, a real screen, and the read back into lines.
/// `one line` rewrites the top row, which is what a keystroke echo costs.
#[test]
#[ignore]
fn perf_terminal_grid() {
    let dir = tempfile::tempdir().unwrap();
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    let (cols, rows) = (140u16, 14u16);
    let mut engine = Engine::spawn(
        dir.path(),
        cols,
        rows,
        (8.0, 16.0),
        2.0,
        rune_tty::theme(true),
        wake,
    )
    .unwrap();
    engine.feed(vt_screen(rows as usize, cols as usize).as_bytes());

    // The pane's own path: one frame reused, so this is the read back and not an allocation
    // per row.
    let mut frame = rune_tty::grid::Frame::default();
    let mut full = Vec::new();
    let screen = vt_screen(rows as usize, cols as usize);
    for _ in 0..200 {
        engine.feed(screen.as_bytes());
        let at = Instant::now();
        engine.fill(&mut frame);
        full.push(at.elapsed().as_secs_f64() * 1e3);
        std::hint::black_box(&frame);
    }
    let mut echo = Vec::new();
    for i in 0..200 {
        engine.feed(format!("\x1b[1;1H\x1b[36mcompiled {i}\x1b[0m").as_bytes());
        let at = Instant::now();
        engine.fill(&mut frame);
        echo.push(at.elapsed().as_secs_f64() * 1e3);
        std::hint::black_box(&frame);
    }
    // An idle drain is what a pane pays per frame when nothing new arrived: the shells's bytes
    // are all parsed, and the frame loop still asks whether anything moved.
    let mut idle = Vec::new();
    for _ in 0..200 {
        let at = Instant::now();
        std::hint::black_box(engine.drain());
        idle.push(at.elapsed().as_secs_f64() * 1e6);
    }
    let idle = {
        idle.sort_by(f64::total_cmp);
        idle
    };
    let (fm, fp) = stats(full);
    let (em, ep) = stats(echo);
    println!("grid whole screen {cols}x{rows}: median {fm:.3} ms, p95 {fp:.3} ms");
    println!("grid one line changed: median {em:.3} ms, p95 {ep:.3} ms");
    println!(
        "drain idle: median {:.3} us, p95 {:.3} us",
        idle[idle.len() / 2],
        idle[(idle.len() as f64 * 0.95) as usize]
    );
}

/// The commit the pane costs on top of the browser: idle, and with one row changing (a
/// keystroke echo). The folder is small so the terminal is what the frame is made of.
#[test]
#[ignore]
fn perf_terminal_commit() {
    let dir = fixture();
    let mut rt: Runtime<App> = runtime(app(&dir));
    rt.state.term.show_for_test(styled_frame(14, 140));
    rt.frame(0.0);

    let mut idle = Vec::new();
    for _ in 0..120 {
        let at = Instant::now();
        rt.frame(1.0 / 60.0);
        idle.push(at.elapsed().as_secs_f64() * 1e3);
    }
    let mut typing = Vec::new();
    for i in 0..120 {
        let last = rt.state.term.frame.lines.len() - 1;
        rt.state.term.frame.lines[last].text = format!("{i:>4} compiled sheath in 1.2s");
        rt.invalidate();
        let at = Instant::now();
        rt.frame(1.0 / 60.0);
        typing.push(at.elapsed().as_secs_f64() * 1e3);
    }
    let (im, ip) = stats(idle);
    let (tm, tp) = stats(typing);
    println!("terminal idle frame: median {im:.3} ms, p95 {ip:.3} ms");
    println!("terminal one-row frame: median {tm:.3} ms, p95 {tp:.3} ms");

    // A key that reaches the shell: the encode path alone, without the PTY round trip.
    let at = Instant::now();
    for _ in 0..1_000 {
        rt.key(KeyEvent {
            key: Key::Text("a".into()),
            mods: Mods::default(),
        });
    }
    println!(
        "key routed with no shell: {:.4} ms each",
        at.elapsed().as_secs_f64() * 1e3 / 1_000.0
    );
}

/// A frame the pane draws with nothing new from the shell: the fractional step of a smooth
/// scroll, the frame after a blink, a view build that only moved a hover. The grid is read back
/// into the same frame the raster streams, so this is the whole cost of that frame at this level.
#[test]
#[ignore]
fn perf_terminal_unchanged_frame() {
    let dir = tempfile::tempdir().unwrap();
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    let (cols, rows) = (140u16, 14u16);
    let mut engine = Engine::spawn(
        dir.path(),
        cols,
        rows,
        (8.0, 16.0),
        2.0,
        rune_tty::theme(true),
        wake,
    )
    .unwrap();
    engine.feed(vt_screen(rows as usize, cols as usize).as_bytes());
    let mut frame = Frame::default();
    engine.fill(&mut frame);
    // The fractional scroll the pane asks for on every wheel notch: the overscan rows are
    // already read, so the edges are a no-op and only the readback is left.
    engine.ensure_edges(true);
    engine.fill(&mut frame);
    let mut samples = Vec::new();
    for _ in 0..500 {
        let at = Instant::now();
        engine.ensure_edges(true);
        engine.fill(&mut frame);
        samples.push(at.elapsed().as_secs_f64() * 1e6);
        std::hint::black_box(&frame);
    }
    let (median, p95) = stats(samples);
    println!(
        "unchanged frame {cols}x{rows} (a smooth scroll): median {median:.1} us, p95 {p95:.1} us"
    );
}

/// The pane against the heaviest thing a program can draw: a full-screen video player, on a real PTY,
/// through the same parse, readback and raster the app runs every frame. The demo is started as
/// the pane's shell, so its capability queries are answered by the engine the way they are for a
/// real terminal. Reports the frame rate the pane sustains while the demo runs, and where the
/// time went.
#[test]
#[ignore]
fn perf_video_stream() {
    use rune_tty::raster::{Palette, picture};

    if std::env::var_os("PERF_COMMAND").is_none() {
        eprintln!("set PERF_COMMAND to the video player and its arguments");
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let script = root.path().join("demo");
    std::fs::write(&script, "#!/bin/sh\nexec $PERF_COMMAND\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    // The pane starts `$SHELL` with no arguments, so the demo script is the shell for this run.
    // SAFETY: this is a manual benchmark, and the process was started by the test harness.
    unsafe { std::env::set_var("SHELL", &script) };
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    // The pane's own size, so the same demo can be measured at a few grid sizes: PERF_COLS and
    // PERF_ROWS override it for a run that asks how throughput scales.
    let dim = |name: &str, default: u16| {
        std::env::var(name)
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(default)
    };
    let (cols, rows) = (dim("PERF_COLS", 180), dim("PERF_ROWS", 48));
    let mut engine = Engine::spawn(
        root.path(),
        cols,
        rows,
        (8.0, 16.0),
        2.0,
        rune_tty::theme(true),
        wake,
    )
    .expect("an engine");
    unsafe { std::env::remove_var("SHELL") };

    let palette = Palette {
        bg: rune_tty::raster::rgb(rune_theme_opencode::kit(true).c.bg_base),
        selection: rune_tty::raster::rgb(rune_theme_opencode::kit(true).c.text_accent),
        search: rune_tty::raster::rgb(rune_theme_opencode::kit(true).c.text_accent),
    };
    let mut frame = Frame::default();
    // The first fill composes the whole image; every later one diffs against it, as in the app.
    engine.drain();
    engine.fill(&mut frame);
    let start = Instant::now();
    let deadline = start + std::time::Duration::from_secs(20);
    // Only frames with new content are built, the way the app's poll builds one per wake.
    let mut drawn = 0u64;
    let mut changed_frames = 0u64;
    let mut parse = std::time::Duration::ZERO;
    let mut readback = std::time::Duration::ZERO;
    let mut raster = std::time::Duration::ZERO;
    let mut spans: Vec<f64> = Vec::new();
    let mut last_parsed = engine.stats().parsed;
    let mut last_report = start;
    let mut last_image = 0u32;
    let mut image_changes = 0u64;
    // PERF_PACED runs the loop the way the app's frame loop does: one poll per display frame,
    // so the parse budget is a share of the wall clock instead of the whole of it.
    let paced = std::env::var("PERF_PACED").is_ok();
    let frame_time = std::time::Duration::from_secs_f64(1.0 / 60.0);
    let pace = |at: Instant| {
        if paced {
            let spent = at.elapsed();
            if spent < frame_time {
                std::thread::sleep(frame_time - spent);
            }
        }
    };
    while Instant::now() < deadline && !engine.shell_exited() {
        let at = Instant::now();
        let changed = engine.drain();
        let after_drain = Instant::now();
        if !changed {
            if paced {
                pace(at);
            } else {
                std::thread::yield_now();
            }
            continue;
        }
        changed_frames += 1;
        engine.fill(&mut frame);
        let after_fill = Instant::now();
        std::hint::black_box(picture(&frame, 8.0, 16.0, 2.0, palette));
        let after_picture = Instant::now();
        parse += after_drain - at;
        readback += after_fill - after_drain;
        raster += after_picture - after_fill;
        spans.push(after_picture.duration_since(at).as_secs_f64() * 1e3);
        drawn += 1;
        if let Some(placement) = frame.images.first()
            && placement.image != last_image
        {
            last_image = placement.image;
            image_changes += 1;
        }
        if last_report.elapsed() >= std::time::Duration::from_secs(1) {
            last_report = Instant::now();
            let stats = engine.stats();
            let image = frame.images.first().map(|p| {
                format!(
                    "{}x{} -> {}x{} pt, {}x{} px source",
                    p.data.width, p.data.height, p.w, p.h, p.source[2], p.source[3]
                )
            });
            println!(
                "{:>5.1}s: {:.1} frames/s drawn, {} image changes, image {:?}, {} MiB parsed/s, {} KiB queued, lag {:?}",
                start.elapsed().as_secs_f64(),
                drawn as f64 / start.elapsed().as_secs_f64(),
                image_changes,
                image,
                (stats.parsed - last_parsed) >> 20,
                stats.queued >> 10,
                stats.lag
            );
            last_parsed = stats.parsed;
        }
        pace(at);
    }
    if std::env::var("PERF_DUMP").is_ok() {
        for line in &frame.lines {
            println!("| {}", line.text);
        }
    }
    let elapsed = start.elapsed().as_secs_f64();
    let stats = engine.stats();
    spans.sort_by(f64::total_cmp);
    let pct = |f: f64| spans[((spans.len() - 1) as f64 * f) as usize];
    println!(
        "a full-screen video player {cols}x{rows}: {drawn} drawn frames ({changed_frames} changed) in {elapsed:.2}s — {:.1} fps",
        drawn as f64 / elapsed
    );
    println!(
        "  frame p50 {:.1} ms, p95 {:.1} ms, max {:.1} ms",
        pct(0.5),
        pct(0.95),
        spans[spans.len() - 1]
    );
    println!(
        "  parse {:.2} ms/frame, readback {:.2} ms/frame, raster {:.2} ms/frame",
        parse.as_secs_f64() * 1e3 / drawn as f64,
        readback.as_secs_f64() * 1e3 / drawn as f64,
        raster.as_secs_f64() * 1e3 / drawn as f64,
    );
    println!(
        "  {} MiB parsed total, {} KiB still queued, lag {:?}, {image_changes} video frames",
        stats.parsed >> 20,
        stats.queued >> 10,
        stats.lag
    );
}

/// A long folder beside the pane, to show what the terminal's frames cost on top of a list.
#[test]
#[ignore]
fn perf_terminal_beside_a_long_list() {
    let dir = long_folder(2_000);
    let mut rt: Runtime<App> = runtime(app(&dir));
    rt.state.term.show_for_test(styled_frame(14, 140));
    rt.frame(0.0);
    let mut idle = Vec::new();
    for _ in 0..120 {
        let at = Instant::now();
        rt.frame(1.0 / 60.0);
        idle.push(at.elapsed().as_secs_f64() * 1e3);
    }
    let mut typing = Vec::new();
    for i in 0..120 {
        let last = rt.state.term.frame.lines.len() - 1;
        rt.state.term.frame.lines[last].text = format!("{i:>4} compiled sheath in 1.2s");
        rt.invalidate();
        let at = Instant::now();
        rt.frame(1.0 / 60.0);
        typing.push(at.elapsed().as_secs_f64() * 1e3);
    }
    let (im, ip) = stats(idle);
    let (tm, tp) = stats(typing);
    println!("list+terminal idle frame: median {im:.3} ms, p95 {ip:.3} ms");
    println!("list+terminal one-row frame: median {tm:.3} ms, p95 {tp:.3} ms");
}

/// How fast the pane takes plain bytes off the pty: the ceiling any program that streams a lot
/// of output runs into. A video frame through the kitty graphics protocol is megabytes, so this
/// is what decides how many of them the screen can show.
#[test]
#[ignore]
fn perf_pty_throughput() {
    let root = tempfile::tempdir().unwrap();
    let script = root.path().join("flood");
    std::fs::write(
        &script,
        "#!/bin/sh\nhead -c 100000000 /dev/zero | tr '\\0' 'y'\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    // SAFETY: this is a manual benchmark, and the process was started by the test harness.
    unsafe { std::env::set_var("SHELL", &script) };
    let wake: rune::Waker = std::sync::Arc::new(|| {});
    let mut engine = Engine::spawn(
        root.path(),
        80,
        24,
        (8.0, 16.0),
        1.0,
        rune_tty::theme(true),
        wake,
    )
    .expect("an engine");
    unsafe { std::env::remove_var("SHELL") };
    let start = Instant::now();
    let mut settled = 0;
    while start.elapsed() < std::time::Duration::from_secs(60) {
        if engine.drain() {
            settled = 0;
        } else if engine.shell_exited() && engine.stats().queued == 0 {
            settled += 1;
            if settled > 20 {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }
    let stats = engine.stats();
    let elapsed = start.elapsed().as_secs_f64();
    let mib = stats.parsed as f64 / (1024.0 * 1024.0);
    println!(
        "pty throughput: {mib:.1} MiB in {elapsed:.2}s — {:.1} MiB/s, {} KiB left queued",
        mib / elapsed,
        stats.queued >> 10
    );
}
