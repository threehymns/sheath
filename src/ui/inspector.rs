use crate::app::{App, Preview};
use crate::format;
use rune::{Align, El, Tx, col, el, icon, row, text};
use rune_kit::{Button, ButtonVariant, ControlSize, Kit, file_icon, glyphs};

pub(super) fn inspector(s: &App, k: &'static Kit) -> El {
    if s.selection_len() > 1 {
        return selection_inspector(s, k);
    }
    let Some(entry) = s.selected_entry() else {
        return col()
            .key("inspector")
            .instant_changes()
            .w_full()
            .h_full()
            .px(20.0)
            .center()
            .gap(8.0)
            .child(icon(&glyphs::FILE, 40.0).color(k.c.icon_faint))
            .child(
                text("No selection")
                    .font(k.ty.ui_medium)
                    .color(k.c.text_muted),
            )
            .child(
                text("Select a file or folder to inspect it.")
                    .font(k.ty.caption())
                    .color(k.c.text_faint)
                    .align(Align::Center),
            );
    };
    let glyph = if entry.is_dir {
        &glyphs::FOLDER
    } else {
        file_icon(&entry.name)
    };
    let icon_color = if entry.is_dir {
        k.c.icon_accent
    } else {
        k.c.icon_muted
    };
    let parent = entry
        .path
        .parent()
        .map(|p| p.display().to_string())
        .unwrap_or_default();
    let mut panel = col()
        .key("inspector")
        .instant_changes()
        .w_full()
        .h_full()
        .min_h(0.0)
        .px(16.0)
        .py(14.0)
        .gap(12.0)
        .child(
            col()
                .w_full()
                .gap(3.0)
                .center()
                .child(icon(glyph, 52.0).color(icon_color))
                .child(
                    text(entry.name.clone())
                        .font(k.ty.ui_medium)
                        .color(k.c.text_base)
                        .align(Align::Center)
                        .max_lines(2)
                        .w_full(),
                )
                .child(
                    text(entry.kind())
                        .font(k.ty.caption())
                        .color(k.c.text_muted)
                        .align(Align::Center)
                        .w_full(),
                ),
        )
        .child(rule(k))
        .child(
            col()
                .key("info")
                .w_full()
                .gap(6.0)
                .child(info_row(
                    k,
                    "Size",
                    if entry.is_dir {
                        "—".into()
                    } else {
                        format::size(entry.size)
                    },
                ))
                .child(info_row(k, "Created", format::full_time(entry.created)))
                .child(info_row(k, "Modified", format::full_time(entry.modified)))
                .child(info_row(k, "Permissions", entry.permissions.clone()))
                .child(info_row_with_help(k, "Where", parent.clone(), parent)),
        )
        .child(actions(k, entry))
        .child(rule(k))
        .child(preview_section(s, k));
    if entry.readonly {
        panel = panel.child(
            row()
                .gap(6.0)
                .center_y()
                .child(icon(k.glyph.lock, 12.0).color(k.c.text_faint))
                .child(text("Read-only").font(k.ty.caption()).color(k.c.text_faint)),
        );
    }
    panel
}

fn selection_inspector(s: &App, k: &Kit) -> El {
    let count = s.selection_len();
    let folders = s.selection_folders();
    let files = count - folders;
    let size = s.selection_size();
    col()
        .key("inspector")
        .instant_changes()
        .w_full()
        .h_full()
        .min_h(0.0)
        .px(16.0)
        .py(14.0)
        .gap(12.0)
        .child(
            col()
                .key("selection-summary")
                .w_full()
                .gap(3.0)
                .center()
                .child(icon(&glyphs::FOLDER, 52.0).color(k.c.icon_accent))
                .child(
                    text(format!("{count} items selected"))
                        .font(k.ty.ui_medium)
                        .color(k.c.text_base)
                        .align(Align::Center)
                        .w_full(),
                )
                .child(
                    text("Move and Trash act on all of them.")
                        .font(k.ty.caption())
                        .color(k.c.text_muted)
                        .align(Align::Center)
                        .w_full(),
                ),
        )
        .child(rule(k))
        .child(
            col()
                .key("info")
                .w_full()
                .gap(6.0)
                .child(info_row(k, "Items", count.to_string()))
                .child(info_row(
                    k,
                    "Folders",
                    if folders == 0 {
                        "—".into()
                    } else {
                        folders.to_string()
                    },
                ))
                .child(info_row(
                    k,
                    "Files",
                    if files == 0 {
                        "—".into()
                    } else {
                        files.to_string()
                    },
                ))
                .child(info_row(
                    k,
                    "Size",
                    if files == 0 {
                        "—".into()
                    } else {
                        format::size(size)
                    },
                )),
        )
        .child(rule(k))
        .child(
            text("Select one item to inspect it. Rename works on one item at a time.")
                .font(k.ty.caption())
                .color(k.c.text_faint)
                .wrap(),
        )
}

pub(super) fn selection_names(s: &App, ids: &[String], k: &Kit) -> El {
    let names: Vec<&str> = ids.iter().filter_map(|id| s.entry_name(id)).collect();
    let shown = names.len().min(5);
    let mut list = names[..shown].join("\n");
    if names.len() > shown {
        list.push_str(&format!("\nand {} more", names.len() - shown));
    }
    text(list).font(k.ty.caption()).color(k.c.text_muted).wrap()
}

fn rule(k: &Kit) -> El {
    el().h(1.0).w_full().fill(k.c.border_muted)
}

fn info_row(k: &Kit, label: &str, value: String) -> El {
    info_row_with_help(k, label, value, String::new())
}

fn info_row_with_help(k: &Kit, label: &str, value: String, help: String) -> El {
    let mut value_el = text(value)
        .font(k.ty.caption())
        .color(k.c.text_muted)
        .grow(1.0)
        .shrink()
        .min_w(0.0)
        .truncate();
    if !help.is_empty() {
        value_el = value_el.help(help);
    }
    row()
        .w_full()
        .gap(8.0)
        .center_y()
        .child(
            text(label)
                .font(k.ty.caption())
                .color(k.c.text_faint)
                .w(66.0),
        )
        .child(value_el)
}

fn actions(k: &Kit, entry: &crate::app::Entry) -> El {
    let path = entry.path.to_string_lossy().into_owned();
    row()
        .key("actions")
        .w_full()
        .gap(8.0)
        .child(
            Button::new("Open")
                .variant(ButtonVariant::Contrast)
                .control_size(ControlSize::Regular)
                .on_press(|s: &mut App| s.open_selected())
                .build(k)
                .grow(1.0),
        )
        .child(
            Button::new("Copy Path")
                .variant(ButtonVariant::Outline)
                .control_size(ControlSize::Regular)
                .on_press_tx(move |_: &mut App, tx: &mut Tx| tx.copy(path.clone()))
                .build(k),
        )
}

fn preview_section(s: &App, k: &'static Kit) -> El {
    let mut panel = col()
        .key("preview")
        .w_full()
        .grow(1.0)
        .min_h(0.0)
        .gap(6.0)
        .child(
            text("Preview")
                .font(k.ty.small_medium)
                .color(k.c.text_muted),
        );
    match &s.preview {
        Some(Preview::Text(body)) if body.is_empty() => {
            panel = panel.child(note(k, "Empty file"));
        }
        Some(Preview::Text(body)) => {
            let body: String = body.chars().take(6_000).collect();
            panel = panel.child(super::scroll_area::scrolled(
                s,
                "preview-scroll",
                col().w_full().grow(1.0).min_h(0.0).scroll_edge_fade(12.0),
                el().w_full()
                    .padding(10.0)
                    .radius(k.radius.md)
                    .fill(k.c.layer_02)
                    .child(
                        text(body)
                            .font(k.ty.mono.lh(16.0))
                            .color(k.c.text_muted)
                            .wrap_anywhere(),
                    ),
            ));
        }
        Some(Preview::Folder(count)) => {
            let label = if *count == 1 {
                "1 item".to_owned()
            } else {
                format!("{count} items")
            };
            panel = panel.child(note(k, label));
        }
        Some(Preview::TooLarge) => {
            panel = panel.child(note(k, "Too large to preview · 24 KB limit"))
        }
        Some(Preview::Binary) => panel = panel.child(note(k, "No text preview")),
        None => panel = panel.child(note(k, "No preview")),
    }
    panel
}

fn note(k: &Kit, body: impl Into<String>) -> El {
    text(body)
        .font(k.ty.caption())
        .color(k.c.text_faint)
        .py(4.0)
        .wrap()
}
