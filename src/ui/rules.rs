//! The Rules category: every rule in file order, with its matcher, draggable, and deletable.

use crate::app::{App, RULE_ROW_HEIGHT, RuleDrag, RuleEdit, RuleProp, rule_key};
use crate::config;
use crate::rules::{EntryKind, Kind, Matcher, Prop};
use rune::{Anim, Cursor, Drag, El, Phase, TextEdit, Tx, col, icon, row, text};
use rune_kit::{
    Button, ButtonVariant, ControlSize, IconButton, IconButtonVariant, Kit, SortDir, TextInput,
};
use rune_theme_opencode as theme;
use std::collections::BTreeMap;

use super::dialogs::error_text;
use super::settings::{SETTINGS_ROW_INSET, settings_section};

/// The hairline `settings_section` draws between two rules: a row stepping aside by its height
/// plus this lands where the next row was, which is the gap a drop goes into.
const RULE_ROW_GAP: f32 = 1.0;

/// A file that does not parse is shown as an error and left alone.
pub(super) fn rules_settings(s: &App, k: &Kit) -> El {
    let mut page = col().key("settings-rules").w_full().gap(24.0);
    if let Some(error) = s
        .store
        .error(config::CONFIG)
        .or(s.store.write_error.as_deref())
        .or(s.rule_error.as_deref())
    {
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
                )
                .child(
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
    // A stale list must not be edited: its positions are no longer the file's.
    let read_only = s.rule_error.is_some();
    if !read_only {
        page = page.child(
            row().w_full().justify_end().child(
                Button::new("Add Rule")
                    .variant(ButtonVariant::Outline)
                    .control_size(ControlSize::Small)
                    .on_press(|s: &mut App| s.save_view_rule())
                    .build(k)
                    .key("add-rule"),
            ),
        );
    }
    // The command that opened the tab found nothing to show.
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
    if s.rules.rules.is_empty() {
        return page.child(
            text(
                "No rules yet. Add Rule writes one for the folder on screen, or write a \
                 folder-rule in config.kdl.",
            )
            .font(k.ty.caption())
            .color(k.c.text_muted)
            .mx(SETTINGS_ROW_INSET)
            .wrap(),
        );
    }
    let mut rows = Vec::new();
    for index in 0..s.rules.rules.len() {
        let key = rule_key(&s.rules.rules, index).expect("every index names a rule");
        rows.push(rule_row(s, k, index, read_only, &key));
        if !read_only && let Some(edit) = s.rule_edit.as_ref().filter(|edit| edit.index == index) {
            rows.push(rule_editor(k, edit));
        }
    }
    page.child(settings_section(
        k,
        "settings-rule-list",
        "In file order",
        rows,
    ))
}

/// One rule: its matcher, the kind of rule it is, a handle to drag it by, and a delete button.
/// Dragging rewrites the order of the rule nodes, and the rule's comments travel with it.
fn rule_row(s: &App, k: &Kit, index: usize, read_only: bool, key: &str) -> El {
    let Some(rule) = s.rules.rules.get(index) else {
        return row();
    };
    let mut line = row()
        .key(key)
        .role("listitem")
        .label(rule.summary())
        .w_full()
        .min_h(RULE_ROW_HEIGHT)
        .px(SETTINGS_ROW_INSET)
        .py(8.0)
        .gap(12.0)
        .center_y()
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
                    text(rule.details())
                        .font(k.ty.caption())
                        .color(k.c.text_muted)
                        .truncate(),
                ),
        )
        .child(
            text(rule.kind_label())
                .font(k.ty.caption())
                .color(k.c.text_muted),
        );
    if !read_only {
        line = line.cursor(Cursor::Pointer).on_drag(
            move |s: &mut App, drag: &mut Drag, tx: &mut Tx| match drag.phase {
                // A press is not yet a drag: the row opens its editor on a click, so nothing
                // moves until the pointer has travelled far enough to mean a move.
                Phase::Down => {}
                Phase::Move => {
                    // The form sits between two rows, so the list only has one row height
                    // while no form is open. A clean form closes when the drag really starts;
                    // one holding changes stays, and the drag does not start.
                    if s.rule_edit.is_some() {
                        if s.rule_edit.as_ref().is_some_and(|edit| edit.dirty())
                            || !RuleDrag::travelled(drag.dy)
                        {
                            return;
                        }
                        s.close_rule_edit();
                    }
                    // The rows the drag passes spring aside; the row in hand follows the
                    // pointer exactly, which is why the travel is measured on the screen and
                    // not on the row the pointer is dragging.
                    tx.animated();
                    let count = s.rules.rules.len();
                    s.rule_drag
                        .follow(index, count, drag.dy, drag.h, RULE_ROW_GAP);
                }
                Phase::Up => {
                    // The gap the drag opened is the order it drops into, so the reorder lands
                    // in the same frame the rows' offsets clear: there is nothing to animate,
                    // and the list must not spring in from the order the drag started in.
                    if let Some((from, to)) = s.rule_drag.end(drag.cancelled()) {
                        tx.instant();
                        s.move_rule(from, to);
                    }
                }
            },
        )
            .child(
                IconButton::new(&theme::icons::EDIT)
                    .variant(IconButtonVariant::GhostMuted)
                    .control_size(ControlSize::Small)
                    .label(format!("Edit {}", rule.summary()))
                    .on_press(move |s: &mut App| s.toggle_rule_edit(index))
                    .build(k)
                    .key(format!("edit-rule-{index}")),
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
    }
    // The row in hand lifts above the rest and follows the pointer, and the rows it passes
    // step aside, so the gap it would drop into is the order it would land in.
    line = line.offset(0.0, s.rule_drag.shift(index));
    if s.rule_drag.dragging() == Some(index) {
        // Opaque on its own shadow, so the rows it passes do not show through, and instant so
        // it tracks the pointer rather than springing after it.
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

/// The open rule's form: its name and kind, a line per match and exclude, and a button per
/// property that cycles through every value the file accepts, `inherit` included. It edits a
/// draft; Save writes it and Cancel drops it.
fn rule_editor(k: &Kit, edit: &RuleEdit) -> El {
    let head = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(rule_field(k, &edit.fields, "name", "Rule name", 220.0))
        .child(
            Button::new(match edit.draft.kind {
                Kind::Folder => "Folder rule",
                Kind::Entry => "Entry rule",
            })
            .variant(ButtonVariant::Outline)
            .control_size(ControlSize::Small)
            .on_press(|s: &mut App| s.cycle_rule_kind())
            .build(k)
            .key("rule-editor-kind"),
        );
    let head = head.child(
        row()
            .grow(1.0)
            .justify_end()
            .gap(6.0)
            .child(
                Button::new(if edit.dirty() {
                    "Discard Changes"
                } else {
                    "Cancel"
                })
                .variant(ButtonVariant::GhostMuted)
                .control_size(ControlSize::Small)
                .on_press(|s: &mut App| s.discard_rule_edit())
                .build(k)
                .key("rule-editor-cancel"),
            )
            .child(
                Button::new("Save")
                    .variant(ButtonVariant::Contrast)
                    .control_size(ControlSize::Small)
                    .on_press(|s: &mut App| s.save_rule_edit())
                    .build(k)
                    .key("rule-editor-save"),
            ),
    );
    let mut form = col()
        .key(format!("rule-editor-{}", edit.index))
        .w_full()
        .px(SETTINGS_ROW_INSET)
        .py(10.0)
        .gap(10.0)
        .child(head);
    if edit.dirty() {
        form = form.child(
            text("Unsaved changes")
                .font(k.ty.caption())
                .color(k.c.text_muted)
                .wrap()
                .key("rule-editor-dirty"),
        );
    }
    if let Some(message) = &edit.error {
        form = form.child(error_text(k, message).key("rule-editor-error"));
    }
    for (exclude, title, add) in [
        (false, "Match", "add-match"),
        (true, "Exclude", "add-exclude"),
    ] {
        let matchers = if exclude {
            &edit.draft.excludes
        } else {
            &edit.draft.matches
        };
        form = form.child(
            row()
                .w_full()
                .center_y()
                .gap(8.0)
                .child(
                    text(title)
                        .font(k.ty.caption())
                        .color(k.c.text_muted)
                        .grow(1.0),
                )
                .child(
                    Button::new(format!("Add {title}"))
                        .variant(ButtonVariant::Outline)
                        .control_size(ControlSize::Small)
                        .on_press(move |s: &mut App| s.add_matcher(exclude))
                        .build(k)
                        .key(add),
                ),
        );
        for (index, matcher) in matchers.iter().enumerate() {
            form = form.child(matcher_row(k, edit, matcher, exclude, index));
        }
    }
    form.child(property_row(k, edit))
}

/// One `match` or `exclude` directive: a field per key the rule's kind accepts, and a button
/// that drops it.
fn matcher_row(k: &Kit, edit: &RuleEdit, matcher: &Matcher, exclude: bool, index: usize) -> El {
    let list = if exclude { "x" } else { "m" };
    let prefix = format!("{list}{index}");
    let mut line = row().w_full().gap(6.0).center_y().child(rule_field(
        k,
        &edit.fields,
        &format!("{prefix}.path"),
        "path glob",
        190.0,
    ));
    match edit.draft.kind {
        Kind::Folder => {
            line = line
                .child(rule_field(
                    k,
                    &edit.fields,
                    &format!("{prefix}.min-items"),
                    "min items",
                    80.0,
                ))
                .child(rule_field(
                    k,
                    &edit.fields,
                    &format!("{prefix}.max-items"),
                    "max items",
                    80.0,
                ));
        }
        Kind::Entry => {
            line = line
                .child(rule_field(
                    k,
                    &edit.fields,
                    &format!("{prefix}.name"),
                    "name glob",
                    130.0,
                ))
                .child(rule_field(
                    k,
                    &edit.fields,
                    &format!("{prefix}.extension"),
                    "extension",
                    90.0,
                ))
                .child(
                    Button::new(match matcher.kind {
                        None => "kind: any".to_owned(),
                        Some(EntryKind::File) => "kind: file".to_owned(),
                        Some(EntryKind::Folder) => "kind: folder".to_owned(),
                    })
                    .variant(ButtonVariant::Outline)
                    .control_size(ControlSize::Small)
                    .on_press(move |s: &mut App| s.cycle_matcher_kind(exclude, index))
                    .build(k)
                    .key(format!("matcher-{prefix}-kind")),
                );
        }
    }
    line.child(
        IconButton::new(&theme::icons::TRASH)
            .variant(IconButtonVariant::GhostMuted)
            .control_size(ControlSize::Small)
            .label("Remove matcher")
            .on_press(move |s: &mut App| s.remove_matcher(exclude, index))
            .build(k)
            .key(format!("remove-{prefix}")),
    )
}

/// One button per property the rule's kind accepts; pressing one advances its value.
fn property_row(k: &Kit, edit: &RuleEdit) -> El {
    let mut buttons = Vec::new();
    match edit.draft.kind {
        Kind::Folder => {
            buttons.push((
                "view",
                RuleProp::View,
                describe(edit.draft.props.view, |view| {
                    if view.is_grid() { "grid" } else { "list" }
                }),
            ));
            buttons.push((
                "sort",
                RuleProp::Sort,
                describe(edit.draft.props.sort, |sort| sort.as_str()),
            ));
            buttons.push((
                "direction",
                RuleProp::Direction,
                describe(edit.draft.props.direction, |direction| match direction {
                    SortDir::Ascending => "ascending",
                    SortDir::Descending => "descending",
                }),
            ));
            buttons.push((
                "show-hidden",
                RuleProp::ShowHidden,
                describe(edit.draft.props.show_hidden, |on| if on { "show" } else { "hide" }),
            ));
        }
        Kind::Entry => buttons.push((
            "hidden",
            RuleProp::Hidden,
            describe(edit.draft.props.hidden, |on| if on { "hide" } else { "show" }),
        )),
    }
    let mut line = row().w_full().gap(6.0);
    for (name, prop, value) in buttons {
        line = line.child(
            Button::new(format!("{name}: {value}"))
                .variant(ButtonVariant::Outline)
                .control_size(ControlSize::Small)
                .on_press(move |s: &mut App| s.cycle_rule_prop(prop))
                .build(k)
                .key(format!("prop-{name}")),
        );
    }
    line
}

fn describe<T: Copy>(prop: Prop<T>, label: impl Fn(T) -> &'static str) -> String {
    match prop {
        Prop::Unset => "default".to_owned(),
        Prop::Inherit => "inherit".to_owned(),
        Prop::Set(value) => label(value).to_owned(),
    }
}

/// One text field of the editor, wired back to the draft. Typing only changes the draft; the
/// file is written when the form is saved.
fn rule_field(
    k: &Kit,
    fields: &BTreeMap<String, TextEdit>,
    key: &str,
    placeholder: &str,
    width: f32,
) -> El {
    let id = key.to_owned();
    let target = id.clone();
    TextInput::new(fields.get(key).expect("the editor seeds every field"))
        .placeholder(placeholder)
        .width(Some(width))
        .on_edit(move |s: &mut App| {
            s.rule_fields_mut()
                .get_mut(&target)
                .expect("the editor seeds every field")
        })
        .on_change(|s: &mut App, _: &mut Tx| s.apply_rule_fields())
        .build(k)
        .key(format!("rule-field-{id}"))
}
