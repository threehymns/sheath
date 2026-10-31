use crate::app::{App, RULE_ROW_HEIGHT, RuleDrag, rule_key};
use crate::config;
use crate::icons;
use crate::rules::Kind;
use rune::{Anim, Drag, El, Phase, Tx, col, icon, row, text};
use rune_kit::{Button, ButtonVariant, ControlSize, IconButton, IconButtonVariant, Kit};
use rune_theme_opencode as theme;

use super::dialogs::error_text;
use super::rule_editor::rule_editor;
use super::settings::{SETTINGS_CARD_WIDTH, SETTINGS_ROW_INSET, settings_section_action};

const RULE_ROW_GAP: f32 = 1.0 + 2.0 * ROW_PAD;

const ROW_MARGIN: f32 = 4.0;

const ROW_PAD: f32 = 4.0;

pub(super) fn rules_settings(s: &App, k: &Kit) -> El {
    let mut page = col().key("settings-rules").w_full().gap(24.0);
    let read_only = s.rule_error.is_some();
    if let Some(error) = s
        .store
        .error(config::CONFIG)
        .or(s.store.write_error.as_deref())
        .or(s.rule_error.as_deref())
    {
        let mut banner = col()
            .key("rules-error")
            .w_full()
            .gap(8.0)
            .child(error_text(k, error));
        if read_only {
            banner = banner.child(
                text("The rules stay read only until the file parses again.")
                    .font(k.ty.caption())
                    .color(k.c.text_muted)
                    .mx(SETTINGS_ROW_INSET)
                    .wrap(),
            );
        }
        page = page.child(
            banner.child(
                row().gap(8.0).child(
                    Button::new("Open in Editor")
                        .variant(ButtonVariant::Outline)
                        .control_size(ControlSize::Small)
                        .on_press(|s: &mut App| s.open_config_in_editor())
                        .build(k)
                        .key("open-config-editor"),
                ),
            ),
        );
    }
    if let Some(message) = &s.rule_query {
        page = page.child(
            text(message.clone())
                .key("rules-no-match")
                .font(k.ty.caption())
                .color(k.c.text_muted)
                .mx(SETTINGS_ROW_INSET)
                .wrap(),
        );
    }
    page = page.child(rule_section(s, k, Kind::Folder, read_only));
    page.child(rule_section(s, k, Kind::Entry, read_only))
}

fn rule_section(s: &App, k: &Kit, kind: Kind, read_only: bool) -> El {
    let (section_key, title, empty) = match kind {
        Kind::Folder => (
            "settings-folder-rules",
            "Folder rules",
            "No folder rules yet. Add Rule starts one for the folder on screen, or write a \
             folder-rule in config.kdl.",
        ),
        Kind::Entry => (
            "settings-entry-rules",
            "Entry rules",
            "No entry rules yet. Add Rule starts one for the selected entry, or write an \
             entry-rule in config.kdl.",
        ),
    };
    let members = section_indices(s, kind);
    let adding = if read_only {
        None
    } else {
        s.rule_edit
            .as_ref()
            .filter(|edit| edit.new && edit.draft.kind == kind)
    };
    let mut rows = Vec::new();
    if members.is_empty() && adding.is_none() {
        rows.push(empty_note(k, empty));
    }
    for (slot, &index) in members.iter().enumerate() {
        let Some(key) = rule_key(&s.rules.rules, index) else {
            continue;
        };
        let editing = if read_only {
            None
        } else {
            s.rule_edit
                .as_ref()
                .filter(|edit| !edit.new && edit.index == index)
        };
        let row = match editing {
            Some(edit) => rule_editor(k, edit),
            None => rule_row(s, k, index, slot, kind, read_only, &key),
        };
        rows.push(col().w_full().child(row));
    }
    if let Some(edit) = adding {
        rows.push(col().w_full().child(rule_editor(k, edit)));
    }
    settings_section_action(
        k,
        section_key,
        title,
        add_rule_button(k, kind, read_only),
        rows,
        s.rule_drag.dragging().is_none(),
    )
}

fn add_rule_button(k: &Kit, kind: Kind, read_only: bool) -> Option<El> {
    if read_only {
        return None;
    }
    let key = match kind {
        Kind::Folder => "add-folder-rule",
        Kind::Entry => "add-entry-rule",
    };
    Some(
        Button::new("Add Rule")
            .variant(ButtonVariant::Outline)
            .control_size(ControlSize::Small)
            .on_press(move |s: &mut App| s.open_new_rule(kind))
            .build(k)
            .key(key),
    )
}

fn section_indices(s: &App, kind: Kind) -> Vec<usize> {
    s.rules
        .rules
        .iter()
        .enumerate()
        .filter(|(_, rule)| rule.kind == kind)
        .map(|(index, _)| index)
        .collect()
}

fn empty_note(k: &Kit, message: &str) -> El {
    row().w_full().px(SETTINGS_ROW_INSET).py(10.0).child(
        text(message)
            .font(k.ty.caption())
            .color(k.c.text_muted)
            .wrap(),
    )
}

fn rule_row(
    s: &App,
    k: &Kit,
    index: usize,
    slot: usize,
    kind: Kind,
    read_only: bool,
    key: &str,
) -> El {
    let Some(rule) = s.rules.rules.get(index) else {
        return row();
    };
    let mut line = row()
        .key(key)
        .role("listitem")
        .label(rule.summary())
        .w(SETTINGS_CARD_WIDTH - 2.0 * ROW_MARGIN)
        .mx(ROW_MARGIN)
        .my(ROW_PAD)
        .min_h(RULE_ROW_HEIGHT)
        .px(SETTINGS_ROW_INSET - ROW_MARGIN)
        .py(5.0)
        .gap(12.0)
        .center_y()
        .radius(k.radius.lg - ROW_MARGIN)
        .hover(k.c.hover)
        .hover_group()
        .child(icon(&icons::GRIP, 14.0).color(k.c.icon_faint))
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
                    text(rule.details())
                        .font(k.ty.caption())
                        .color(k.c.text_muted)
                        .truncate(),
                ),
        );
    if s.rule_delete_ask == Some(index) {
        line = line
            .child(
                text("Delete this rule?")
                    .font(k.ty.caption())
                    .color(k.c.text_muted),
            )
            .child(
                Button::new("Cancel")
                    .variant(ButtonVariant::GhostMuted)
                    .control_size(ControlSize::Small)
                    .on_press(|s: &mut App| s.cancel_delete_rule())
                    .build(k)
                    .key(format!("cancel-delete-rule-{index}")),
            )
            .child(
                Button::new("Delete")
                    .variant(ButtonVariant::Danger)
                    .control_size(ControlSize::Small)
                    .on_press(move |s: &mut App| s.delete_rule(index))
                    .build(k)
                    .key(format!("confirm-delete-rule-{index}")),
            );
    } else if !read_only {
        line = line
            .on_drag(
                move |s: &mut App, drag: &mut Drag, tx: &mut Tx| match drag.phase {
                    Phase::Down => s.rule_dragged = false,
                    Phase::Move => {
                        if s.rule_edit.is_some() {
                            if s.rule_edit.as_ref().is_some_and(|edit| edit.dirty())
                                || !RuleDrag::travelled(drag.dy)
                            {
                                if RuleDrag::travelled(drag.dy) {
                                    s.rule_dragged = true;
                                }
                                return;
                            }
                            s.close_rule_edit();
                        }
                        tx.animated();
                        let count = s
                            .rules
                            .rules
                            .iter()
                            .filter(|rule| rule.kind == kind)
                            .count();
                        s.rule_drag
                            .follow(kind, slot, count, drag.dy, drag.h, RULE_ROW_GAP);
                        if s.rule_drag.moves() {
                            s.rule_dragged = true;
                        }
                    }
                    Phase::Up => {
                        if let Some((from, to)) = s.rule_drag.end(drag.cancelled()) {
                            let members = section_indices(s, kind);
                            if let (Some(&from), Some(&to)) = (members.get(from), members.get(to)) {
                                tx.instant();
                                s.move_rule(from, to);
                            }
                        }
                    }
                },
            )
            .on_press(move |s: &mut App| {
                if !s.rule_dragged {
                    s.toggle_rule_edit(index);
                }
            })
            .child(
                IconButton::new(&theme::icons::TRASH)
                    .variant(IconButtonVariant::GhostMuted)
                    .control_size(ControlSize::Small)
                    .label(format!("Delete {}", rule.summary()))
                    .on_press(move |s: &mut App| s.ask_delete_rule(index))
                    .build(k)
                    .key(format!("delete-rule-{index}"))
                    .group_opacity(0.0, 1.0),
            );
    }
    line = line.offset(0.0, s.rule_drag.shift(kind, slot));
    if s.rule_drag.dragging() == Some((kind, slot)) {
        line = k.elevate(
            line.raise()
                .offset_motion(Anim::INSTANT)
                .fill(k.c.layer_02)
                .border(1.0, k.c.border_focus),
            &k.elevation.raised,
        );
    }
    line
}
