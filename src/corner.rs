//! A bite taken out of a panel's corner for a control that sits there, with the panel's edge
//! curving around it instead of running behind it.
//!
//! Rune rounds a rectangle's corners outward only (`El::radius`, `El::corners`,
//! `El::clip_corners`), so the notch's fillets are drawn: the backdrop fills the bite with what
//! is behind the panel and redraws the panel's border around it. Paint it over the panel's corner
//! and place the control over that.

use rune::{Color, El, LineCap, Shape, canvas};

/// Points per quarter-circle of fillet. Rune antialiases shape edges, and a quarter is small
/// enough that this reads as a curve rather than a bevel.
const ARC_STEPS: usize = 16;

/// A hairline, matching the panel border it continues.
const BORDER_WIDTH: f32 = 0.5;

/// A rectangular bite out of a panel's top-right corner, measured from that corner.
///
/// The panel's edge rounds into the notch with its own corner radius where the notch meets its
/// top and right edges, and the notch's inner corner keeps a cup of panel around whatever sits
/// in it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CornerCut {
    width: f32,
    height: f32,
    radius: f32,
    cup: f32,
}

impl CornerCut {
    /// A notch `size` wide and tall from the panel's top-right corner.
    ///
    /// `radius` is the panel's own corner radius, used where the notch meets the panel's top and
    /// right edges, so the two corners it leaves are ordinary ones. `cup` is the fillet at the
    /// notch's inner corner: a control set in the notch with `gap` of panel kept around its left
    /// and bottom edges stays concentric with it when `cup` is `gap` plus the control's own
    /// corner radius.
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

    /// The element's size, which is the notch plus the room its outer fillets take beyond the
    /// panel's top and right edges.
    pub fn bounds(&self) -> (f32, f32) {
        (self.width + self.radius, self.height + self.radius)
    }

    /// The bite, painted: `backdrop` is what sits behind the panel (it fills the bite and blends
    /// with the surface around the cut corner), `edge` the panel's border colour, redrawn along
    /// the fillets and the notch's edges so the panel's own border under the bite goes.
    ///
    /// Position the element so the notch's left edge is at the panel's right edge less
    /// [`bounds`](Self::bounds)'s width plus the notch's width; see
    /// [`CornerControl`], which places it for you.
    pub fn build(&self, backdrop: Color, edge: Color) -> El {
        let (width, height) = self.bounds();
        let shape = Shape::new()
            .fill(self.outline(), backdrop)
            .stroke_with(self.edge(), false, BORDER_WIDTH, LineCap::Butt, edge)
            // The bite is measured from the corner it cuts, so its right and top edges hold
            // still while the panel is laid out around it; stretching the drawing to the
            // element's box keeps those edges on the panel's while the rest of it grows or
            // shrinks into place.
            .view_box(width, height);
        canvas(shape).size(width, height)
    }

    /// The bite's fill: the edge closed across the panel's top and right edges, so the backdrop
    /// covers the corner they used to round.
    fn outline(&self) -> Vec<[f32; 2]> {
        let mut points = self.edge();
        points.push([self.width + self.radius, 0.0]);
        points
    }

    /// Where the panel's edge runs on its way around the bite, from where the top edge rounds
    /// into it to where it rounds back onto the panel's right edge. The panel's own edges beyond
    /// those points border nothing after the cut, so they are painted over, not drawn again.
    fn edge(&self) -> Vec<[f32; 2]> {
        let (w, h, r, c) = (self.width, self.height, self.radius, self.cup);
        // The notch's left edge, and the panel's right edge; the origin is `r` left of the
        // notch, so the panel's top edge starts at zero.
        let (left, right) = (r, r + w);
        let mut points = Vec::with_capacity(ARC_STEPS * 3 + 4);
        if r > 0.0 {
            // The panel's top edge rounds down into the notch, as a corner of its own.
            arc(&mut points, [0.0, r], r, 270.0, 360.0);
        } else {
            points.push([0.0, 0.0]);
        }
        if c > 0.0 {
            // The cup at the notch's inner corner, concentric with the control in it.
            points.push([left, h - c]);
            arc(&mut points, [left + c, h - c], c, 180.0, 90.0);
        } else {
            points.push([left, h]);
        }
        points.push([right - r, h]);
        if r > 0.0 {
            // The notch's bottom edge rounds onto the panel's right edge.
            arc(&mut points, [right - r, h + r], r, 270.0, 360.0);
        } else {
            points.push([right, h]);
        }
        points
    }
}

/// Appends the arc of `radius` around `center` from `from` to `to` degrees (y down, so 270° is
/// straight up), both ends included.
fn arc(points: &mut Vec<[f32; 2]>, center: [f32; 2], radius: f32, from: f32, to: f32) {
    for step in 0..=ARC_STEPS {
        let angle = (from + (to - from) * step as f32 / ARC_STEPS as f32).to_radians();
        points.push([
            center[0] + radius * angle.cos(),
            center[1] + radius * angle.sin(),
        ]);
    }
}

/// A control set into a dialog's top-right corner, outside its panel: the panel is cut away
/// around it and its own corners stay concentric with the dialog's.
///
/// Use it in a dialog that lays its panel out first: paint [`backdrop`](Self::backdrop) over the
/// panel's corner and then add [`place`](Self::place)'s control, so the control is drawn last.
pub struct CornerControl {
    /// The dialog's width, to measure the corner from its right edge.
    pub dialog_width: f32,
    /// The dialog's corner radius: the control's is this less `inset`.
    pub dialog_radius: f32,
    /// How far in from the dialog's top and right edges the control sits.
    pub inset: f32,
    /// The control's size.
    pub size: (f32, f32),
    /// Panel kept around the control's left and bottom edges.
    pub gap: f32,
}

impl CornerControl {
    /// The bite this control needs in the panel: ordinary panel corners where it meets the
    /// panel's edges, and a cup concentric with the control's inner corner.
    pub fn cut(&self) -> CornerCut {
        let radius = self.corner_radius();
        CornerCut::new(
            (self.size.0 + self.gap, self.size.1 + self.gap),
            radius,
            self.gap + radius,
        )
    }

    /// The radius that keeps the control's corners concentric with the dialog's.
    pub fn corner_radius(&self) -> f32 {
        (self.dialog_radius - self.inset).max(0.0)
    }

    /// The bite painted over the panel's corner: `backdrop` is what is behind the panel, `edge`
    /// the panel's border colour.
    pub fn backdrop(&self, backdrop: Color, edge: Color) -> El {
        let cut = self.cut();
        cut.build(backdrop, edge)
            .absolute(self.panel_right() - cut.bounds().0, self.inset)
            .key("corner-cut")
    }

    /// `control`, sized and rounded for the corner.
    pub fn place(&self, control: El) -> El {
        control
            .absolute(self.panel_right() - self.size.0, self.inset)
            .w(self.size.0)
            .h(self.size.1)
            .radius(self.corner_radius())
    }

    /// The panel's right edge, in the dialog's points.
    fn panel_right(&self) -> f32 {
        self.dialog_width - self.inset
    }
}
