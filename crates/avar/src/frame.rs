//! The window's frame: the tabs around the tab on screen. A window holding a single shell draws
//! none of it — the terminal is the window until a second pane or tab asks for the chrome. One
//! holding a split draws its panes straight on the window's background, with the tab rail (or
//! strip) once there is a second tab and a slim top bar only where the desktop leaves the
//! titlebar to the app. How the window is composed lives here and nowhere else, so the panes and
//! the window's state stay out of each other's way.

use std::time::Instant;

use rune::{Color, Cursor, Cx, Drag, El, Phase, Spring, Tx, col, el, row};
use rune_kit::shell::{PaneMotion, SidebarToggle, SplitView};
use rune_kit::{ControlSize, Kit, Toolbar};
use rune_theme_opencode as theme;

use crate::Window;
use crate::action::Action;

/// The window background showing around the panes of a window that draws its own titlebar.
const FRAME: f32 = 8.0;
/// The rail's contents: inset from the window's edge on one side, the panes on the other.
const RAIL_PADDING: (f32, f32) = (12.0, 8.0);
/// The rail's resize grip, between it and the panes.
const GRIP: f32 = 6.0;

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
        .map(|id| s.tabs.layout(id, s.scale.get(), k, s.pane_headers()))
        .unwrap_or_else(rune::el);
    // A window showing a single shell is the terminal and nothing else: no title bar band, no
    // header or card around the grid, no rail, no toolbar. The chrome arrives with the second
    // pane or tab, and the chords that make either of them work do so without any of it.
    let single = s.tabs.is_single();
    let horizontal = s.horizontal_tabs();
    // The rail and the strip choose between tabs, so a single tab that is only split has no use
    // for either.
    let tabs = s.tabs.len() > 1;
    // The top bar is the app's own title strip. It only earns its place where the desktop leaves
    // the window's titlebar to the app: on Wayland the compositor draws server-side decorations
    // or Rune draws its own client-side titlebar, X11's window managers draw theirs, and Windows
    // keeps the system frame, so a bar of the app's own would be a second titlebar.
    let top_bar = s.top_bar() && !system_titlebar();
    let content = if single {
        main
    } else if top_bar {
        let main = if horizontal && tabs {
            col()
                .key("workspace")
                .w_full()
                .h_full()
                .min_h(0.0)
                .fill(k.c.bg_deep)
                .child(s.tabs.strip(k))
                .child(main.grow(1.0).min_h(0.0))
        } else {
            main
        };
        let mut shell = SplitView::new(main)
            .titlebar_inset(cx.titlebar_inset())
            .frame_inset(FRAME)
            .toolbar(toolbar(s))
            .toolbar_in_panel()
            .pane_motion(
                PaneMotion::new()
                    .spring(Spring::new(0.32, 0.0))
                    .scale_with_width(320.0, 0.16, 0.32)
                    .fade_tail(0.15),
            )
            .reduced_motion(cx.is_reduced_motion());
        if !horizontal && tabs {
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
    } else if horizontal && tabs {
        col()
            .key("workspace")
            .w_full()
            .h_full()
            .min_h(0.0)
            .fill(k.c.bg_deep)
            .child(s.tabs.strip(k))
            .child(main.grow(1.0).min_h(0.0))
    } else if tabs {
        row()
            .key("workspace")
            .w_full()
            .h_full()
            .min_h(0.0)
            .fill(k.c.bg_deep)
            .child(rail(s, k))
            .child(main.grow(1.0).min_w(0.0))
    } else {
        main
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

/// Whether the window already has a titlebar: the compositor's or Rune's client-side one on
/// Wayland, the window manager's on X11, the system frame on Windows. Only macOS leaves the bar
/// to the app, with its traffic lights over the content.
fn system_titlebar() -> bool {
    !cfg!(target_os = "macos")
}

/// The window's top bar: the tab's name and nothing else. The window controls are the desktop's,
/// and every command the bar could hold has a chord and the command bar.
fn toolbar(s: &Window) -> Toolbar {
    Toolbar::new().title(s.tabs.title())
}

/// The tab rail in a window with no top bar: a pane beside the panes, resizable by the grip on
/// its edge. Without a titlebar band there is no toggle to hide it, so it stays open.
fn rail(s: &Window, k: &Kit) -> El {
    let width = s.rail.shown_width();
    let grip = el()
        .key("rail-grip")
        .role("separator")
        .label("Resize tabs")
        .w(GRIP)
        .h_full()
        .fill(Color::TRANSPARENT)
        .hover(k.c.hover)
        .cursor(Cursor::ResizeColumn)
        .transition(rune::Transition::none())
        .on_drag(|s: &mut Window, d: &mut Drag, tx: &mut Tx| match d.phase {
            Phase::Down => {
                d.data = s.rail.width();
                tx.instant();
            }
            Phase::Move => {
                tx.instant();
                s.rail.drag_to(d.data + d.dx);
            }
            Phase::Up => {}
        })
        .on_double_press(|s: &mut Window| s.rail.reset_width());
    row()
        .key("tab-rail-row")
        .w(width + GRIP)
        .h_full()
        .min_h(0.0)
        .motion(Spring::NONE)
        .child(
            col()
                .key("tab-rail")
                .w(width)
                .h_full()
                .pt(RAIL_PADDING.0)
                .pb(RAIL_PADDING.0)
                .pl(RAIL_PADDING.0)
                .pr(RAIL_PADDING.1)
                .child(s.tabs.rail(k)),
        )
        .child(grip)
}
