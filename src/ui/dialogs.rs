use crate::app::{App, Sheet, SheetKind};
use crate::icons;
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

pub(super) fn place_menu(s: &App, k: &Kit, layers: &mut Vec<El>) {
    let Some(menu) = &s.place_menu else {
        return;
    };
    let Some(place) = s.places.get(&menu.id) else {
        return;
    };
    let name = place.name.clone();
    let icon = if place.path.is_dir() {
        &glyphs::FOLDER_OPEN
    } else {
        &glyphs::FILE
    };
    let can_reveal = place.path.parent().is_some();
    context_menu("place-menu", el(), Some(menu.press))
        .min_width(220.0)
        .item(
            MenuItem::new(format!("Open “{name}”"))
                .icon(icon)
                .on_select(|s: &mut App| s.open_menu_place()),
        )
        .item(
            MenuItem::new("Show in Folder")
                .icon(&glyphs::FOLDER)
                .disabled(!can_reveal)
                .on_select(|s: &mut App| s.reveal_menu_place()),
        )
        .separator()
        .item(
            MenuItem::new("Edit Name…")
                .icon(&theme::icons::EDIT)
                .on_select(|s: &mut App| s.begin_menu_place_rename()),
        )
        .item(
            MenuItem::new("Rename on Disk…")
                .icon(&glyphs::FOLDER)
                .disabled(place.path.file_name().is_none())
                .on_select(|s: &mut App| s.begin_menu_place_rename_on_disk()),
        )
        .item(
            MenuItem::new("Remove from Sidebar")
                .icon(&theme::icons::TRASH)
                .on_select(|s: &mut App| s.remove_menu_place()),
        )
        .on_close(|s: &mut App| s.close_place_menu())
        .build(k, layers);
}

pub(super) fn error_text(k: &Kit, error: &str) -> El {
    text(error.to_owned())
        .font(k.ty.caption())
        .color(k.c.danger.fg)
        .wrap()
}

/// The menu behind a right click on the header: which columns the list keeps.
///
/// Name is always there, since a list of rows with no names says nothing.
pub(super) fn header_menu(s: &App, k: &Kit, layers: &mut Vec<El>) {
    let Some(at) = s.header_menu else {
        return;
    };
    let column = |on: bool, label: &str| {
        MenuItem::new(label).icon(if on {
            &icons::CHECK_SQUARE
        } else {
            &icons::SQUARE
        })
    };
    context_menu("header-menu", el(), Some(at))
        .min_width(200.0)
        .item(
            column(s.config.show_kind, "Kind Column").on_select(|s: &mut App| {
                s.close_header_menu();
                s.toggle_show_kind();
            }),
        )
        .item(
            column(s.config.show_size, "Size Column").on_select(|s: &mut App| {
                s.close_header_menu();
                s.toggle_show_size();
            }),
        )
        .item(
            column(s.config.show_modified, "Date Modified Column").on_select(|s: &mut App| {
                s.close_header_menu();
                s.toggle_show_modified();
            }),
        )
        .separator()
        .item(
            column(s.config.folders_first, "Folders First").on_select(|s: &mut App| {
                s.close_header_menu();
                s.toggle_folders_first();
            }),
        )
        .on_close(|s: &mut App| s.close_header_menu())
        .build(k, layers);
}

/// The menu behind a right click that landed past the last row: what the folder itself offers.
pub(super) fn empty_menu(s: &App, k: &Kit, layers: &mut Vec<El>) {
    let Some(at) = s.empty_menu else {
        return;
    };
    let any = !s.visible.is_empty();
    context_menu("empty-menu", el(), Some(at))
        .min_width(220.0)
        .item(
            MenuItem::new("New Folder")
                .icon(&icons::FOLDER_PLUS)
                .shortcut(s.keybinds.menu_label(Action::NewFolder))
                .on_select_tx(|s: &mut App, tx: &mut rune::Tx| {
                    s.close_empty_menu();
                    s.new_folder(tx);
                }),
        )
        .item(
            MenuItem::new("New File")
                .icon(&glyphs::FILE)
                .shortcut(s.keybinds.menu_label(Action::NewFile))
                .on_select_tx(|s: &mut App, tx: &mut rune::Tx| {
                    s.close_empty_menu();
                    s.new_file(tx);
                }),
        )
        .separator()
        .item(
            MenuItem::new("Paste")
                .icon(&icons::CLIPBOARD)
                .shortcut(s.keybinds.menu_label(Action::Paste))
                .disabled(s.clipboard.is_none())
                .on_select(|s: &mut App| {
                    s.close_empty_menu();
                    s.paste();
                }),
        )
        .separator()
        .item(
            MenuItem::new("Select All")
                .icon(&icons::CHECK_SQUARE)
                .shortcut(s.keybinds.menu_label(Action::SelectAll))
                .disabled(!any)
                .on_select(|s: &mut App| {
                    s.close_empty_menu();
                    s.select_all();
                }),
        )
        .item(
            MenuItem::new("Refresh")
                .icon(&icons::REFRESH)
                .shortcut(s.keybinds.menu_label(Action::Refresh))
                .on_select(|s: &mut App| {
                    s.close_empty_menu();
                    s.reload();
                }),
        )
        .separator()
        .item(
            MenuItem::new(if s.show_hidden {
                "Hide Hidden Files"
            } else {
                "Show Hidden Files"
            })
            .icon(k.glyph.eye)
            .shortcut(s.keybinds.menu_label(Action::ToggleHidden))
            .on_select(|s: &mut App| {
                s.close_empty_menu();
                s.toggle_hidden();
            }),
        )
        .item(
            MenuItem::new("Add This Folder to the Sidebar")
                .icon(&icons::FOLDER_PLUS)
                .on_select(|s: &mut App| {
                    s.close_empty_menu();
                    s.add_current_place();
                }),
        )
        .item(
            MenuItem::new("Settings")
                .icon(&theme::icons::SETTINGS_GEAR)
                .shortcut(s.keybinds.menu_label(Action::OpenSettings))
                .on_select(|s: &mut App| {
                    s.close_empty_menu();
                    s.open_settings();
                }),
        )
        .on_close(|s: &mut App| s.close_empty_menu())
        .build(k, layers);
}

pub(super) fn row_menu(s: &App, k: &Kit, at: PointerPress, layers: &mut Vec<El>) {
    let count = s.selection_len();
    let many = count > 1;
    let hidden = s
        .selected
        .as_ref()
        .and_then(|id| s.entries.iter().find(|entry| &entry.id == id))
        .is_some_and(|entry| s.entry_hidden(entry));
    context_menu("row-menu", el(), Some(at))
        .min_width(240.0)
        .item(
            MenuItem::new(if many {
                format!("Open {count} Items")
            } else {
                "Open".to_owned()
            })
            .icon(&glyphs::FILE)
            .disabled(many)
            .on_select(|s: &mut App| {
                s.close_menu();
                s.open_selected();
            }),
        )
        .item(
            MenuItem::new("Reveal in File Manager")
                .icon(&glyphs::FOLDER_OPEN)
                .disabled(count == 0)
                .on_select(|s: &mut App| {
                    s.close_menu();
                    s.reveal_selected();
                }),
        )
        .item(
            MenuItem::new("Open Terminal Here")
                .icon(&icons::TERMINAL)
                .on_select(|s: &mut App| {
                    s.close_menu();
                    s.open_terminal_in_selection();
                }),
        )
        .separator()
        .item(
            MenuItem::new(if many {
                format!("Rename {count} Items…")
            } else {
                "Rename".to_owned()
            })
            .icon(&theme::icons::EDIT)
            .shortcut(s.keybinds.menu_label(Action::Rename))
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
        .item(
            MenuItem::new(if many {
                format!("Delete {count} Items Permanently")
            } else {
                "Delete Permanently".to_owned()
            })
            .icon(&theme::icons::WARNING)
            .shortcut(s.keybinds.menu_label(Action::Delete))
            .on_select(|s: &mut App| {
                s.close_menu();
                s.begin_delete();
            }),
        )
        .item(
            MenuItem::new("Copy")
                .icon(&theme::icons::COPY)
                .shortcut(s.keybinds.menu_label(Action::Copy))
                .on_select(|s: &mut App| {
                    s.close_menu();
                    s.copy_selection(false);
                }),
        )
        .item(
            MenuItem::new("Cut")
                .icon(&icons::SCISSORS)
                .shortcut(s.keybinds.menu_label(Action::Cut))
                .on_select(|s: &mut App| {
                    s.close_menu();
                    s.copy_selection(true);
                }),
        )
        .item(
            MenuItem::new("Paste")
                .icon(&icons::CLIPBOARD)
                .shortcut(s.keybinds.menu_label(Action::Paste))
                .disabled(s.clipboard.is_none())
                .on_select(|s: &mut App| {
                    s.close_menu();
                    s.paste();
                }),
        )
        .item(
            MenuItem::new("Duplicate")
                .icon(&theme::icons::OUTLINE_COPY)
                .shortcut(s.keybinds.menu_label(Action::Duplicate))
                .on_select(|s: &mut App| {
                    s.close_menu();
                    s.duplicate_selection();
                }),
        )
        .item(
            MenuItem::new("Make Alias")
                .icon(&icons::LINK)
                .on_select(|s: &mut App| {
                    s.close_menu();
                    s.make_aliases();
                }),
        )
        .item(
            MenuItem::new("Copy Name")
                .icon(&icons::CLIPBOARD)
                .disabled(count == 0)
                .on_select_tx(|s: &mut App, tx: &mut rune::Tx| {
                    s.close_menu();
                    s.copy_names(tx);
                }),
        )
        .separator()
        .item(
            MenuItem::new("Save This Layout as a Rule")
                .icon(&theme::icons::EDIT)
                .on_select(|s: &mut App| {
                    s.close_menu();
                    s.save_layout_rule();
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
