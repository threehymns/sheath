//! The picker that lists every rule by its matcher and jumps to its definition.

use crate::app::App;
use rune::{Cursor, El, ScrollSpec, col, el, row, text};
use rune_kit::{Kit, scrim};

pub(super) fn rule_picker_layer(s: &App, k: &Kit, layers: &mut Vec<El>) {
    let Some(selected) = s.rule_picker else {
        return;
    };
    let panel = if k.dark { k.c.bg_base } else { k.c.layer_02 };
    let mut list = col().key("rule-picker-list").w_full().gap(1.0).px(6.0).py(6.0);
    if s.rules.rules.is_empty() {
        list = list.child(
            row().key("rule-picker-none").px(12.0).py(14.0).child(
                text("No rules yet")
                    .font(k.ty.ui)
                    .color(k.c.text_faint),
            ),
        );
    }
    for (index, rule) in s.rules.rules.iter().enumerate() {
        let active = index == selected;
        let mut line = row()
            .key(format!("rule-picker-{index}"))
            .w_full()
            .h(32.0)
            .px(8.0)
            .gap(10.0)
            .center_y()
            .radius(k.radius.sm)
            .cursor(Cursor::Pointer)
            .on_press(move |s: &mut App| s.choose_rule(index))
            .child(
                text(rule.summary())
                    .font(k.ty.ui)
                    .color(k.c.text_base)
                    .grow(1.0)
                    .truncate(),
            )
            .child(
                text(rule.kind_label())
                    .font(k.ty.caption())
                    .color(k.c.text_muted),
            );
        if active {
            line = line.fill(k.c.hover);
        }
        list = list.child(line);
    }
    let panel = col()
        .key("rule-picker")
        .w(520.0)
        .max_h(440.0)
        .radius(10.0)
        .fill(panel)
        .border(1.0, k.c.border_muted)
        .clip_rounded(10.0)
        .modal(true, |s: &mut App| s.close_rule_picker())
        .child(
            row().px(10.0).py(8.0).child(
                text("Rules")
                    .font(k.ty.ui_medium)
                    .color(k.c.text_muted),
            ),
        )
        .child(
            el().key("rule-picker-rule")
                .h(1.0)
                .w_full()
                .fill(k.c.border_muted),
        )
        .child(
            col()
                .key("rule-picker-scroll")
                .w_full()
                .max_h(380.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .scroll_edge_fade(12.0)
                .child(list),
        );
    layers.push(
        scrim("rule-picker-scrim", |s: &mut App| s.close_rule_picker())
            .fill(k.alpha(k.c.black, 0.28)),
    );
    layers.push(
        col()
            .key("rule-picker-slot")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .pt(72.0)
            .child(row().w_full().justify_center().child(panel)),
    );
}
