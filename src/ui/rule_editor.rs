//! The rule form: the editor the Rules tab opens under a row. It lays out the draft's name,
//! kind, matches, excludes and properties, each under its own heading; every control edits the
//! draft, and Save writes it. Save and Discard Changes are up only while the draft differs from
//! the file, so a form opened on a rule has nothing to press.

use crate::app::{App, RuleEdit, RuleMenu, RuleProp};
use crate::corner::CornerCut;
use crate::rules::{EntryKind, Kind, Matcher};
use rune::{Color, El, Shadow, TextEdit, Tx, col, row, text};
use rune_kit::{
    Button, ButtonVariant, ControlSize, IconButton, IconButtonVariant, Kit, Select,
    SelectAppearance, TextInput,
};
use rune_theme_opencode as theme;
use std::collections::BTreeMap;

use super::settings::{SETTINGS_CARD_WIDTH, SETTINGS_ROW_INSET};

/// The width of a property's field, and of the two menus that are not properties.
const PROP_WIDTH: f32 = 150.0;
const KIND_WIDTH: f32 = 110.0;

/// The form's actions: each button's width, the space between them, the small control's height
/// (20 points), and the card kept around them. The bite is cut to whichever are up, so it never
/// leaves more card around them than the gap.
const CANCEL_WIDTH: f32 = 66.0;
const SAVE_WIDTH: f32 = 52.0;
const ACTIONS_HEIGHT: f32 = 20.0;
const ACTIONS_SPACING: f32 = 6.0;
const ACTIONS_GAP: f32 = 8.0;

/// The sliver of card kept around the form, whose corners the form's are set concentric with.
const WELL_MARGIN: f32 = 4.0;

/// The height of the line under the form's head that a refused Save explains itself on. The
/// line is always in the layout at this height, empty or not, so a message arriving or leaving
/// cannot move the sections under it; it is one line tall, so a longer sentence cannot either.
const PROBLEM_HEIGHT: f32 = 16.0;

/// The form's width: the card's, less the sliver on either side.
const WELL_WIDTH: f32 = SETTINGS_CARD_WIDTH - 2.0 * WELL_MARGIN;

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
    // The form is cut into the card rather than laid on top of it: a sliver of card around it,
    // corners concentric with the card's, a fill a clear step darker, and an inset shade along
    // the top edge.
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

/// The form's actions in a bite out of its top-right corner, on the card rather than in the
/// form: the cut is painted over the corner first, so the buttons look set into the card beside
/// it, as a dialog's Done sits outside its panel. The gap around them is the same on every side
/// — the bite is the buttons plus a gap all round — and the rounded corners stay concentric:
/// the buttons' radius is the form's less the gap, and the bite's inner corner is a gap wider
/// than theirs, so both arcs share a centre. Cancel is always there, so a form with nothing to
/// write can still be closed, and Save is the form's primary action, up once there is something
/// to write.
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
    // A rule the file does not hold yet has something to write from the moment its form opens.
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

/// The form's corner radius: the card's less the sliver kept around it, so the two curves are
/// concentric.
fn well_radius(k: &Kit) -> f32 {
    (k.radius.lg - WELL_MARGIN).max(0.0)
}

/// One `match` or `exclude` directive: a field per key the rule's kind accepts, and a button
/// that drops it.
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
            // The line is the group: the button is found once the pointer rests on it.
            .group_opacity(0.0, 1.0),
    )
}

/// The form's line for what Save refused, in the form's own words. It is drawn whether or not
/// there is anything to say — an empty one shows nothing — so pressing Save, and typing the
/// refusal away again, leaves every row under it where it was.
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

/// What an open form is filled with: black laid straight over the card, so the form is a clear
/// step darker than the rows above it in either appearance. A solved token would depend on what
/// the kit blends over, which is not the card the form actually sits on.
fn well(k: &Kit) -> Color {
    k.c.black.alpha(if k.dark { 0.45 } else { 0.05 })
}

/// A section's line for when it holds nothing, in the same faded type as the rest.
fn empty_line(k: &Kit, message: &str) -> El {
    text(message).font(k.ty.small).color(k.c.text_faint).wrap()
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
                // The menu hangs off this key, so the button is the list's trigger.
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
        .on_change(|s: &mut App, tx: &mut Tx| {
            // A keystroke is the user's own hand, so its commit lands at once and cannot
            // animate. Save is drawn from `shown_dirty`, left as it was here: the next frame
            // catches it up, and that commit is the one that animates. Nothing else would ask
            // for that frame once the key is drawn, so this does.
            let shown = s.rule_edit.as_ref().map(|edit| edit.shown_dirty);
            s.apply_rule_fields();
            if let (Some(shown), Some(edit)) = (shown, s.rule_edit.as_mut()) {
                if edit.dirty() != shown {
                    tx.wake_after(std::time::Duration::ZERO);
                }
                edit.shown_dirty = shown;
            }
        })
        .build(k)
        .key(format!("rule-field-{id}"))
}
