//! The modal layers over the browser: the rename, move and trash sheets, and the row menu.

use crate::app::{App, Sheet, SheetKind};
use crate::keybinds::Action;
use rune::{El, PointerPress, col, el, row, text};
use rune_kit::{
    Button, ButtonVariant, ControlSize, Dialog, Kit, MenuItem, TextInput, context_menu, glyphs,
};
use rune_theme_opencode as theme;

use super::inspector::selection_names;

pub(super) fn sheet_layer(s: &App, sheet: &Sheet, k: &Kit, layers: &mut Vec<El>) {
    match sheet.kind {
        SheetKind::Rename => prompt_sheet(
            sheet,
            k,
            layers,
            "rename",
            "Rename",
            "Choose a new name for the selected item.",
            "Rename",
            "Name",
            "Press Return to rename. Escape cancels.",
        ),
        SheetKind::Move => prompt_sheet(
            sheet,
            k,
            layers,
            "move",
            "Move",
            if sheet.many() {
                "Move the selected items into another folder."
            } else {
                "Move the selected item into another folder."
            },
            "Move",
            "Destination folder",
            "A folder path. Use ~ for your home folder.",
        ),
        SheetKind::Trash => trash_sheet(s, sheet, k, layers),
    }
}

#[allow(clippy::too_many_arguments)]
fn prompt_sheet(
    sheet: &Sheet,
    k: &Kit,
    layers: &mut Vec<El>,
    id: &str,
    title: &str,
    description: &str,
    action: &str,
    placeholder: &str,
    hint: &str,
) {
    let input = TextInput::new(&sheet.edit)
        .placeholder(placeholder)
        .width(None)
        .on_edit(|s: &mut App| s.sheet_edit())
        .on_submit(|s: &mut App, tx| s.submit_sheet(tx))
        .build(k)
        .key(format!("{id}-field"));
    let mut body = col()
        .key(format!("{id}-body"))
        .w_full()
        .px(16.0)
        .pb(16.0)
        .gap(10.0)
        .child(
            text(sheet.name.clone())
                .font(k.ty.ui_medium)
                .color(k.c.text_base)
                .max_lines(2)
                .w_full(),
        )
        .child(input)
        .child(text(hint).font(k.ty.caption()).color(k.c.text_faint).wrap());
    if let Some(error) = &sheet.error {
        body = body.child(error_text(k, error));
    }
    let cancel = Button::new("Cancel")
        .variant(ButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .on_press(|s: &mut App| s.close_sheet())
        .build(k)
        .key(format!("{id}-cancel"));
    let submit = Button::new(action)
        .variant(ButtonVariant::Contrast)
        .control_size(ControlSize::Regular)
        .on_press_tx(|s: &mut App, tx| s.submit_sheet(tx))
        .build(k)
        .key(format!("{id}-submit"))
        .default_button();
    body = body.child(
        row()
            .w_full()
            .justify_end()
            .gap(8.0)
            .child(cancel)
            .child(submit),
    );
    Dialog::new(id, title)
        .description(description)
        .width(400.0)
        .fit()
        .open(true)
        .child(body)
        .on_close(|s: &mut App| s.close_sheet())
        .build(k, layers);
}

fn trash_sheet(s: &App, sheet: &Sheet, k: &Kit, layers: &mut Vec<El>) {
    let mut body = col()
        .key("trash-body")
        .w_full()
        .px(16.0)
        .pb(16.0)
        .gap(10.0)
        .child(
            text(sheet.name.clone())
                .font(k.ty.ui_medium)
                .color(k.c.text_base)
                .max_lines(2)
                .w_full(),
        );
    if sheet.many() {
        body = body.child(selection_names(s, &sheet.targets, k));
    }
    if let Some(error) = &sheet.error {
        body = body.child(error_text(k, error));
    }
    let cancel = Button::new("Cancel")
        .variant(ButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .on_press(|s: &mut App| s.close_sheet())
        .build(k)
        .key("trash-cancel")
        .default_button();
    let confirm = Button::new("Move to Trash")
        .variant(ButtonVariant::Danger)
        .icon(&theme::icons::TRASH)
        .control_size(ControlSize::Regular)
        .on_press_tx(|s: &mut App, tx| s.submit_sheet(tx))
        .build(k)
        .key("trash-submit");
    body = body.child(
        row()
            .w_full()
            .justify_end()
            .gap(8.0)
            .child(cancel)
            .child(confirm),
    );
    Dialog::new("trash", "Move to Trash?")
        .description(if sheet.many() {
            "The items will move to the system Trash."
        } else {
            "The item will move to the system Trash."
        })
        .width(400.0)
        .fit()
        .open(true)
        .child(body)
        .on_close(|s: &mut App| s.close_sheet())
        .build(k, layers);
}

pub(super) fn error_text(k: &Kit, error: &str) -> El {
    text(error.to_owned())
        .font(k.ty.caption())
        .color(k.c.danger.fg)
        .wrap()
}

/// The row menu a right-click opens: the file operations, at the press point. A selection of
/// several rows moves or trashes all of them, so the labels say how many.
pub(super) fn row_menu(s: &App, k: &Kit, at: PointerPress, layers: &mut Vec<El>) {
    let count = s.selection_len();
    let many = count > 1;
    let hidden = s
        .selected
        .as_ref()
        .and_then(|id| s.entries.iter().find(|entry| &entry.id == id))
        .is_some_and(|entry| entry.hidden());
    context_menu("row-menu", el(), Some(at))
        .item(
            // One name at a time: the sheet renames a single row.
            MenuItem::new("Rename")
                .icon(&theme::icons::EDIT)
                .shortcut(s.keybinds.menu_label(Action::Rename))
                .disabled(many)
                .on_select(|s: &mut App| {
                    s.close_menu();
                    s.begin_rename();
                }),
        )
        .item(
            MenuItem::new(if many {
                format!("Move {count} Items…")
            } else {
                "Move to Folder…".to_owned()
            })
            .icon(&glyphs::FOLDER)
            .shortcut(s.keybinds.menu_label(Action::Move))
            .on_select(|s: &mut App| {
                s.close_menu();
                s.begin_move();
            }),
        )
        .separator()
        .item(
            MenuItem::new(if many {
                format!("Move {count} Items to Trash")
            } else {
                "Move to Trash".to_owned()
            })
            .icon(&theme::icons::TRASH)
            .shortcut(s.keybinds.menu_label(Action::Trash))
            .on_select(|s: &mut App| {
                s.close_menu();
                s.begin_trash();
            }),
        )
        .separator()
        .item(
            MenuItem::new("Save This View as a Rule")
                .icon(&theme::icons::EDIT)
                .on_select(|s: &mut App| {
                    s.close_menu();
                    s.save_view_rule();
                }),
        )
        .item(
            MenuItem::new("Show All Rules")
                .icon(&glyphs::KEYBOARD)
                .on_select(|s: &mut App| {
                    s.close_menu();
                    s.open_rule_picker();
                }),
        )
        .item(
            MenuItem::new(if hidden {
                "Always Show This Entry"
            } else {
                "Always Hide This Entry"
            })
            .icon(k.glyph.eye)
            .disabled(many)
            .on_select(move |s: &mut App| {
                s.close_menu();
                s.pin_entry(!hidden);
            }),
        )
        .on_close(|s: &mut App| s.close_menu())
        .build(k, layers);
}
