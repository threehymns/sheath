mod dialogs;
mod grid;
mod inspector;
mod palette;
mod path_bar;
mod rule_editor;
mod rule_picker;
mod rules;
mod scroll_area;
mod settings;
mod sidebar;
mod table;

use std::path::Component;

use rune::{Cx, El, Icon, col, el, icon, row, text};
use rune_kit::{ControlSize, IconButton, IconButtonVariant, Kit, SplitView, Toolbar};
use rune_theme_opencode as theme;

use crate::app::App;
use crate::icons;
use crate::keybinds::Action;

use dialogs::{place_menu, row_menu, sheet_layer};
use palette::palette_layer;
use settings::settings_layer;
use table::{empty_state, file_table};

pub fn view(s: &App, cx: &mut Cx<App>) -> El {
    let k = theme::kit(cx.system_dark());
    let shell = SplitView::new(main(s, k))
        .sidebar(
            &s.sidebar,
            |s: &mut App| &mut s.sidebar,
            sidebar::sidebar(s, k),
        )
        .inspector(
            &s.inspector,
            |s: &mut App| &mut s.inspector,
            inspector::inspector(s, k),
        )
        .toolbar(toolbar(s, k))
        .titlebar_inset(cx.titlebar_inset())
        .sidebar_header(text("Sheath").font(k.ty.ui_medium).color(k.c.text_muted))
        .build(k);
    let mut layers = Vec::new();
    if let Some(box_rect) = s.marquee_rect() {
        let view = s.rows_view;
        layers.push(
            el().key("marquee-clip")
                .absolute(view[0], view[1])
                .w(view[2])
                .h(view[3])
                .clip_rounded(0.0)
                .child(
                    el().key("marquee")
                        .absolute(box_rect[0] - view[0], box_rect[1] - view[1])
                        .w(box_rect[2])
                        .h(box_rect[3])
                        .radius(3.0)
                        .fill(k.alpha(k.c.text_accent, 0.12))
                        .border(1.0, k.alpha(k.c.text_accent, 0.55)),
                ),
        );
    }
    if let Some(line) = s.place_insert_line() {
        layers.push(
            el().key("place-insert")
                .absolute(line[0], line[1])
                .w(line[2])
                .h(line[3])
                .radius(1.0)
                .fill(k.c.text_accent),
        );
    }
    if let Some(preview) = s.drag_preview() {
        layers.push(
            el().key("drag-badge")
                .absolute(preview.at.0 + 14.0, preview.at.1 + 14.0)
                .radius(6.0)
                .px(10.0)
                .py(6.0)
                .gap(6.0)
                .center_y()
                .fill(k.c.layer_02)
                .border(1.0, k.c.border_muted)
                .child(icon(preview.icon, 14.0).color(k.c.icon_muted))
                .child(
                    text(preview.label)
                        .font(k.ty.caption())
                        .color(k.c.text_base)
                        .max_lines(1),
                ),
        );
    }
    if s.palette.is_some() {
        palette_layer(s, k, &mut layers);
    }
    if s.rule_picker.is_some() {
        rule_picker::rule_picker_layer(s, k, &mut layers);
    }
    if let Some(at) = s.menu {
        row_menu(s, k, at, &mut layers);
    }
    if s.place_menu.is_some() {
        place_menu(s, k, &mut layers);
    }
    if let Some(sheet) = &s.sheet {
        sheet_layer(s, sheet, k, &mut layers);
    }
    if s.settings.is_some() {
        settings_layer(s, k, &mut layers);
    }
    if !s.notices.is_empty() {
        let mut region = rune_kit::ToastRegion::new();
        for (index, notice) in s.notices.iter().enumerate() {
            let mut toast = rune_kit::Toast::new(notice.key(), notice.title.clone())
                .description(notice.detail.clone());
            toast = toast.action(
                k,
                if notice.copied {
                    "Copied"
                } else {
                    "Copy details"
                },
                !notice.copied,
                move |s: &mut App, tx: &mut rune::Tx| s.copy_notice(index, tx),
            );
            toast = toast.on_close(move |s: &mut App| s.dismiss_notice(index));
            region = region.toast(toast.build(k));
        }
        layers.push(region.build(k));
    }
    rune_kit::root(shell, layers)
}

fn toolbar(s: &App, k: &'static Kit) -> Toolbar {
    let back = nav_button(
        &icons::CHEVRON_LEFT,
        "Back",
        s.keybinds.accelerator(Action::Back),
        s.can_back(),
        |s: &mut App| s.back(),
        k,
    )
    .key("nav-back");
    let forward = nav_button(
        &icons::CHEVRON_RIGHT,
        "Forward",
        s.keybinds.accelerator(Action::Forward),
        s.can_forward(),
        |s: &mut App| s.forward(),
        k,
    )
    .key("nav-forward");
    let up = nav_button(
        &icons::ARROW_UP,
        "Enclosing Folder",
        s.keybinds.accelerator(Action::EnclosingFolder),
        s.can_up(),
        |s: &mut App| s.up(),
        k,
    )
    .key("nav-up");
    let mut search = rune_kit::TextInput::new(&s.search)
        .placeholder("Search this folder")
        .leading_icon(k.glyph.search)
        .width(Some(248.0))
        .on_edit(|s: &mut App| &mut s.search)
        .on_change(|s: &mut App, _| s.apply_filter());
    if s.search.text.is_empty() {
        let caps = s.keybinds.keycaps(Action::FocusSearch);
        if !caps.is_empty() {
            let surface = k.c.bg_base.mix(k.c.white, 0.04);
            search = search.trailing(crate::keycaps::tray(
                k,
                "search-shortcut",
                &caps,
                surface,
                crate::keycaps::KeySize::Small,
            ));
        }
    } else {
        search = search.clear_button().on_action(|s: &mut App, _| {
            s.search.clear();
            s.apply_filter();
        });
    }
    let search = search.build(k).key("search");

    let new_folder = IconButton::new(&icons::FOLDER_PLUS)
        .variant(IconButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .label("New Folder")
        .help("New Folder")
        .shortcut(s.keybinds.accelerator(Action::NewFolder))
        .on_press_tx(|s: &mut App, tx| s.new_folder(tx))
        .build(k)
        .key("new-folder");

    let (hidden_icon, hidden_label) = if s.show_hidden {
        (k.glyph.eye, "Hide Hidden Files")
    } else {
        (k.glyph.eye_off, "Show Hidden Files")
    };
    let hidden = IconButton::new(hidden_icon)
        .variant(IconButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .label(hidden_label)
        .help(hidden_label)
        .shortcut(s.keybinds.accelerator(Action::ToggleHidden))
        .expanded(s.show_hidden)
        .on_press(|s: &mut App| s.toggle_hidden())
        .build(k)
        .key("toggle-hidden");

    let (layout_icon, layout_label) = if s.is_grid() {
        (&icons::LIST, "List Layout")
    } else {
        (&icons::GRID, "Grid Layout")
    };
    let layout = IconButton::new(layout_icon)
        .variant(IconButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .label(layout_label)
        .help(layout_label)
        .expanded(s.is_grid())
        .on_press(|s: &mut App| s.toggle_layout())
        .build(k)
        .key("toggle-layout");

    let inspector = IconButton::new(&icons::INSPECTOR)
        .variant(IconButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .label("Toggle Inspector")
        .help("Toggle Inspector")
        .expanded(s.inspector.is_open())
        .on_press(|s: &mut App| s.toggle_inspector())
        .build(k)
        .key("toggle-inspector");

    let settings = IconButton::new(&theme::icons::SETTINGS_GEAR)
        .variant(IconButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .label("Settings")
        .help("Settings")
        .shortcut(s.keybinds.accelerator(Action::OpenSettings))
        .on_press(|s: &mut App| s.open_settings())
        .build(k)
        .key("open-settings");

    Toolbar::new()
        .leading(back)
        .leading(forward)
        .leading(up)
        .center(search)
        .trailing(new_folder)
        .trailing(hidden)
        .trailing(layout)
        .trailing(inspector)
        .trailing(settings)
}

fn nav_button(
    glyph: &'static Icon,
    label: &'static str,
    hint: String,
    enabled: bool,
    action: impl Fn(&mut App) + 'static,
    k: &Kit,
) -> El {
    IconButton::new(glyph)
        .variant(IconButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .label(label)
        .help(label)
        .shortcut(hint)
        .disabled(!enabled)
        .on_press(action)
        .build(k)
}

fn main(s: &App, k: &'static Kit) -> El {
    let mut content = col()
        .key("browser")
        .w_full()
        .h_full()
        .min_h(0.0)
        .child(path_bar::path_bar(s, k));
    if let Some(error) = &s.error {
        content = content.child(
            row()
                .key("error")
                .w_full()
                .px(12.0)
                .py(8.0)
                .gap(8.0)
                .center_y()
                .fill(k.alpha(k.c.danger.bg, 0.5))
                .child(icon(&theme::icons::WARNING, 14.0).color(k.c.danger.fg))
                .child(
                    text(error.clone())
                        .font(k.ty.caption())
                        .color(k.c.danger.fg)
                        .grow(1.0)
                        .shrink()
                        .truncate()
                        .help(error.clone()),
                ),
        );
    }
    if s.visible.is_empty() && !s.is_scanning() {
        content.child(empty_state(s, k).grow(1.0).min_h(0.0))
    } else if s.is_grid() {
        content.child(grid::file_grid(s, k, s.layout_cut).grow(1.0).min_h(0.0))
    } else {
        content.child(file_table(s, k, s.layout_cut).grow(1.0).min_h(0.0))
    }
}
