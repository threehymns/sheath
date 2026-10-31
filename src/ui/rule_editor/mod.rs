mod fields;

use crate::app::{App, RuleEdit, RuleMenu, RuleProp};
use crate::corner::CornerCut;
use crate::rules::{EntryKind, Kind, Matcher};
use fields::{rule_field, rule_name_field};
use rune::{Color, El, Shadow, Tx, col, row, text};
use rune_kit::{
    Button, ButtonVariant, ControlSize, IconButton, IconButtonVariant, Kit, Select,
    SelectAppearance,
};
use rune_theme_opencode as theme;

use super::settings::{SETTINGS_CARD_WIDTH, SETTINGS_ROW_INSET};

const PROP_WIDTH: f32 = 150.0;
const KIND_WIDTH: f32 = 110.0;

const CANCEL_WIDTH: f32 = 66.0;
const SAVE_WIDTH: f32 = 52.0;
const ACTIONS_HEIGHT: f32 = 20.0;
const ACTIONS_SPACING: f32 = 6.0;
const ACTIONS_GAP: f32 = 8.0;

// Card kept around the form, whose radius is set concentric with it.
const WELL_MARGIN: f32 = 4.0;

// Fixed height of the refusal line, so a message arriving or leaving moves nothing below it.
const PROBLEM_HEIGHT: f32 = 16.0;

const WELL_WIDTH: f32 = SETTINGS_CARD_WIDTH - 2.0 * WELL_MARGIN;

const KINDS: &[&str] = &["any", "file", "folder"];

pub(super) fn rule_editor(k: &Kit, edit: &RuleEdit) -> El {
    let head = row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(rule_name_field(k, &edit.fields));
    let mut form = col()
        .key(format!("rule-editor-{}", edit.index))
        .w(WELL_WIDTH)
        .mx(WELL_MARGIN)
        .my(WELL_MARGIN)
        .px(SETTINGS_ROW_INSET - WELL_MARGIN)
        .py(10.0)
        .radius(well_radius(k))
        .fill(well(k))
        .border(1.0, k.c.border_muted)
        .box_shadow(Shadow {
            x: 0.0,
            y: 1.0,
            blur: 2.0,
            spread: 0.0,
            color: k.c.black.alpha(0.55),
            inset: true,
        })
        .box_shadow(Shadow {
            x: 0.0,
            y: -1.0,
            blur: 0.0,
            spread: 0.0,
            color: k.c.black.alpha(0.45),
            inset: true,
        })
        .gap(10.0)
        .child(head)
        .child(problem_line(k, edit.error.as_deref()));
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
    let mut form = form.child(properties(k, edit));
    for corner in actions(k, edit) {
        form = form.child(corner);
    }
    form
}

fn actions(k: &Kit, edit: &RuleEdit) -> Vec<El> {
    let mut buttons = row()
        .key("rule-editor-actions")
        .justify_end()
        .items_center()
        .gap(ACTIONS_SPACING)
        .child(
            Button::new("Cancel")
                .variant(ButtonVariant::Neutral)
                .control_size(ControlSize::Small)
                .on_press(|s: &mut App| s.discard_rule_edit())
                .build(k)
                .key("rule-editor-cancel")
                .w(CANCEL_WIDTH),
        );
    let mut width = CANCEL_WIDTH;
    if edit.new || edit.shown_dirty {
        buttons = buttons.child(
            Button::new("Save")
                .variant(ButtonVariant::Contrast)
                .control_size(ControlSize::Small)
                .on_press(|s: &mut App| s.save_rule_edit())
                .build(k)
                .key("rule-editor-save")
                .w(SAVE_WIDTH),
        );
        width += ACTIONS_SPACING + SAVE_WIDTH;
    }
    let form_radius = well_radius(k);
    let actions_radius = (form_radius - ACTIONS_GAP).max(0.0);
    let cut = CornerCut::new(
        (
            width + 2.0 * ACTIONS_GAP,
            ACTIONS_HEIGHT + 2.0 * ACTIONS_GAP,
        ),
        form_radius,
        ACTIONS_GAP + actions_radius,
    );
    vec![
        cut.build(k.c.bg_base, k.c.border_muted)
            .absolute(WELL_WIDTH - cut.bounds().0, 0.0)
            .key("rule-editor-corner"),
        buttons
            .absolute(WELL_WIDTH - ACTIONS_GAP - width, ACTIONS_GAP)
            .w(width)
            .h(ACTIONS_HEIGHT)
            .radius(actions_radius),
    ]
}

fn well_radius(k: &Kit) -> f32 {
    (k.radius.lg - WELL_MARGIN).max(0.0)
}

fn matcher_row(k: &Kit, edit: &RuleEdit, matcher: &Matcher, exclude: bool, index: usize) -> El {
    let list = if exclude { "x" } else { "m" };
    let prefix = format!("{list}{index}");
    let mut line = row()
        .w_full()
        .gap(6.0)
        .center_y()
        .hover_group()
        .child(rule_field(
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
            .key(format!("remove-{prefix}"))
            .group_opacity(0.0, 1.0),
    )
}

fn problem_line(k: &Kit, problem: Option<&str>) -> El {
    let mut line = row()
        .key("rule-editor-problem")
        .w_full()
        .h(PROBLEM_HEIGHT)
        .items_center();
    if let Some(problem) = problem {
        line = line.child(
            text(problem.to_owned())
                .key("rule-editor-error")
                .font(k.ty.caption().lh(PROBLEM_HEIGHT))
                .color(k.c.danger.fg)
                .max_lines(1)
                .w_full(),
        );
    }
    line
}

fn well(k: &Kit) -> Color {
    k.c.black.alpha(if k.dark { 0.45 } else { 0.05 })
}

fn empty_line(k: &Kit, message: &str) -> El {
    text(message).font(k.ty.small).color(k.c.text_faint).wrap()
}

fn kind_label(kind: Option<EntryKind>) -> &'static str {
    match kind {
        None => "any",
        Some(EntryKind::File) => "file",
        Some(EntryKind::Folder) => "folder",
    }
}

fn properties(k: &Kit, edit: &RuleEdit) -> El {
    let unset: Vec<&str> = RuleProp::all(edit.draft.kind)
        .iter()
        .copied()
        .filter(|prop| !prop.is_set(&edit.draft.props))
        .map(RuleProp::name)
        .collect();
    let mut section = col().key("rule-editor-properties").w_full().gap(6.0).child(
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
                Button::new("Add Property")
                    .variant(ButtonVariant::Outline)
                    .control_size(ControlSize::Small)
                    .disabled(unset.is_empty())
                    .on_press(|s: &mut App| s.toggle_rule_menu(RuleMenu::AddProperty))
                    .build(k)
                    .key("add-property-trigger"),
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
                .group_opacity(0.0, 1.0),
        )
}

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
                .on_choose(move |s: &mut App, value: &str, _: &mut Tx| s.set_rule_prop(prop, value))
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
