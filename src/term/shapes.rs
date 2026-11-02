//! Cell-filling characters as geometry: box rules, blocks and quadrants described as rectangles
//! inside one cell, so the pane draws them instead of asking the font for a glyph.
//!
//! A font's glyph for `█` or `┼` is only exactly one cell if the family was designed that way,
//! and a hairline where two of them meet reads as a broken bar or a rule that does not join.
//! These rectangles are anchored to the cell's own edges — a horizontal rule starts at the cell's
//! left edge and stops at its centre, a vertical one at its top edge — so they tile by
//! construction at any font, size or scale.
//!
//! `table` holds which rectangles a code point stands for; this module turns them into positions.
//! Only the native engine calls any of it: the engine turns each rectangle into a
//! [`Shape`](crate::term::grid::Shape) on the frame and the view paints it as an absolute rect.

mod table;

use table::Stroke;

/// The most rectangles one cell needs: a double cross is eight half-rules, a rounded corner is
/// eight treads of its arc, a three-quadrant block is three.
pub const MAX_SHAPES: usize = 12;

/// The size one cell is drawn at, and how many device pixels go to a point.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Metrics {
    /// One cell across, in points.
    pub cell: f32,
    /// One row tall, in points.
    pub line: f32,
    /// Device pixels per point.
    pub scale: f32,
}

impl Default for Metrics {
    fn default() -> Self {
        Metrics {
            cell: 8.0,
            line: 16.0,
            scale: 1.0,
        }
    }
}

impl Metrics {
    /// A light stroke's width, in points: one device pixel, so a rule stays a hairline however
    /// much the display is scaled.
    fn unit(self) -> f32 {
        1.0 / self.scale.max(0.5)
    }

    /// A width in points as a fraction of the cell. Clamped so a heavy double pair on a tiny
    /// cell still fits inside it.
    fn across(self, points: f32) -> f32 {
        (points / self.cell.max(1.0)).clamp(0.0, 1.0 / 6.0)
    }

    /// A height in points as a fraction of the row.
    fn down(self, points: f32) -> f32 {
        (points / self.line.max(1.0)).clamp(0.0, 1.0 / 6.0)
    }
}

/// The rectangles one character is drawn as, each `[x0, y0, x1, y1]` in fractions of the cell:
/// `x` across it, `y` down it. A fixed array, because a cell is rebuilt on every dirty row and
/// this path must not allocate.
#[derive(Clone, Copy, Debug)]
pub struct Shapes {
    rects: [[f32; 4]; MAX_SHAPES],
    len: usize,
}

impl Shapes {
    fn new() -> Shapes {
        Shapes {
            rects: [[0.0; 4]; MAX_SHAPES],
            len: 0,
        }
    }

    fn push(&mut self, rect: [f32; 4]) {
        if self.len < MAX_SHAPES {
            self.rects[self.len] = rect;
            self.len += 1;
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn get(&self, index: usize) -> [f32; 4] {
        self.rects[index]
    }
}

/// The rectangles a cell's character is drawn as, or `None` when the font's own glyph is used.
pub fn cell(c: char, m: Metrics) -> Option<Shapes> {
    if let Some(block) = table::block(c) {
        return Some(block);
    }
    let unit = m.unit();
    let x_unit = m.across(unit);
    let y_unit = m.down(unit);
    if let Some((centre, start)) = table::rounded(c) {
        let mut out = Shapes::new();
        arc(&mut out, centre, start, x_unit, y_unit);
        return Some(out);
    }
    let [up, down, left, right] = table::strokes(c)?;
    let weight = |stroke: Stroke| match stroke {
        Stroke::Heavy | Stroke::Dash(_, true) => unit * 2.0,
        _ => unit,
    };
    let vertical_pair = matches!(up, Stroke::Double) || matches!(down, Stroke::Double);
    let horizontal_pair = matches!(left, Stroke::Double) || matches!(right, Stroke::Double);
    // A single rule stops at the cell's centre, so a horizontal and a vertical crossing there
    // meet exactly. A double pair straddles the centre, so a rule running the other way crosses
    // both of its strokes instead of stopping in the gap between them.
    let cross_y = if horizontal_pair { 1.5 * y_unit } else { 0.0 };
    let cross_x = if vertical_pair { 1.5 * x_unit } else { 0.0 };
    // A rule leaving the cell both ways at the same weight is one band across the whole cell:
    // drawing it as two halves is the same pixels and one element more per cell.
    let full_v = up == down && matches!(up, Stroke::Light | Stroke::Heavy);
    let full_h = left == right && matches!(left, Stroke::Light | Stroke::Heavy);

    let mut out = Shapes::new();
    if full_v {
        let width = m.across(weight(up));
        out.push([0.5 - width / 2.0, 0.0, 0.5 + width / 2.0, 1.0]);
    }
    if full_h {
        let height = m.down(weight(left));
        out.push([0.0, 0.5 - height / 2.0, 1.0, 0.5 + height / 2.0]);
    }
    let (up, down) = if full_v {
        (Stroke::Blank, Stroke::Blank)
    } else {
        (up, down)
    };
    let (left, right) = if full_h {
        (Stroke::Blank, Stroke::Blank)
    } else {
        (left, right)
    };
    match up {
        Stroke::Blank => {}
        Stroke::Dash(count, _) => {
            let width = m.across(weight(up));
            dash(count, |y0, y1| {
                out.push([0.5 - width / 2.0, y0, 0.5 + width / 2.0, y1]);
            });
        }
        Stroke::Double => {
            for (x0, x1) in pair(0.5, x_unit) {
                out.push([x0, 0.0, x1, 0.5 - cross_y]);
            }
        }
        stroke => {
            let width = m.across(weight(stroke));
            out.push([0.5 - width / 2.0, 0.0, 0.5 + width / 2.0, 0.5 + cross_y]);
        }
    }
    match down {
        Stroke::Blank => {}
        Stroke::Dash(count, _) => {
            let width = m.across(weight(down));
            dash(count, |y0, y1| {
                out.push([0.5 - width / 2.0, y0, 0.5 + width / 2.0, y1]);
            });
        }
        Stroke::Double => {
            for (x0, x1) in pair(0.5, x_unit) {
                out.push([x0, 0.5 + cross_y, x1, 1.0]);
            }
        }
        stroke => {
            let width = m.across(weight(stroke));
            out.push([0.5 - width / 2.0, 0.5 - cross_y, 0.5 + width / 2.0, 1.0]);
        }
    }
    match left {
        Stroke::Blank => {}
        Stroke::Dash(count, _) => {
            let height = m.down(weight(left));
            dash(count, |x0, x1| {
                out.push([x0, 0.5 - height / 2.0, x1, 0.5 + height / 2.0]);
            });
        }
        Stroke::Double => {
            for (y0, y1) in pair(0.5, y_unit) {
                out.push([0.0, y0, 0.5 - cross_x, y1]);
            }
        }
        stroke => {
            let height = m.down(weight(stroke));
            out.push([0.0, 0.5 - height / 2.0, 0.5 + cross_x, 0.5 + height / 2.0]);
        }
    }
    match right {
        Stroke::Blank => {}
        Stroke::Dash(count, _) => {
            let height = m.down(weight(right));
            dash(count, |x0, x1| {
                out.push([x0, 0.5 - height / 2.0, x1, 0.5 + height / 2.0]);
            });
        }
        Stroke::Double => {
            for (y0, y1) in pair(0.5, y_unit) {
                out.push([0.5 + cross_x, y0, 1.0, y1]);
            }
        }
        stroke => {
            let height = m.down(weight(stroke));
            out.push([0.5 - cross_x, 0.5 - height / 2.0, 1.0, 0.5 + height / 2.0]);
        }
    }
    Some(out)
}

/// The quarter arc about `centre` that starts in quarter `start`, as a staircase of treads: each
/// one is a step's box widened by the stroke, so the curve is drawn from the same pixels a
/// straight rule is, never leaves its cell, and joins the neighbouring cells' rules flush.
fn arc(out: &mut Shapes, centre: (f32, f32), start: u8, x_unit: f32, y_unit: f32) {
    /// Steps per quarter: the treads stay about a device pixel wide at a normal cell size, so the
    /// corner reads as a curve and not as two shorter square corners.
    const STEPS: u8 = 8;
    let from = f32::from(start) * std::f32::consts::FRAC_PI_2;
    let point = |step: u8| {
        let turn = f32::from(step) / f32::from(STEPS);
        let angle = from + turn * std::f32::consts::FRAC_PI_2;
        (centre.0 + 0.5 * angle.cos(), centre.1 + 0.5 * angle.sin())
    };
    let mut previous = point(0);
    for step in 1..=STEPS {
        let next = point(step);
        out.push([
            (previous.0.min(next.0) - 0.5 * x_unit).clamp(0.0, 1.0),
            (previous.1.min(next.1) - 0.5 * y_unit).clamp(0.0, 1.0),
            (previous.0.max(next.0) + 0.5 * x_unit).clamp(0.0, 1.0),
            (previous.1.max(next.1) + 0.5 * y_unit).clamp(0.0, 1.0),
        ]);
        previous = next;
    }
}

/// The two strokes of a double rule, as `[start, end]` spans either side of the centreline: each
/// one light, with a light-sized gap between them.
fn pair(centre: f32, thickness: f32) -> [(f32, f32); 2] {
    let t = thickness;
    [
        (centre - 1.5 * t, centre - 0.5 * t),
        (centre + 0.5 * t, centre + 1.5 * t),
    ]
}

/// Calls `emit` with the start and end of each dash of a dashed rule, in cell fractions.
fn dash(count: u8, mut emit: impl FnMut(f32, f32)) {
    let count = count.max(1);
    let step = 1.0 / f32::from(count);
    for index in 0..count {
        let start = f32::from(index) * step;
        emit(start, start + 0.5 * step);
    }
}
