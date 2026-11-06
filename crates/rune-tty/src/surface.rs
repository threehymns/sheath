//! The grid surface: the rows, the cursor, the hovered link's underline and the pointer's drag,
//! with no input attached. The pane's chrome is next door; the app's pane and the standalone
//! terminal window each wire the handlers they have to this.

use crate::PAD;
use crate::Terminal;
use crate::grid::CursorShape;
use crate::pane::MouseAction;
#[cfg(any(feature = "text-renderer", target_arch = "wasm32"))]
use crate::rows::line_row;
#[cfg(all(not(target_arch = "wasm32"), not(feature = "text-renderer")))]
use rune::SurfaceFit;
use rune::{Cursor, Drag, El, Phase, Spring, Transition, Tx, col, el, text};
use rune_kit::Kit;

/// The rows, the cursor and the notice, with no input attached: the app's pane and the standalone
/// terminal window both draw this, and each wires the handlers it has to it.
pub fn screen_of(t: &Terminal, scale: f32, k: &'static Kit) -> El {
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
    // The native default draws the whole grid as one streamed image: its selection, shapes and
    // overscan rows come with it. The text renderer builds those as elements instead and is a
    // build-time fallback (`text-renderer`) and the only one on the web.
    if let Some(picture) = texture(t, scale, k) {
        lines = lines.child(picture);
    }
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
    }

    // The cursor is drawn over the grid, whichever renderer made it: the image carries none, so
    // a blink and a move cost no upload, and both paths draw the same shapes.
    if let Some((x, y)) = t.frame.cursor.filter(|_| t.cursor_on()) {
        // The program's own cursor colour (OSC 12) when it set one, else the theme's accent,
        // muted while the pane is unfocused.
        let color = t.frame.cursor_color.unwrap_or(if t.focus {
            k.c.text_accent
        } else {
            k.c.text_muted
        });
        let fill = k.alpha(color, if t.focus { 0.55 } else { 0.35 });
        let edge = (t.line * 0.08).max(2.0);
        let bar = (t.cell * 0.125).max(2.0);
        let at = |key: &str, dx: f32, dy: f32, w: f32, h: f32| {
            el().key(format!("terminal-cursor-{key}"))
                .absolute(f32::from(x) * t.cell + dx, f32::from(y) * t.line + dy)
                .w(w)
                .h(h)
                .transition(Transition::none())
                .fill(fill)
        };
        // DECSCUSR: a bar for an editor's insert mode, and the block the pane has always drawn
        // for the shapes it does not single out.
        lines = match t.frame.cursor_shape {
            CursorShape::Block => lines.child(at("block", 0.0, 0.0, t.cell, t.line)),
            CursorShape::Bar => lines.child(at("bar", 0.0, 0.0, bar, t.line)),
            CursorShape::Underline => {
                lines.child(at("underline", 0.0, t.line - edge, t.cell, edge))
            }
            CursorShape::Hollow => lines
                .child(at("top", 0.0, 0.0, t.cell, edge))
                .child(at("bottom", 0.0, t.line - edge, t.cell, edge))
                .child(at("left", 0.0, 0.0, edge, t.line))
                .child(at("right", t.cell - edge, 0.0, edge, t.line)),
        };
    }

    // A hovered OSC 8 link is underlined under the cells it covers, the way Ghostty shows which
    // link a click would open. It is drawn over the grid, not into it: the streamed image is not
    // rebuilt for a pointer move, so crossing a row of links costs nothing but the hit test.
    if let Some((row, start, end)) = t.hovered() {
        lines = lines.child(
            el().key("terminal-link-underline")
                .absolute(
                    f32::from(start) * t.cell,
                    f32::from(row) * t.line + (t.line - 1.5).max(0.0),
                )
                .w(f32::from(end.saturating_sub(start) + 1) * t.cell)
                .h(1.5)
                .transition(Transition::none())
                .fill(k.c.text_accent),
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
        .px(PAD)
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
    } else if let Some(error) = &t.error {
        // The shell did not start, so no rows will ever arrive: the reason the pane stays empty
        // belongs in the pane itself, where a window that draws no headers still shows it,
        // instead of "Starting the shell…" forever.
        view = view.child(
            text(error.clone())
                .key("terminal-start-error")
                .font(k.ty.caption())
                .color(k.c.danger.fg)
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
    if t.hovered().is_some() {
        // A link under the pointer reads as something to click, the way Ghostty shows it.
        view = view.cursor(Cursor::Pointer);
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
    // The image is the grid itself: the cursor is an element of the view drawn over it, so a
    // blink or a move does not rebuild the image.
    let palette = crate::raster::Palette {
        bg: crate::raster::rgb(t.frame.background.unwrap_or(k.c.bg_base)),
        selection: crate::raster::rgb(k.c.text_accent),
        search: crate::raster::rgb(k.c.text_accent),
    };
    let picture = t.picture(scale, palette)?;
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

/// Wires the grid's wheel, hover, drag and layout to the pane's terminal. `at` names the
/// terminal for the state that holds it: both windows run the same chain, so only that lookup
/// differs between them.
pub fn wire<S: 'static>(
    screen: El,
    at: impl Fn(&mut S) -> Option<&mut Terminal> + Clone + 'static,
) -> El {
    let wheel_at = at.clone();
    screen
        .on_wheel(move |s: &mut S, wheel, tx: &mut Tx| {
            let Some(term) = wheel_at(s) else {
                return false;
            };
            // A program that captured the mouse scrolls its own view; shift takes the mouse
            // back and scrolls the pane's history, as Ghostty's `mouse-shift-capture` does.
            let shift = tx.mods().shift;
            if !shift
                && let Some(cell) = term.hovered_cell()
                && term.mouse_wheel(wheel.dy > 0.0, cell, false)
            {
                return true;
            }
            term.wheel(wheel.dy)
        })
        .on_pointer_move_local({
            let at = at.clone();
            move |s: &mut S, p, tx: &mut Tx| {
                if let Some(term) = at(s) {
                    let cell = terminal_cell(term, (p.x, p.y));
                    term.hover(cell, tx.mods().shift);
                }
            }
        })
        .on_hover_change({
            let at = at.clone();
            move |s: &mut S, inside: bool, _: &mut Tx| {
                if !inside && let Some(term) = at(s) {
                    term.hover(None, false);
                }
            }
        })
        .on_drag({
            let at = at.clone();
            move |s: &mut S, drag: &mut Drag, tx: &mut Tx| {
                if let Some(term) = at(s) {
                    terminal_drag(term, drag, tx);
                }
            }
        })
        .on_layout_tx(move |s: &mut S, rect, tx: &mut Tx| {
            if let Some(term) = at(s) {
                layout_screen(term, rect);
            }
            tx.live_only();
        })
}

/// Sizes the grid from the screen's layout box. Rune reports the border box, so the padding the
/// screen draws with is still inside the width: the columns are the ones the padded content
/// holds. Left in, the extra columns start at the content origin and draw over the right padding
/// and past the pane, which reads as the terminal spilling out of its parent.
pub fn layout_screen(t: &mut Terminal, rect: [f32; 4]) {
    let [x, y, w, h] = rect;
    let cell = t.cell;
    t.layout([x, y, (w - 2.0 * PAD).max(0.0), h], cell);
}

/// Where a local point falls in the grid, or `None` when it is outside it. The padding and the
/// scroll's fraction are taken out here, so the pane's hover only ever sees a cell.
pub fn terminal_cell(t: &Terminal, at: (f32, f32)) -> Option<(u16, u16)> {
    let x = at.0 - PAD;
    let y = at.1 - t.offset();
    if x < 0.0 || y < 0.0 {
        return None;
    }
    let (col, row) = cell_of(t, x, y);
    if col >= f32::from(t.frame.cols) || row >= f32::from(t.frame.rows) {
        return None;
    }
    Some((col as u16, row as u16))
}

/// The cell a point names, clamped to the grid's edge rather than rejected: a drag that runs off
/// the pane still selects to the row or column under its edge.
fn cell_of(t: &Terminal, x: f32, y: f32) -> (f32, f32) {
    (
        (x / t.cell.max(1.0)).floor().clamp(0.0, 4000.0),
        (y / t.line.max(1.0)).floor().clamp(0.0, 4000.0),
    )
}

pub fn terminal_drag(t: &mut Terminal, drag: &Drag, tx: &mut Tx) {
    let (x, y) = cell_of(t, drag.x - PAD, drag.y - t.offset());
    let at = (x as u16, y as u16);
    let mods = tx.mods();
    // A program that asked for the mouse gets the gesture; otherwise a drag selects text. Shift
    // takes the mouse back for the selection, which is what Ghostty's `mouse-shift-capture`
    // leaves off by default, so a drag over `tmux` or `vim` still copies.
    let tracking = t.mouse_tracking() && !mods.shift;
    match drag.phase {
        Phase::Down => {
            t.set_focus(true);
            // Cmd+click (Ctrl+click off macOS) opens the OSC 8 link under the pointer, the way
            // Ghostty does. The rest of this gesture belongs to the link, not to a selection.
            if (mods.cmd || mods.ctrl)
                && !tracking
                && let Some(uri) = t.hyperlink_at(at)
            {
                t.link_press = true;
                let _ = crate::open_url(&uri);
                return;
            }
            if tracking {
                t.mouse(MouseAction::Press, at, false);
            } else {
                // A second press on the same cell is a double click, which takes the word under
                // it, and a third takes its line, as they do in Ghostty. Alt turns the drag into
                // a block over the columns between its ends.
                let clicks = t.click_count(at);
                t.clear_selection();
                t.begin_selection(at, clicks, mods.alt);
            }
        }
        Phase::Move => {
            if t.link_press {
                return;
            }
            if tracking {
                t.mouse(MouseAction::Drag, at, false);
            } else {
                t.extend_selection(at);
            }
        }
        Phase::Up => {
            if std::mem::take(&mut t.link_press) {
                return;
            }
            if tracking {
                t.mouse(MouseAction::Release, at, false);
            }
        }
    }
}
