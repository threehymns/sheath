//! The window's frame: rune-kit's split view around the tab rail and the tab on screen, and the
//! toolbar over it. How the window is composed lives here and nowhere else, so the panes and the
//! window's state stay out of each other's way.

use std::time::Instant;

use rune::{Cx, El, Spring};
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
    let content = SplitView::new(main)
        .sidebar(&s.rail, |s: &mut Window| &mut s.rail, s.tabs.rail(k))
        .toolbar(toolbar(s, k))
        .titlebar_inset(cx.titlebar_inset())
        .frame_inset(FRAME)
        .toolbar_in_panel()
        .sidebar_padding(RAIL_PADDING.0, RAIL_PADDING.1)
        .sidebar_toggle(
            SidebarToggle::new()
                .size(ControlSize::Large)
                .past_lights(16.0)
                .edge(12.0)
                .gap(4.0)
                .in_header(),
        )
        .pane_motion(
            PaneMotion::new()
                .spring(Spring::new(0.32, 0.0))
                .scale_with_width(320.0, 0.16, 0.32)
                .fade_tail(0.15),
        )
        .reduced_motion(cx.is_reduced_motion())
        .build(k);
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
/// toggle sits beside these, so the rail hides the way the app's sidebar does.
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
        .trailing(new)
}
