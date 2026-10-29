//! A shortcut's keys, drawn as low-profile keycaps in a recessed tray, after Hex
//! (`anomalyco/hex`, `src/rune_ui/controls.rs`): a flat face on a cap whose skirt shows below
//! it, sitting in a well lit from above so its top edge falls into shadow. Rune Kit's `Keybind`
//! draws flat chips, so the app draws the depth itself.

use rune::{Color, El, Shadow, TextStyle, col, row, text};
use rune_kit::Kit;

/// The cap's rim beside and above the face.
const RIM: f32 = 0.5;
/// Between a tray's edge and its keys: a tray is as tall as a button.
const PADDING: f32 = 3.0;
/// Between the keys of one chord.
const GAP: f32 = 3.0;
/// A key's height, skirt included.
const HEIGHT: f32 = 22.0;
/// The skirt showing below a key at rest.
const SKIRT: f32 = 1.5;
const FACE_RADIUS: f32 = 4.0;
/// Beside the legend on the face.
const FACE_PADDING: f32 = 6.0;

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

/// `parts` as one chord's keys, in a well recessed into the surface. `key` names the tray; its
/// keys are `keycap0`, `keycap1`, …, so tests can find each one.
pub fn tray(k: &Kit, key: impl Into<String>, parts: &[String]) -> El {
    let c = &k.c;
    let (top, bottom, rim, well) = if k.dark {
        (
            c.bg_deep,
            c.bg_base.mix(c.layer_01, 0.7),
            k.light(0.07),
            0.6,
        )
    } else {
        (
            c.layer_04.mix(c.layer_03, 0.35),
            c.layer_03.mix(c.layer_02, 0.6),
            k.dark_a(0.07),
            0.14,
        )
    };
    // Concentric with the caps inside it.
    let radius = FACE_RADIUS + RIM + PADDING;
    row()
        .key(key)
        .padding(PADDING)
        .gap(GAP)
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
                .map(|(index, part)| keycap(k, index, part)),
        )
}

/// One key: a face on a cap whose skirt shows below it.
fn keycap(k: &Kit, index: usize, label: &str) -> El {
    let m = Material::new(k);
    let face_height = HEIGHT - RIM - SKIRT;
    let cap = col()
        .key("cap")
        .px(RIM)
        .pt(RIM)
        .pb(SKIRT)
        .radius(FACE_RADIUS + RIM)
        .continuous_corners()
        .fill(m.cap_bottom)
        .gradient(m.cap_top, m.cap_bottom)
        .shadow(0.5, 0.5, k.dark_a(m.contact))
        // The soft cast the cap throws from its skirt.
        .box_shadow(Shadow {
            y: SKIRT,
            blur: 2.0 * SKIRT,
            color: k.dark_a(m.cast),
            ..Shadow::default()
        })
        .child(
            row()
                .key("face")
                .min_w(HEIGHT - 2.0 * RIM)
                .h(face_height)
                .px(FACE_PADDING)
                .center()
                .radius(FACE_RADIUS)
                .continuous_corners()
                .fill(m.face)
                .gradient(k.light(0.03), k.dark_a(0.02))
                .child(
                    text(label.to_owned())
                        .key("label")
                        .font(legend())
                        .color(m.ink),
                ),
        );
    col().key(format!("keycap{index}")).h(HEIGHT).child(cap)
}

/// The legend's type: small and tight, like the print on a key.
fn legend() -> TextStyle {
    TextStyle::new(11.0, 400.0).lh(14.0)
}
