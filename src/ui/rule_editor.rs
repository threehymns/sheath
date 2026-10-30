//! The rule form: the editor the Rules tab opens under a row. It lays out the draft's name,
//! kind, matches, excludes and properties, each under its own heading; every control edits the
//! draft, and Save writes it. Save and Discard Changes are up only while the draft differs from
//! the file, so a form opened on a rule has nothing to press.

use crate::app::{App, RuleEdit, RuleMenu, RuleProp};
use crate::rules::{EntryKind, Kind, Matcher};
use rune::{El, TextEdit, Tx, col, row, text};
use rune_kit::{
    Button, ButtonVariant, ControlSize, IconButton, IconButtonVariant, Kit, Select,
    SelectAppearance, TextInput,
};
use rune_theme_opencode as theme;
use std::collections::BTreeMap;

use super::dialogs::error_text;
use super::settings::SETTINGS_ROW_INSET;

/// The width of a property's field, and of the two menus that are not properties.
const PROP_WIDTH: f32 = 150.0;
const ADD_WIDTH: f32 = 140.0;
const KIND_WIDTH: f32 = 110.0;

/// What an entry matcher's `kind` accepts, as its field lists them.
const KINDS: &[&str] = &["any", "file", "folder"];

/// The open rule's form: its name and kind, a line per match and exclude, and a line per
/// property the rule sets. A property's field lists every value the file accepts, `inherit`
/// included, and the button beside it takes the property back out; Add Property offers the
/// properties the rule does not set yet. It edits a draft; Save writes it and Discard Changes
/// drops it.
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
    let mut head = head;
    if edit.dirty() {
        head = head.child(
            row()
                .grow(1.0)
                .justify_end()
                .gap(6.0)
                .child(
                    Button::new("Discard Changes")
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
    }
    let mut form = col()
        .key(format!("rule-editor-{}", edit.index))
        .w_full()
        .px(SETTINGS_ROW_INSET)
        .py(10.0)
        .gap(10.0)
        .child(head);
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
        if matchers.is_empty() {
            form = form.child(
                empty_line(
                    k,
                    match (exclude, edit.draft.kind) {
                        (false, Kind::Folder) => "No matches. The rule applies to every folder.",
                        (false, Kind::Entry) => "No matches. The rule applies to every entry.",
                        (true, _) => "Nothing excluded.",
                    },
                )
                .key(if exclude { "no-excludes" } else { "no-matches" }),
            );
        }
    }
    form.child(properties(k, edit))
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
                    Select::new(format!("matcher-{prefix}-kind"), KINDS)
                        .current(Some(kind_label(matcher.kind)))
                        .appearance(SelectAppearance::Field)
                        .width(KIND_WIDTH)
                        .open(edit.menu == Some(RuleMenu::Matcher { exclude, index }))
                        .on_toggle(move |s: &mut App| {
                            s.toggle_rule_menu(RuleMenu::Matcher { exclude, index })
                        })
                        .trigger(k),
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

/// A section's line for when it holds nothing, in the same faded type as the rest.
fn empty_line(k: &Kit, message: &str) -> El {
    text(message)
        .font(k.ty.small)
        .color(k.c.text_faint)
        .wrap()
}

/// The kind a field lists, as the file's `kind` words it.
fn kind_label(kind: Option<EntryKind>) -> &'static str {
    match kind {
        None => "any",
        Some(EntryKind::File) => "file",
        Some(EntryKind::Folder) => "folder",
    }
}

/// The properties the rule sets, one line apiece, under their own heading: the field beside
/// each name lists every value the file accepts, and the button at the line's end — shown
/// while the pointer is over the line — takes the property back out, so the rules above
/// decide it. Add Property offers the properties the rule does not set yet.
fn properties(k: &Kit, edit: &RuleEdit) -> El {
    let unset: Vec<&str> = RuleProp::all(edit.draft.kind)
        .iter()
        .copied()
        .filter(|prop| !prop.is_set(&edit.draft.props))
        .map(RuleProp::name)
        .collect();
    let mut section = col()
        .key("rule-editor-properties")
        .w_full()
        .gap(6.0)
        .child(
            row()
                .w_full()
                .center_y()
                .gap(8.0)
                .child(
                    text("Properties")
                        .font(k.ty.caption())
                        .color(k.c.text_muted)
                        .grow(1.0),
                )
                .child(
                    Select::new("add-property", &unset)
                        .placeholder("Add Property")
                        .appearance(SelectAppearance::Field)
                        .width(ADD_WIDTH)
                        .open(edit.menu == Some(RuleMenu::AddProperty))
                        .disabled(unset.is_empty())
                        .on_toggle(|s: &mut App| s.toggle_rule_menu(RuleMenu::AddProperty))
                        .trigger(k),
                ),
        );
    let set: Vec<RuleProp> = RuleProp::all(edit.draft.kind)
        .iter()
        .copied()
        .filter(|prop| prop.is_set(&edit.draft.props))
        .collect();
    if set.is_empty() {
        section = section.child(empty_line(k, "No properties set.").key("no-properties"));
    }
    for prop in set {
        section = section.child(property_line(k, edit, prop));
    }
    section
}

/// One property the rule sets: its name, the field that lists its values, and the button that
/// takes it out of the rule.
fn property_line(k: &Kit, edit: &RuleEdit, prop: RuleProp) -> El {
    let name = prop.name();
    row()
        .key(format!("prop-{name}"))
        .w_full()
        .center_y()
        .gap(6.0)
        .hover_group()
        .child(text(name).font(k.ty.ui).color(k.c.text_base).w(96.0))
        .child(
            Select::new(format!("prop-select-{name}"), prop.options())
                .current(prop.label(&edit.draft.props))
                .appearance(SelectAppearance::Field)
                .width(PROP_WIDTH)
                .open(edit.menu == Some(RuleMenu::Prop(prop)))
                .on_toggle(move |s: &mut App| s.toggle_rule_menu(RuleMenu::Prop(prop)))
                .trigger(k),
        )
        .child(
            IconButton::new(&theme::icons::TRASH)
                .variant(IconButtonVariant::GhostMuted)
                .control_size(ControlSize::Small)
                .label(format!("Remove {name}"))
                .help(format!(
                    "Take `{name}` out of the rule, so the rule above decides it"
                ))
                .on_press(move |s: &mut App| s.clear_rule_prop(prop))
                .build(k)
                .key(format!("clear-prop-{name}"))
                // The line is the group: the button is there to be found once the pointer
                // rests on it, and out of the way the rest of the time.
                .group_opacity(0.0, 1.0),
        )
}

/// The form's open menu, mounted after the settings panel so it paints above the panel it
/// hangs from. The form's fields are the triggers; the list anchors to whichever one asked.
pub(super) fn rule_editor_layers(s: &App, k: &Kit, layers: &mut Vec<El>) {
    let Some(edit) = &s.rule_edit else {
        return;
    };
    let Some(menu) = edit.menu else {
        return;
    };
    match menu {
        RuleMenu::AddProperty => {
            let unset: Vec<&str> = RuleProp::all(edit.draft.kind)
                .iter()
                .copied()
                .filter(|prop| !prop.is_set(&edit.draft.props))
                .map(RuleProp::name)
                .collect();
            let _ = Select::new("add-property", &unset)
                .appearance(SelectAppearance::Field)
                .width(ADD_WIDTH)
                .open(true)
                .on_close(|s: &mut App| s.close_rule_menu())
                .on_choose(|s: &mut App, name: &str, _: &mut Tx| s.add_rule_prop(name))
                .build(k, layers);
        }
        RuleMenu::Prop(prop) => {
            let _ = Select::new(format!("prop-select-{}", prop.name()), prop.options())
                .current(prop.label(&edit.draft.props))
                .appearance(SelectAppearance::Field)
                .width(PROP_WIDTH)
                .open(true)
                .on_close(|s: &mut App| s.close_rule_menu())
                .on_choose(move |s: &mut App, value: &str, _: &mut Tx| {
                    s.set_rule_prop(prop, value)
                })
                .build(k, layers);
        }
        RuleMenu::Matcher { exclude, index } => {
            let matchers = if exclude {
                &edit.draft.excludes
            } else {
                &edit.draft.matches
            };
            let current = matchers.get(index).map(|matcher| matcher.kind);
            let prefix = format!("{}{index}", if exclude { "x" } else { "m" });
            let _ = Select::new(format!("matcher-{prefix}-kind"), KINDS)
                .current(current.map(kind_label))
                .appearance(SelectAppearance::Field)
                .width(KIND_WIDTH)
                .open(true)
                .on_close(|s: &mut App| s.close_rule_menu())
                .on_choose(move |s: &mut App, value: &str, _: &mut Tx| {
                    s.set_matcher_kind(exclude, index, value)
                })
                .build(k, layers);
        }
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
