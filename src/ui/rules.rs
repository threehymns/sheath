//! The Rules category: one section per rule kind, each listing its rules in file order with
//! their matchers, draggable within the section and deletable.

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

/// The space between two rules: the hairline `settings_section` draws between them, plus the
/// sliver kept above and below each row. A row stepping aside by its height plus this lands
/// where the next row was, which is the gap a drop goes into.
const RULE_ROW_GAP: f32 = 1.0 + 2.0 * ROW_PAD;

/// The sliver of card kept around a rule's row, which its hover fill is rounded to: the row's
/// radius is the card's less this, so the two curves are concentric.
const ROW_MARGIN: f32 = 4.0;

/// The card kept above and below a row's hover fill, so the shade neither touches the hairline
/// over it nor the card's edge at either end of the list.
const ROW_PAD: f32 = 4.0;

/// A file that does not parse is shown as an error and left alone. The rules themselves are
/// drawn in two sections, one per kind, each with the button that writes another of it.
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
    page = page.child(rule_section(s, k, Kind::Folder, read_only));
    page.child(rule_section(s, k, Kind::Entry, read_only))
}

/// One of the page's two sections: the rules of one kind in file order, under the button that
/// writes another. A folder rule is written for the folder on screen; an entry rule is written
/// for the entry the browser has selected, so that button waits for one.
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
    // A form on a rule the file does not hold yet stands at the end of the section it will be
    // appended to.
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
        // An open form takes the row's place instead of unfolding under it: the row's summary
        // and kind are the form's own fields, so the header would only repeat them.
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
        // A form springs the card open around it, so the card keeps its rows inside its edge;
        // a row in hand rides above the card's rows and must not be cut by it.
        s.rule_drag.dragging().is_none(),
    )
}

/// The section's Add Rule, while the file parses: it opens the form on a rule the file does not
/// hold yet, so nothing is written until Save. A folder rule starts from the folder on screen
/// and the layout it is showing, an entry rule from the entry the browser has selected, so either
/// button is live whatever the browser holds.
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

/// The positions of one kind's rules, in file order: the rows its section draws, and the places
/// its drag counts in, so a rule never reorders past a rule of the other kind.
fn section_indices(s: &App, kind: Kind) -> Vec<usize> {
    s.rules
        .rules
        .iter()
        .enumerate()
        .filter(|(_, rule)| rule.kind == kind)
        .map(|(index, _)| index)
        .collect()
}

/// A section's line for when it holds no rules yet, where a row would be.
fn empty_note(k: &Kit, message: &str) -> El {
    row().w_full().px(SETTINGS_ROW_INSET).py(10.0).child(
        text(message)
            .font(k.ty.caption())
            .color(k.c.text_muted)
            .wrap(),
    )
}

/// One rule: its matcher, the kind of rule it is, and the controls for it. A press anywhere on
/// the row opens the rule's form; dragging rewrites the order of the rule nodes, and the rule's
/// comments travel with it. Delete asks in the row before it removes anything. `slot` is the
/// row's place in its section, which is what the drag counts in.
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
        // The name and the line under it come to 38 points; with this they sit in the row's 48.
        .py(5.0)
        .gap(12.0)
        .center_y()
        // A row is a press target, so it takes the kit's hover shade; its corners are the card's
        // less the sliver it keeps, which is what makes the two curves concentric.
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
        // The row asks in place instead of opening a modal for one line of the list, and keeps
        // its height, so nothing below it moves.
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
                    // A press is not yet a drag: the row opens its editor on a click, so nothing
                    // moves until the pointer has travelled far enough to mean a move.
                    Phase::Down => s.rule_dragged = false,
                    Phase::Move => {
                        // The form sits between two rows, so the list only has one row height
                        // while no form is open. A clean form closes when the drag really
                        // starts; one holding changes stays, and the drag does not start.
                        if s.rule_edit.is_some() {
                            if s.rule_edit.as_ref().is_some_and(|edit| edit.dirty())
                                || !RuleDrag::travelled(drag.dy)
                            {
                                // A gesture the form refuses is still not a click: its release
                                // must not toggle the form shut and drop what it holds.
                                if RuleDrag::travelled(drag.dy) {
                                    s.rule_dragged = true;
                                }
                                return;
                            }
                            s.close_rule_edit();
                        }
                        // The rows the drag passes spring aside; the row in hand follows the
                        // pointer exactly, which is why the travel is measured on the screen
                        // and not on the row the pointer is dragging. The drag counts in the
                        // section's own places, so a rule never passes one of the other kind.
                        tx.animated();
                        let count = s
                            .rules
                            .rules
                            .iter()
                            .filter(|rule| rule.kind == kind)
                            .count();
                        s.rule_drag
                            .follow(kind, slot, count, drag.dy, drag.h, RULE_ROW_GAP);
                        // A release over the row arrives as a press too, so a gesture that
                        // passed over another place must not open the row's form.
                        if s.rule_drag.moves() {
                            s.rule_dragged = true;
                        }
                    }
                    Phase::Up => {
                        // The gap the drag opened is the order it drops into, so the reorder
                        // lands in the same frame the rows' offsets clear: there is nothing to
                        // animate, and the list must not spring in from the order the drag
                        // started in.
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
                // A press on the row opens the rule's form, whether it lands on the name or
                // on the space beside it.
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
                    // The row is the group: the button is found once the pointer rests on
                    // the row, and out of the way the rest of the time.
                    .group_opacity(0.0, 1.0),
            );
    }
    // The row in hand lifts above the rest and follows the pointer, and the rows it passes
    // step aside, so the gap it would drop into is the order it would land in.
    line = line.offset(0.0, s.rule_drag.shift(kind, slot));
    if s.rule_drag.dragging() == Some((kind, slot)) {
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
