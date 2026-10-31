//! A bite out of a panel's corner for a control that sits there.
//!
//! Rune only rounds corners outward, so the fillets are drawn: the backdrop fills the bite and
//! redraws the panel's border around it. Paint the bite over the corner, then the control.

use rune::{Color, El, LineCap, Shape, canvas};

// Segments per quarter-circle, enough that a fillet reads as a curve rather than a bevel.
const ARC_STEPS: usize = 16;

// Hairline matching the panel border the bite continues.
const BORDER_WIDTH: f32 = 0.5;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CornerCut {
    width: f32,
    height: f32,
    radius: f32,
    cup: f32,
}

impl CornerCut {
    pub fn new(size: (f32, f32), radius: f32, cup: f32) -> CornerCut {
        let (width, height) = (size.0.max(0.0), size.1.max(0.0));
        let radius = radius.clamp(0.0, width.min(height));
        let cup = cup.clamp(0.0, width.min(height) - radius);
        CornerCut {
            width,
            height,
            radius,
            cup,
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
        points.push([self.width + self.radius, 0.0]);
        points
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
        points
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

pub struct CornerControl {
    pub dialog_width: f32,
    pub dialog_radius: f32,
    pub inset: f32,
    pub size: (f32, f32),
    pub gap: f32,
}

impl CornerControl {
    pub fn cut(&self) -> CornerCut {
        let radius = self.corner_radius();
        CornerCut::new(
            (self.size.0 + self.gap, self.size.1 + self.gap),
            radius,
            self.gap + radius,
        )
    }

    pub fn corner_radius(&self) -> f32 {
        (self.dialog_radius - self.inset).max(0.0)
    }

    pub fn backdrop(&self, backdrop: Color, edge: Color) -> El {
        let cut = self.cut();
        cut.build(backdrop, edge)
            .absolute(self.panel_right() - cut.bounds().0, self.inset)
            .key("corner-cut")
    }

    pub fn place(&self, control: El) -> El {
        control
            .absolute(self.panel_right() - self.size.0, self.inset)
            .w(self.size.0)
            .h(self.size.1)
            .radius(self.corner_radius())
    }

    fn panel_right(&self) -> f32 {
        self.dialog_width - self.inset
    }
}
