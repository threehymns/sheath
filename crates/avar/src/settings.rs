//! Avar's settings window: the dialog shell and its rail, drawn the way the app draws them. The
//! page bodies stay shared, so the shortcuts editor is `actions_ui::keybind_page` inside this
//! frame rather than a dialog of its own.

use actions_ui::keybind_page;
use actions_ui::settings::{settings_row, settings_section};
use chrome::{Corner, CornerControl};
use rune::{Effect, El, Icon, SpringPair, Transition, Tx, Vec2, col, icon, row, text};
use rune_kit::{
    Button, ButtonVariant, ControlSize, Kit, Select, SelectAppearance, Switch, glyphs, scrim,
};

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

/// How the tab on screen gives way to the next one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TabTransition {
    /// No motion: the next tab is simply there.
    #[default]
    Cut,
    /// The next tab fades in as the last one fades out.
    Fade,
    /// The next tab slides in from the leading edge while the last one leaves through the
    /// trailing one.
    SlideHorizontal,
    /// The same along the vertical axis: in from the top, out through the bottom.
    SlideVertical,
}

impl TabTransition {
    pub(crate) const ALL: [TabTransition; 4] = [
        TabTransition::Cut,
        TabTransition::Fade,
        TabTransition::SlideHorizontal,
        TabTransition::SlideVertical,
    ];

    /// The name `settings.kdl` writes.
    pub(crate) fn name(self) -> &'static str {
        match self {
            TabTransition::Cut => "cut",
            TabTransition::Fade => "fade",
            TabTransition::SlideHorizontal => "slide-horizontal",
            TabTransition::SlideVertical => "slide-vertical",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            TabTransition::Cut => "Cut",
            TabTransition::Fade => "Fade",
            TabTransition::SlideHorizontal => "Slide horizontal",
            TabTransition::SlideVertical => "Slide vertical",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<TabTransition> {
        TabTransition::ALL
            .into_iter()
            .find(|transition| transition.name() == name)
    }

    pub(crate) fn from_label(label: &str) -> Option<TabTransition> {
        TabTransition::ALL
            .into_iter()
            .find(|transition| transition.label() == label)
    }

    /// The presence effect alone; the window layers the chosen timing on top.
    pub(crate) fn effect(self) -> Transition {
        match self {
            TabTransition::Cut => Transition::none(),
            TabTransition::Fade => Transition::opacity(),
            TabTransition::SlideHorizontal => Transition::slide(),
            TabTransition::SlideVertical => {
                let offset = |y: f32| Effect {
                    offset_fraction: Vec2::new(0.0, y),
                    ..Effect::IDENTITY
                };
                Transition {
                    insertion: offset(-1.0),
                    removal: offset(1.0),
                    ..Transition::slide()
                }
            }
        }
    }
}

/// The spring a tab transition runs on, named after the motion tokens Rune already ships: from
/// barely there through the springy presets that overshoot.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum TabTiming {
    Instant,
    Snap,
    #[default]
    Settle,
    Reflow,
    Glide,
    Morph,
    Pop,
    Drift,
}

impl TabTiming {
    pub(crate) const ALL: [TabTiming; 8] = [
        TabTiming::Instant,
        TabTiming::Snap,
        TabTiming::Settle,
        TabTiming::Reflow,
        TabTiming::Glide,
        TabTiming::Morph,
        TabTiming::Pop,
        TabTiming::Drift,
    ];

    /// The name `settings.kdl` writes.
    pub(crate) fn name(self) -> &'static str {
        match self {
            TabTiming::Instant => "instant",
            TabTiming::Snap => "snap",
            TabTiming::Settle => "settle",
            TabTiming::Reflow => "reflow",
            TabTiming::Glide => "glide",
            TabTiming::Morph => "morph",
            TabTiming::Pop => "pop",
            TabTiming::Drift => "drift",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            TabTiming::Instant => "Instant",
            TabTiming::Snap => "Snap",
            TabTiming::Settle => "Settle",
            TabTiming::Reflow => "Reflow",
            TabTiming::Glide => "Glide",
            TabTiming::Morph => "Morph",
            TabTiming::Pop => "Pop",
            TabTiming::Drift => "Drift",
        }
    }

    pub(crate) fn from_name(name: &str) -> Option<TabTiming> {
        TabTiming::ALL
            .into_iter()
            .find(|timing| timing.name() == name)
    }

    pub(crate) fn from_label(label: &str) -> Option<TabTiming> {
        TabTiming::ALL
            .into_iter()
            .find(|timing| timing.label() == label)
    }

    pub(crate) fn springs(self) -> SpringPair {
        use rune::tok::motion::pair;
        match self {
            TabTiming::Instant => pair::INSTANT,
            TabTiming::Snap => pair::SNAP,
            TabTiming::Settle => pair::SETTLE,
            TabTiming::Reflow => pair::REFLOW,
            TabTiming::Glide => pair::GLIDE,
            TabTiming::Morph => pair::MORPH,
            TabTiming::Pop => pair::POP,
            TabTiming::Drift => pair::DRIFT,
        }
    }
}

/// Which dropdown on the appearance page is open. The kit's select is a plain trigger, so the
/// window keeps the one listbox it has open.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Pick {
    Transition,
    Timing,
}

/// The scrim and the dialog, in the order they are pushed over the terminal.
pub fn layer(s: &Window, tab: SettingsTab, k: &Kit, layers: &mut Vec<El>) {
    // The page's dropdowns open above the dialog, so their listboxes are collected here and
    // pushed after it rather than in page order.
    let mut menus = Vec::new();
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
        SettingsTab::Appearance => appearance_page(s, k, &mut menus),
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
    layers.extend(menus);
}

/// The appearance page: the layout the window draws its tabs and panes in, and the motion a tab
/// swap runs on.
fn appearance_page(s: &Window, k: &Kit, menus: &mut Vec<El>) -> El {
    let horizontal = Switch::new(s.horizontal_tabs())
        .hide_label()
        .on_toggle(|s: &mut Window| s.set_horizontal_tabs(!s.horizontal_tabs()))
        .build(k)
        .key("horizontal-tabs");
    let transitions = TabTransition::ALL.map(TabTransition::label);
    let transition = Select::new("tab-transition", &transitions)
        .appearance(SelectAppearance::Field)
        .width(176.0)
        .current(Some(s.tab_transition().label()))
        .open(s.pick() == Some(Pick::Transition))
        .on_toggle(|s: &mut Window| s.toggle_pick(Pick::Transition))
        .on_close(|s: &mut Window| s.clear_pick())
        .on_choose(|s: &mut Window, value: &str, _: &mut Tx| {
            if let Some(transition) = TabTransition::from_label(value) {
                s.set_tab_transition(transition);
            }
            s.clear_pick();
        })
        .build(k, menus);
    let timings = TabTiming::ALL.map(TabTiming::label);
    let timing = Select::new("tab-transition-timing", &timings)
        .appearance(SelectAppearance::Field)
        .width(176.0)
        .current(Some(s.tab_timing().label()))
        .open(s.pick() == Some(Pick::Timing))
        .disabled(s.tab_transition() == TabTransition::Cut)
        .on_toggle(|s: &mut Window| s.toggle_pick(Pick::Timing))
        .on_close(|s: &mut Window| s.clear_pick())
        .on_choose(|s: &mut Window, value: &str, _: &mut Tx| {
            if let Some(timing) = TabTiming::from_label(value) {
                s.set_tab_timing(timing);
            }
            s.clear_pick();
        })
        .build(k, menus);
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
            vec![
                settings_row(
                    k,
                    &rune_kit::shell::SIDEBAR,
                    "Horizontal workspace tabs",
                    Some(
                        "Draw the tabs as a bar across the top of the window instead of a rail \
                         down its side.",
                    ),
                    horizontal,
                ),
                settings_row(
                    k,
                    &icons::SPLIT_RIGHT,
                    "Tab transition",
                    Some("How the tab on screen gives way to the next one."),
                    transition,
                ),
                settings_row(
                    k,
                    &rune_kit::glyphs::DASH,
                    "Transition timing",
                    Some("The spring the tab transition runs on. A cut has no timing to run on."),
                    timing,
                ),
            ],
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
