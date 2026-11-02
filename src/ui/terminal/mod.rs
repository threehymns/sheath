#[cfg(any(feature = "text-renderer", target_arch = "wasm32"))]
mod rows;

use crate::app::App;
use crate::icons;
use crate::term::{MouseAction, ProgressKind, Terminal};
#[cfg(any(feature = "text-renderer", target_arch = "wasm32"))]
use rows::line_row;
#[cfg(all(not(target_arch = "wasm32"), not(feature = "text-renderer")))]
use rune::SurfaceFit;
use rune::{Cursor, Drag, El, Phase, Spring, Transition, Tx, col, el, row, text};
use rune_kit::{ControlSize, IconButton, IconButtonVariant, Kit};

pub(super) const GRIP_HEIGHT: f32 = 7.0;

// The pane's edge is its handle: dragging it sets the height, and the shell reflows to the grid
// the new rect fits as it moves. Pushed past the minimum it collapses, and the same handle sits
// at the bottom of the frame to drag the pane back open. It is pinned rather than laid out, so
// opening or collapsing the pane never moves the rows above it.
pub(super) fn grip(s: &App, k: &'static Kit) -> El {
    let height = s.term.height();
    let open = s.term.is_open();
    let mut grip = el()
        .key("terminal-grip")
        .transition(Transition::none())
        .pin(
            Some(0.0),
            None,
            Some(0.0),
            // The line rides the pane's edge: pinned to the pane's top while it is open, and
            // to the frame's bottom edge while it is collapsed.
            Some(if open { height } else { 0.0 }),
        )
        .h(GRIP_HEIGHT)
        .pt(GRIP_HEIGHT - 1.0)
        .cursor(Cursor::ResizeRow)
        .hover(k.alpha(k.c.text_accent, 0.25))
        .on_drag(
            move |s: &mut App, drag: &mut Drag, _: &mut Tx| match drag.phase {
                Phase::Down => {
                    // A collapsed pane has no visible height: the drag up from the frame edge
                    // is measured from zero, so the height follows the pointer.
                    drag.data = if s.term.is_open() { height } else { 0.0 };
                    s.term.begin_drag();
                    s.term.focus = false;
                }
                Phase::Move => {
                    // `dy` is travel from the press, not the handle's own position, so one
                    // gesture can collapse the pane and bring it straight back by moving up
                    // again: the target height keeps following the pointer either way.
                    if !drag.cancelled() {
                        s.term.drag(drag.data - drag.dy, s.cwd.clone());
                    }
                }
                Phase::Up => {
                    if drag.cancelled() {
                        return;
                    }
                    if s.term.is_open() {
                        s.term.set_height(drag.data - drag.dy);
                        s.remember_terminal_height();
                    } else if !s.term.did_open() && drag.dy <= 0.0 {
                        // Only a press that never showed the pane is the click that opens it;
                        // a drag that opened and collapsed it again stays collapsed.
                        s.term.expand(s.cwd.clone(), s.term.height());
                        s.remember_terminal_height();
                    }
                }
            },
        );
    // The handle paints nothing but the seam line along its bottom edge, so its whole height
    // reads as the edge between the panes instead of being half swallowed by the header below
    // it. A collapsed pane has nothing below the handle, so the line goes with it.
    if open {
        grip = grip.child(
            super::divider(k)
                .key("terminal-divider")
                .transition(Transition::none()),
        );
    }
    grip
}

// Everything the canvas is drawn from cuts in and out: Rune fades every element on mount, so
// each part has to say so, or the pane and its rows arrive as a fade over the frame behind them.
pub(super) fn pane(s: &App, k: &'static Kit) -> El {
    col()
        .key("terminal")
        .transition(Transition::none())
        .w_full()
        .h(s.term.height())
        .min_h(0.0)
        .fill(k.c.bg_base)
        .child(header(s, k))
        .child(screen(s, k).grow(1.0).min_h(0.0))
}

fn header(s: &App, k: &'static Kit) -> El {
    // A program that sets no title leaves the header on the shell's own directory when it
    // reports one (OSC 7), so `cd` in the pane still moves the header even without a prompt
    // that renames the window.
    let title = if !s.term.frame.title.trim().is_empty() {
        s.term.frame.title.clone()
    } else if let Some(dir) = s.term.pwd() {
        dir.display().to_string()
    } else {
        s.cwd.display().to_string()
    };
    let mut bar = row()
        .key("terminal-header")
        .w_full()
        .h(26.0)
        .px(8.0)
        .gap(6.0)
        .center_y()
        .fill(k.c.bg_deep)
        .transition(Transition::none())
        .child(
            text(title)
                .key("terminal-title")
                .font(k.ty.caption())
                .color(k.c.text_muted)
                .grow(1.0)
                .shrink()
                .min_w(0.0)
                .truncate()
                .transition(Transition::none()),
        )
        .child(
            IconButton::new(&icons::CHEVRON_DOWN)
                .variant(IconButtonVariant::GhostMuted)
                .control_size(ControlSize::Small)
                .label("Collapse")
                .help("Collapse the pane; its shell keeps running")
                .on_press(|s: &mut App| s.term.collapse())
                .build(k)
                .key("terminal-collapse")
                .transition(Transition::none()),
        )
        .child(
            IconButton::new(&icons::X)
                .variant(IconButtonVariant::GhostMuted)
                .control_size(ControlSize::Small)
                .label("Close")
                .help("Close the pane and end its shell")
                .on_press(|s: &mut App| s.term.close())
                .build(k)
                .key("terminal-close")
                .transition(Transition::none()),
        );
    if let Some(report) = progress(&s.term, k) {
        bar = bar.child(report);
    }
    if let Some(error) = &s.term.error {
        bar = bar.child(
            text(error.clone())
                .key("terminal-error")
                .font(k.ty.caption())
                .color(k.c.danger.fg)
                .max_lines(2)
                .transition(Transition::none()),
        );
    }
    col()
        .key("terminal-bar")
        .transition(Transition::none())
        .w_full()
        .child(bar)
        .child(
            el().w_full()
                .h(1.0)
                .fill(k.c.border_muted)
                .transition(Transition::none()),
        )
}

/// OSC 9;4: the progress a program reports for its own work, drawn in the header it shares with
/// the title. A program that reports none gets no room, so the common shell is unchanged.
fn progress(t: &Terminal, k: &'static Kit) -> Option<El> {
    let report = t.progress()?;
    let (label, color) = match report.kind {
        ProgressKind::Set => (
            report
                .percent
                .map_or_else(|| "Working…".to_owned(), |percent| format!("{percent}%")),
            k.c.text_accent,
        ),
        ProgressKind::Indeterminate => ("Working…".to_owned(), k.c.text_accent),
        ProgressKind::Pause => ("Paused".to_owned(), k.c.text_muted),
        ProgressKind::Error => ("Failed".to_owned(), k.c.danger.fg),
    };
    let mut bar = row()
        .key("terminal-progress")
        .h(14.0)
        .gap(6.0)
        .center_y()
        .shrink()
        .transition(Transition::none())
        .child(
            text(label)
                .key("terminal-progress-label")
                .font(k.ty.caption())
                .color(color)
                .transition(Transition::none()),
        );
    if let Some(percent) = report.percent {
        // A determinate report gets a track as well: the number alone reads as a label, and
        // the bar is what makes it read as how much is left.
        let track = 48.0;
        let width = (track * f32::from(percent) / 100.0).max(1.0);
        bar = bar.child(
            el().key("terminal-progress-track")
                .w(track)
                .h(3.0)
                .clip_rounded(1.5)
                .fill(k.alpha(k.c.text_muted, 0.25))
                .transition(Transition::none())
                .child(
                    el().key("terminal-progress-fill")
                        .w(width)
                        .h(3.0)
                        .clip_rounded(1.5)
                        .fill(color)
                        .transition(Transition::none()),
                ),
        );
    }
    Some(bar)
}

fn screen(s: &App, k: &'static Kit) -> El {
    screen_of(&s.term, s.scale.get(), k)
        .on_wheel(|s: &mut App, wheel, _: &mut Tx| s.term.wheel(wheel.dy))
        .on_drag(|s: &mut App, drag: &mut Drag, _: &mut Tx| terminal_drag(&mut s.term, drag))
        .on_press(|s: &mut App| s.term.focus = true)
        .on_layout_tx(|s: &mut App, rect, tx: &mut Tx| {
            let cell = s.term.cell;
            s.term.layout(rect, cell);
            tx.live_only();
        })
}

/// The rows, the cursor and the notice, with no input attached: the app's pane and the standalone
/// terminal window both draw this, and each wires the handlers it has to it.
pub(crate) fn screen_of(t: &Terminal, scale: f32, k: &'static Kit) -> El {
    // The text rows are drawn on the font's own line box, the one the pane measured its cell
    // from and the one cell-filling glyphs are drawn to. The theme's line height is for
    // paragraphs. Only the text renderer needs them.
    #[cfg(any(feature = "text-renderer", target_arch = "wasm32"))]
    let mono = t.text_style(k.ty.mono);
    #[cfg(any(feature = "text-renderer", target_arch = "wasm32"))]
    let (cell, height) = (t.cell, t.line);
    // The offset is the scroll itself, so it must land on the frame it is asked for: a spring
    // here lags the fraction and then chases a whole line every time the viewport steps, which
    // reads as the top row sliding half out and snapping back.
    // The rows sit on the terminal's own background: a program that set one with OSC 11
    // recolours the whole pane, not just the cells it painted.
    let mut lines = col()
        .key("terminal-lines")
        .w_full()
        .fill(t.frame.background.unwrap_or(k.c.bg_base))
        .offset(0.0, t.offset())
        .transition(Transition::none())
        // A terminal cell changes in one frame: no layout, colour or fill eases. A spring here
        // makes a background bar or a rule fade from its old colour over the run it covers, and
        // that run is exactly the stretch of the row to the right of the cell that changed.
        .motion(Spring::NONE)
        .instant_changes();
    // The native default draws the whole grid as one streamed image: its own cursor, selection,
    // shapes and overscan rows come with it. The text renderer builds those as elements instead
    // and is a build-time fallback (`text-renderer`) and the only one on the web.
    if let Some(picture) = texture(t, scale, k) {
        lines = lines.child(picture);
    } else {
        #[cfg(any(feature = "text-renderer", target_arch = "wasm32"))]
        {
            // The row above the viewport, hung one line up so the fraction of it the scroll shows
            // fills the top edge instead of background that fills in when the viewport steps.
            if let Some(lead) = &t.frame.lead {
                lines = lines.child(
                    line_row("lead".to_owned(), lead, mono, cell, height, scale, k)
                        .absolute(0.0, -height),
                );
            }
            for (index, line) in t.frame.lines.iter().enumerate() {
                lines = lines.child(line_row(
                    format!("t{index}"),
                    line,
                    mono,
                    cell,
                    height,
                    scale,
                    k,
                ));
            }
            // The row below the viewport, hung under the last line: the pane is seldom a whole
            // number of lines tall, and the strip the last row does not reach is this row's top.
            if let Some(trail) = &t.frame.trail {
                let top = t.frame.rows as f32 * height;
                lines = lines.child(
                    line_row("trail".to_owned(), trail, mono, cell, height, scale, k)
                        .absolute(0.0, top),
                );
            }
            if let Some((x, y)) = t.frame.cursor {
                lines = lines.child(
                    el().key("terminal-cursor")
                        .absolute(f32::from(x) * t.cell, f32::from(y) * t.line)
                        .w(t.cell)
                        .h(t.line)
                        .transition(Transition::none())
                        .fill(if t.focus {
                            k.alpha(k.c.text_accent, 0.55)
                        } else {
                            k.alpha(k.c.text_muted, 0.35)
                        }),
                );
            }
        }
    }

    let mut view = col()
        .key("terminal-screen")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .clip_rounded(0.0)
        // No vertical padding: the clip is the visible box, so it has to be the row band. Any
        // inset above or below the rows is a strip that is cut by the clip instead of being
        // filled by the row entering there, which makes a row pop at half its height.
        .px(10.0)
        .transition(Transition::none());
    if let Some(notice) = &t.notice {
        view = view.child(
            text(notice.clone())
                .key("terminal-notice")
                .font(k.ty.caption())
                .color(k.c.text_faint)
                .wrap()
                .transition(Transition::none()),
        );
    } else if t.frame.lines.is_empty() {
        view = view.child(
            text("Starting the shell…")
                .key("terminal-starting")
                .font(k.ty.caption())
                .color(k.c.text_faint)
                .transition(Transition::none()),
        );
    } else {
        view = view.child(lines);
    }
    view
}

/// The grid as one image, the desktop default. It covers the lead and trail rows as well, hung
/// one row up inside the scrolled container: the clip cuts the part above the viewport, and the
/// fraction of a row the scroll shows is already in the image. A notice, an error or an empty
/// frame is left to the text below.
#[cfg(all(not(target_arch = "wasm32"), not(feature = "text-renderer")))]
fn texture(t: &Terminal, scale: f32, k: &'static Kit) -> Option<El> {
    if t.notice.is_some() || t.error.is_some() || t.frame.lines.is_empty() {
        return None;
    }
    let palette = crate::term::raster::Palette {
        bg: crate::term::raster::rgb(t.frame.background.unwrap_or(k.c.bg_base)),
        cursor: crate::term::raster::rgb(if t.focus {
            k.c.text_accent
        } else {
            k.c.text_muted
        }),
        cursor_alpha: if t.focus { 140 } else { 89 },
        selection: crate::term::raster::rgb(k.c.text_accent),
    };
    let picture = crate::term::raster::picture(&t.frame, t.cell, t.line, scale, palette)?;
    Some(
        el().key("terminal-texture")
            .absolute(0.0, -t.line)
            .w(f32::from(t.frame.cols) * t.cell)
            .h((f32::from(t.frame.rows) + 2.0) * t.line)
            .transition(Transition::none())
            .pixels_fit(&picture, SurfaceFit::Fill),
    )
}

#[cfg(any(target_arch = "wasm32", feature = "text-renderer"))]
fn texture(_: &Terminal, _: f32, _: &'static Kit) -> Option<El> {
    None
}

pub(crate) fn terminal_drag(t: &mut Terminal, drag: &Drag) {
    let cell = t.cell.max(1.0);
    let line = t.line.max(1.0);
    let at = (
        (drag.x / cell).floor().clamp(0.0, 4000.0) as u16,
        (drag.y / line).floor().clamp(0.0, 4000.0) as u16,
    );
    // A program that asked for the mouse gets the gesture; otherwise a drag selects text.
    let tracking = t.mouse_tracking();
    match drag.phase {
        Phase::Down => {
            t.focus = true;
            if tracking {
                t.mouse(MouseAction::Press, at, false);
            } else {
                t.clear_selection();
                t.begin_selection(at);
            }
        }
        Phase::Move => {
            if tracking {
                t.mouse(MouseAction::Drag, at, false);
            } else {
                t.extend_selection(at);
            }
        }
        Phase::Up => {
            if tracking {
                t.mouse(MouseAction::Release, at, false);
            }
        }
    }
}

impl App {
    /// The terminal's own chords, checked before the shell sees the key: copy the selection and
    /// paste the clipboard, which a bare Ctrl/Cmd-C and -V can't mean while a shell is typing.
    pub fn terminal_chord(&mut self, event: &rune::KeyEvent, tx: &mut Tx) -> bool {
        let m = event.mods;
        let shift = m.shift;
        if shift && event.is_command('c') {
            self.terminal_copy(tx);
            return true;
        }
        if shift && event.is_command('v') {
            self.terminal_paste();
            return true;
        }
        false
    }
}
