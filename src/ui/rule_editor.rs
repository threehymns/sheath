//! The rule form: the editor the Rules tab opens under a row. It lays out the draft's name,
//! kind, matchers and properties; every control edits the draft, and Save writes it.

use crate::app::{App, RuleEdit, RuleProp};
use crate::rules::{EntryKind, Kind, Matcher, Prop};
use rune::{El, TextEdit, Tx, col, row, text};
use rune_kit::{
    Button, ButtonVariant, ControlSize, IconButton, IconButtonVariant, Kit, SortDir, TextInput,
};
use rune_theme_opencode as theme;
use std::collections::BTreeMap;

use super::dialogs::error_text;
use super::settings::SETTINGS_ROW_INSET;

/// The open rule's form: its name and kind, a line per match and exclude, and a button per
/// property that cycles through every value the file accepts, `inherit` included. It edits a
/// draft; Save writes it and Cancel drops it.
pub(super) fn rule_editor(k: &Kit, edit: &RuleEdit) -> El {
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
