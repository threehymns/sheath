//! One row of the text renderer: its selection, backgrounds, cell-filling shapes and styled
//! spans, built as elements. The desktop default streams the whole grid as one texture instead
//! and never calls this; the text renderer and the web build draw rows this way.

#![cfg(any(feature = "text-renderer", target_arch = "wasm32"))]

use crate::grid::{Line, UnderlineKind};
use rune::{El, Span, Transition, el, text};
use rune_kit::Kit;

pub(crate) fn line_row(
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
    // Search matches go in first, so the selection and the cell backgrounds draw over them, the
    // order the texture uses too.
    for mark in &line.matches {
        let width = f32::from(mark.cols.1.saturating_sub(mark.cols.0) + 1) * cell;
        element = element.child(
            el().key(format!("match{}", mark.cols.0))
                .absolute(f32::from(mark.cols.0) * cell, 0.0)
                .w(width)
                .h(height)
                .transition(Transition::none())
                .fill(k.alpha(k.c.text_accent, if mark.current { 0.42 } else { 0.18 })),
        );
    }
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
    // Overlay keys come from the cell they cover, never their position in the vector: a cell
    // that gains or loses a background or a shape changes the element at that cell alone. A
    // positional key would renumber every element to its right, remapping their rectangles
    // for a frame, which reads as a flash travelling along the rest of the row.
    for run in &line.runs {
        let Some(bg) = run.pen.bg else { continue };
        let width = f32::from(run.cols.1.saturating_sub(run.cols.0)) * cell;
        element = element.child(
            el().key(format!("bg{}", run.cols.0))
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
    // One cell can need several rectangles (a double cross, an arc), so the key is the cell's
    // column and the rectangle's ordinal within that cell. Both are stable when cells to the
    // left change shape; only the rects of the cell that changed are replaced.
    let mut ordinal = 0u16;
    let mut column = None;
    for shape in &line.shapes {
        if column != Some(shape.col) {
            column = Some(shape.col);
            ordinal = 0;
        }
        let key = format!("sh{}_{ordinal}", shape.col);
        ordinal = ordinal.saturating_add(1);
        let [x0, y0, x1, y1] = shape.rect;
        let (left, right) = (snap_x(shape.col, x0), snap_x(shape.col, x1));
        let top = (y0 * height * scale).round() / scale;
        let bottom = (y1 * height * scale).round() / scale;
        element = element.child(
            el().key(key)
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
            // A span can carry one rule, not the style SGR 4 asked for: the raster renderer
            // draws the squiggle and the doubled rule, and this one underlines the text.
            underline: (run.pen.underline != UnderlineKind::None)
                .then(|| run.pen.underline_color.unwrap_or(run.pen.fg)),
            italic: run.pen.italic,
        });
    }
    let mut body = text(line.text.clone())
        .key("text")
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
