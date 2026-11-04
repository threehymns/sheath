//! The window's frame: rune-kit's split view around the tabs and the tab on screen, and the
//! toolbar over it. A window holding a single shell draws none of that — the terminal is the
//! window until a second pane or tab asks for the chrome. How the window is composed lives here
//! and nowhere else, so the panes and the window's state stay out of each other's way.

use std::time::Instant;

use rune::{Cx, El, Spring, col};
use rune_kit::shell::{PaneMotion, SidebarToggle, SplitView};
use rune_kit::{ControlSize, IconButton, IconButtonVariant, Kit, Toolbar};
use rune_theme_opencode as theme;

use crate::Window;
use crate::action::Action;
use crate::icons;
use crate::panes::Side;

/// The window background showing around the blocks, and between the rail and them.
const FRAME: f32 = 8.0;
/// The rail's contents: inset from the window's edge on one side, the blocks on the other.
const RAIL_PADDING: (f32, f32) = (12.0, 8.0);

/// The whole window, with the command bar and the settings window layered over it.
pub fn view(s: &Window, cx: &mut Cx<Window>) -> El {
    let k = theme::kit(cx.system_dark());
    s.dark.set(cx.system_dark());
    s.scale.set(cx.env().scale.max(1.0));
    s.areas.begin_view();
    let at = Instant::now();
    // The tab on screen is one layout of blocks: its own tree, its own shells. A tab with no
    // blocks left (the frame between the last shell ending and the window closing) draws
    // nothing rather than reading a layout that is gone.
    let main = s
        .tabs
        .active_id()
        .map(|id| s.tabs.layout(id, s.scale.get(), k))
        .unwrap_or_else(rune::el);
    // A window showing a single shell is the terminal and nothing else: no title bar band, no
    // card around the grid, no rail, no toolbar. The chrome arrives with the second pane or tab,
    // and the chords that make either of them work do so without it.
    let single = s.tabs.is_single();
    let horizontal = s.horizontal_tabs();
    let content = if single {
        main
    } else {
        let main = if horizontal {
            col()
                .key("workspace")
                .w_full()
                .h_full()
                .min_h(0.0)
                .child(s.tabs.strip(k))
                .child(main.grow(1.0).min_h(0.0))
        } else {
            main
        };
        let mut shell = SplitView::new(main)
            .titlebar_inset(cx.titlebar_inset())
            .frame_inset(FRAME)
            .toolbar(toolbar(s, k))
            .toolbar_in_panel()
            .pane_motion(
                PaneMotion::new()
                    .spring(Spring::new(0.32, 0.0))
                    .scale_with_width(320.0, 0.16, 0.32)
                    .fade_tail(0.15),
            )
            .reduced_motion(cx.is_reduced_motion());
        if !horizontal {
            shell = shell
                .sidebar(&s.rail, |s: &mut Window| &mut s.rail, s.tabs.rail(k))
                .sidebar_padding(RAIL_PADDING.0, RAIL_PADDING.1)
                .sidebar_toggle(
                    SidebarToggle::new()
                        .size(ControlSize::Large)
                        .past_lights(16.0)
                        .edge(12.0)
                        .gap(4.0)
                        .in_header(),
                );
        }
        shell.build(k)
    };
    s.built.set(s.built.get() + at.elapsed());
    let mut layers = Vec::new();
    if s.palette.is_some() {
        let (backdrop, slot) = actions_ui::palette_layers(s, k, Action::icon);
        layers.push(backdrop);
        layers.push(slot);
    }
    if let Some(tab) = s.settings {
        crate::settings::layer(s, tab, k, &mut layers);
    }
    rune_kit::root(content, layers)
}

/// The window's toolbar: the tab's name and the three ways to make another block. The rail's
/// toggle sits beside these, so the rail hides the way the app's sidebar does. A window holding
/// one shell has none of this.
fn toolbar(s: &Window, k: &Kit) -> Toolbar {
    let split_right = IconButton::new(&icons::SPLIT_RIGHT)
        .variant(IconButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .label("Split right")
        .help("Split the pane to the right")
        .shortcut(s.keybinds.menu_label(Action::SplitRight))
        .on_press(|s: &mut Window| s.tabs.split(Side::Right))
        .build(k)
        .key("split-right");
    let split_down = IconButton::new(&icons::SPLIT_DOWN)
        .variant(IconButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .label("Split down")
        .help("Split the pane downwards")
        .shortcut(s.keybinds.menu_label(Action::SplitDown))
        .on_press(|s: &mut Window| s.tabs.split(Side::Down))
        .build(k)
        .key("split-down");
    let balance = IconButton::new(&icons::BALANCE)
        .variant(IconButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .label("Balance panes")
        .help("Make every split even")
        .shortcut(s.keybinds.menu_label(Action::BalancePanes))
        .on_press(|s: &mut Window| s.tabs.balance())
        .build(k)
        .key("balance-panes");
    let new = IconButton::new(&icons::PLUS)
        .variant(IconButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .label("New tab")
        .help("New tab")
        .shortcut(s.keybinds.menu_label(Action::NewTab))
        .on_press(|s: &mut Window| s.new_tab())
        .build(k)
        .key("new-tab");
    Toolbar::new()
        .title(s.tabs.title())
        .trailing(split_right)
        .trailing(split_down)
        .trailing(balance)
        .trailing(new)
}
