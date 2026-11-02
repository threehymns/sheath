use std::time::Instant;

use rune::{Key, KeyEvent, Mods, Runtime};

use crate::app::App;
use crate::term::engine::Engine;
use crate::term::grid::{Frame, Line, Pen, Run};
use crate::tests::helpers::{app, fixture, long_folder, runtime};

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
        crate::term::theme(true),
        wake,
    )
    .unwrap();
    engine.feed(vt_screen(rows as usize, cols as usize).as_bytes());

    // The pane's own path: one frame reused, so this is the read back and not an allocation
    // per row.
    let mut frame = crate::term::grid::Frame::default();
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
    let (fm, fp) = stats(full);
    let (em, ep) = stats(echo);
    println!("grid whole screen {cols}x{rows}: median {fm:.3} ms, p95 {fp:.3} ms");
    println!("grid one line changed: median {em:.3} ms, p95 {ep:.3} ms");
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
