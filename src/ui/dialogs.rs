use crate::app::{App, Sheet, SheetKind};
use rune::{El, col, row, text};
use rune_kit::{Button, ButtonVariant, ControlSize, Dialog, Kit, TextInput};
use rune_theme_opencode as theme;

use super::inspector::selection_names;

mod menus;
pub(super) use menus::{empty_menu, header_menu, place_menu, row_menu};

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
        SheetKind::RenameMany => prompt_sheet(
            sheet,
            k,
            layers,
            "rename-many",
            "Rename Items",
            "Rename the selected items together, in the order they read.",
            "Rename",
            "Name",
            "Each item takes this name and a number, keeping its extension: Item 1.txt, \
             Item 2.txt. Press Return to rename. Escape cancels.",
        ),
        SheetKind::RenamePlace => prompt_sheet(
            sheet,
            k,
            layers,
            "rename-place",
            "Rename",
            "Choose a new name for this folder or file itself.",
            "Rename",
            "Name",
            "Renames it on disk; the sidebar follows. Press Return to rename. Escape cancels.",
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
        SheetKind::Delete => delete_sheet(s, sheet, k, layers),
        SheetKind::Navigate => prompt_sheet(
            sheet,
            k,
            layers,
            "go-to",
            "Go to Folder",
            "Type a path to open its folder.",
            "Go",
            "Folder path",
            "Use ~ for your home folder. Press Return to go. Escape cancels.",
        ),
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
        .gap(10.0);
    if !sheet.name.is_empty() {
        body = body.child(
            text(sheet.name.clone())
                .font(k.ty.ui_medium)
                .color(k.c.text_base)
                .max_lines(2)
                .w_full(),
        );
    }
    body = body
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

fn delete_sheet(s: &App, sheet: &Sheet, k: &Kit, layers: &mut Vec<El>) {
    let mut body = col()
        .key("delete-body")
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
        .key("delete-cancel")
        .default_button();
    let confirm = Button::new("Delete")
        .variant(ButtonVariant::Danger)
        .icon(&theme::icons::TRASH)
        .control_size(ControlSize::Regular)
        .on_press_tx(|s: &mut App, tx| s.submit_sheet(tx))
        .build(k)
        .key("delete-submit");
    body = body.child(
        row()
            .w_full()
            .justify_end()
            .gap(8.0)
            .child(cancel)
            .child(confirm),
    );
    Dialog::new("delete", "Delete Permanently?")
        .description(if sheet.many() {
            "The items will be deleted and cannot be recovered from the Trash."
        } else {
            "The item will be deleted and cannot be recovered from the Trash."
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
