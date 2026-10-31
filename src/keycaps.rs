//! A shortcut's keys, drawn as low-profile keycaps in a recessed tray, after Hex
//! (`anomalyco/hex`, `src/rune_ui/controls.rs`): a flat face on a cap whose skirt shows below
//! it, sitting in a well lit from above so its top edge falls into shadow.

use rune::{Color, El, Shadow, TextStyle, col, row, text};
use rune_kit::Kit;

/// How large a chord's keys are drawn: the settings fields have room for `Regular`, while a
/// field's trailing hint or a command bar row uses `Small`, so one key fits in a 28-point box.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeySize {
    Regular,
    Small,
}

/// One size's proportions: the cap's parts and the legend printed on its face.
struct Metrics {
    /// The cap's rim beside and above the face.
    rim: f32,
    /// Between a tray's edge and its keys: a tray is as tall as a button.
    padding: f32,
    /// Between the keys of one chord.
    gap: f32,
    /// A key's height, skirt included.
    height: f32,
    /// The skirt showing below a key at rest.
    skirt: f32,
    face_radius: f32,
    /// Beside the legend on the face.
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
                // Small and tight, like the print on a key.
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

/// The keys' material, from the theme's tokens: in light a warm off-white face on a cap that
/// shades to warm grey; in dark a space-black face on a cap that falls to the deepest
/// background.
struct Material {
    cap_top: Color,
    cap_bottom: Color,
    face: Color,
    ink: Color,
    /// The tight shadow under the cap and the soft one it casts at rest.
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

/// `parts` as one chord's keys, in a well recessed into `surface`: the colour the tray is
/// painted on, such as a row's highlight or a field's fill. The well is that surface sunk,
/// darker at its top edge and a hair above it at the bottom, so it keeps its depth on any
/// surface instead of turning into a hole when the surface lightens. `key` names the tray, and
/// its keys are `keycap0`, `keycap1`, …, so tests can find each one.
pub fn tray(
    k: &Kit,
    key: impl Into<String>,
    parts: &[String],
    surface: Color,
    size: KeySize,
) -> El {
    let metrics = size.metrics();
    let c = &k.c;
    // Opaque colours, mixed from the surface: the gradient and the fill would compound if they
    // carried alpha, which showed as a bright lip under the keys. The mixes reproduce Hex's
    // page tokens on the base surface, then travel with it.
    let mix = |tint: Color, t: f32| surface.mix(tint, t);
    // The rim stays translucent, as Hex's is: an opaque mix drew a hard light outline around
    // the well, where Hex only lets the well's shadow and fill read.
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
    // Concentric with the caps inside it.
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
        // The well: the shadow falls from the top edge in, not out.
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

/// One key: a face on a cap whose skirt shows below it.
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
        // The soft cast the cap throws from its skirt.
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
