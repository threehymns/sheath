//! The command bar: the actions with their shortcuts, filtered by what is typed.

use crate::app::App;
use crate::icons;
use crate::keybinds::Action;
use rune::{Cursor, El, Icon, Key, ScrollSpec, Tx, col, el, icon, row, text};
use rune_kit::{Kit, TextInput, glyphs, scrim};
use rune_theme_opencode as theme;

/// The command bar: a field over the registry's actions, each with its shortcut.
pub(super) fn palette_layer(s: &App, k: &Kit, layers: &mut Vec<El>) {
    let Some(palette) = &s.palette else {
        return;
    };
    let panel = if k.dark { k.c.bg_base } else { k.c.layer_02 };
    let actions = s.palette_actions();
    let field = TextInput::new(&palette.query)
        .placeholder("Type a command")
        .leading_icon(k.glyph.search)
        .width(None)
        .on_edit(|s: &mut App| s.palette_edit())
        .on_change(|s: &mut App, _| s.palette_requery())
        .on_submit(|s: &mut App, tx| s.run_palette_command(tx))
        .on_key(|s: &mut App, event, tx| {
            // The field owns the keyboard here, so the bar's keys are the ones it leaves over.
            if event.key == Key::Escape {
                s.close_palette();
                return true;
            }
            if s.keybinds.matches(Action::CommandBar, event) {
                s.close_palette();
                return true;
            }
            if event.mods.cmd || event.mods.ctrl || event.mods.alt {
                return false;
            }
            match event.key {
                Key::Down => s.palette_move(1),
                Key::Up => s.palette_move(-1),
                Key::Enter if !tx.key_repeat() => s.run_palette_command(tx),
                _ => return false,
            }
            true
        })
        .build(k)
        .key("command-field");
    // The padding lives on the list, not on each row: a row that is `w_full` and also has
    // margins would be wider than the bar and clip at its right edge.
    let mut list = col().key("command-list").w_full().gap(1.0).px(6.0).py(6.0);
    if actions.is_empty() {
        list = list.child(
            row().key("command-none").px(12.0).py(14.0).child(
                text("No commands match")
                    .font(k.ty.ui)
                    .color(k.c.text_faint),
            ),
        );
    }
    for (i, action) in actions.iter().copied().enumerate() {
        let enabled = s.action_enabled(action);
        let selected = i == palette.selected;
        let mut row = row()
            .key(format!("command-{}", action.id()))
            .w_full()
            .h(32.0)
            .px(8.0)
            .gap(10.0)
            .center_y()
            .radius(k.radius.sm)
            .child(icon(action_icon(action, k), 14.0).color(if enabled {
                k.c.icon_muted
            } else {
                k.c.icon_faint
            }))
            .child(
                text(action.label())
                    .font(k.ty.ui)
                    .color(if enabled {
                        k.c.text_base
                    } else {
                        k.c.text_faint
                    })
                    .grow(1.0)
                    .truncate(),
            );
        let caps = s.keybinds.keycaps(action);
        if !caps.is_empty() {
            // The keys sit on their row, so a highlighted row's tint carries into the well.
            // The row paints its overlay over the panel; compositing it here gives the opaque
            // colour the tray mixes from.
            let hover = k.c.hover;
            let surface = if selected {
                hover.mix(panel, 1.0 - hover.a)
            } else {
                panel
            };
            row = row.child(crate::keycaps::tray(
                k,
                format!("keys-{}", action.id()),
                &caps,
                surface,
            ));
        }
        if selected {
            row = row.fill(k.c.hover);
        }
        if enabled {
            row = row
                .cursor(Cursor::Pointer)
                .on_press_tx(move |s: &mut App, tx: &mut Tx| s.choose_command(action, tx));
        }
        list = list.child(row);
    }
    let panel = col()
        .key("command-bar")
        .w(560.0)
        .max_h(440.0)
        .radius(10.0)
        // A step below the page, so the shortcuts' recessed trays read as wells rather than
        // sitting on the same brightness.
        .fill(panel)
        .border(1.0, k.c.border_muted)
        .clip_rounded(10.0)
        // A modal, so Escape closes it (rather than just blurring the field) and focus stays
        // inside while it's up.
        .modal(true, |s: &mut App| s.close_palette())
        .child(row().px(10.0).py(8.0).child(field))
        .child(
            el().key("command-rule")
                .h(1.0)
                .w_full()
                .fill(k.c.border_muted),
        )
        .child(
            col()
                .key("command-scroll")
                .w_full()
                .max_h(380.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .scroll_edge_fade(12.0)
                .child(list),
        );
    layers.push(
        scrim("command-scrim", |s: &mut App| s.close_palette()).fill(k.alpha(k.c.black, 0.28)),
    );
    layers.push(
        col()
            .key("command-slot")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .pt(72.0)
            .child(row().w_full().justify_center().child(panel)),
    );
}

pub(super) fn action_icon(action: Action, k: &Kit) -> &'static Icon {
    match action {
        Action::CommandBar => &glyphs::KEYBOARD,
        Action::OpenSettings => &theme::icons::SETTINGS_GEAR,
        Action::Back => &icons::CHEVRON_LEFT,
        Action::Forward => &icons::CHEVRON_RIGHT,
        Action::EnclosingFolder => &icons::ARROW_UP,
        Action::Refresh => &icons::REFRESH,
        Action::NewFolder => &icons::FOLDER_PLUS,
        Action::ToggleHidden => k.glyph.eye,
        Action::Rename => &theme::icons::EDIT,
        Action::Move => &glyphs::FOLDER,
        Action::Trash => &theme::icons::TRASH,
        Action::Copy => &theme::icons::COPY,
        Action::Cut => &icons::SCISSORS,
        Action::Paste => &icons::CLIPBOARD,
        Action::Duplicate => &theme::icons::OUTLINE_COPY,
        Action::ViewFolderRules | Action::ViewEntryRules | Action::SaveViewRule => {
            &theme::icons::EDIT
        }
        Action::ShowEntry | Action::HideEntry => k.glyph.eye,
    }
}
