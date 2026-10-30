//! The Rules category: every rule in file order, with its matcher, draggable, and deletable.

use crate::app::App;
use crate::config;
use rune::{Cursor, Drag, El, Phase, Tx, col, icon, row, text};
use rune_kit::{ControlSize, IconButton, IconButtonVariant, Kit};
use rune_theme_opencode as theme;

use super::dialogs::error_text;
use super::settings::{SETTINGS_ROW_INSET, settings_section};

/// A file that does not parse is shown as an error and left alone.
pub(super) fn rules_settings(s: &App, k: &Kit) -> El {
    let mut page = col().key("settings-rules").w_full().gap(24.0);
    if let Some(error) = s.store.error(config::CONFIG).or(s.rule_error.as_deref()) {
        page = page.child(
            col()
                .key("rules-error")
                .w_full()
                .gap(8.0)
                .child(error_text(k, error))
                .child(
                    text("The rules stay read only until the file parses again.")
                        .font(k.ty.caption())
                        .color(k.c.text_muted)
                        .mx(SETTINGS_ROW_INSET)
                        .wrap(),
                ),
        );
    }
    if s.rules.rules.is_empty() {
        return page.child(
            text(
                "No rules yet. Use Save This View as a Rule from the command bar, or write a \
                 folder-rule in config.kdl.",
            )
            .font(k.ty.caption())
            .color(k.c.text_muted)
            .mx(SETTINGS_ROW_INSET)
            .wrap(),
        );
    }
    let rows = (0..s.rules.rules.len())
        .map(|index| rule_row(s, k, index))
        .collect();
    page.child(settings_section(
        k,
        "settings-rule-list",
        "In file order",
        rows,
    ))
}

/// One rule: its matcher, the kind of rule it is, a handle to drag it by, and a delete button.
/// Dragging rewrites the order of the rule nodes, and the rule's comments travel with it.
fn rule_row(s: &App, k: &Kit, index: usize) -> El {
    let Some(rule) = s.rules.rules.get(index) else {
        return row();
    };
    let focused = s.rule_focus == Some(index);
    let mut line = row()
        .key(format!("rule-{index}"))
        .role("listitem")
        .label(rule.summary())
        .w_full()
        .min_h(56.0)
        .px(SETTINGS_ROW_INSET)
        .py(8.0)
        .gap(12.0)
        .center_y()
        .cursor(Cursor::Pointer)
        .on_drag(move |s: &mut App, drag: &mut Drag, _: &mut Tx| match drag.phase {
            Phase::Down => s.rule_drag = Some((index, index)),
            Phase::Move => {
                if let Some((from, _)) = s.rule_drag {
                    let step = (drag.y / 56.0).round() as isize;
                    let last = s.rules.rules.len().saturating_sub(1) as isize;
                    let to = (from as isize + step).clamp(0, last) as usize;
                    s.rule_drag = Some((from, to));
                }
            }
            Phase::Up => {
                if let Some((from, to)) = s.rule_drag.take() {
                    s.move_rule(from, to);
                }
            }
        })
        .child(icon(&theme::icons::EDIT, 14.0).color(k.c.icon_faint))
        .child(
            col()
                .grow(1.0)
                .min_w(0.0)
                .gap(2.0)
                .child(
                    text(rule.summary())
                        .font(k.ty.ui)
                        .color(k.c.text_base)
                        .truncate(),
                )
                .child(
                    text(rule.kind_label())
                        .font(k.ty.caption())
                        .color(k.c.text_muted),
                ),
        )
        .child(
            IconButton::new(&theme::icons::TRASH)
                .variant(IconButtonVariant::GhostMuted)
                .control_size(ControlSize::Small)
                .label(format!("Delete {}", rule.summary()))
                .on_press(move |s: &mut App| s.delete_rule(index))
                .build(k)
                .key(format!("delete-rule-{index}")),
        );
    if focused {
        line = line.fill(k.c.layer_02);
    }
    line
}
