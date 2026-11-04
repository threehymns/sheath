//! Avar: a window that is only the terminal. The same crate, rows and scroll as the pane inside
//! Sheath, with none of the file manager around it — a shell, a full-window grid, and the wheel
//! and drag that go with it.
//!
//! ```sh
//! cargo run -p avar            # a shell in the folder you are in
//! cargo run -p avar -- ~/src   # ... or in another one
//! ```

use std::cell::Cell;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use rune::{Cx, Drag, El, KeyEvent, Tx, Waker, col};
use rune_theme_opencode as theme;
use rune_tty::{Terminal, layout_screen, screen_of, terminal_cell, terminal_drag};

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
    /// due. `AVAR_TERM_STATS=1` prints them once a second: the frame rate the window really
    /// drew, how far behind the shell it is, and how much is still queued.
    frames: u64,
    parsed: u64,
    parse: Duration,
    readback: Duration,
    built: Cell<Duration>,
    logged: Instant,
}

impl Window {
    /// Asks for the frame a pane change needs when the shell will not: a selection that came or
    /// went is drawn by the next frame, not by anything the shell writes.
    fn wake_frame(&self, tx: &mut Tx) {
        if self.term.frame_stale() {
            tx.wake_after(Duration::ZERO);
        }
    }

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
        .on_wheel(|s: &mut Window, wheel, tx: &mut Tx| {
            // The same split the app's pane makes: a program that captured the mouse scrolls its
            // own view, and shift takes the mouse back for the history.
            let shift = tx.mods().shift;
            if !shift
                && let Some(at) = s.term.hovered_cell()
                && s.term.mouse_wheel(wheel.dy > 0.0, at, false)
            {
                return true;
            }
            s.term.wheel(wheel.dy)
        })
        .on_pointer_move_local(|s: &mut Window, p, tx: &mut Tx| {
            let at = terminal_cell(&s.term, (p.x, p.y));
            s.term.hover(at, tx.mods().shift);
        })
        .on_hover_change(|s: &mut Window, inside: bool, _: &mut Tx| {
            if !inside {
                s.term.hover(None, false);
            }
        })
        .on_drag(|s: &mut Window, drag: &mut Drag, tx: &mut Tx| {
            terminal_drag(&mut s.term, drag, tx)
        })
        .on_press(|s: &mut Window| s.term.set_focus(true))
        .on_layout_tx(|s: &mut Window, rect, tx: &mut Tx| {
            layout_screen(&mut s.term, rect);
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
    let redraw = s.term.poll(s.wake.as_ref(), rune_tty::theme(dark));
    for text in s.term.take_copies() {
        tx.copy(text);
    }
    // The window is only the grid, with no toasts to put a program's OSC 9 or 777 notification
    // in: drained so they do not pile up behind it, and otherwise dropped.
    s.term.take_notifications();
    // The window is only its shell: when the shell exits there is nothing left to show, so it
    // closes the way Ghostty's window does when its command ends. This binary is the window, so
    // ending the process is closing it.
    if s.term.shell_exited() {
        s.term.close();
        std::process::exit(0);
    }
    if let Some(wait) = s.term.blink_wait() {
        tx.wake_after(wait);
    }
    s.frames += 1;
    let elapsed = s.logged.elapsed();
    if elapsed >= Duration::from_secs(1)
        && std::env::var_os("AVAR_TERM_STATS").is_some()
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
    if event.mods.shift && event.is_command('a') {
        s.term.select_all();
        s.wake_frame(tx);
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
    let taken = s.term.key(event);
    // Escape dismissing a selection moves rows the shell knows nothing about, and the frame's
    // poll has already run by the time this key is routed: without a wake the wash stays on
    // screen until something else draws.
    if taken {
        s.wake_frame(tx);
    }
    taken
}

fn main() {
    rune::set_app_id("dev.sheath.Avar");
    let (fonts, families) = rune_tty::fonts::spec();
    rune::run(rune::AppSpec {
        min_size: Some((480.0, 240.0)),
        fonts,
        families,
        on_key: Some(on_key),
        poll: Some(poll),
        setup: Some(setup),
        ..rune::AppSpec::new("Avar", (1180.0, 760.0), Window::start, view)
    });
}
