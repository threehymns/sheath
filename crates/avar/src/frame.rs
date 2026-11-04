//! The window's frame: rune-kit's split view around the tab rail and the tab on screen, and the
//! pane's toolbar. How the window is composed lives here and nowhere else, so the panes and the
//! window's state stay out of each other's way.

use std::time::Instant;

use rune::{Cx, El, Spring};
use rune_kit::shell::{PaneMotion, PaneState, SidebarToggle, SplitView};
use rune_kit::{ControlSize, IconButton, IconButtonVariant, Kit, Toolbar};
use rune_theme_opencode as theme;

use crate::Window;
use crate::action::Action;
use crate::icons;
use crate::tabs::{self, Which};

/// The window background showing around the main pane, and between the rail and the split.
const FRAME: f32 = 8.0;
/// The rail's contents: inset from the window's edge on one side, the main pane on the other.
const RAIL_PADDING: (f32, f32) = (12.0, 8.0);

/// The whole window, with the command bar and the settings window layered over it.
pub fn view(s: &Window, cx: &mut Cx<Window>) -> El {
    let k = theme::kit(cx.system_dark());
    s.dark.set(cx.system_dark());
    s.scale.set(cx.env().scale.max(1.0));
    s.areas.begin_view();
    let at = Instant::now();
    let id = s.tabs.active_id();
    let main = id
        .map(|id| tabs::screen(s, id, Which::Main, k))
        .unwrap_or_else(rune::el);
    // The split is built while it is closed too: its pane slides open on the split view's
    // spring, and only content that is already there can do that.
    let split = id
        .filter(|_| s.tabs.has_split())
        .map(|id| tabs::screen(s, id, Which::Split, k))
        .unwrap_or_else(rune::el);
    let pane = s.tabs.pane();
    let content = SplitView::new(main)
        .sidebar(&s.rail, |s: &mut Window| &mut s.rail, s.tabs.list(k))
        .inspector(&pane, active_pane, split)
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

/// The split pane's state lives in the tab, so the divider drags the tab that is on screen.
fn active_pane(s: &mut Window) -> &mut PaneState {
    s.tabs.pane_mut()
}

/// The pane's own toolbar: the tab's name, the split's toggle and the new tab button. The rail's
/// toggle sits beside these, so the rail hides the way the app's sidebar does.
fn toolbar(s: &Window, k: &Kit) -> Toolbar {
    let open = s.tabs.split_shown();
    let split = IconButton::new(&rune_kit::shell::INSPECTOR)
        .variant(IconButtonVariant::GhostMuted)
        .control_size(ControlSize::Regular)
        .expanded(open)
        .label(if open { "Close split" } else { "Split pane" })
        .help(if open { "Close split" } else { "Split pane" })
        .shortcut(s.keybinds.menu_label(Action::SplitPane))
        .on_press(|s: &mut Window| s.tabs.toggle_split())
        .build(k)
        .key("split-pane");
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
        .trailing(split)
        .trailing(new)
}
