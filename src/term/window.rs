//! A window that is only the terminal: the same engine, rows and scroll as the pane inside the
//! app, with none of the file manager around it. `cargo run --bin terminal` opens it, which is
//! how the pane is looked at on its own — a shell, a full-window grid, and the wheel and drag
//! that go with it.

use std::cell::Cell;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rune::{Cx, Drag, El, KeyEvent, Tx, Waker, col};
use rune_theme_opencode as theme;

use crate::term::Terminal;
use crate::ui::terminal::{screen_of, terminal_drag};

/// Where the shell starts: the first path on the command line that is a folder, else the folder
/// the window was started in.
fn start_dir() -> PathBuf {
    std::env::args_os()
        .skip(1)
        .map(PathBuf::from)
        .find(|path| path.is_dir())
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("/"))
}

pub struct Window {
    term: Terminal,
    /// The appearance the last frame drew in, so the shell starts in the colours on screen.
    dark: Cell<bool>,
    /// The device scale: a terminal's cell is a whole number of its pixels.
    scale: Cell<f32>,
    /// The window's waker, which the terminal's reader thread holds to ask for a frame.
    wake: Option<Waker>,
    /// Frames drawn since the last report, the bytes parsed by then, and when that report was
    /// due. `SHEATH_TERM_STATS=1` prints them once a second: the frame rate the window really
    /// drew, how far behind the shell it is, and how much is still queued.
    frames: u64,
    parsed: u64,
    parse: Duration,
    readback: Duration,
    built: Cell<Duration>,
    logged: Instant,
}

impl Window {
    pub fn start() -> Window {
        let mut term = Terminal::new();
        term.open(start_dir());
        Window {
            term,
            dark: Cell::new(true),
            scale: Cell::new(1.0),
            wake: None,
            frames: 0,
            parsed: 0,
            parse: Duration::ZERO,
            readback: Duration::ZERO,
            built: Cell::new(Duration::ZERO),
            logged: Instant::now(),
        }
    }
}

pub fn setup(s: &mut Window, wake: Waker) {
    s.wake = Some(wake);
}

pub fn view(s: &Window, cx: &mut Cx<Window>) -> El {
    let k = theme::kit(cx.system_dark());
    s.dark.set(cx.system_dark());
    s.scale.set(cx.env().scale.max(1.0));
    let at = Instant::now();
    let screen = screen_of(&s.term, s.scale.get(), k)
        .on_wheel(|s: &mut Window, wheel, _: &mut Tx| s.term.wheel(wheel.dy))
        .on_drag(|s: &mut Window, drag: &mut Drag, tx: &mut Tx| {
            terminal_drag(&mut s.term, drag, tx)
        })
        .on_press(|s: &mut Window| s.term.focus = true)
        .on_layout_tx(|s: &mut Window, rect, tx: &mut Tx| {
            let cell = s.term.cell;
            s.term.layout(rect, cell);
            tx.live_only();
        });
    s.built.set(s.built.get() + at.elapsed());
    col()
        .key("terminal-window")
        .w_full()
        .h_full()
        .min_h(0.0)
        .fill(k.c.bg_base)
        .child(screen)
}

/// Drains the PTY and rebuilds the frame when it changed. Runs in the frame's poll, so the waker
/// the reader thread holds only has to ask for the next frame.
pub fn poll(s: &mut Window, _t: f64, tx: &mut Tx) -> bool {
    let dark = s.dark.get();
    s.term.measure(theme::kit(dark).ty.mono, s.scale.get());
    let redraw = s.term.poll(s.wake.as_ref(), crate::term::theme(dark));
    for text in s.term.take_copies() {
        tx.copy(text);
    }
    // The window is only the grid, with no toasts to put a program's OSC 9 or 777 notification
    // in: drained so they do not pile up behind it, and otherwise dropped.
    s.term.take_notifications();
    if let Some(wait) = s.term.blink_wait() {
        tx.wake_after(wait);
    }
    s.frames += 1;
    let elapsed = s.logged.elapsed();
    if elapsed >= Duration::from_secs(1)
        && std::env::var_os("SHEATH_TERM_STATS").is_some()
        && let Some(stats) = s.term.stats()
    {
        let frames = std::mem::take(&mut s.frames).max(1);
        let per_frame = |time: Duration| time.as_secs_f64() * 1000.0 / frames as f64;
        let frame = elapsed.as_secs_f64() * 1000.0 / frames as f64;
        let parse = per_frame(stats.parse - s.parse);
        let readback = per_frame(stats.readback - s.readback);
        let built = per_frame(s.built.take());
        // What is left of the frame once parsing, readback and the view build are taken out:
        // Rune's layout, paint encoding, the GPU pass and any idle wait.
        let rest = (frame - parse - readback - built).max(0.0);
        let rate = (stats.parsed - s.parsed) as f64 / elapsed.as_secs_f64() / (1024.0 * 1024.0);
        eprintln!(
            "term: {frames} fps ({frame:.0} ms): parse {parse:.1}, readback {readback:.1}, \
             view {built:.1}, rest {rest:.1} ms | {rate:.1} MiB/s, queue {} KiB, lag {} ms",
            stats.queued / 1024,
            stats.lag.as_millis()
        );
        s.parsed = stats.parsed;
        s.parse = stats.parse;
        s.readback = stats.readback;
        s.logged = Instant::now();
    }
    redraw
}

pub fn on_key(s: &mut Window, event: &KeyEvent, tx: &mut Tx) -> bool {
    // A bare Ctrl/Cmd-C and -V can't mean copy and paste while a shell is typing, so the window's
    // own chords are the shifted ones, as the app's pane has them.
    if event.mods.shift && event.is_command('c') {
        if let Some(text) = s.term.copy_selection()
            && !text.trim().is_empty()
        {
            tx.copy(text);
        }
        return true;
    }
    if event.mods.shift && event.is_command('v') {
        if let Ok(mut clipboard) = arboard::Clipboard::new()
            && let Ok(text) = clipboard.get_text()
        {
            s.term.paste(&text);
        }
        return true;
    }
    s.term.key(event)
}

pub fn launch() {
    rune::set_app_id("dev.sheath.Sheath.Terminal");
    rune::run(rune::AppSpec {
        min_size: Some((480.0, 240.0)),
        fonts: crate::FONTS,
        families: rune_theme_opencode::FAMILIES,
        on_key: Some(on_key),
        poll: Some(poll),
        setup: Some(setup),
        ..rune::AppSpec::new("Sheath Terminal", (1180.0, 760.0), Window::start, view)
    });
}
