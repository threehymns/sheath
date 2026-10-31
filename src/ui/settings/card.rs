//! A settings page's cards: a titled group of rows on the page, the raised card they sit in, and
//! the rows themselves, each with its control at the row's end.

use rune::{El, Icon, col, el, icon, row, text};
use rune_kit::Kit;

use super::{SETTINGS_ROW_HEIGHT, SETTINGS_ROW_INSET};

/// A titled group of rows in a raised card, after Hex: the label sits on the page's text edge
/// and the card hangs [`SETTINGS_ROW_INSET`] past it, so the rows' text lines up with it.
pub(super) fn settings_section(k: &Kit, key: &str, title: &str, rows: Vec<El>) -> El {
    settings_section_action(k, key, title, None, rows, true)
}

/// The same card, with a control at the end of its label line. The Rules tab's sections each
/// carry their own button. `clip` cuts the rows to the box the card presents, so a form can
/// spring open inside it; a rule drag turns the clip off so the lifted row is not cut. The clip
/// goes on the rows' box rather than the card, whose edge is the ring of its raised shadow.
pub(crate) fn settings_section_action(
    k: &Kit,
    key: &str,
    title: &str,
    action: Option<El>,
    rows: Vec<El>,
    clip: bool,
) -> El {
    let mut body = col().key(format!("{key}-rows")).w_full();
    for (index, row) in rows.into_iter().enumerate() {
        if index > 0 {
            body = body.child(settings_rule(k));
        }
        body = body.child(row);
    }
    let body = if clip {
        body.clip_rounded(k.radius.lg)
    } else {
        body
    };
    let card = k.elevate(
        col()
            .key(format!("{key}-card"))
            .w_full()
            .radius(k.radius.lg)
            .fill(k.c.bg_base)
            .child(body),
        &k.elevation.raised,
    );
    // The inset sits on the label and the action, not on the row: margins on a full-width row
    // widen it past the page's edge, leaving the action flush. Each label carries its section's
    // key, so no two sections share one.
    let mut label = row().w_full().min_h(20.0).gap(8.0).center_y().child(
        text(title)
            .key(format!("{key}-label"))
            .font(k.ty.ui_medium.lh(16.0))
            .color(k.c.text_muted)
            .mx(SETTINGS_ROW_INSET)
            .grow(1.0)
            .truncate(),
    );
    if let Some(action) = action {
        label = label.child(action.mr(SETTINGS_ROW_INSET));
    }
    col().key(key).w_full().gap(8.0).child(label).child(card)
}

/// A hairline between two rows, inset from the card's edges.
fn settings_rule(k: &Kit) -> El {
    row()
        .w_full()
        .px(SETTINGS_ROW_INSET)
        .child(el().h(1.0).grow(1.0).fill(k.c.border_muted))
}

/// One setting: an icon, the label and its explanation, and the control at the row's end.
pub(super) fn settings_row(
    k: &Kit,
    glyph: &'static Icon,
    label: &str,
    description: Option<&str>,
    control: El,
) -> El {
    let mut labels = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(2.0)
        .child(text(label).font(k.ty.ui).color(k.c.text_base).truncate());
    if let Some(description) = description {
        labels = labels.child(
            text(description.to_owned())
                .font(k.ty.caption())
                .color(k.c.text_muted)
                .wrap(),
        );
    }
    row()
        .w_full()
        .min_h(SETTINGS_ROW_HEIGHT)
        .px(SETTINGS_ROW_INSET)
        .py(8.0)
        .gap(12.0)
        .center_y()
        .child(icon(glyph, 16.0).color(k.c.icon_muted))
        .child(labels)
        .child(control)
}
