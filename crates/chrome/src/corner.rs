//! A bite out of a panel's corner for a control that sits there.
//!
//! Rune only rounds corners outward, so the fillets are drawn: the backdrop fills the bite and
//! redraws the panel's border around it. Paint the bite over the corner, then the control.

use rune::{Color, El, LineCap, Shape, canvas};

// Segments per quarter-circle, enough that a fillet reads as a curve rather than a bevel.
const ARC_STEPS: usize = 16;

// Hairline matching the panel border the bite continues.
const BORDER_WIDTH: f32 = 0.5;

/// Which corner of the panel the bite is cut from.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Corner {
    #[default]
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CornerCut {
    width: f32,
    height: f32,
    radius: f32,
    cup: f32,
    corner: Corner,
}

impl CornerCut {
    pub fn new(size: (f32, f32), radius: f32, cup: f32) -> CornerCut {
        CornerCut::at(Corner::TopRight, size, radius, cup)
    }

    pub fn at(corner: Corner, size: (f32, f32), radius: f32, cup: f32) -> CornerCut {
        let (width, height) = (size.0.max(0.0), size.1.max(0.0));
        let radius = radius.clamp(0.0, width.min(height));
        let cup = cup.clamp(0.0, width.min(height) - radius);
        CornerCut {
            width,
            height,
            radius,
            cup,
            corner,
        }
    }

    pub fn bounds(&self) -> (f32, f32) {
        (self.width + self.radius, self.height + self.radius)
    }

    pub fn build(&self, backdrop: Color, edge: Color) -> El {
        let (width, height) = self.bounds();
        let shape = Shape::new()
            .fill(self.outline(), backdrop)
            .stroke_with(self.edge(), false, BORDER_WIDTH, LineCap::Butt, edge)
            .view_box(width, height);
        canvas(shape).size(width, height)
    }

    fn outline(&self) -> Vec<[f32; 2]> {
        let mut points = self.edge();
        points.push(self.point(self.width + self.radius, 0.0));
        points
    }

    // The path is drawn for a top-right bite, where the cup is on the control's left and bottom
    // edges. The other corners are the same path mirrored inside the drawing's own box.
    fn point(&self, x: f32, y: f32) -> [f32; 2] {
        let (width, height) = self.bounds();
        match self.corner {
            Corner::TopRight => [x, y],
            Corner::TopLeft => [width - x, y],
            Corner::BottomRight => [x, height - y],
            Corner::BottomLeft => [width - x, height - y],
        }
    }

    fn edge(&self) -> Vec<[f32; 2]> {
        let (w, h, r, c) = (self.width, self.height, self.radius, self.cup);
        let (left, right) = (r, r + w);
        let mut points = Vec::with_capacity(ARC_STEPS * 3 + 4);
        if r > 0.0 {
            arc(&mut points, [0.0, r], r, 270.0, 360.0);
        } else {
            points.push([0.0, 0.0]);
        }
        if c > 0.0 {
            points.push([left, h - c]);
            arc(&mut points, [left + c, h - c], c, 180.0, 90.0);
        } else {
            points.push([left, h]);
        }
        points.push([right - r, h]);
        if r > 0.0 {
            arc(&mut points, [right - r, h + r], r, 270.0, 360.0);
        } else {
            points.push([right, h]);
        }
        points.into_iter().map(|[x, y]| self.point(x, y)).collect()
    }
}

fn arc(points: &mut Vec<[f32; 2]>, center: [f32; 2], radius: f32, from: f32, to: f32) {
    for step in 0..=ARC_STEPS {
        let angle = (from + (to - from) * step as f32 / ARC_STEPS as f32).to_radians();
        points.push([
            center[0] + radius * angle.cos(),
            center[1] + radius * angle.sin(),
        ]);
    }
}

/// A control placed in a panel's corner, and the bite that makes room for it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CornerControl {
    /// The panel's rect: x, y, width, height. The height is read only for a bottom corner.
    pub panel: [f32; 4],
    pub corner: Corner,
    /// The panel's own radius; the bite stays concentric with it.
    pub radius: f32,
    pub inset: f32,
    pub size: (f32, f32),
    pub gap: f32,
}

impl CornerControl {
    pub fn cut(&self) -> CornerCut {
        let radius = self.corner_radius();
        CornerCut::at(
            self.corner,
            (self.size.0 + self.gap, self.size.1 + self.gap),
            radius,
            self.gap + radius,
        )
    }

    pub fn corner_radius(&self) -> f32 {
        (self.radius - self.inset).max(0.0)
    }

    pub fn backdrop(&self, backdrop: Color, edge: Color) -> El {
        let cut = self.cut();
        let (width, _) = cut.bounds();
        cut.build(backdrop, edge)
            .absolute(self.left(width), self.top())
            .key("corner-cut")
    }

    pub fn place(&self, control: El) -> El {
        control
            .absolute(self.left(self.size.0), self.top())
            .w(self.size.0)
            .h(self.size.1)
            .radius(self.corner_radius())
    }

    fn left(&self, width: f32) -> f32 {
        match self.corner {
            Corner::TopRight | Corner::BottomRight => {
                self.panel[0] + self.panel[2] - self.inset - width
            }
            Corner::TopLeft | Corner::BottomLeft => self.panel[0] + self.inset,
        }
    }

    fn top(&self) -> f32 {
        match self.corner {
            Corner::TopRight | Corner::TopLeft => self.panel[1] + self.inset,
            Corner::BottomRight | Corner::BottomLeft => {
                self.panel[1] + self.panel[3] - self.inset - self.size.1
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_cut_ends_on_the_panels_edge() {
        let control = CornerControl {
            panel: [0.0, 0.0, 880.0, 600.0],
            corner: Corner::TopRight,
            radius: 16.0,
            inset: 8.0,
            size: (76.0, 26.0),
            gap: 8.0,
        };
        let cut = control.cut();
        let (width, _) = cut.bounds();
        assert_eq!(control.left(width) + width, 880.0 - 8.0);
        assert_eq!(control.left(control.size.0) + control.size.0, 880.0 - 8.0);
        assert_eq!(control.top(), 8.0);
    }

    #[test]
    fn a_left_corner_mirrors_inside_the_bite() {
        let cut = CornerCut::at(Corner::TopLeft, (40.0, 20.0), 4.0, 4.0);
        let (width, _) = cut.bounds();
        let points = cut.edge();
        assert!(points.iter().all(|[x, _]| (0.0..=width).contains(x)));
        assert!(
            points.iter().any(|[x, _]| (*x - width).abs() < 0.01),
            "a mirrored bite reaches the drawing's right edge"
        );
    }
}
