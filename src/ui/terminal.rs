use crate::app::App;
use crate::icons;
use crate::term::MouseAction;
use crate::term::grid::Line;
use rune::{Cursor, Drag, El, Phase, Span, Transition, Tx, col, el, row, text};
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
    let title = if s.term.frame.title.trim().is_empty() {
        s.cwd.display().to_string()
    } else {
        s.term.frame.title.clone()
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

fn screen(s: &App, k: &'static Kit) -> El {
    let t = &s.term;
    // The rows are drawn on the font's own line box, the one the pane measured its cell from and
    // the one cell-filling glyphs are drawn to. The theme's line height is for paragraphs.
    let mono = t.text_style(k.ty.mono);
    let (cell, height) = (t.cell, t.line);
    let scale = s.scale.get();
    // The offset is the scroll itself, so it must land on the frame it is asked for: a spring
    // here lags the fraction and then chases a whole line every time the viewport steps, which
    // reads as the top row sliding half out and snapping back.
    let mut lines = col()
        .key("terminal-lines")
        .w_full()
        .offset(0.0, t.offset())
        .transition(Transition::none())
        .instant_changes();
    // The row above the viewport, hung one line up so the fraction of it the scroll shows fills
    // the top edge instead of background that fills in when the viewport steps.
    if let Some(lead) = &t.frame.lead {
        lines = lines.child(
            line_row("lead".to_owned(), lead, mono, cell, height, scale, k).absolute(0.0, -height),
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
    // The row below the viewport, hung under the last line: the pane is seldom a whole number of
    // lines tall, and the strip the last row does not reach is this row's top.
    if let Some(trail) = &t.frame.trail {
        let top = t.frame.rows as f32 * height;
        lines = lines.child(
            line_row("trail".to_owned(), trail, mono, cell, height, scale, k).absolute(0.0, top),
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
        .transition(Transition::none())
        .on_wheel(|s: &mut App, wheel, _: &mut Tx| s.term.wheel(wheel.dy))
        .on_drag(|s: &mut App, drag: &mut Drag, _: &mut Tx| terminal_drag(s, drag))
        .on_press(|s: &mut App| s.term.focus = true)
        .on_layout_tx(|s: &mut App, rect, tx: &mut Tx| {
            let cell = s.term.cell;
            s.term.layout(rect, cell);
            tx.live_only();
        });
    if let Some(notice) = &s.term.notice {
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

fn line_row(
    name: String,
    line: &Line,
    mono: rune::TextStyle,
    cell: f32,
    height: f32,
    scale: f32,
    k: &'static Kit,
) -> El {
    // Rows are cut, never faded: a row slipping past an edge is clipped by the clip box, and a
    // mount or exit fade would make it pop in and out instead.
    let mut element = el()
        .key(name)
        .w_full()
        .h(height)
        .transition(Transition::none());
    if let Some((start, end)) = line.selection {
        let width = f32::from(end.saturating_sub(start) + 1) * cell;
        element = element.child(
            el().key("selected")
                .absolute(f32::from(start) * cell, 0.0)
                .w(width)
                .h(height)
                .transition(Transition::none())
                .fill(k.alpha(k.c.text_accent, 0.24)),
        );
    }
    // Cell backgrounds are rectangles over the run's cells: flush with each other, the row edges
    // and the pane, which is what a terminal draws. The shaper's own span background pads and
    // rounds, so it is left off.
    for (index, run) in line.runs.iter().enumerate() {
        let Some(bg) = run.pen.bg else { continue };
        let width = f32::from(run.cols.1.saturating_sub(run.cols.0)) * cell;
        element = element.child(
            el().key(format!("bg{index}"))
                .absolute(f32::from(run.cols.0) * cell, 0.0)
                .w(width)
                .h(height)
                .transition(Transition::none())
                .fill(bg),
        );
    }
    // Cell-filling characters are rectangles, not glyphs: their edges are snapped to whole
    // device pixels from the cell's own column, so two cells' rules abut instead of leaving the
    // hairline a font's glyph leaves when the family's design does not tile.
    let snap_x =
        |col: u16, fraction: f32| ((f32::from(col) + fraction) * cell * scale).round() / scale;
    for (index, shape) in line.shapes.iter().enumerate() {
        let [x0, y0, x1, y1] = shape.rect;
        let (left, right) = (snap_x(shape.col, x0), snap_x(shape.col, x1));
        let top = (y0 * height * scale).round() / scale;
        let bottom = (y1 * height * scale).round() / scale;
        element = element.child(
            el().key(format!("shape{index}"))
                .absolute(left, top)
                .w(right - left)
                .h(bottom - top)
                .transition(Transition::none())
                .fill(shape.fg),
        );
    }
    let mut spans = Vec::with_capacity(line.runs.len());
    let mut default = k.c.text_base;
    for run in &line.runs {
        default = run.pen.fg;
        spans.push(Span {
            start: run.start as u32,
            end: run.end as u32,
            weight: run.pen.bold.then_some(700.0),
            mono: true,
            size: None,
            color: Some(run.pen.fg),
            bg: None,
            bg_pad: 0.0,
            underline: run.pen.underline.then_some(run.pen.fg),
            italic: run.pen.italic,
        });
    }
    let mut body = text(line.text.clone())
        .font(mono)
        .color(default)
        .spans(spans)
        .shrink()
        .min_w(0.0)
        .transition(Transition::none());
    let strikes: Vec<(usize, usize, Option<rune::Color>)> = line
        .runs
        .iter()
        .filter(|run| run.pen.strike)
        .map(|run| (run.start, run.end, Some(run.pen.fg)))
        .collect();
    if !strikes.is_empty() {
        body = body.strikes(strikes);
    }
    element.child(body)
}

fn terminal_drag(s: &mut App, drag: &Drag) {
    let cell = s.term.cell.max(1.0);
    let line = s.term.line.max(1.0);
    let at = (
        (drag.x / cell).floor().clamp(0.0, 4000.0) as u16,
        (drag.y / line).floor().clamp(0.0, 4000.0) as u16,
    );
    // A program that asked for the mouse gets the gesture; otherwise a drag selects text.
    let tracking = s.term.mouse_tracking();
    match drag.phase {
        Phase::Down => {
            s.term.focus = true;
            if tracking {
                s.term.mouse(MouseAction::Press, at, false);
            } else {
                s.term.clear_selection();
                s.term.begin_selection(at);
            }
        }
        Phase::Move => {
            if tracking {
                s.term.mouse(MouseAction::Drag, at, false);
            } else {
                s.term.extend_selection(at);
            }
        }
        Phase::Up => {
            if tracking {
                s.term.mouse(MouseAction::Release, at, false);
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
