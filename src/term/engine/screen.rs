//! What the program drew, read back out of libghostty: one run per pen change, cell backgrounds
//! and cell-filling shapes resolved against the terminal's palette, and the overscan rows above
//! and below the viewport. The view is the only consumer.

use std::time::Instant;

use ghostty_vt::render::{CellIteration, CellIterator, CursorVisualStyle, Dirty, RowIterator};
use ghostty_vt::style::{RgbColor, StyleColor, Underline};
use rune::Color;

use super::Engine;
use crate::term::grid::{CursorShape, Frame, Line, Pen, Run, Shape};
use crate::term::shapes::{self, Metrics};

impl Engine {
    /// The first or the last row of the viewport. The last one is walked to, since a row's cells
    /// are only valid until the next `next()`.
    pub(super) fn read_edge(&mut self, last: bool) -> Option<Line> {
        let snapshot = self.render.update(&self.vt).ok()?;
        let colors = snapshot.colors().ok()?;
        let default_fg = color(colors.foreground);
        let default_bg = color(colors.background);
        let mut rows_iter = RowIterator::new().ok()?;
        let mut iter = rows_iter.update(&snapshot).ok()?;
        let mut cells_iter = CellIterator::new().ok()?;
        let mut scratch = std::mem::take(&mut self.scratch);
        let mut line = None;
        while let Some(row) = iter.next() {
            let selection = row
                .selection()
                .ok()
                .flatten()
                .map(|range| (range.start_x, range.end_x));
            let Ok(mut cells) = cells_iter.update(row) else {
                break;
            };
            let mut candidate = Line {
                selection,
                ..Line::default()
            };
            read_cells(
                &mut cells,
                &mut candidate,
                &mut scratch,
                default_fg,
                default_bg,
                self.metrics,
            );
            line = Some(candidate);
            if !last {
                break;
            }
        }
        self.scratch = scratch;
        let _ = snapshot.set_dirty(Dirty::Full);
        line
    }

    /// Reads the screen into `out`, and adds the time it took to the frame's accounting.
    pub fn fill(&mut self, out: &mut Frame) {
        let at = Instant::now();
        self.fill_into(out);
        self.readback_time += at.elapsed();
    }

    /// Reads the screen into `out`, reusing its buffers. Only rows libghostty reports as dirty
    /// are read again, and a cell with no styling skips the colour calls that would find none;
    /// cells past a row's last visible glyph are not built at all.
    fn fill_into(&mut self, out: &mut Frame) {
        if out.title != self.title {
            out.title.clone_from(&self.title);
        }
        let Ok(snapshot) = self.render.update(&self.vt) else {
            out.lines.clear();
            out.lead = None;
            return;
        };
        let Ok(colors) = snapshot.colors() else {
            out.lines.clear();
            out.lead = None;
            return;
        };
        // A program that set its own background with OSC 11 (a TUI matching its theme, say)
        // draws on that colour, not on the one the pane started with.
        out.background = Some(color(colors.background));
        let cols = snapshot.cols().unwrap_or(self.cols);
        let rows = snapshot.rows().unwrap_or(self.rows);
        if out.cols != cols || out.rows != rows || out.lines.len() != rows as usize {
            self.force_full = true;
        }
        out.cols = cols;
        out.rows = rows;
        let full = self.force_full || !matches!(snapshot.dirty().ok(), Some(Dirty::Partial));
        self.force_full = false;
        let default_fg = color(colors.foreground);
        let default_bg = color(colors.background);
        while out.lines.len() < rows as usize {
            out.lines.push(Line::default());
        }
        out.lines.truncate(rows as usize);
        let Ok(mut rows_iter) = RowIterator::new() else {
            return;
        };
        let Ok(mut iter) = rows_iter.update(&snapshot) else {
            return;
        };
        let Ok(mut cells_iter) = CellIterator::new() else {
            return;
        };
        let mut scratch = std::mem::take(&mut self.scratch);
        for index in 0..rows as usize {
            let Some(row) = iter.next() else {
                break;
            };
            if !full && !row.dirty().unwrap_or(true) {
                continue;
            }
            let selection = row
                .selection()
                .ok()
                .flatten()
                .map(|range| (range.start_x, range.end_x));
            let Ok(mut cells) = cells_iter.update(row) else {
                break;
            };
            let line = &mut out.lines[index];
            line.text.clear();
            line.runs.clear();
            line.shapes.clear();
            line.selection = selection;
            read_cells(
                &mut cells,
                line,
                &mut scratch,
                default_fg,
                default_bg,
                self.metrics,
            );
            let _ = row.set_dirty(false);
        }
        self.scratch = scratch;
        let _ = snapshot.set_dirty(Dirty::Clean);
        // DECSCUSR picks the shape and OSC 12 the colour: a program that asks for a bar cursor
        // in its insert mode, or tints the cursor to match its own theme, draws in Ghostty
        // exactly what it asked for, and the pane reads the same two pieces of state.
        out.cursor = snapshot
            .cursor_viewport()
            .ok()
            .flatten()
            .filter(|_| snapshot.cursor_visible().unwrap_or(true))
            .map(|at| (at.x, at.y));
        out.cursor_shape = cursor_shape(
            snapshot
                .cursor_visual_style()
                .unwrap_or(CursorVisualStyle::Block),
        );
        // Only a colour the program set: libghostty reports the effective colour, which is the
        // theme's own until an OSC 12 (or 112, back again) moves it, and the pane draws its
        // unfocused cursor in a different colour from the theme's.
        let default_cursor = self.vt.default_cursor_color().ok().flatten();
        out.cursor_blink = snapshot.cursor_blinking().unwrap_or(false);
        out.cursor_color = snapshot
            .cursor_color()
            .ok()
            .flatten()
            .filter(|set| Some(*set) != default_cursor)
            .map(color);
        if self.edges_at == Some(self.applied) {
            out.lead.clone_from(&self.lead);
            out.trail.clone_from(&self.trail);
        } else {
            out.lead = None;
            out.trail = None;
        }
    }
}

/// The one character a cell holds, when it holds exactly one.
fn single(text: &str) -> Option<char> {
    let mut chars = text.chars();
    match (chars.next(), chars.next()) {
        (Some(c), None) => Some(c),
        _ => None,
    }
}

/// Fills one line from a row's cells: one run per change of pen, and no cell work for the blank
/// tail of a row.
fn read_cells(
    cells: &mut CellIteration<'_>,
    line: &mut Line,
    scratch: &mut String,
    default_fg: Color,
    default_bg: Color,
    metrics: Metrics,
) {
    let mut used = 0usize;
    let mut used_cols = 0u16;
    let mut col = 0u16;
    while let Some(cell) = cells.next() {
        let styled = cell.has_styling().unwrap_or(true);
        let pen = if styled {
            let style = cell.style().unwrap_or_default();
            let fg = cell.fg_color().ok().flatten().map(color);
            let bg = cell.bg_color().ok().flatten().map(color);
            let (fg, bg) = if style.inverse {
                (bg.unwrap_or(default_bg), Some(fg.unwrap_or(default_fg)))
            } else {
                (fg.unwrap_or(default_fg), bg)
            };
            Pen {
                fg,
                bg: bg.filter(|paint| *paint != default_bg),
                bold: style.bold,
                italic: style.italic,
                faint: style.faint,
                underline: !matches!(style.underline, Underline::None)
                    || matches!(style.underline_color, StyleColor::Rgb(_)),
                strike: style.strikethrough,
            }
        } else {
            Pen {
                fg: default_fg,
                ..Pen::default()
            }
        };
        scratch.clear();
        if cell.graphemes_utf8(scratch).is_err() || scratch.is_empty() {
            scratch.push(' ');
        }
        // A cell-filling character is drawn from the cell's own rectangle, not from the font's
        // glyph: the glyph is only exactly one cell if the family was designed that way, and a
        // hairline between two of them reads as a broken bar or a rule that does not join. The
        // text keeps a space in that cell, so the byte-to-cell mapping the runs use still holds
        // and the font draws nothing underneath the shape.
        let mut filled = false;
        if let Some(c) = single(scratch)
            && let Some(shape) = shapes::cell(c, metrics)
        {
            for index in 0..shape.len() {
                line.shapes.push(Shape {
                    col,
                    rect: shape.get(index),
                    fg: pen.fg,
                });
            }
            scratch.clear();
            scratch.push(' ');
            filled = true;
        }
        let start = line.text.len();
        line.text.push_str(scratch);
        let end = line.text.len();
        // A blank cell still belongs to the row when it carries a background or a style: a
        // full-width bar is spaces, and trimming them would cut the bar short. A shape is
        // visible for the same reason, so its column is kept too.
        if scratch != " " || styled || filled {
            used = end;
            used_cols = col + 1;
        }
        let cols = (col, col + 1);
        col = col.saturating_add(1);
        match line.runs.last_mut() {
            Some(run) if run.pen == pen => {
                run.end = end;
                run.cols.1 = cols.1;
            }
            _ => line.runs.push(Run {
                start,
                end,
                cols,
                pen,
            }),
        }
    }
    // Trailing blanks only pad the row; dropping them keeps every line's text short.
    if line.text.len() > used {
        line.text.truncate(used);
        line.runs.retain(|run| run.start < used);
        for run in &mut line.runs {
            run.cols.1 = run.cols.1.min(used_cols);
        }
        if let Some(last) = line.runs.last_mut() {
            last.end = last.end.min(used);
        }
    }
}

/// DECSCUSR's shape, in the renderer's own terms. Any shape a later libghostty adds draws as
/// the default block rather than nothing.
fn cursor_shape(style: CursorVisualStyle) -> CursorShape {
    match style {
        CursorVisualStyle::Bar => CursorShape::Bar,
        CursorVisualStyle::Underline => CursorShape::Underline,
        CursorVisualStyle::BlockHollow => CursorShape::Hollow,
        _ => CursorShape::Block,
    }
}

fn color(rgb: RgbColor) -> Color {
    let decode = rune::color::srgb_decode;
    Color {
        r: decode(f32::from(rgb.r) / 255.0),
        g: decode(f32::from(rgb.g) / 255.0),
        b: decode(f32::from(rgb.b) / 255.0),
        a: 1.0,
    }
}
