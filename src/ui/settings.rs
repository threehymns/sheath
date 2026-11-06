use crate::app::{App, SettingsTab};
use crate::config;
use crate::icons;
use crate::keybinds::Action;
use crate::settings::Setting;
use chrome::{Corner, CornerControl};
use rune::{El, Icon, Tx, col, icon, row, text};
use rune_kit::{
    Button, ButtonVariant, ControlSize, IconButton, IconButtonVariant, Kit, Switch, glyphs, scrim,
};

use rune_theme_opencode as theme;
use std::path::PathBuf;

use super::dialogs::error_text;

mod metrics;

pub(super) use actions_ui::settings::{settings_row, settings_section, settings_section_action};
pub(super) use metrics::*;

pub(super) fn settings_layer(s: &App, k: &Kit, layers: &mut Vec<El>) {
    let Some(tab) = s.settings else {
        return;
    };
    let mut nav = col().key("settings-nav").w_full().gap(4.0);
    for category in SettingsTab::ALL {
        let selected = category == tab;
        let mut item = row()
            .key(format!("settings-tab-{}", category.id()))
            .role("button")
            .label(category.label())
            .w_full()
            .h(28.0)
            .px(6.0)
            .gap(8.0)
            .center_y()
            .radius(k.radius.sm)
            .child(icon(tab_icon(category), 16.0).color(if selected {
                k.c.icon_base
            } else {
                k.c.icon_muted
            }))
            .child(
                text(category.label())
                    .font(k.ty.ui)
                    .color(if selected {
                        k.c.text_base
                    } else {
                        k.c.text_muted
                    })
                    .grow(1.0)
                    .truncate(),
            )
            .hover(k.c.hover)
            .on_press(move |s: &mut App| s.open_tab(category));
        if selected {
            item = item.fill(k.c.layer_02);
        }
        nav = nav.child(item);
    }
    let version = row().h(20.0).px(6.0).center_y().child(
        text(format!("Version {}", env!("CARGO_PKG_VERSION")))
            .key("settings-version")
            .font(k.ty.small)
            .color(k.c.text_faint),
    );
    let sidebar = col()
        .key("settings-sidebar")
        .w(SETTINGS_RAIL)
        .h_full()
        .pt(16.0)
        .pb(16.0)
        .pl(10.0)
        .pr(6.0)
        .child(nav.grow(1.0))
        .child(version);
    let body = match tab {
        SettingsTab::General => general_settings(s, k),
        SettingsTab::Rules => super::rules::rules_settings(s, k),
        SettingsTab::Keybinds => keybinds_settings(s, k),
    };
    let panel_radius = SETTINGS_RADIUS - SETTINGS_INSET;
    let panel = col()
        .key("settings-panel")
        .grow(1.0)
        .shrink()
        .min_h(0.0)
        .mt(SETTINGS_INSET)
        .mr(SETTINGS_INSET)
        .mb(SETTINGS_INSET)
        .radius(panel_radius)
        .fill(k.c.bg_base)
        .border(0.5, k.c.border_weak)
        .padding(0.5)
        .clip_rounded(panel_radius)
        .child(settings_header(k, tab))
        .child(scroll::area(
            s,
            "settings-scroll",
            col().w_full().grow(1.0).min_h(0.0).scroll_edge_fade(16.0),
            col()
                .key("settings-content")
                .w_full()
                .px(SETTINGS_GUTTER - SETTINGS_ROW_INSET)
                .pt(4.0)
                .pb(24.0)
                .gap(24.0)
                .child(body),
        ));
    let done = CornerControl {
        panel: [0.0, 0.0, SETTINGS_DIALOG_WIDTH, 0.0],
        corner: Corner::TopRight,
        radius: SETTINGS_RADIUS,
        inset: SETTINGS_INSET,
        size: DONE_SIZE,
        gap: DONE_GAP,
    };
    let dialog = k.elevate(
        row()
            .key("settings-dialog")
            .w(SETTINGS_DIALOG_WIDTH)
            .h_full()
            .max_h(600.0)
            .radius(SETTINGS_RADIUS)
            .fill(k.c.bg_deep)
            .border(1.0, k.c.border_muted)
            .clip_rounded(SETTINGS_RADIUS)
            .modal(true, |s: &mut App| s.dismiss_settings())
            .on_press(|_: &mut App| {})
            .child(sidebar)
            .child(panel)
            .child(done.backdrop(k.c.bg_deep, k.c.border_weak))
            .child(
                done.place(
                    Button::new("Done")
                        .variant(ButtonVariant::Neutral)
                        .control_size(ControlSize::Regular)
                        .on_press(|s: &mut App| s.close_settings())
                        .build(k),
                )
                .key("close-settings")
                .default_button(),
            ),
        &k.elevation.overlay,
    );
    layers.push(
        scrim("settings-scrim", |s: &mut App| s.close_settings()).fill(k.alpha(k.c.black, 0.32)),
    );
    layers.push(
        col()
            .key("settings")
            .absolute(0.0, 0.0)
            .w_full()
            .h_full()
            .padding(32.0)
            .justify_center()
            .items_center()
            .child(dialog),
    );
    if tab == SettingsTab::Rules {
        super::rule_editor::rule_editor_layers(s, k, layers);
    }
}

fn settings_header(k: &Kit, tab: SettingsTab) -> El {
    row()
        .key("settings-header")
        .w_full()
        .h(52.0)
        .px(SETTINGS_GUTTER)
        .gap(8.0)
        .center_y()
        .child(
            text(tab.label())
                .key("settings-title")
                .font(k.ty.body_medium)
                .color(k.c.text_base)
                .grow(1.0)
                .truncate(),
        )
}

fn tab_icon(tab: SettingsTab) -> &'static Icon {
    match tab {
        SettingsTab::General => &theme::icons::SETTINGS_GEAR,
        SettingsTab::Rules => &theme::icons::EDIT,
        SettingsTab::Keybinds => &glyphs::KEYBOARD,
    }
}

// These switches edit the fallbacks, not what the open folder resolved to, so they read
// `config`. The order of `Setting::ALL` is the order inside each section.
fn general_settings(s: &App, k: &Kit) -> El {
    let mut page = col().key("settings-general").w_full().gap(24.0);
    for group in SettingsGroup::ALL {
        let rows: Vec<El> = Setting::ALL
            .iter()
            .copied()
            .filter(|setting| SettingsGroup::of(*setting) == group && available(*setting))
            .map(|setting| setting_row_for(s, k, setting))
            .collect();
        if !rows.is_empty() {
            page = page.child(settings_section(k, group.key(), group.label(), rows));
        }
    }
    page = page.child(files_settings(s, k));
    if let Some(error) = s.store.error(config::CONFIG) {
        page = page.child(error_text(k, error));
    }
    if let Some(error) = &s.store.write_error {
        page = page.child(error_text(k, error));
    }
    page
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SettingsGroup {
    Browser,
    Behavior,
    Columns,
    Scrolling,
}

impl SettingsGroup {
    const ALL: [SettingsGroup; 4] = [
        SettingsGroup::Browser,
        SettingsGroup::Behavior,
        SettingsGroup::Columns,
        SettingsGroup::Scrolling,
    ];

    fn of(setting: Setting) -> SettingsGroup {
        match setting {
            Setting::ShowHidden
            | Setting::Inspector
            | Setting::GridLayout
            | Setting::ShowThumbnails
            | Setting::SystemIcons => SettingsGroup::Browser,
            Setting::FoldersFirst | Setting::ConfirmTrash | Setting::ConfirmDelete => {
                SettingsGroup::Behavior
            }
            Setting::ShowKind | Setting::ShowSize | Setting::ShowModified => SettingsGroup::Columns,
            Setting::ScrollInertia | Setting::Overscroll => SettingsGroup::Scrolling,
        }
    }

    fn label(self) -> &'static str {
        match self {
            SettingsGroup::Browser => "Browser",
            SettingsGroup::Behavior => "Behavior",
            SettingsGroup::Columns => "Columns",
            SettingsGroup::Scrolling => "Scrolling",
        }
    }

    fn key(self) -> &'static str {
        match self {
            SettingsGroup::Browser => "settings-browser",
            SettingsGroup::Behavior => "settings-behavior",
            SettingsGroup::Columns => "settings-columns",
            SettingsGroup::Scrolling => "settings-scrolling",
        }
    }
}

fn setting_row_for(s: &App, k: &Kit, setting: Setting) -> El {
    let switch = Switch::new(setting.value(&s.config))
        .label(setting_label(setting))
        .hide_label()
        .on_toggle(move |s: &mut App| s.toggle_setting(setting))
        .build(k)
        .key(format!("setting-{}", setting.id()));
    settings_row(
        k,
        setting_icon(setting, k),
        setting_label(setting),
        setting_help(setting),
        setting_control(s, k, setting, switch),
    )
}

fn setting_label(setting: Setting) -> &'static str {
    match setting {
        Setting::ShowHidden => "Show hidden files",
        Setting::Inspector => "Show inspector",
        Setting::GridLayout => "Grid layout",
        Setting::ShowThumbnails => "Show thumbnails",
        Setting::SystemIcons => "System icons",
        Setting::FoldersFirst => "Folders first",
        Setting::ConfirmTrash => "Confirm before trashing",
        Setting::ConfirmDelete => "Confirm before deleting",
        Setting::ShowKind => "Kind column",
        Setting::ShowSize => "Size column",
        Setting::ShowModified => "Date Modified column",
        Setting::ScrollInertia => "Scroll inertia",
        Setting::Overscroll => "Overscroll",
    }
}

fn setting_help(setting: Setting) -> Option<&'static str> {
    match setting {
        Setting::ShowHidden => Some("Also list names that start with a dot."),
        Setting::Inspector => Some("The details and preview pane beside the table."),
        Setting::GridLayout => Some("Show the folder as tiles instead of a table of rows."),
        Setting::ShowThumbnails => {
            Some("Show a small picture for each image, in the list and the grid.")
        }
        Setting::SystemIcons => Some("Use your desktop’s icons for folders and files."),
        Setting::FoldersFirst => Some("Keep folders above files in the list and the grid."),
        Setting::ConfirmTrash => Some("Ask before moving items to the Trash."),
        Setting::ConfirmDelete => {
            Some("Ask before deleting items for good, since the Trash cannot bring them back.")
        }
        Setting::ShowKind => Some("Show what each row is beside its name."),
        Setting::ShowSize => Some("Show each file's size in the list."),
        Setting::ShowModified => Some("Show when each row last changed."),
        Setting::ScrollInertia => Some("Keep the list coasting after the wheel stops."),
        Setting::Overscroll => {
            Some("Pull the list past its first and last row, then let it spring back.")
        }
    }
}

fn setting_icon(setting: Setting, k: &Kit) -> &'static Icon {
    match setting {
        Setting::ShowHidden => k.glyph.eye,
        Setting::Inspector => &icons::INSPECTOR,
        Setting::GridLayout => &icons::GRID,
        Setting::ShowThumbnails => &icons::PICTURES,
        Setting::SystemIcons => &glyphs::FOLDER,
        Setting::FoldersFirst => &icons::LIST,
        Setting::ConfirmTrash => &theme::icons::TRASH,
        Setting::ConfirmDelete => &theme::icons::WARNING,
        Setting::ShowKind => &icons::LIST,
        Setting::ShowSize => &icons::INSPECTOR,
        Setting::ShowModified => &icons::REFRESH,
        Setting::ScrollInertia => &icons::ARROW_UP,
        Setting::Overscroll => &icons::REFRESH,
    }
}

/// A browser tab has no desktop icon theme, so that switch stays out of the web build.
fn available(setting: Setting) -> bool {
    !(cfg!(target_arch = "wasm32") && setting == Setting::SystemIcons)
}

fn files_settings(s: &App, k: &Kit) -> El {
    settings_section(
        k,
        "settings-files",
        "Files",
        vec![
            file_path_row(
                k,
                &glyphs::KEYBOARD,
                "Keybinds",
                s.store.path(config::KEYBINDS),
                "copy-keybinds-path",
            ),
            file_path_row(
                k,
                &theme::icons::SETTINGS_GEAR,
                "Settings",
                s.store.path(config::CONFIG),
                "copy-settings-path",
            ),
        ],
    )
}

fn keybinds_settings(s: &App, k: &Kit) -> El {
    actions_ui::keybind_page(s, k, Action::icon, &icons::UNDO)
}

fn setting_control(s: &App, k: &Kit, setting: Setting, control: El) -> El {
    let mut row = row().gap(6.0).center_y();
    if s.config.is_set(setting) {
        row = row.child(
            IconButton::new(&icons::UNDO)
                .variant(IconButtonVariant::GhostMuted)
                .control_size(ControlSize::Small)
                .label(format!("Reset {}", setting.name()))
                .help("Reset to the default, taking the key out of config.kdl")
                .on_press(move |s: &mut App| s.reset_setting(setting))
                .build(k)
                .key(format!("reset-{}", setting.id())),
        );
    }
    row.child(control)
}

fn file_path_row(
    k: &Kit,
    glyph: &'static Icon,
    label: &str,
    path: Option<PathBuf>,
    key: &str,
) -> El {
    let shown = path
        .as_ref()
        .map(|path| path.display().to_string())
        .unwrap_or_else(|| "Unavailable".into());
    let Some(_) = path else {
        return settings_row(k, glyph, label, Some(&shown), text("").font(k.ty.caption()));
    };
    settings_row(
        k,
        glyph,
        label,
        Some(&format!("Saved in {shown}")),
        Button::new("Copy Path")
            .variant(ButtonVariant::Outline)
            .control_size(ControlSize::Small)
            .on_press_tx(move |_: &mut App, tx: &mut Tx| tx.copy(shown.clone()))
            .build(k)
            .key(key),
    )
}
