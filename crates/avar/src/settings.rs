//! Avar's settings window: the dialog shell and its rail, drawn the way the app draws them. The
//! page bodies stay shared, so the shortcuts editor is `actions_ui::keybind_page` inside this
//! frame rather than a dialog of its own.

use actions_ui::keybind_page;
use actions_ui::settings::{settings_row, settings_section};
use chrome::{Corner, CornerControl};
use rune::{El, Icon, col, icon, row, text};
use rune_kit::{Button, ButtonVariant, ControlSize, Kit, Switch, glyphs, scrim};

use crate::Window;
use crate::action::Action;
use crate::icons;

const DIALOG_WIDTH: f32 = 880.0;
const RAIL: f32 = 220.0;
const GUTTER: f32 = 32.0;
const RADIUS: f32 = 16.0;
// How far the panel sits in from the dialog; the radius loses this much to stay concentric.
const INSET: f32 = 8.0;
const DONE_SIZE: (f32, f32) = (76.0, 26.0);
const DONE_GAP: f32 = 8.0;

/// The settings sections, in the order the rail lists them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsTab {
    Appearance,
    Shortcuts,
}

impl SettingsTab {
    pub const ALL: [SettingsTab; 2] = [SettingsTab::Appearance, SettingsTab::Shortcuts];

    pub fn id(self) -> &'static str {
        match self {
            SettingsTab::Appearance => "appearance",
            SettingsTab::Shortcuts => "shortcuts",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SettingsTab::Appearance => "Appearance",
            SettingsTab::Shortcuts => "Shortcuts",
        }
    }

    fn icon(self) -> &'static Icon {
        match self {
            SettingsTab::Appearance => &rune_kit::shell::SIDEBAR,
            SettingsTab::Shortcuts => &glyphs::KEYBOARD,
        }
    }
}

/// The scrim and the dialog, in the order they are pushed over the terminal.
pub fn layer(s: &Window, tab: SettingsTab, k: &Kit, layers: &mut Vec<El>) {
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
            .child(icon(category.icon(), 16.0).color(if selected {
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
            .on_press(move |s: &mut Window| s.open_tab(category));
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
        .w(RAIL)
        .h_full()
        .pt(16.0)
        .pb(16.0)
        .pl(10.0)
        .pr(6.0)
        .child(nav.grow(1.0))
        .child(version);
    let body = match tab {
        SettingsTab::Appearance => appearance_page(s, k),
        SettingsTab::Shortcuts => keybind_page(s, k, Action::icon, &icons::UNDO),
    };
    let panel_radius = RADIUS - INSET;
    let panel = col()
        .key("settings-panel")
        .grow(1.0)
        .shrink()
        .min_h(0.0)
        .mt(INSET)
        .mr(INSET)
        .mb(INSET)
        .radius(panel_radius)
        .fill(k.c.bg_base)
        .border(0.5, k.c.border_weak)
        .padding(0.5)
        .clip_rounded(panel_radius)
        .child(header(k, tab))
        .child(scroll::area(
            s,
            "settings-scroll",
            col().w_full().grow(1.0).min_h(0.0).scroll_edge_fade(16.0),
            col()
                .key("settings-content")
                .w_full()
                .px(GUTTER - actions_ui::settings::ROW_INSET)
                .pt(4.0)
                .pb(24.0)
                .gap(24.0)
                .child(body),
        ));
    // The Done button sits in a bite taken out of the dialog's top-right corner.
    let done = CornerControl {
        panel: [0.0, 0.0, DIALOG_WIDTH, 0.0],
        corner: Corner::TopRight,
        radius: RADIUS,
        inset: INSET,
        size: DONE_SIZE,
        gap: DONE_GAP,
    };
    let dialog = k.elevate(
        row()
            .key("settings-dialog")
            .w(DIALOG_WIDTH)
            .h_full()
            .max_h(600.0)
            .radius(RADIUS)
            .fill(k.c.bg_deep)
            .border(1.0, k.c.border_muted)
            .clip_rounded(RADIUS)
            .modal(true, |s: &mut Window| s.close_settings())
            .on_press(|_: &mut Window| {})
            .child(sidebar)
            .child(panel)
            .child(done.backdrop(k.c.bg_deep, k.c.border_weak))
            .child(
                done.place(
                    Button::new("Done")
                        .variant(ButtonVariant::Neutral)
                        .control_size(ControlSize::Regular)
                        .on_press(|s: &mut Window| s.close_settings())
                        .build(k),
                )
                .key("close-settings")
                .default_button(),
            ),
        &k.elevation.overlay,
    );
    layers.push(
        scrim("settings-scrim", |s: &mut Window| s.close_settings()).fill(k.alpha(k.c.black, 0.32)),
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

/// The appearance page: the layout the window draws its tabs and panes in.
fn appearance_page(s: &Window, k: &Kit) -> El {
    let horizontal = Switch::new(s.horizontal_tabs())
        .hide_label()
        .on_toggle(|s: &mut Window| s.set_horizontal_tabs(!s.horizontal_tabs()))
        .build(k)
        .key("horizontal-tabs");
    let headers = Switch::new(s.pane_headers())
        .hide_label()
        .on_toggle(|s: &mut Window| s.set_pane_headers(!s.pane_headers()))
        .build(k)
        .key("pane-headers");
    let top_bar = Switch::new(s.top_bar())
        .hide_label()
        .on_toggle(|s: &mut Window| s.set_top_bar(!s.top_bar()))
        .build(k)
        .key("top-bar");
    col()
        .w_full()
        .gap(24.0)
        .child(settings_section(
            k,
            "appearance-tabs",
            "Tabs",
            vec![settings_row(
                k,
                &rune_kit::shell::SIDEBAR,
                "Horizontal workspace tabs",
                Some(
                    "Draw the tabs as a bar across the top of the window instead of a rail down \
                     its side.",
                ),
                horizontal,
            )],
        ))
        .child(settings_section(
            k,
            "appearance-panes",
            "Panes",
            vec![settings_row(
                k,
                &icons::SPLIT_RIGHT,
                "Pane headers",
                Some(
                    "Show each pane's folder, and its split, zoom and close buttons, above its \
                     terminal.",
                ),
                headers,
            )],
        ))
        .child(settings_section(
            k,
            "appearance-window",
            "Window",
            vec![settings_row(
                k,
                &rune_kit::glyphs::DASH,
                "Top bar",
                Some(
                    "Show a slim bar with the tab's name above the panes. It stays hidden \
                     where the window already has a titlebar: the compositor's or Rune's own \
                     on Wayland, the window manager's on X11, the system's on Windows.",
                ),
                top_bar,
            )],
        ))
}

fn header(k: &Kit, tab: SettingsTab) -> El {
    row()
        .key("settings-header")
        .w_full()
        .h(52.0)
        .px(GUTTER)
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
