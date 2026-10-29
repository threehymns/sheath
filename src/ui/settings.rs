//! The settings panel, laid out after Hex: a navigation rail, a raised page of cards, and
//! Done in the dialog's corner.

use crate::app::{App, SettingsTab};
use crate::corner::CornerControl;
use crate::icons;
use crate::keybinds::{Action, Keybinds};
use crate::settings::{Setting, Settings};
use rune::{El, Icon, ScrollSpec, Tx, col, el, icon, row, text};
use rune_kit::{
    Button, ButtonVariant, ControlSize, IconButton, IconButtonVariant, Kit, Switch, glyphs, scrim,
    shortcut_error_message,
};
use rune_theme_opencode as theme;
use std::path::PathBuf;

use super::dialogs::error_text;
use super::palette::action_icon;

/// Hex's settings metrics: a 220 pt navigation rail, a 32 pt page gutter, cards hanging 16 pt
/// past it so their rows' text lands on the gutter, and 48 pt rows.
const SETTINGS_RAIL: f32 = 220.0;

const SETTINGS_GUTTER: f32 = 32.0;

const SETTINGS_ROW_INSET: f32 = 16.0;

const SETTINGS_ROW_HEIGHT: f32 = 48.0;

/// The dialog's radius, rounder than the kit's own so the panel inside it keeps a visible curve.
const SETTINGS_RADIUS: f32 = 16.0;

/// How far the panel is set in from the dialog's edges, which is how much comes off the radius so
/// the two corners stay concentric.
const SETTINGS_INSET: f32 = 8.0;

/// Done, in the dialog's top-right corner: the panel is cut away around it, so it is on the
/// dialog's surface rather than in the pane.
const DONE_SIZE: (f32, f32) = (76.0, 26.0);

/// Panel left around Done's left and bottom edges.
const DONE_GAP: f32 = 8.0;

/// Settings after Hex's, as a modal: a dialog over the browser holding navigation on its
/// background and the chosen page in a raised panel beside it, with Done in the dialog's
/// top-right corner, outside the panel, so the way out is a button rather than a line in the
/// rail. The scrim, the dialog and Escape all close it.
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
    // Version sits at the rail's foot, as Hex's does.
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
        SettingsTab::Keybinds => keybinds_settings(s, k),
    };
    // Concentric with the dialog's corners: the same 8 pt inset taken off the radius.
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
        .child(
            col()
                .key("settings-scroll")
                .w_full()
                .grow(1.0)
                .min_h(0.0)
                .scroll(ScrollSpec {
                    stick_bottom: false,
                })
                .scroll_edge_fade(16.0)
                .child(
                    col()
                        .key("settings-content")
                        .w_full()
                        .px(SETTINGS_GUTTER - SETTINGS_ROW_INSET)
                        .pt(4.0)
                        .pb(24.0)
                        .gap(24.0)
                        .child(body),
                ),
        );
    let done = CornerControl {
        dialog_width: 880.0,
        dialog_radius: SETTINGS_RADIUS,
        inset: SETTINGS_INSET,
        size: DONE_SIZE,
        gap: DONE_GAP,
    };
    let dialog = k.elevate(
        row()
            .key("settings-dialog")
            .w(done.dialog_width)
            .h_full()
            .max_h(600.0)
            .radius(SETTINGS_RADIUS)
            .fill(k.c.bg_deep)
            .border(1.0, k.c.border_muted)
            .clip_rounded(SETTINGS_RADIUS)
            // A modal, so Escape closes it and focus stays inside while it is up.
            .modal(true, |s: &mut App| s.close_settings())
            .on_press(|_: &mut App| {})
            .child(sidebar)
            .child(panel)
            // The bite, then Done in it: the dialog's surface at its corner, not the panel's.
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
}

/// The pane's title (Hex's pane header). Done is in the dialog's corner, so nothing sits in the
/// header but the page's name.
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

/// A titled group of rows in a raised card, after Hex: the label sits on the page's text edge
/// and the card hangs [`SETTINGS_ROW_INSET`] past it, so the rows' text lines up with it.
fn settings_section(k: &Kit, key: &str, title: &str, rows: Vec<El>) -> El {
    let mut card = k.elevate(
        col()
            .key(format!("{key}-card"))
            .w_full()
            .radius(k.radius.lg)
            .fill(k.c.bg_base),
        &k.elevation.raised,
    );
    for (index, row) in rows.into_iter().enumerate() {
        if index > 0 {
            card = card.child(settings_rule(k));
        }
        card = card.child(row);
    }
    col()
        .key(key)
        .w_full()
        .gap(8.0)
        .child(
            text(title)
                .key("settings-section-label")
                .font(k.ty.ui_medium.lh(16.0))
                .color(k.c.text_muted)
                .mx(SETTINGS_ROW_INSET),
        )
        .child(card)
}

/// A hairline between two rows, inset from the card's edges.
fn settings_rule(k: &Kit) -> El {
    row()
        .w_full()
        .px(SETTINGS_ROW_INSET)
        .child(el().h(1.0).grow(1.0).fill(k.c.border_muted))
}

/// One setting: an icon, the label and its explanation, and the control at the row's end.
fn settings_row(
    k: &Kit,
    glyph: &'static Icon,
    label: &str,
    description: Option<&str>,
    control: El,
) -> El {
    let mut labels = col()
        .grow(1.0)
        .min_w(0.0)
        .gap(2.0)
        .child(text(label).font(k.ty.ui).color(k.c.text_base).truncate());
    if let Some(description) = description {
        labels = labels.child(
            text(description.to_owned())
                .font(k.ty.caption())
                .color(k.c.text_muted)
                .wrap(),
        );
    }
    row()
        .w_full()
        .min_h(SETTINGS_ROW_HEIGHT)
        .px(SETTINGS_ROW_INSET)
        .py(8.0)
        .gap(12.0)
        .center_y()
        .child(icon(glyph, 16.0).color(k.c.icon_muted))
        .child(labels)
        .child(control)
}

fn tab_icon(tab: SettingsTab) -> &'static Icon {
    match tab {
        SettingsTab::General => &theme::icons::SETTINGS_GEAR,
        SettingsTab::Keybinds => &glyphs::KEYBOARD,
    }
}

fn general_settings(s: &App, k: &Kit) -> El {
    let hidden = Switch::new(s.show_hidden)
        .label("Show hidden files")
        .hide_label()
        .on_toggle(|s: &mut App| s.toggle_hidden())
        .build(k)
        .key("setting-hidden");
    let inspector = Switch::new(s.inspector.is_open())
        .label("Show inspector")
        .hide_label()
        .on_toggle(|s: &mut App| s.toggle_inspector())
        .build(k)
        .key("setting-inspector");
    let inertia = Switch::new(s.config.scroll_inertia)
        .label("Scroll inertia")
        .hide_label()
        .on_toggle(|s: &mut App| s.toggle_scroll_inertia())
        .build(k)
        .key("setting-inertia");
    let overscroll = Switch::new(s.config.overscroll)
        .label("Overscroll")
        .hide_label()
        .on_toggle(|s: &mut App| s.toggle_overscroll())
        .build(k)
        .key("setting-overscroll");
    let browser = settings_section(
        k,
        "settings-browser",
        "Browser",
        vec![
            settings_row(
                k,
                k.glyph.eye,
                "Show hidden files",
                Some("Also list names that start with a dot."),
                setting_control(s, k, Setting::ShowHidden, hidden),
            ),
            settings_row(
                k,
                &icons::INSPECTOR,
                "Show inspector",
                Some("The details and preview pane beside the table."),
                setting_control(s, k, Setting::Inspector, inspector),
            ),
        ],
    );
    let scrolling = settings_section(
        k,
        "settings-scrolling",
        "Scrolling",
        vec![
            settings_row(
                k,
                &icons::ARROW_UP,
                "Scroll inertia",
                Some("Keep the list coasting after the wheel stops."),
                setting_control(s, k, Setting::ScrollInertia, inertia),
            ),
            settings_row(
                k,
                &icons::REFRESH,
                "Overscroll",
                Some("Pull the list past its first and last row, then let it spring back."),
                setting_control(s, k, Setting::Overscroll, overscroll),
            ),
        ],
    );
    let files = settings_section(
        k,
        "settings-files",
        "Files",
        vec![
            file_path_row(
                k,
                &glyphs::KEYBOARD,
                "Keybinds",
                Keybinds::file(),
                "copy-keybinds-path",
            ),
            file_path_row(
                k,
                &theme::icons::SETTINGS_GEAR,
                "Settings",
                Settings::file(),
                "copy-settings-path",
            ),
        ],
    );
    let mut page = col()
        .key("settings-general")
        .w_full()
        .gap(24.0)
        .child(browser)
        .child(scrolling)
        .child(files);
    if let Some(error) = &s.config.error {
        page = page.child(error_text(k, error));
    }
    page
}

fn keybinds_settings(s: &App, k: &Kit) -> El {
    let mut rows = Vec::new();
    for action in Action::ALL {
        if !s.recorders.contains_key(&action) {
            continue;
        }
        rows.push(settings_row(
            k,
            action_icon(action, k),
            action.label(),
            None,
            binding_control(s, k, action, recorder_field(s, k, action)),
        ));
    }
    let saved = match Keybinds::file() {
        Some(path) => format!(
            "Click a shortcut and press the keys to use instead. Backspace clears it, Escape \
             cancels while recording. Saved in {}.",
            path.display()
        ),
        None => "Click a shortcut and press the keys to use instead. Backspace clears it, \
                 Escape cancels while recording."
            .to_owned(),
    };
    let mut page = col()
        .key("settings-keybinds")
        .w_full()
        .gap(24.0)
        .child(settings_section(k, "settings-shortcuts", "Shortcuts", rows))
        .child(
            text(saved)
                .key("keybind-saved")
                .font(k.ty.caption())
                .color(k.c.text_muted)
                .mx(SETTINGS_ROW_INSET)
                .wrap(),
        );
    if let Some(error) = &s.keybinds.error {
        page = page.child(error_text(k, error));
    }
    page
}

/// One action's shortcut: click to record, Escape cancels while listening, Backspace clears.
/// The keys are drawn as the app's keycaps, so they match the command bar rather than Rune
/// Kit's flat chips.
fn recorder_field(s: &App, k: &Kit, action: Action) -> El {
    let recorder = s
        .recorders
        .get(&action)
        .expect("every action has a recorder");
    let labels = recorder.keycaps(s.keybinds.chord(action));
    let placeholder = if recorder.is_recording() {
        "Press shortcut"
    } else {
        "Record shortcut"
    };
    // The keys sit straight on the card, as Hex's do; only a recording takes a surface of its
    // own, so the well the caps sit in follows it.
    let surface = if recorder.is_recording() {
        k.c.layer_01
    } else {
        k.c.bg_base
    };
    let error = recorder
        .error()
        .map(|error| shortcut_error_message(error, recorder.refused()));
    let mut field = row()
        .key(format!("record-{}-field", action.id()))
        .role("button")
        .label(if labels.is_empty() {
            placeholder.to_owned()
        } else {
            labels.join(" ")
        })
        .min_w(96.0)
        .h(28.0)
        .px(6.0)
        .center_y()
        .justify_end()
        .radius(k.radius.sm)
        .focusable(true)
        .focus_ring(2.0, 2.0, k.c.border_focus)
        .on_press(move |s: &mut App| {
            s.recorders
                .get_mut(&action)
                .expect("every action has a recorder")
                .begin();
        })
        .on_physical_input(move |s: &mut App, event, _| s.record_key(action, event))
        .child(if labels.is_empty() {
            text(placeholder)
                .font(k.ty.ui_compact)
                .color(k.c.text_faint)
        } else {
            crate::keycaps::tray(k, format!("record-{}-keys", action.id()), &labels, surface)
        });
    if recorder.is_recording() {
        field = field.fill(surface);
    }
    let mut root = col()
        .key(format!("record-{}", action.id()))
        .gap(4.0)
        .items_end()
        .child(field);
    if let Some(error) = error {
        root = root.child(
            text(error)
                .font(k.ty.small_medium)
                .color(k.c.danger.fg)
                .wrap(),
        );
    }
    root
}

/// An action's shortcut, with an undo beside it while `keybinds.kdl` names the action.
/// Resetting takes its line out, so the built-in chord is what the file no longer says.
fn binding_control(s: &App, k: &Kit, action: Action, field: El) -> El {
    let mut row = row().gap(6.0).center_y();
    if s.keybinds.is_custom(action) {
        row = row.child(
            IconButton::new(&icons::UNDO)
                .variant(IconButtonVariant::GhostMuted)
                .control_size(ControlSize::Small)
                .label(format!("Reset {}", action.label()))
                .help("Reset to the default, taking the shortcut out of keybinds.kdl")
                .on_press(move |s: &mut App| s.reset_binding(action))
                .build(k)
                .key(format!("reset-binding-{}", action.id())),
        );
    }
    row.child(field)
}

/// A setting's control: its switch, with an undo beside it while the config file names the
/// setting. Resetting takes the key out, so the default is what the file no longer says.
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

/// A file the settings are saved in, with a button that copies its path.
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
