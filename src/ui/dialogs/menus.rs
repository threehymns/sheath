use crate::app::App;
use crate::icons;
use crate::keybinds::Action;
use rune::{El, PointerPress, el};
use rune_kit::{Kit, MenuEntry, MenuItem, context_menu, glyphs};
use rune_theme_opencode as theme;

pub(crate) fn place_menu(s: &App, k: &Kit, layers: &mut Vec<El>) {
    let Some(menu) = &s.place_menu else {
        return;
    };
    let Some(place) = s.places.get(&menu.id) else {
        return;
    };
    let name = place.name.clone();
    let icon = if crate::vfs::is_dir(&place.path) {
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

/// The menu behind a right click on the header: which columns the list keeps.
///
/// Name is always there, since a list of rows with no names says nothing.
pub(crate) fn header_menu(s: &App, k: &Kit, layers: &mut Vec<El>) {
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
                s.toggle_setting(crate::settings::Setting::ShowKind);
            }),
        )
        .item(
            column(s.config.show_size, "Size Column").on_select(|s: &mut App| {
                s.close_header_menu();
                s.toggle_setting(crate::settings::Setting::ShowSize);
            }),
        )
        .item(
            column(s.config.show_modified, "Date Modified Column").on_select(|s: &mut App| {
                s.close_header_menu();
                s.toggle_setting(crate::settings::Setting::ShowModified);
            }),
        )
        .separator()
        .item(
            column(s.config.folders_first, "Folders First").on_select(|s: &mut App| {
                s.close_header_menu();
                s.toggle_setting(crate::settings::Setting::FoldersFirst);
            }),
        )
        .on_close(|s: &mut App| s.close_header_menu())
        .build(k, layers);
}

/// The menu behind a right click that landed past the last row: what the folder itself offers.
pub(crate) fn empty_menu(s: &App, k: &Kit, layers: &mut Vec<El>) {
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

pub(crate) fn row_menu(s: &App, k: &Kit, at: PointerPress, layers: &mut Vec<El>) {
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
            MenuItem::new("Copy Path")
                .icon(&icons::CLIPBOARD)
                .shortcut(s.keybinds.menu_label(Action::CopyPath))
                .disabled(count == 0)
                .on_select_tx(|s: &mut App, tx: &mut rune::Tx| {
                    s.close_menu();
                    s.copy_paths(tx);
                }),
        )
        .separator()
        .item(
            MenuItem::new("Rules")
                .icon(&theme::icons::EDIT)
                .submenu(
                    vec![
                        MenuEntry::item(
                            MenuItem::new("Save This Layout as a Rule")
                                .icon(&theme::icons::EDIT)
                                .on_select(|s: &mut App| {
                                    s.close_menu();
                                    s.save_layout_rule();
                                }),
                        ),
                        MenuEntry::item(
                            MenuItem::new("Show All Rules")
                                .icon(&glyphs::KEYBOARD)
                                .on_select(|s: &mut App| {
                                    s.close_menu();
                                    s.open_rule_picker();
                                }),
                        ),
                        MenuEntry::item(
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
                        ),
                    ],
                    s.menu_rules,
                )
                .on_select(|s: &mut App| {
                    s.toggle_menu_rules();
                }),
        )
        .on_close(|s: &mut App| s.close_menu())
        .build(k, layers);
}
