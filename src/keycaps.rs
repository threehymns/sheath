//! A shortcut's keys, drawn as low-profile keycaps in a recessed tray, after `anomalyco/hex`
//! (`src/rune_ui/controls.rs`).

use rune::{Color, El, Shadow, TextStyle, col, row, text};
use rune_kit::Kit;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeySize {
    Regular,
    Small,
}

struct Metrics {
    rim: f32,
    padding: f32,
    gap: f32,
    height: f32,
    skirt: f32,
    face_radius: f32,
    face_padding: f32,
    legend: TextStyle,
}

impl KeySize {
    fn metrics(self) -> Metrics {
        match self {
            KeySize::Regular => Metrics {
                rim: 0.5,
                padding: 3.0,
                gap: 3.0,
                height: 22.0,
                skirt: 1.5,
                face_radius: 4.0,
                face_padding: 6.0,
                legend: TextStyle::new(11.0, 400.0).lh(14.0),
            },
            KeySize::Small => Metrics {
                rim: 0.5,
                padding: 2.0,
                gap: 2.0,
                height: 16.0,
                skirt: 1.0,
                face_radius: 3.0,
                face_padding: 4.5,
                legend: TextStyle::new(9.5, 400.0).lh(11.5),
            },
        }
    }
}

struct Material {
    cap_top: Color,
    cap_bottom: Color,
    face: Color,
    ink: Color,
    contact: f32,
    cast: f32,
}

impl Material {
    fn new(k: &Kit) -> Material {
        let c = &k.c;
        if k.dark {
            Material {
                cap_top: c.layer_03.mix(c.layer_04, 0.35),
                cap_bottom: c.bg_deep,
                face: c.bg_base.mix(c.layer_01, 0.6),
                ink: c.text_base.mix(c.text_faint, 0.15),
                contact: 0.8,
                cast: 0.5,
            }
        } else {
            Material {
                cap_top: c.white,
                cap_bottom: c.layer_04.mix(c.text_faint, 0.28).mix(c.warning.fg, 0.05),
                face: c.layer_02.mix(c.warning.bg, 0.35),
                ink: c.text_base.mix(c.text_muted, 0.15),
                contact: 0.2,
                cast: 0.12,
            }
        }
    }
}

pub fn tray(
    k: &Kit,
    key: impl Into<String>,
    parts: &[String],
    surface: Color,
    size: KeySize,
) -> El {
    let metrics = size.metrics();
    let c = &k.c;
    let mix = |tint: Color, t: f32| surface.mix(tint, t);
    let (top, bottom, rim, well) = if k.dark {
        (
            mix(c.bg_deep, 0.7),
            mix(c.layer_01, 0.5),
            k.light(0.07),
            0.6,
        )
    } else {
        (
            mix(c.layer_04, 0.77),
            mix(c.layer_03, 0.84),
            k.dark_a(0.07),
            0.14,
        )
    };
    let radius = metrics.face_radius + metrics.rim + metrics.padding;
    row()
        .key(key)
        .padding(metrics.padding)
        .gap(metrics.gap)
        .center_y()
        .radius(radius)
        .continuous_corners()
        .clip_rounded(radius)
        .fill(bottom)
        .gradient(top, bottom)
        .border(0.5, rim)
        .box_shadow(Shadow {
            y: 1.0,
            blur: 2.5,
            color: k.dark_a(well),
            inset: true,
            ..Shadow::default()
        })
        .children(
            parts
                .iter()
                .enumerate()
                .map(|(index, part)| keycap(k, index, part, &metrics)),
        )
}

fn keycap(k: &Kit, index: usize, label: &str, metrics: &Metrics) -> El {
    let m = Material::new(k);
    let face_height = metrics.height - metrics.rim - metrics.skirt;
    let cap = col()
        .key("cap")
        .px(metrics.rim)
        .pt(metrics.rim)
        .pb(metrics.skirt)
        .radius(metrics.face_radius + metrics.rim)
        .continuous_corners()
        .fill(m.cap_bottom)
        .gradient(m.cap_top, m.cap_bottom)
        .shadow(0.5, 0.5, k.dark_a(m.contact))
        .box_shadow(Shadow {
            y: metrics.skirt,
            blur: 2.0 * metrics.skirt,
            color: k.dark_a(m.cast),
            ..Shadow::default()
        })
        .child(
            row()
                .key("face")
                .min_w(metrics.height - 2.0 * metrics.rim)
                .h(face_height)
                .px(metrics.face_padding)
                .center()
                .radius(metrics.face_radius)
                .continuous_corners()
                .fill(m.face)
                .gradient(k.light(0.03), k.dark_a(0.02))
                .child(
                    text(label.to_owned())
                        .key("label")
                        .font(metrics.legend)
                        .color(m.ink),
                ),
        );
    col()
        .key(format!("keycap{index}"))
        .h(metrics.height)
        .child(cap)
}
