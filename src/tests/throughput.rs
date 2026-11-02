//! Terminal-pane throughput spike, ignored by default. It draws a 40×180 grid of styled text —
//! the size of a bottom pane in the app's default window — through the real Rune runtime,
//! headlessly, and times whole frames while the grid changes.
//!
//! Run with:
//!
//! ```sh
//! cargo test --release throughput_terminal -- --ignored --nocapture --test-threads=1
//! ```

use rune::text::TextSystem;
use rune::{
    Color, Cx, El, Env, GrowthSpec, Runtime, ScrollSpec, Span, TextStyle, WheelEvent, col, el, row,
    text,
};
use rune_kit::Kit;
use rune_theme_opencode as theme;
use std::borrow::Cow;
use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

/// A bottom pane in the app's default window: 40 rows of 180 columns.
const ROWS: usize = 40;
const COLS: usize = 180;
/// Scrollback depth for the smooth-scroll scenario.
const SCROLLBACK: usize = 4000;
/// 13 pt IBM Plex Mono at 18 pt line height, as the diff view reads code.
const LINE_H: f32 = 18.0;
const FRAME: f64 = 1.0 / 60.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// Four colour runs a row: ordinary prompt or log output.
    Runs,
    /// A colour change at every cell: the worst case a program can hand a terminal.
    Dense,
    /// One text element per colour run instead of spans on one element.
    Elements,
    /// A scroll viewport over SCROLLBACK rows, moved a few points a frame.
    Scroll,
}

impl Mode {
    fn label(self) -> &'static str {
        match self {
            Mode::Runs => "4 runs/row (spans)",
            Mode::Dense => "180 runs/row (spans)",
            Mode::Elements => "4 runs/row (elements)",
            Mode::Scroll => "smooth scroll, 4000 rows",
        }
    }

    fn spans_per_row(self) -> usize {
        match self {
            Mode::Dense => COLS,
            _ => 4,
        }
    }

    fn scrolls(self) -> bool {
        self == Mode::Scroll
    }
}

/// One row of the terminal at scrollback index `i`, as spans on one text element.
fn text_row(i: usize, font: TextStyle, palette: [Color; 4]) -> El {
    let per = COLS / 4;
    let spans = (0..4)
        .map(|c| Span::new(c * per, (c + 1) * per).color(palette[c % palette.len()]))
        .collect::<Vec<_>>();
    text(base_row(i))
        .key(format!("row-{i}"))
        .font(font)
        .spans(spans)
}

struct Row {
    text: String,
}

struct Screen {
    rows: Vec<Row>,
    mode: Mode,
    tick: u64,
    /// The scroll viewport's offset, so the run can prove it moved.
    offset: Rc<Cell<f32>>,
}

impl Screen {
    fn new(mode: Mode) -> Screen {
        Screen {
            rows: (0..ROWS).map(|r| Row { text: base_row(r) }).collect(),
            mode,
            tick: 0,
            offset: Rc::new(Cell::new(0.0)),
        }
    }

    /// Rewrites the first `rows` rows, as a program printing or scrolling would.
    fn touch(&mut self, rows: usize) {
        self.tick += 1;
        for row in self.rows.iter_mut().take(rows) {
            let head = format!("{:06}", self.tick % 1_000_000);
            row.text.replace_range(..6, &head);
        }
    }

    fn spans(&self, k: &Kit) -> Vec<Span> {
        let palette = palette(k);
        let per = COLS / self.mode.spans_per_row();
        (0..self.mode.spans_per_row())
            .map(|i| Span::new(i * per, (i + 1) * per).color(palette[i % palette.len()]))
            .collect()
    }
}

/// ASCII only, so a span's byte range is its cell range.
fn base_row(r: usize) -> String {
    const WORDS: [&str; 8] = [
        "cargo",
        "test",
        "--release",
        "sheath",
        "compiling",
        "rune",
        "ok",
        "done",
    ];
    let mut line = String::with_capacity(COLS);
    let mut w = r;
    while line.len() < COLS {
        line.push_str(WORDS[w % WORDS.len()]);
        line.push(' ');
        w += 1;
    }
    line.truncate(COLS);
    line
}

fn palette(k: &Kit) -> [Color; 4] {
    [k.c.text_base, k.c.success.fg, k.c.danger.fg, k.c.text_muted]
}

fn view(s: &Screen, cx: &mut Cx<Screen>) -> El {
    let k = theme::kit(cx.system_dark());
    let font = TextStyle {
        size: 13.0,
        line_height: LINE_H,
        ..k.ty.mono
    };
    let palette = palette(k);
    if s.mode.scrolls() {
        let probe = s.offset.clone();
        let viewport = col()
            .key("terminal-scroll")
            .w_full()
            .grow(1.0)
            .min_h(0.0)
            .scroll(ScrollSpec {
                stick_bottom: false,
            })
            .on_scroll_offset(move |at| probe.set(at.offset[1]));
        let list = col()
            .key("scroll-rows")
            .w_full()
            .growth(GrowthSpec::default())
            .virtual_rows(
                SCROLLBACK,
                LINE_H,
                |_: &Screen, i: usize| Cow::Owned(format!("row-{i}")),
                move |_: &Screen, i: usize| text_row(i, font, palette),
            );
        return el()
            .key("terminal")
            .w_full()
            .h_full()
            .fill(k.c.bg_base)
            .child(viewport.child(list));
    }
    let mut screen = col()
        .key("terminal")
        .w_full()
        .h_full()
        .min_h(0.0)
        .px(8.0)
        .py(8.0)
        .fill(k.c.bg_base);
    for (i, r) in s.rows.iter().enumerate() {
        screen = screen.child(match s.mode {
            Mode::Elements => {
                let per = COLS / 4;
                let mut line = row().key(format!("row-{i}")).h(LINE_H);
                for (c, color) in palette.iter().enumerate() {
                    line = line.child(
                        text(r.text[c * per..(c + 1) * per].to_owned())
                            .key(format!("run-{i}-{c}"))
                            .font(font)
                            .color(*color),
                    );
                }
                line
            }
            Mode::Runs | Mode::Dense | Mode::Scroll => text(r.text.clone())
                .key(format!("row-{i}"))
                .font(font)
                .color(k.c.text_base)
                .spans(s.spans(k)),
        });
    }
    screen
}

fn runtime(mode: Mode) -> Runtime<Screen> {
    let text = TextSystem::new(rune_theme_opencode::FONTS, rune_theme_opencode::FAMILIES);
    Runtime::with_text(
        Screen::new(mode),
        view,
        (1440.0, 760.0),
        Env::new(true, 2.0),
        text,
    )
}

struct Report {
    label: String,
    median: f64,
    p95: f64,
    mean: f64,
    spans: usize,
    elements: usize,
}

fn measure(
    label: &str,
    rt: &mut Runtime<Screen>,
    frames: usize,
    mut each: impl FnMut(&mut Runtime<Screen>),
) -> Report {
    for _ in 0..20 {
        each(rt);
        rt.frame(FRAME);
    }
    let mut samples = Vec::with_capacity(frames);
    for _ in 0..frames {
        each(rt);
        let at = Instant::now();
        rt.frame(FRAME);
        samples.push(at.elapsed());
    }
    let mut ms: Vec<f64> = samples
        .iter()
        .map(|d: &Duration| d.as_secs_f64() * 1e3)
        .collect();
    ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = ms.iter().sum::<f64>() / ms.len() as f64;
    Report {
        label: label.to_owned(),
        median: ms[ms.len() / 2],
        p95: ms[(ms.len() as f64 * 0.95) as usize],
        mean,
        spans: rt.state.mode.spans_per_row() * ROWS,
        elements: rt.elements().len(),
    }
}

fn print(reports: &[Report]) {
    println!(
        "\n{:<38} {:>10} {:>8} {:>8} {:>9} {:>7} {:>8}",
        "scenario", "median ms", "p95 ms", "fps", "cells/s", "spans", "elements"
    );
    for r in reports {
        println!(
            "{:<38} {:>10.2} {:>8.2} {:>8.0} {:>9.0} {:>7} {:>8}",
            r.label,
            r.median,
            r.p95,
            1000.0 / r.mean,
            ROWS as f64 * COLS as f64 * 1000.0 / r.mean,
            r.spans,
            r.elements
        );
    }
    println!();
}

#[test]
#[ignore]
fn throughput_terminal() {
    let mut reports = Vec::new();
    for mode in [Mode::Runs, Mode::Dense, Mode::Elements] {
        let mut rt = runtime(mode);
        rt.frame(0.0);
        reports.push(measure(
            &format!("{} full repaint", mode.label()),
            &mut rt,
            120,
            |rt| rt.state.touch(ROWS),
        ));
    }
    let mut rt = runtime(Mode::Runs);
    rt.frame(0.0);
    reports.push(measure(
        "4 runs/row, 1 row changed (typing)",
        &mut rt,
        120,
        |rt| rt.state.touch(1),
    ));
    let mut rt = runtime(Mode::Scroll);
    rt.frame(0.0);
    reports.push(measure("smooth scroll, 4000 rows", &mut rt, 120, |rt| {
        rt.wheel_event(WheelEvent::new(0.0, -9.0).precise(true).at(720.0, 380.0));
    }));
    println!(
        "scroll offset after the run: {:.0} pt",
        rt.state.offset.get()
    );
    print(&reports);
    println!("Headless Rune: view build, layout, shaping and paint encoding; no GPU draw.");
}
